use escpost::render;
use escpost_profiles::PrinterProfile;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

use crate::cli::ServeArgs;
use crate::error::CliError;
use crate::{net, profiles, web};

/// Port 9100 is the common RAW/AppSocket transport used by network printers. A
/// busy default escalates through this range, and every listener binds
/// loopback so captured receipt data is not exposed by default.
const FIRST_RAW_PORT: u16 = 9100;
const LAST_RAW_PORT: u16 = 9109;

pub(crate) async fn run(arguments: ServeArgs) -> Result<(), CliError> {
    // The profile defaults to REFERENCE, so a virtual printer previews without
    // any prompt and never needs an interactive terminal.
    let profile = profiles::load(&arguments.profile)?;
    eprintln!("Profile: {}", arguments.profile);

    let raw = net::bind_loopback(arguments.listen, FIRST_RAW_PORT..=LAST_RAW_PORT)
        .await
        .map_err(|failure| match failure {
            net::BindFailure::Address { address, source } => {
                CliError::BindRawPrinter { address, source }
            }
            net::BindFailure::RangeExhausted => CliError::NoAutomaticRawPort,
        })?;
    let raw_address = raw.local_addr().map_err(CliError::ServeRawPrinter)?;
    if !raw_address.ip().is_loopback() {
        eprintln!("warning: the RAW printer accepts receipt data beyond loopback on {raw_address}");
    }
    eprintln!("RAW printer: {raw_address}");

    let web_listener = web::bind(arguments.web_listen).await?;
    let jobs = web::JobStore::awaiting_jobs(format!(
        "Waiting for the first job. Configure a local ERP or POS application to send its RAW ESC/POS print jobs to {raw_address}."
    ));

    // Accept jobs while the web viewer runs. The viewer owns the foreground and
    // returns on Ctrl+C; stop accepting once it does.
    let acceptor = tokio::spawn(accept_jobs(raw, jobs.clone(), profile));
    let result = web::serve(web_listener, jobs, arguments.browser).await;
    acceptor.abort();
    result
}

async fn accept_jobs(listener: TcpListener, jobs: web::JobStore, profile: &'static PrinterProfile) {
    loop {
        match listener.accept().await {
            // A transient accept error must not tear down the listener; the next
            // client can still connect.
            Ok((stream, _peer)) => {
                tokio::spawn(capture_job(stream, jobs.clone(), profile));
            }
            Err(_) => continue,
        }
    }
}

async fn capture_job(mut stream: TcpStream, jobs: web::JobStore, profile: &'static PrinterProfile) {
    let mut bytes = Vec::new();
    if stream.read_to_end(&mut bytes).await.is_err() {
        return;
    }
    // A connection that closes without sending anything is not a job.
    if bytes.is_empty() {
        return;
    }
    // Rendering is synchronous and CPU-bound; run it off the async workers so a
    // job in flight cannot stall the web viewer's responses.
    match tokio::task::spawn_blocking(move || render(&bytes, profile)).await {
        Ok(Ok(rendered)) => jobs.replace_render(rendered).await,
        Ok(Err(error)) => {
            eprintln!("warning: could not render captured job: {error}");
            jobs.set_error(error.to_string()).await;
        }
        // A panic or cancellation in the render task leaves no job to preview.
        Err(_) => {}
    }
}
