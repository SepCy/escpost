use std::io::{self, IsTerminal};
use std::path::Path;

use escpost::render;

use crate::cli::RenderArgs;
use crate::error::CliError;
use crate::{output, profiles, source};

pub(crate) fn run(arguments: RenderArgs, non_interactive: bool) -> Result<(), CliError> {
    if arguments.output.is_none() && arguments.output_dir.is_none() {
        return Err(CliError::MissingOutput);
    }

    let input = source::load(&arguments.source, arguments.format)?;
    let can_prompt = !non_interactive
        && arguments.source != Path::new("-")
        && io::stdin().is_terminal()
        && io::stderr().is_terminal();
    let profile_id = profiles::resolve(arguments.profile, input.profile, can_prompt)?;
    let profile = profiles::load(&profile_id)?;
    let rendered =
        render(&input.bytes, profile).map_err(|error| CliError::Render(error.to_string()))?;

    if let Some(output_path) = arguments.output {
        output::write_single(&rendered, &output_path, arguments.sheet)?;
    }
    if let Some(output_directory) = arguments.output_dir {
        output::write_all(&rendered, &output_directory)?;
    }
    Ok(())
}
