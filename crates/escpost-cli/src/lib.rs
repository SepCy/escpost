//! Native ESCPost developer command-line interface.

mod cli;
mod configuration;
mod error;
mod output;
mod print;
mod printers;
mod profiles;
mod render;
mod source;
mod watch;
mod web;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::error::CliError;

pub async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Render(arguments) => render::run(arguments, cli.non_interactive).await,
        Command::Print(arguments) => print::run(arguments),
        Command::Printers(arguments) => printers::run(arguments),
    }
}
