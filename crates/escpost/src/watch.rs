use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use escpost_render::{RenderOptions, render_with_trace_and_options};

use crate::cli::InputFormat;
use crate::error::CliError;
use crate::{output, profiles, source, web};

const WATCH_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone)]
pub(crate) struct WatchConfig {
    pub(crate) source: PathBuf,
    pub(crate) format: InputFormat,
    pub(crate) profile: String,
    pub(crate) output: Option<PathBuf>,
    pub(crate) output_dir: Option<PathBuf>,
    pub(crate) sheet: Option<usize>,
    pub(crate) scale: u32,
    pub(crate) antialias: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct SourceStamp {
    modified: Option<SystemTime>,
    length: u64,
}

pub(crate) fn start(config: WatchConfig, jobs: web::JobStore) -> Result<(), CliError> {
    let watched_path = source::watch_path(&config.source)?;
    let initial_stamp = inspect(&watched_path)?;
    tokio::spawn(run(config, watched_path, initial_stamp, jobs));
    Ok(())
}

async fn run(
    config: WatchConfig,
    watched_path: PathBuf,
    mut previous_stamp: SourceStamp,
    jobs: web::JobStore,
) {
    let mut interval = tokio::time::interval(WATCH_INTERVAL);
    loop {
        interval.tick().await;
        let current_stamp = match inspect(&watched_path) {
            Ok(stamp) => stamp,
            Err(error) => {
                jobs.set_error(error.to_string()).await;
                continue;
            }
        };
        if current_stamp == previous_stamp {
            continue;
        }
        previous_stamp = current_stamp;

        let render_config = config.clone();
        match tokio::task::spawn_blocking(move || rerender(&render_config)).await {
            Ok(Ok(rendered)) => jobs.replace_render(rendered).await,
            Ok(Err(error)) => jobs.set_error(error.to_string()).await,
            Err(error) => {
                jobs.set_error(format!("watched render task failed: {error}"))
                    .await;
            }
        }
    }
}

fn rerender(config: &WatchConfig) -> Result<escpost_render::TracedRenderResult, CliError> {
    let input = source::load(&config.source, config.format)?;
    let profile = profiles::load(&config.profile)?;
    let options = RenderOptions {
        scale: config.scale,
        antialias: config.antialias,
        ..RenderOptions::default()
    };
    let rendered = render_with_trace_and_options(&input.bytes, profile, &options)
        .map_err(|error| CliError::Render(error.to_string()))?;
    if let Some(path) = &config.output {
        output::write_single(&rendered.render, path, config.sheet)?;
    }
    if let Some(directory) = &config.output_dir {
        output::write_all(&rendered.render, directory)?;
    }
    Ok(rendered)
}

fn inspect(path: &PathBuf) -> Result<SourceStamp, CliError> {
    let metadata = fs::metadata(path).map_err(|source| CliError::InspectWatchedSource {
        path: path.clone(),
        source,
    })?;
    Ok(SourceStamp {
        modified: metadata.modified().ok(),
        length: metadata.len(),
    })
}
