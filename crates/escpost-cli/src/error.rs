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

    #[error("an output destination is required; pass --output <PNG>")]
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

    #[error("could not inspect watched source {path}: {source}")]
    InspectWatchedSource {
        path: PathBuf,
        source: std::io::Error,
    },
}
