//! Dot-accurate ESC/POS rendering.

mod barcode;
mod command;
mod databar;
mod error;
mod graphics;
mod international;
mod qr;
mod state;
mod surface;
mod symbols;
mod text;

pub use error::{LimitKind, RenderError};
pub use surface::MonoSurface;

use command::{execute_esc_command, execute_gs_command};
use escpost_profiles::{Approximation, PrinterProfile};
use state::PrinterState;
use surface::encode_png;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderOptions {
    pub limits: RenderLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderLimits {
    pub max_input_bytes: usize,
    pub max_command_payload_bytes: usize,
    pub max_sheet_width_dots: u32,
    pub max_sheet_height_dots: u32,
    pub max_sheets: usize,
    pub max_total_dots: u64,
}

impl Default for RenderLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 16 * 1024 * 1024,
            max_command_payload_bytes: 8 * 1024 * 1024,
            max_sheet_width_dots: 4096,
            max_sheet_height_dots: 1_000_000,
            max_sheets: 32,
            max_total_dots: 200_000_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderResult {
    pub sheets: Vec<RenderedSheet>,
    pub device_events: Vec<DeviceEvent>,
    pub approximations: Vec<Approximation>,
    pub metadata: RenderMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEvent {
    CashDrawerPulse {
        connector: u8,
        on_time_units: u8,
        off_time_units: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderMetadata {
    pub renderer_version: &'static str,
    pub profile_id: String,
    pub canonical_profile_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedSheet {
    pub surface: MonoSurface,
    pub png: Vec<u8>,
}

pub fn render(data: &[u8], profile: &PrinterProfile) -> Result<RenderResult, RenderError> {
    render_with_options(data, profile, &RenderOptions::default())
}

pub fn render_with_options(
    data: &[u8],
    profile: &PrinterProfile,
    options: &RenderOptions,
) -> Result<RenderResult, RenderError> {
    validate_initial_limits(data, profile, &options.limits)?;
    let mut state = PrinterState::new(profile, options.limits);
    let mut offset = 0;

    while offset < data.len() {
        let byte = data[offset];
        if byte != 0x0a {
            state.clear_pending_gs_v_0_lf();
        }

        offset += match byte {
            0x09 => {
                state.horizontal_tab()?;
                1
            }
            0x0a => {
                state.line_feed()?;
                1
            }
            0x0d => {
                state.carriage_return()?;
                1
            }
            0x1b => execute_esc_command(&data[offset..], offset, &mut state)?,
            0x1d => execute_gs_command(&data[offset..], offset, &mut state)?,
            // ESC/POS code pages retain ASCII in 20h–7Eh and assign printable
            // characters to 80h–FFh. Control bytes remain parser input.
            byte @ (0x20..=0x7e | 0x80..=0xff) => {
                state.print_byte(byte, offset)?;
                1
            }
            byte => return Err(RenderError::UnsupportedDataByte { byte, offset }),
        };
    }

    let device_events = std::mem::take(&mut state.device_events);
    let mut sheets = Vec::new();
    for surface in state.into_surfaces()? {
        let png = encode_png(&surface)?;
        sheets.push(RenderedSheet { surface, png });
    }

    Ok(RenderResult {
        sheets,
        device_events,
        approximations: profile.approximations.clone(),
        metadata: RenderMetadata {
            renderer_version: env!("CARGO_PKG_VERSION"),
            profile_id: profile.id.clone(),
            canonical_profile_sha256: profile.canonical_profile_sha256.clone(),
        },
    })
}

fn validate_initial_limits(
    data: &[u8],
    profile: &PrinterProfile,
    limits: &RenderLimits,
) -> Result<(), RenderError> {
    if data.len() > limits.max_input_bytes {
        return Err(RenderError::LimitExceeded {
            kind: LimitKind::InputBytes,
            value: data.len() as u64,
            limit: limits.max_input_bytes as u64,
        });
    }
    if profile.geometry.printable_width_dots > limits.max_sheet_width_dots {
        return Err(RenderError::LimitExceeded {
            kind: LimitKind::SheetWidthDots,
            value: u64::from(profile.geometry.printable_width_dots),
            limit: u64::from(limits.max_sheet_width_dots),
        });
    }
    Ok(())
}
