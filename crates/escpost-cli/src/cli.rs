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

    /// Send a known ESC/POS byte stream unchanged to a configured printer.
    Print(PrintArgs),

    /// Capture RAW TCP print jobs and preview them in the web viewer.
    Serve(ServeArgs),

    /// List available printers and manage discovery or pairing.
    Printers(PrintersArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ServeArgs {
    /// Printer profile used to render captured jobs.
    #[arg(long, default_value = "REFERENCE")]
    pub(crate) profile: String,

    /// Address for the RAW TCP printer. When omitted, the first free loopback
    /// port from 9100 through 9109 is used.
    #[arg(long)]
    pub(crate) listen: Option<SocketAddr>,

    /// Address for the web viewer. When omitted, the first free loopback port
    /// from 9000 through 9099 is used.
    #[arg(long)]
    pub(crate) web_listen: Option<SocketAddr>,

    /// Open the web viewer in the default browser.
    #[arg(long)]
    pub(crate) browser: bool,
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

#[derive(Debug, Args)]
pub(crate) struct PrintArgs {
    /// Raw ESC/POS file, hexadecimal file, case directory, or - for stdin.
    pub(crate) source: PathBuf,

    /// Input representation.
    #[arg(long, value_enum, default_value_t = InputFormat::Auto)]
    pub(crate) format: InputFormat,

    /// Configured printer name.
    #[arg(long)]
    pub(crate) printer: Option<String>,

    /// Read printer configuration from this exact file.
    #[arg(long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct PrintersArgs {
    /// Read printer configuration from this exact file.
    #[arg(long, global = true, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: PrintersCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PrintersCommand {
    /// List currently usable printers.
    List(ListPrintersArgs),

    /// Register a printer in the local configuration.
    Add(AddPrinterArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ListPrintersArgs {
    /// Show only one connection transport.
    #[arg(long, value_enum)]
    pub(crate) transport: Option<InventoryTransport>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum InventoryTransport {
    Usb,
    Network,
}

#[derive(Debug, Args)]
pub(crate) struct AddPrinterArgs {
    /// Developer-assigned printer name.
    pub(crate) name: Option<String>,

    /// Connection transport.
    #[arg(long, value_enum)]
    pub(crate) transport: Option<PrinterTransport>,

    /// Network hostname or IP address.
    #[arg(long)]
    pub(crate) host: Option<String>,

    /// Raw TCP port. Defaults to 9100.
    #[arg(long)]
    pub(crate) port: Option<u16>,

    /// Select a USB printer by vendor ID (decimal or `0x`-prefixed hexadecimal).
    #[arg(long, value_parser = parse_usb_id)]
    pub(crate) vendor_id: Option<u16>,

    /// Select a USB printer by product ID (decimal or `0x`-prefixed hexadecimal).
    #[arg(long, value_parser = parse_usb_id)]
    pub(crate) product_id: Option<u16>,

    /// Select a USB printer by exact serial number.
    #[arg(long)]
    pub(crate) serial: Option<String>,

    /// Optional rendering profile.
    #[arg(long)]
    pub(crate) profile: Option<String>,
}

/// Parse a USB vendor or product identifier given in decimal or `0x`-prefixed
/// hexadecimal, matching how the same identifiers are stored in `printers.toml`.
fn parse_usb_id(value: &str) -> Result<u16, String> {
    let text = value.trim();
    let parsed = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hexadecimal) => u16::from_str_radix(hexadecimal, 16),
        None => text.parse::<u16>(),
    };
    parsed.map_err(|_| {
        format!("expected a decimal or 0x-prefixed 16-bit USB identifier, found `{value}`")
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum PrinterTransport {
    Usb,
    Network,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(crate) enum InputFormat {
    #[default]
    Auto,
    Binary,
    Hex,
}
