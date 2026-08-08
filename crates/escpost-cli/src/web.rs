use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use escpost::{
    CommandTrace, DecodedCommand, Effect, Justification, StateChange, TracedRenderResult,
};
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
    waiting_hint: Option<String>,
    completion: Option<&'static str>,
    receiving: usize,
    /// Profile the server renders with, shown before the first job arrives.
    session_profile: String,
    /// Whether renders are anti-aliased grayscale previews. The viewer smooths
    /// those and keeps the faithful 1-bit dots crisp.
    antialias: bool,
    /// Wall-clock time the current job completed, in Unix epoch milliseconds.
    completed_at: Option<u64>,
    /// The current job's exact captured bytes, offered for download.
    raw_input: Option<Vec<u8>>,
}

/// Current wall-clock time in Unix epoch milliseconds, for job completion.
fn epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

struct RenderedJob {
    profile: String,
    /// Non-fatal render diagnostics, pre-formatted for display.
    warnings: Vec<String>,
    sheets: Vec<RenderedWebSheet>,
}

struct RenderedWebSheet {
    name: String,
    width_dots: u32,
    height_dots: u32,
    png: Vec<u8>,
    commands: Vec<CommandResponse>,
}

#[derive(Serialize)]
struct RenderResponse {
    profile: String,
    generation: u64,
    error: Option<String>,
    /// Guidance shown while no job has been captured yet, e.g. by `serve`.
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
    /// How a captured job ended: "closed" or "timeout". Absent for renders.
    #[serde(skip_serializing_if = "Option::is_none")]
    completion: Option<&'static str>,
    /// True while a connection is still sending a job that has not completed.
    receiving: bool,
    /// When the current job completed, in Unix epoch milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_at: Option<u64>,
    /// True when the current job's raw bytes can be downloaded from /job.
    input_available: bool,
    /// True when renders are anti-aliased grayscale, so the viewer smooths them.
    antialias: bool,
    /// Non-fatal render diagnostics for the current job, ready to display.
    warnings: Vec<String>,
    sheets: Vec<SheetResponse>,
}

#[derive(Serialize)]
struct SheetResponse {
    name: String,
    order: usize,
    width_dots: u32,
    height_dots: u32,
    url: String,
    /// Experimental subset of decoded commands associated with this sheet.
    commands: Vec<CommandResponse>,
}

#[derive(Clone, Serialize)]
struct CommandResponse {
    byte_start: usize,
    byte_end: usize,
    name: &'static str,
    detail: String,
    effects: Vec<EffectResponse>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum EffectResponse {
    StateChange {
        state: &'static str,
        before: &'static str,
        after: &'static str,
    },
    Motion {
        before: PositionResponse,
        after: PositionResponse,
    },
    Paint {
        bounds: RegionResponse,
    },
}

#[derive(Clone, Serialize)]
struct PositionResponse {
    x: u32,
    y: u32,
}

#[derive(Clone, Serialize)]
struct RegionResponse {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl JobStore {
    pub(crate) fn with_render(rendered: TracedRenderResult, antialias: bool) -> Self {
        Self {
            state: Arc::new(RwLock::new(JobStoreState {
                jobs: VecDeque::from([Arc::new(RenderedJob::from(rendered))]),
                error: None,
                generation: 1,
                waiting_hint: None,
                completion: None,
                receiving: 0,
                session_profile: String::new(),
                antialias,
                completed_at: Some(epoch_millis()),
                raw_input: None,
            })),
        }
    }

    /// Create a store with no job yet. The web viewer shows `hint` and the
    /// `profile` until the first job arrives, which suits a listener that
    /// renders on demand with a known profile.
    pub(crate) fn awaiting_jobs(profile: String, hint: String, antialias: bool) -> Self {
        Self {
            state: Arc::new(RwLock::new(JobStoreState {
                jobs: VecDeque::new(),
                error: None,
                generation: 0,
                waiting_hint: Some(hint),
                completion: None,
                receiving: 0,
                session_profile: profile,
                antialias,
                completed_at: None,
                raw_input: None,
            })),
        }
    }

    /// Mark that a connection has started sending a job. The viewer reports this
    /// until the matching `end_capture`.
    pub(crate) async fn begin_capture(&self) {
        self.state.write().await.receiving += 1;
    }

    /// Mark that an in-progress job has finished sending (completed or dropped).
    pub(crate) async fn end_capture(&self) {
        let mut state = self.state.write().await;
        state.receiving = state.receiving.saturating_sub(1);
    }

    /// Replace the preview with a render that has no capture semantics, such as
    /// `render --web`. Its source is a file, so nothing is offered for download.
    pub(crate) async fn replace_render(&self, rendered: TracedRenderResult) {
        self.store_render(rendered, None, None).await;
    }

    /// Replace the preview with a captured job, recording how it ended so the
    /// viewer can distinguish a closed connection from an idle timeout, and
    /// keeping its exact bytes for download.
    pub(crate) async fn replace_captured(
        &self,
        rendered: TracedRenderResult,
        completion: &'static str,
        raw_input: Vec<u8>,
    ) {
        self.store_render(rendered, Some(completion), Some(raw_input))
            .await;
    }

    async fn store_render(
        &self,
        rendered: TracedRenderResult,
        completion: Option<&'static str>,
        raw_input: Option<Vec<u8>>,
    ) {
        let mut state = self.state.write().await;
        state.jobs = VecDeque::from([Arc::new(RenderedJob::from(rendered))]);
        state.error = None;
        state.generation += 1;
        state.completion = completion;
        state.completed_at = Some(epoch_millis());
        state.raw_input = raw_input;
    }

