//! Dot-accurate ESC/POS rendering.

mod barcode;
mod command;
mod databar;
mod error;
mod font;
mod graphics;
mod international;
mod qr;
mod state;
mod surface;
mod symbols;
mod text;

pub use error::{LimitKind, RenderError, RenderWarning};
pub use surface::MonoSurface;

use command::{execute_esc_command, execute_gs_command};
use escpost_profiles::PrinterProfile;
use state::PrinterState;
use surface::{RenderSurface, encode_png};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    pub limits: RenderLimits,
    /// Subpixels per dot. `1` is dot resolution; `N > 1` renders at `N ×`
    /// density. Independent of `antialias`.
    pub scale: u32,
    /// When `false`, glyph coverage is thresholded to hard dots and the sheet
    /// encodes as a faithful 1-bit PNG (the printer's real output, used by
    /// golden tests). When `true`, glyph edges keep their coverage and the sheet
    /// encodes as an 8-bit grayscale preview — cosmetic only, never what prints.
    pub antialias: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            limits: RenderLimits::default(),
            scale: 1,
            antialias: false,
        }
    }
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
    /// Non-fatal diagnostics from an otherwise successful render, such as a cut
    /// requested on a profile whose printer has no cutter.
    pub warnings: Vec<RenderWarning>,
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
    let rendered = render_surfaces::<MonoSurface>(data, profile, options)?;
    let mut sheets = Vec::new();
    for surface in rendered.surfaces {
        let png = encode_png(&surface)?;
        sheets.push(RenderedSheet { surface, png });
    }

    Ok(RenderResult {
        sheets,
        device_events: rendered.device_events,
        warnings: rendered.warnings,
        metadata: RenderMetadata {
            renderer_version: env!("CARGO_PKG_VERSION"),
            profile_id: profile.id.clone(),
            canonical_profile_sha256: profile.canonical_profile_sha256.clone(),
        },
    })
}

struct SurfaceRender<S> {
    surfaces: Vec<S>,
    device_events: Vec<DeviceEvent>,
    warnings: Vec<RenderWarning>,
}

fn render_surfaces<S: RenderSurface>(
    data: &[u8],
    profile: &PrinterProfile,
    options: &RenderOptions,
) -> Result<SurfaceRender<S>, RenderError> {
    validate_initial_limits(data, profile, &options.limits)?;
    let mut state = PrinterState::new(profile, options.limits, options.scale, options.antialias);
    let mut offset = 0;

    while offset < data.len() {
        let byte = data[offset];
        state.begin_command(offset);
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
    let warnings = std::mem::take(&mut state.warnings);
    Ok(SurfaceRender {
        surfaces: state.into_surfaces()?,
        device_events,
        warnings,
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

#[cfg(test)]
mod trace_spike_tests {
    use super::{RenderOptions, render, render_surfaces};
    use crate::surface::tracing::TracingSurface;
    use escpost_profiles::compile_profile;

    const CAPABILITIES_JSON: &[u8] =
        include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
    const REFERENCE_PROFILE: &str = include_str!("../../../profiles/REFERENCE/profile.toml");

    #[test]
    fn traced_render_attributes_centered_text_to_its_input_byte() {
        let profile = compile_profile(CAPABILITIES_JSON, REFERENCE_PROFILE)
            .expect("the reference profile should compile");
        let input = [0x1b, b'a', 1, b'A', 0x0a];

        let ordinary = render(&input, &profile).expect("ordinary rendering should succeed");
        let traced = render_surfaces::<TracingSurface>(&input, &profile, &RenderOptions::default())
            .expect("traced rendering should succeed");
        let traced_sheet = &traced.surfaces[0];

        assert_eq!(traced_sheet.inner, ordinary.sheets[0].surface);
        let text_regions = traced_sheet
            .painted_regions
            .iter()
            .filter(|region| region.command_offset == 3)
            .collect::<Vec<_>>();
        assert!(!text_regions.is_empty());
        assert!(
            text_regions
                .iter()
                .all(|region| region.x >= 282 && region.x < 294)
        );
        assert!(
            traced_sheet
                .painted_regions
                .iter()
                .all(|region| region.command_offset != 4),
            "LF must move the text without taking ownership of its pixels"
        );
    }
}
