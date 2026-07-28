use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use escpost::RenderResult;
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use crate::error::CliError;

const INDEX_HTML: &str = include_str!("../assets/index.html");
const FIRST_AUTOMATIC_PORT: u16 = 9000;
const LAST_AUTOMATIC_PORT: u16 = 9099;

#[derive(Clone)]
pub(crate) struct JobStore {
    state: Arc<RwLock<JobStoreState>>,
}

struct JobStoreState {
    jobs: VecDeque<Arc<RenderedJob>>,
    error: Option<String>,
    generation: u64,
}

struct RenderedJob {
    profile: String,
    sheets: Vec<RenderedWebSheet>,
}

struct RenderedWebSheet {
    name: String,
    width_dots: u32,
    height_dots: u32,
    png: Vec<u8>,
}

#[derive(Serialize)]
struct RenderResponse {
    profile: String,
    generation: u64,
    error: Option<String>,
    sheets: Vec<SheetResponse>,
}

#[derive(Serialize)]
struct SheetResponse {
    name: String,
    order: usize,
    width_dots: u32,
    height_dots: u32,
    url: String,
}

impl JobStore {
    pub(crate) fn with_render(rendered: RenderResult) -> Self {
        Self {
            state: Arc::new(RwLock::new(JobStoreState {
                jobs: VecDeque::from([Arc::new(RenderedJob::from(rendered))]),
                error: None,
                generation: 1,
            })),
        }
    }

    pub(crate) async fn replace_render(&self, rendered: RenderResult) {
        let mut state = self.state.write().await;
        state.jobs = VecDeque::from([Arc::new(RenderedJob::from(rendered))]);
        state.error = None;
        state.generation += 1;
    }

    pub(crate) async fn set_error(&self, error: String) {
        self.state.write().await.error = Some(error);
    }

    async fn snapshot(&self) -> Option<(Arc<RenderedJob>, u64, Option<String>)> {
        let state = self.state.read().await;
        state
            .jobs
            .front()
            .cloned()
            .map(|job| (job, state.generation, state.error.clone()))
    }
}

impl From<RenderResult> for RenderedJob {
    fn from(rendered: RenderResult) -> Self {
        Self {
            profile: rendered.metadata.profile_id,
            sheets: rendered
                .sheets
                .into_iter()
                .enumerate()
                .map(|(index, sheet)| RenderedWebSheet {
                    name: format!("sheet-{:03}.png", index + 1),
                    width_dots: sheet.surface.width(),
                    height_dots: sheet.surface.height(),
                    png: sheet.png,
                })
                .collect(),
        }
    }
}

pub(crate) async fn bind(requested: Option<SocketAddr>) -> Result<TcpListener, CliError> {
    if let Some(address) = requested {
        return TcpListener::bind(address)
            .await
            .map_err(|source| CliError::BindWeb { address, source });
    }

    for port in FIRST_AUTOMATIC_PORT..=LAST_AUTOMATIC_PORT {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        match TcpListener::bind(address).await {
            Ok(listener) => return Ok(listener),
            Err(error) if error.kind() == ErrorKind::AddrInUse => {}
            Err(source) => return Err(CliError::BindWeb { address, source }),
        }
    }
    Err(CliError::NoAutomaticWebPort)
}

pub(crate) async fn serve(
    listener: TcpListener,
    jobs: JobStore,
    open_browser: bool,
) -> Result<(), CliError> {
    let address = listener.local_addr().map_err(CliError::ServeWeb)?;
    let url = format!("http://{address}/");
    if !address.ip().is_loopback() {
        eprintln!("warning: receipt data is exposed beyond loopback on {address}");
    }
    eprintln!("Web viewer: {url}");
    eprintln!("Press Ctrl+C to stop.");
    if open_browser {
        webbrowser::open(&url).map_err(|error| CliError::OpenBrowser(error.to_string()))?;
    }

    let router = Router::new()
        .route("/", get(index))
        .route("/api/render", get(current_render))
        .route("/sheets/{file}", get(sheet_png))
        .with_state(jobs);
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(CliError::ServeWeb)
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn current_render(State(jobs): State<JobStore>) -> Result<Json<RenderResponse>, StatusCode> {
    let (job, generation, error) = jobs.snapshot().await.ok_or(StatusCode::NOT_FOUND)?;
    let sheets = job
        .sheets
        .iter()
        .enumerate()
        .map(|(index, sheet)| SheetResponse {
            name: sheet.name.clone(),
            order: index + 1,
            width_dots: sheet.width_dots,
            height_dots: sheet.height_dots,
            url: format!("/sheets/{}.png", index + 1),
        })
        .collect();
    Ok(Json(RenderResponse {
        profile: job.profile.clone(),
        generation,
        error,
        sheets,
    }))
}

async fn sheet_png(Path(file): Path<String>, State(jobs): State<JobStore>) -> Response {
    let Some(number) = file
        .strip_suffix(".png")
        .and_then(|number| number.parse::<usize>().ok())
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some((job, _, _)) = jobs.snapshot().await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(sheet) = number
        .checked_sub(1)
        .and_then(|index| job.sheets.get(index))
    else {
        return StatusCode::NOT_FOUND.into_response();
    };

    ([(header::CONTENT_TYPE, "image/png")], sheet.png.clone()).into_response()
}

async fn shutdown_signal() {
    // Failure to install a signal handler should stop the server rather than
    // leave a foreground developer command that cannot shut down cleanly.
    let _ = tokio::signal::ctrl_c().await;
}
