use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "escpost",
    version,
    about = "The ESC/POS Tools and Workbench",
    subcommand_required = true,
    arg_required_else_help = true
)]
pub(crate) struct Cli {
    /// Never prompt for missing values.
    #[arg(long, global = true)]
    pub(crate) non_interactive: bool,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Render a known ESC/POS byte stream.
    Render(RenderArgs),
}

#[derive(Debug, Args)]
pub(crate) struct RenderArgs {
    /// Raw ESC/POS file, hexadecimal file, case directory, or - for stdin.
    pub(crate) source: PathBuf,

    /// Input representation.
    #[arg(long, value_enum, default_value_t = InputFormat::Auto)]
    pub(crate) format: InputFormat,

    /// Printer profile used to interpret the input.
    #[arg(long)]
    pub(crate) profile: Option<String>,

    /// Write one PNG to this path, or use - for stdout.
    #[arg(short = 'o', long = "output", conflicts_with = "output_dir")]
    pub(crate) output: Option<PathBuf>,

    /// Write every rendered sheet and a manifest to this directory.
    #[arg(long, conflicts_with = "output")]
    pub(crate) output_dir: Option<PathBuf>,

    /// Select one one-based sheet for single-PNG output.
    #[arg(long, conflicts_with = "output_dir", requires = "output")]
    pub(crate) sheet: Option<usize>,

    /// Start the local web viewer and keep running.
    #[arg(long)]
    pub(crate) web: bool,

    /// Start the web viewer and open it in the default browser.
    #[arg(long)]
    pub(crate) browser: bool,

    /// Exact address for the web viewer.
    #[arg(long)]
    pub(crate) web_listen: Option<SocketAddr>,

    /// Rerender a filesystem source whenever it changes.
    #[arg(long)]
    pub(crate) watch: bool,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(crate) enum InputFormat {
    #[default]
    Auto,
    Binary,
    Hex,
}
