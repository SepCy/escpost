use std::io::{self, IsTerminal};
use std::path::Path;

use escpost::{RenderOptions, render_with_options};

use crate::cli::RenderArgs;
use crate::error::CliError;
use crate::{output, profiles, source};

pub(crate) async fn run(arguments: RenderArgs, non_interactive: bool) -> Result<(), CliError> {
    let web_enabled =
        arguments.web || arguments.browser || arguments.web_listen.is_some() || arguments.watch;
    let binary_stdout = arguments.output.as_deref() == Some(Path::new("-"));
    if arguments.output.is_none() && arguments.output_dir.is_none() && !web_enabled {
        return Err(CliError::MissingOutput);
    }
    if binary_stdout && web_enabled {
        return Err(CliError::StdoutWithWeb);
    }
    if arguments.watch {
        // Reject stdin before trying to consume it. A developer should get the
        // invalid-invocation error immediately, even if a producer never closes.
        source::watch_path(&arguments.source)?;
    }

    let input = source::load(&arguments.source, arguments.format)?;
    let can_prompt = !non_interactive
        && !binary_stdout
        && arguments.source != Path::new("-")
        && io::stdin().is_terminal()
        && io::stderr().is_terminal();
    let profile_id = profiles::resolve(arguments.profile, input.profile, can_prompt)?;
    let profile = profiles::load(&profile_id)?;
    let options = RenderOptions {
        scale: arguments.scale,
        antialias: arguments.antialias,
        ..RenderOptions::default()
    };
    let rendered = render_with_options(&input.bytes, profile, &options)
        .map_err(|error| CliError::Render(error.to_string()))?;
    if !binary_stdout {
        eprintln!("Profile: {profile_id}");
    }
    // Non-fatal diagnostics (e.g. a cut dropped on a profile with no cutter) go
    // to stderr so they surface even when the rendered bytes are piped to stdout.
    for warning in &rendered.warnings {
        eprintln!("warning: {warning}");
    }

    if let Some(output_path) = &arguments.output {
        output::write_single(&rendered, output_path, arguments.sheet)?;
    }
    if let Some(output_directory) = &arguments.output_dir {
        output::write_all(&rendered, output_directory)?;
    }
    if web_enabled {
        let listener = crate::web::bind(arguments.web_listen).await?;
        let jobs = crate::web::JobStore::with_render(rendered);
        if arguments.watch {
            crate::watch::start(
                crate::watch::WatchConfig {
                    source: arguments.source,
                    format: arguments.format,
                    profile: profile_id,
                    output: arguments.output,
                    output_dir: arguments.output_dir,
                    sheet: arguments.sheet,
                    scale: arguments.scale,
                    antialias: arguments.antialias,
                },
                jobs.clone(),
            )?;
        }
        crate::web::serve(listener, jobs, arguments.browser).await?;
    }
    Ok(())
}
