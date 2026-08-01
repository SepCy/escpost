use std::time::Duration;

use escpost::{RenderOptions, render_with_options};
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

    // Zero disables the idle timeout; a negative or non-finite value is invalid.
    let idle_timeout = if arguments.idle_timeout == 0.0 {
        None
    } else if arguments.idle_timeout.is_finite() && arguments.idle_timeout > 0.0 {
        Some(Duration::from_secs_f64(arguments.idle_timeout))
    } else {
        return Err(CliError::InvalidIdleTimeout);
    };

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
    match idle_timeout {
        Some(timeout) => eprintln!("Idle timeout: {timeout:?}"),
        None => eprintln!("Idle timeout: disabled (jobs end when the connection closes)"),
    }

    let web_listener = web::bind(arguments.web_listen).await?;
    let jobs = web::JobStore::awaiting_jobs(
        arguments.profile.clone(),
        format!(
            "Waiting for the first job. Configure a local ERP or POS application to send its RAW ESC/POS print jobs to {raw_address}."
        ),
    );

    // Accept jobs while the web viewer runs. The viewer owns the foreground and
    // returns on Ctrl+C; stop accepting once it does.
    let options = RenderOptions {
        scale: arguments.scale,
        antialias: arguments.antialias,
        ..RenderOptions::default()
    };
    let acceptor = tokio::spawn(accept_jobs(
        raw,
        jobs.clone(),
        profile,
        idle_timeout,
        options,
    ));
    let result = web::serve(web_listener, jobs, arguments.browser).await;
    acceptor.abort();
    result
}

async fn accept_jobs(
    listener: TcpListener,
    jobs: web::JobStore,
    profile: &'static PrinterProfile,
    idle_timeout: Option<Duration>,
    options: RenderOptions,
) {
    loop {
        match listener.accept().await {
            // A transient accept error must not tear down the listener; the next
            // client can still connect.
            Ok((stream, _peer)) => {
                tokio::spawn(capture_job(
                    stream,
                    jobs.clone(),
                    profile,
                    idle_timeout,
                    options,
                ));
            }
            Err(_) => continue,
        }
    }
}

async fn capture_job(
    mut stream: TcpStream,
    jobs: web::JobStore,
    profile: &'static PrinterProfile,
    idle_timeout: Option<Duration>,
    options: RenderOptions,
) {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    // Whether the viewer currently counts this connection as receiving a job.
    let mut receiving = false;
    loop {
        let read = match idle_timeout {
            Some(timeout) => match tokio::time::timeout(timeout, stream.read(&mut chunk)).await {
                Ok(result) => result,
                // Silence for the idle interval completes whatever has arrived.
                Err(_elapsed) => {
                    if !buffer.is_empty() {
                        finalize(
                            &jobs,
                            std::mem::take(&mut buffer),
                            profile,
                            "timeout",
                            options,
                        )
                        .await;
                        jobs.end_capture().await;
                        receiving = false;
                    }
                    continue;
                }
            },
            None => stream.read(&mut chunk).await,
        };
        match read {
            Ok(0) => break,
            Ok(read) => {
                buffer.extend_from_slice(&chunk[..read]);
                if !receiving {
                    jobs.begin_capture().await;
                    receiving = true;
                }
            }
            // A read error abandons whatever was buffered.
            Err(_) => {
                if receiving {
                    jobs.end_capture().await;
                }
                return;
            }
        }
    }
    // The connection closed: any remaining bytes are an explicitly completed job.
    if !buffer.is_empty() {
        finalize(&jobs, buffer, profile, "closed", options).await;
    }
    if receiving {
        jobs.end_capture().await;
    }
}

async fn finalize(
    jobs: &web::JobStore,
    bytes: Vec<u8>,
    profile: &'static PrinterProfile,
    completion: &'static str,
    options: RenderOptions,
) {
    // Rendering is synchronous and CPU-bound; run it off the async workers so a
    // job in flight cannot stall the web viewer's responses. The blocking task
    // returns the bytes so the exact input can be kept for download.
    match tokio::task::spawn_blocking(move || {
        (render_with_options(&bytes, profile, &options), bytes)
    })
    .await
    {
        Ok((Ok(rendered), raw_input)) => {
            jobs.replace_captured(rendered, completion, raw_input).await;
        }
        Ok((Err(error), _)) => {
            eprintln!("warning: could not render captured job: {error}");
            jobs.set_error(error.to_string()).await;
        }
        // A panic or cancellation in the render task leaves no job to preview.
        Err(_) => {}
    }
}
