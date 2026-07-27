//! QR matrix generation behind a small renderer-owned boundary.
//!
//! ESC/POS parsing, printer state, dot scaling, and paper movement stay in the
//! renderer. This module only turns bytes into the unscaled dark/light matrix.

use qrcode::{Color, EcLevel, QrCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrorCorrection {
    Low,
    Medium,
    Quartile,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QrError {
    DataTooLong,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EncodedQr {
    pub(crate) width: usize,
    pub(crate) modules: Vec<bool>,
}

pub(crate) fn encode(data: &[u8], error_correction: ErrorCorrection) -> Result<EncodedQr, QrError> {
    let level = match error_correction {
        ErrorCorrection::Low => EcLevel::L,
        ErrorCorrection::Medium => EcLevel::M,
        ErrorCorrection::Quartile => EcLevel::Q,
        ErrorCorrection::High => EcLevel::H,
    };
    let code =
        QrCode::with_error_correction_level(data, level).map_err(|_| QrError::DataTooLong)?;

    Ok(EncodedQr {
        width: code.width(),
        // qrcode returns the symbol matrix itself. It deliberately excludes
        // the quiet zone because ESC/POS printers leave that to receipt layout.
        modules: code
            .to_colors()
            .into_iter()
            .map(|color| color == Color::Dark)
            .collect(),
    })
}
