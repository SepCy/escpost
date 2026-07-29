use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum CliError {
    #[error("could not load the embedded printer profiles: {0}")]
    LoadProfiles(String),

    #[error("printer profile is required; pass --profile REFERENCE for generic rendering")]
    MissingProfile,

    #[error("unknown printer profile {0:?}")]
    UnknownProfile(String),

    #[error("could not select a printer profile: {0}")]
    ProfilePrompt(String),

    #[error(
        "an output destination is required; pass --output <PNG>, --output-dir <DIRECTORY>, or --web"
    )]
    MissingOutput,

    #[error("could not read ESC/POS input {path}: {source}")]
    ReadInput {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not read ESC/POS input from stdin: {0}")]
    ReadStdin(std::io::Error),

    #[error("directory is not a recognized ESCPost case: {0}")]
    UnrecognizedDirectory(PathBuf),

    #[error("invalid case manifest {path}: {message}")]
    InvalidCaseManifest { path: PathBuf, message: String },

    #[error("unsupported case schema version {0}")]
    UnsupportedCaseSchema(u32),

    #[error("case field {0} must not be empty")]
    EmptyCaseField(&'static str),

    #[error("hexadecimal input is not UTF-8: {0}")]
    InvalidHexEncoding(#[from] std::str::Utf8Error),

    #[error("invalid hexadecimal byte {token:?} at token {position}")]
    InvalidHexByte { token: String, position: usize },

    #[error("could not render ESC/POS input: {0}")]
    Render(String),

    #[error("single-PNG output requires exactly one sheet, but rendering produced {0}")]
    MultipleSheets(usize),

    #[error("sheet {requested} does not exist; rendering produced {available} sheet(s)")]
    SheetOutOfRange { requested: usize, available: usize },

    #[error("could not write PNG output {path}: {source}")]
    WriteOutput {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not create output directory {path}: {source}")]
    CreateOutputDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not serialize the output manifest: {0}")]
    SerializeManifest(#[from] serde_json::Error),

    #[error("could not write PNG output to stdout: {0}")]
    WriteStdout(std::io::Error),

    #[error("refusing to write binary PNG data to an interactive terminal")]
    BinaryOutputToTerminal,

    #[error("PNG stdout cannot be combined with a long-running web viewer")]
    StdoutWithWeb,

    #[error("could not bind web viewer to {address}: {source}")]
    BindWeb {
        address: std::net::SocketAddr,
        source: std::io::Error,
    },

    #[error("no loopback web port from 9000 through 9099 is available")]
    NoAutomaticWebPort,

    #[error("web viewer failed: {0}")]
    ServeWeb(std::io::Error),

    #[error("could not open the default browser: {0}")]
    OpenBrowser(String),

    #[error("watch mode requires a filesystem source, not stdin")]
    WatchStdin,

    #[error("could not enumerate USB devices: {0}")]
    EnumerateUsb(nusb::Error),

    #[error("no USB device matches vendor {vendor_id:#06x} and product {product_id:#06x}")]
    UsbDeviceNotFound { vendor_id: u16, product_id: u16 },

    #[error(
        "{count} USB devices match vendor {vendor_id:#06x} and product {product_id:#06x}; refusing to choose one implicitly"
    )]
    AmbiguousUsbDevices {
        vendor_id: u16,
        product_id: u16,
        count: usize,
    },

    #[error("USB OUT endpoint must be between 0x01 and 0x0f, got {0:#04x}")]
    InvalidUsbOutEndpoint(u8),

    #[error("could not open USB device {vendor_id:#06x}:{product_id:#06x}: {source}")]
    OpenUsbDevice {
        vendor_id: u16,
        product_id: u16,
        source: nusb::Error,
    },

    #[error(
        "could not inspect the active configuration of USB device {vendor_id:#06x}:{product_id:#06x}: {source}"
    )]
    InspectUsbConfiguration {
        vendor_id: u16,
        product_id: u16,
        source: nusb::ActiveConfigurationError,
    },

    #[error("could not detach and claim USB interface {interface}: {source}")]
    ClaimUsbInterface { interface: u8, source: nusb::Error },

    #[error(
        "could not open bulk OUT endpoint {endpoint:#04x} on USB interface {interface}: {source}"
    )]
    OpenUsbOutEndpoint {
        interface: u8,
        endpoint: u8,
        source: nusb::Error,
    },

    #[error("could not write ESC/POS bytes to USB endpoint {endpoint:#04x}: {source}")]
    WriteUsb {
        endpoint: u8,
        source: std::io::Error,
    },

    #[error("could not finish the USB write on endpoint {endpoint:#04x}: {source}")]
    FlushUsb {
        endpoint: u8,
        source: std::io::Error,
    },

    #[error("printer is required; pass --printer <NAME>")]
    MissingPrintPrinter,

    #[error("printer {0:?} is not configured; use `escpost printers list` to see available names")]
    UnknownConfiguredPrinter(String),

    #[error("timed out while connecting to network printer {0}")]
    ConnectNetworkPrinterTimeout(String),

    #[error("could not connect to network printer {target}: {source}")]
    ConnectNetworkPrinter {
        target: String,
        source: std::io::Error,
    },

    #[error("timed out while writing to network printer {0}")]
    WriteNetworkPrinterTimeout(String),

    #[error("could not write to network printer {target}: {source}")]
    WriteNetworkPrinter {
        target: String,
        source: std::io::Error,
    },

    #[error("could not write command output: {0}")]
    WriteHumanOutput(std::io::Error),

    #[error("could not read printer configuration {path}: {source}")]
    ReadPrinterConfiguration {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid printer configuration {path}: {message}")]
    InvalidPrinterConfiguration { path: PathBuf, message: String },

    #[error("printer name is required")]
    MissingPrinterName,

    #[error("printer name must not be blank")]
    BlankPrinterName,

    #[error("printer transport is required")]
    MissingPrinterTransport,

    #[error("USB printer registration requires an interactive terminal")]
    UsbRegistrationRequiresInteractive,

    #[error("--host is only valid for network printers")]
    NetworkHostForUsbPrinter,

    #[error("--port is only valid for network printers")]
    NetworkPortForUsbPrinter,

    #[error("no unconfigured connected USB printers were found")]
    NoUnconfiguredUsbPrinters,

    #[error("network printer host is required")]
    MissingPrinterHost,

    #[error("network printer host must not be blank")]
    BlankPrinterHost,

    #[error("could not read printer information: {0}")]
    PrinterPrompt(String),

    #[error("printer port must be between 1 and 65535")]
    InvalidPrinterPort,

    #[error("printer profile must not be blank")]
    BlankPrinterProfile,

    #[error("printer {0:?} is already configured")]
    PrinterAlreadyConfigured(String),

    #[error("could not create printer configuration directory {path}: {source}")]
    CreatePrinterConfigurationDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not serialize printer configuration: {0}")]
    SerializePrinterConfiguration(String),

    #[error("could not write printer configuration {path}: {source}")]
    WritePrinterConfiguration {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("the operating system did not provide a user configuration directory")]
    NoUserConfigDirectory,

    #[error("could not inspect watched source {path}: {source}")]
    InspectWatchedSource {
        path: PathBuf,
        source: std::io::Error,
    },
}
