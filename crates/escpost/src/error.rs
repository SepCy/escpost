//! Renderer error types.

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    InputBytes,
    CommandPayloadBytes,
    SheetWidthDots,
    SheetHeightDots,
    Sheets,
    TotalDots,
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("truncated {command} command at byte offset {offset}")]
    TruncatedCommand {
        command: &'static str,
        offset: usize,
    },

    #[error("unsupported ESC/POS command ESC {command:#04x} at byte offset {offset}")]
    UnsupportedEscCommand { command: u8, offset: usize },

    #[error("unsupported ESC/POS command GS {command:#04x} at byte offset {offset}")]
    UnsupportedGsCommand { command: u8, offset: usize },

    #[error("unsupported ESC * bit-image mode {mode} at byte offset {offset}")]
    UnsupportedBitImageMode { mode: u8, offset: usize },

    #[error("unsupported character font {font} at byte offset {offset}")]
    UnsupportedCharacterFont { font: u8, offset: usize },

    #[error("unsupported underline mode {mode} at byte offset {offset}")]
    UnsupportedUnderlineMode { mode: u8, offset: usize },

    #[error("unsupported justification {justification} at byte offset {offset}")]
    UnsupportedJustification { justification: u8, offset: usize },

    #[error("unsupported code page {code_page} ({encoding}) at byte offset {offset}")]
    UnsupportedCodePage {
        code_page: u8,
        encoding: String,
        offset: usize,
    },

    #[error("unsupported international character set {character_set} at byte offset {offset}")]
    UnsupportedInternationalCharacterSet { character_set: u8, offset: usize },

    #[error(
        "byte {byte:#04x} is undefined in code page {code_page} ({encoding}) at byte offset {offset}"
    )]
    UndefinedCodePageByte {
        byte: u8,
        code_page: u8,
        encoding: String,
        offset: usize,
    },

    #[error("bundled font has no glyph for {character:?} at byte offset {offset}")]
    MissingGlyph { character: char, offset: usize },

    #[error("unsupported GS v 0 raster bit-image mode {mode} at byte offset {offset}")]
    UnsupportedRasterBitImageMode { mode: u8, offset: usize },

    #[error("unsupported graphics function {function} at byte offset {offset}")]
    UnsupportedGraphicsFunction { function: u8, offset: usize },

    #[error("invalid {system} barcode data at byte offset {offset}: {reason}")]
    InvalidBarcodeData {
        system: &'static str,
        offset: usize,
        reason: &'static str,
    },

    #[error("invalid barcode parameter {parameter}={value} at byte offset {offset}")]
    InvalidBarcodeParameter {
        parameter: &'static str,
        value: u8,
        offset: usize,
    },

    #[error("unsupported QR function {function} at byte offset {offset}")]
    UnsupportedQrFunction { function: u8, offset: usize },

    #[error("unsupported QR model {model} at byte offset {offset}")]
    UnsupportedQrModel { model: u8, offset: usize },

    #[error("invalid QR parameter {parameter}={value} at byte offset {offset}")]
    InvalidQrParameter {
        parameter: &'static str,
        value: u8,
        offset: usize,
    },

    #[error("invalid QR data at byte offset {offset}: {reason}")]
    InvalidQrData { offset: usize, reason: &'static str },

    #[error("QR data storage is empty at byte offset {offset}")]
    QrDataEmpty { offset: usize },

    #[error("invalid graphics parameter {parameter}={value} at byte offset {offset}")]
    InvalidGraphicsParameter {
        parameter: &'static str,
        value: u64,
        offset: usize,
    },

    #[error(
        "invalid graphics dimensions {width_dots} dot(s) by {height_dots} dot(s) \
         at byte offset {offset}"
    )]
    InvalidGraphicsDimensions {
        width_dots: usize,
        height_dots: usize,
        offset: usize,
    },

    #[error("graphics payload has {actual} byte(s), expected {expected}, at byte offset {offset}")]
    InvalidGraphicsPayloadLength {
        expected: usize,
        actual: usize,
        offset: usize,
    },

    #[error("graphics print buffer is empty at byte offset {offset}")]
    GraphicsBufferEmpty { offset: usize },

    #[error("{command} requires the beginning of a line at byte offset {offset}")]
    CommandRequiresBeginningOfLine {
        command: &'static str,
        offset: usize,
    },

    #[error("unsupported GS V cut mode {mode} at byte offset {offset}")]
    UnsupportedCutMode { mode: u8, offset: usize },

    #[error("unsupported ESC p drawer connector {connector} at byte offset {offset}")]
    UnsupportedDrawerConnector { connector: u8, offset: usize },

    #[error("{command} is not supported by printer profile {profile:?} at byte offset {offset}")]
    CommandUnsupportedByProfile {
        command: &'static str,
        profile: String,
        offset: usize,
    },

    #[error(
        "invalid GS v 0 raster dimensions {width_bytes} byte(s) by {height_dots} dot(s) \
         at byte offset {offset}"
    )]
    InvalidRasterBitImageDimensions {
        width_bytes: usize,
        height_dots: usize,
        offset: usize,
    },

    #[error("unsupported data byte {byte:#04x} at byte offset {offset}")]
    UnsupportedDataByte { byte: u8, offset: usize },

    #[error("render limit {kind:?} exceeded: value {value}, limit {limit}")]
    LimitExceeded {
        kind: LimitKind,
        value: u64,
        limit: u64,
    },

    #[error("could not encode the rendered sheet as PNG")]
    EncodePng(#[from] png::EncodingError),
}