    /// The current job's captured bytes and completion time, for download.
    async fn raw_input_download(&self) -> Option<(Vec<u8>, u64)> {
        let state = self.state.read().await;
        let bytes = state.raw_input.clone()?;
        Some((bytes, state.completed_at.unwrap_or(0)))
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

    async fn render_response(&self) -> RenderResponse {
        let state = self.state.read().await;
        let Some(job) = state.jobs.front() else {
            // No job yet: report a waiting state so the viewer can guide the
            // developer rather than showing a bare error.
            return RenderResponse {
                profile: state.session_profile.clone(),
                generation: state.generation,
                error: state.error.clone(),
                hint: state.waiting_hint.clone(),
                completion: None,
                receiving: state.receiving > 0,
                completed_at: None,
                input_available: false,
                antialias: state.antialias,
                warnings: Vec::new(),
                sheets: Vec::new(),
            };
        };
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
                commands: sheet.commands.clone(),
            })
            .collect();
        RenderResponse {
            profile: job.profile.clone(),
            generation: state.generation,
            error: state.error.clone(),
            hint: None,
            completion: state.completion,
            receiving: state.receiving > 0,
            completed_at: state.completed_at,
            input_available: state.raw_input.is_some(),
            antialias: state.antialias,
            warnings: job.warnings.clone(),
            sheets,
        }
    }
}

impl From<TracedRenderResult> for RenderedJob {
    fn from(traced: TracedRenderResult) -> Self {
        let TracedRenderResult { render, trace } = traced;
        let mut trace_sheets = trace.sheets.into_iter();
        Self {
            profile: render.metadata.profile_id,
            warnings: render.warnings.iter().map(ToString::to_string).collect(),
            sheets: render
                .sheets
                .into_iter()
                .enumerate()
                .map(|(index, sheet)| RenderedWebSheet {
                    name: format!("sheet-{:03}.png", index + 1),
                    width_dots: sheet.surface.width(),
                    height_dots: sheet.surface.height(),
                    png: sheet.png,
                    commands: trace_sheets
                        .next()
                        .map(|sheet| command_responses(sheet.commands))
                        .unwrap_or_default(),
                })
                .collect(),
        }
    }
}

fn command_responses(commands: Vec<CommandTrace>) -> Vec<CommandResponse> {
    commands
        .into_iter()
        .map(|command| {
            let (name, detail) = match command.command {
                DecodedCommand::SetJustification(justification) => (
                    "ESC a",
                    format!("Set justification: {}", justification_name(justification)),
                ),
                DecodedCommand::TextByte(byte) => (
                    "Text",
                    if byte.is_ascii_graphic() || byte == b' ' {
                        char::from(byte).to_string()
                    } else {
                        format!("0x{byte:02X}")
                    },
                ),
                DecodedCommand::LineFeed => ("LF", "Print and line feed".to_owned()),
            };
            CommandResponse {
                byte_start: command.byte_range.start,
                byte_end: command.byte_range.end,
                name,
                detail,
                effects: command.effects.into_iter().map(effect_response).collect(),
            }
        })
        .collect()
}

fn effect_response(effect: Effect) -> EffectResponse {
    match effect {
        Effect::StateChange(StateChange::Justification { before, after }) => {
            EffectResponse::StateChange {
                state: "justification",
                before: justification_name(before),
                after: justification_name(after),
            }
        }
        Effect::Motion { before, after } => EffectResponse::Motion {
            before: PositionResponse {
                x: before.x,
                y: before.y,
            },
            after: PositionResponse {
                x: after.x,
                y: after.y,
            },
        },
        Effect::Paint { bounds } => EffectResponse::Paint {
            bounds: RegionResponse {
                x: bounds.x,
                y: bounds.y,
                width: bounds.width,
                height: bounds.height,
            },
        },
    }
}

fn justification_name(justification: Justification) -> &'static str {
    match justification {
        Justification::Left => "left",
        Justification::Center => "center",
        Justification::Right => "right",
    }
}

pub(crate) async fn bind(requested: Option<SocketAddr>) -> Result<TcpListener, CliError> {
    crate::net::bind_loopback(requested, FIRST_AUTOMATIC_PORT..=LAST_AUTOMATIC_PORT)
        .await
        .map_err(|failure| match failure {
            crate::net::BindFailure::Address { address, source } => {
                CliError::BindWeb { address, source }
            }
            crate::net::BindFailure::RangeExhausted => CliError::NoAutomaticWebPort,
        })
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
    if open_browser && let Err(error) = webbrowser::open(&url) {
        eprintln!("warning: could not open the browser ({error}); open {url} manually");
    }

    let router = Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/api/render", get(current_render))
        .route("/sheets/{file}", get(sheet_png))
        .route("/job", get(download_job))
        .with_state(jobs);
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(CliError::ServeWeb)
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

/// Liveness check for containers and automated tests. Returns 200 while the
/// server is accepting requests, independent of whether any job was captured.
async fn health() -> &'static str {
    "ok"
}

async fn current_render(State(jobs): State<JobStore>) -> Json<RenderResponse> {
    Json(jobs.render_response().await)
}

async fn download_job(State(jobs): State<JobStore>) -> Response {
    let Some((bytes, completed_at)) = jobs.raw_input_download().await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // Name the file by completion time so several captures do not collide.
    let disposition = format!("attachment; filename=\"escpost-job-{completed_at}.bin\"");
    (
        [
            (
                header::CONTENT_TYPE,
                String::from("application/octet-stream"),
            ),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response()
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
