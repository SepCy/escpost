//! Dot-accurate ESC/POS rendering.

mod barcode;
mod databar;
mod international;
mod qr;

use encoding_rs::{
    Encoding, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252, WINDOWS_1253, WINDOWS_1254, WINDOWS_1255,
    WINDOWS_1256, WINDOWS_1257, WINDOWS_1258,
};
use escpos2png_profiles::{
    Approximation, BarcodeSystem, CarriageReturnMode, Font as ProfileFont, PrinterProfile,
};
use fontdue::{Font, FontSettings};
use oem_cp::{
    Cp437, Cp720, Cp737, Cp775, Cp850, Cp852, Cp855, Cp857, Cp858, Cp860, Cp861, Cp862, Cp863,
    Cp864, Cp865, Cp866, Cp869, Cp874,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
use thiserror::Error;

const DEFAULT_FONT_BYTES: &[u8] =
    include_bytes!("../../../assets/fonts/noto-sans-mono/NotoSansMono-Regular.ttf");
const GLYPH_ALPHA_THRESHOLD: u8 = 128;
const DEFAULT_BARCODE_HEIGHT_DOTS: u32 = 162;
const DEFAULT_BARCODE_MODULE_WIDTH_DOTS: u32 = 3;
const DEFAULT_QR_MODULE_SIZE_DOTS: u32 = 3;
const MAX_QR_STORE_PARAMETER_BYTES: usize = 7092;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    InputBytes,
    CommandPayloadBytes,
    SheetWidthDots,
    SheetHeightDots,
    Sheets,
    TotalDots,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Justification {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum HriPosition {
    #[default]
    None,
    Above,
    Below,
    AboveAndBelow,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonoSurface {
    width: u32,
    height: u32,
    dots: Vec<bool>,
}

impl MonoSurface {
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn is_printed(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }

        self.dots[(y * self.width + x) as usize]
    }
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
        offset += match data[offset] {
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

fn encode_png(surface: &MonoSurface) -> Result<Vec<u8>, png::EncodingError> {
    let row_bytes = surface.width.div_ceil(8);
    let mut pixels = vec![0xff; (row_bytes * surface.height) as usize];

    // PNG grayscale-1 stores eight left-to-right pixels per byte, with zero
    // representing black. MonoSurface uses the more convenient `true = ink`.
    for y in 0..surface.height {
        for x in 0..surface.width {
            if surface.is_printed(x, y) {
                let index = (y * row_bytes + x / 8) as usize;
                pixels[index] &= !(0x80 >> (x % 8));
            }
        }
    }

    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, surface.width, surface.height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::One);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&pixels)?;
    }

    Ok(encoded)
}

fn execute_esc_command(
    data: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<usize, RenderError> {
    let Some(command) = data.get(1).copied() else {
        return Err(RenderError::TruncatedCommand {
            command: "ESC",
            offset,
        });
    };

    match command {
        0x40 => {
            state.initialize();
            Ok(2)
        }
        0x20 => {
            let Some(spacing) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC SP",
                    offset,
                });
            };
            state.set_right_side_character_spacing(spacing);
            Ok(3)
        }
        0x24 => {
            let Some((&low, &high)) = data.get(2).zip(data.get(3)) else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC $",
                    offset,
                });
            };
            state.set_absolute_print_position(u16::from_le_bytes([low, high]));
            Ok(4)
        }
        0x21 => {
            let Some(mode) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC !",
                    offset,
                });
            };
            state.set_print_mode(mode);
            Ok(3)
        }
        0x2a => execute_esc_star(data, offset, state),
        0x2d => {
            let Some(mode) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC -",
                    offset,
                });
            };
            let thickness = match mode {
                0 | 48 => 0,
                1 | 49 => 1,
                2 | 50 => 2,
                mode => return Err(RenderError::UnsupportedUnderlineMode { mode, offset }),
            };
            state.set_underline(thickness);
            Ok(3)
        }
        0x32 => {
            state.restore_default_line_spacing();
            Ok(2)
        }
        0x33 => {
            let Some(spacing) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC 3",
                    offset,
                });
            };
            state.set_line_spacing(spacing);
            Ok(3)
        }
        0x44 => execute_esc_d(data, offset, state),
        0x45 => {
            let Some(emphasis) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC E",
                    offset,
                });
            };
            state.set_emphasis(emphasis & 0x01 != 0);
            Ok(3)
        }
        0x4a => {
            let Some(distance) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC J",
                    offset,
                });
            };
            state.print_and_feed_motion_units(distance)?;
            Ok(3)
        }
        0x4d => {
            let Some(font) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC M",
                    offset,
                });
            };
            match font {
                0 | 48 => state.select_font_a(),
                1 | 49 => state.select_font_b(),
                font => return Err(RenderError::UnsupportedCharacterFont { font, offset }),
            }
            Ok(3)
        }
        0x52 => {
            let Some(character_set) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC R",
                    offset,
                });
            };
            if character_set > 17 {
                return Err(RenderError::UnsupportedInternationalCharacterSet {
                    character_set,
                    offset,
                });
            }
            state.select_international_character_set(character_set);
            Ok(3)
        }
        0x5c => {
            let Some((&low, &high)) = data.get(2).zip(data.get(3)) else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC \\",
                    offset,
                });
            };
            state.set_relative_print_position(i16::from_le_bytes([low, high]));
            Ok(4)
        }
        0x61 => {
            let Some(justification) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC a",
                    offset,
                });
            };
            let justification = match justification {
                0 | 48 => Justification::Left,
                1 | 49 => Justification::Center,
                2 | 50 => Justification::Right,
                justification => {
                    return Err(RenderError::UnsupportedJustification {
                        justification,
                        offset,
                    });
                }
            };
            state.set_justification(justification);
            Ok(3)
        }
        0x64 => {
            let Some(lines) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC d",
                    offset,
                });
            };
            state.feed_lines(lines)?;
            Ok(3)
        }
        0x70 => {
            let Some((&connector, timing)) = data.get(2).zip(data.get(3..5)) else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC p",
                    offset,
                });
            };
            state.drawer_pulse(connector, timing[0], timing[1], offset)?;
            Ok(5)
        }
        0x74 => {
            let Some(code_page) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC t",
                    offset,
                });
            };
            if state.code_page_encoding(code_page).is_none() {
                return Err(RenderError::UnsupportedCodePage {
                    code_page,
                    encoding: "<not present in printer profile>".to_owned(),
                    offset,
                });
            }

            // Every ESC/POS character table shares printable ASCII. Remember a
            // known table even when its extended or multibyte range is outside
            // v1 so later ASCII can still be rendered faithfully.
            state.select_code_page(code_page);
            Ok(3)
        }
        command => Err(RenderError::UnsupportedEscCommand { command, offset }),
    }
}

fn execute_esc_d(
    data: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<usize, RenderError> {
    let mut columns = Vec::new();
    let mut command_length = 2;

    loop {
        let Some(column) = data.get(command_length).copied() else {
            return Err(RenderError::TruncatedCommand {
                command: "ESC D",
                offset,
            });
        };
        if column == 0 {
            command_length += 1;
            break;
        }

        let no_longer_ascending = columns.last().is_some_and(|&previous| column <= previous);
        if columns.len() == 32 || no_longer_ascending {
            // Epson treats the first excess or non-ascending byte as normal
            // input, so leave it for the outer parser instead of consuming it.
            break;
        }

        columns.push(column);
        command_length += 1;
    }

    state.set_tab_positions(&columns);
    Ok(command_length)
}

fn execute_esc_star(
    data: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<usize, RenderError> {
    if data.len() < 5 {
        return Err(RenderError::TruncatedCommand {
            command: "ESC *",
            offset,
        });
    }
    state.require_column_bit_image(offset)?;

    let mode = data[2];
    let (bytes_per_column, horizontal_scale, vertical_scale) = match mode {
        0 => (1, 2, 3),
        1 => (1, 1, 3),
        32 => (3, 2, 1),
        33 => (3, 1, 1),
        mode => {
            return Err(RenderError::UnsupportedBitImageMode { mode, offset });
        }
    };
    let columns = usize::from(data[3]) + usize::from(data[4]) * 256;
    let payload_length = columns.saturating_mul(bytes_per_column);
    state.validate_command_payload_size(payload_length)?;
    let command_length = 5 + payload_length;
    let Some(payload) = data.get(5..command_length) else {
        return Err(RenderError::TruncatedCommand {
            command: "ESC *",
            offset,
        });
    };

    state.paint_bit_image(payload, bytes_per_column, horizontal_scale, vertical_scale);
    Ok(command_length)
}

fn execute_gs_command(
    data: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<usize, RenderError> {
    let Some(command) = data.get(1).copied() else {
        return Err(RenderError::TruncatedCommand {
            command: "GS",
            offset,
        });
    };

    match command {
        0x21 => {
            let Some(size) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS !",
                    offset,
                });
            };
            state.set_character_size(size);
            Ok(3)
        }
        0x28 => match data.get(2) {
            Some(b'L') => execute_gs_parenthesized_l(data, offset, state),
            Some(b'k') => execute_gs_parenthesized_k(data, offset, state),
            Some(_) => Err(RenderError::UnsupportedGsCommand {
                command: 0x28,
                offset,
            }),
            None => Err(RenderError::TruncatedCommand {
                command: "GS (",
                offset,
            }),
        },
        0x38 => execute_gs_8_l(data, offset, state),
        0x42 => {
            let Some(reverse) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS B",
                    offset,
                });
            };
            state.set_reverse(reverse & 0x01 != 0);
            Ok(3)
        }
        0x48 => {
            let Some(position) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS H",
                    offset,
                });
            };
            let position = match position {
                0 | 48 => HriPosition::None,
                1 | 49 => HriPosition::Above,
                2 | 50 => HriPosition::Below,
                3 | 51 => HriPosition::AboveAndBelow,
                value => {
                    return Err(RenderError::InvalidBarcodeParameter {
                        parameter: "hri_position",
                        value,
                        offset,
                    });
                }
            };
            state.set_hri_position(position);
            Ok(3)
        }
        0x4c => {
            let Some((&low, &high)) = data.get(2).zip(data.get(3)) else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS L",
                    offset,
                });
            };
            state.set_left_margin(u16::from_le_bytes([low, high]));
            Ok(4)
        }
        0x50 => {
            let Some((&horizontal, &vertical)) = data.get(2).zip(data.get(3)) else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS P",
                    offset,
                });
            };
            state.set_motion_units(horizontal, vertical);
            Ok(4)
        }
        0x57 => {
            let Some((&low, &high)) = data.get(2).zip(data.get(3)) else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS W",
                    offset,
                });
            };
            state.set_print_area_width(u16::from_le_bytes([low, high]));
            Ok(4)
        }
        0x66 => {
            let Some(font) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS f",
                    offset,
                });
            };
            match font {
                0 | 48 => state.select_hri_font_a(),
                1 | 49 => state.select_hri_font_b(),
                value => {
                    return Err(RenderError::InvalidBarcodeParameter {
                        parameter: "hri_font",
                        value,
                        offset,
                    });
                }
            }
            Ok(3)
        }
        0x68 => {
            let Some(height) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS h",
                    offset,
                });
            };
            if height == 0 {
                return Err(RenderError::InvalidBarcodeParameter {
                    parameter: "height",
                    value: height,
                    offset,
                });
            }
            state.set_barcode_height(height);
            Ok(3)
        }
        0x6b => execute_gs_k(data, offset, state),
        0x56 => execute_gs_v(data, offset, state),
        0x76 => execute_gs_v0(data, offset, state),
        0x77 => {
            let Some(width) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS w",
                    offset,
                });
            };
            if !(2..=6).contains(&width) {
                return Err(RenderError::InvalidBarcodeParameter {
                    parameter: "module_width",
                    value: width,
                    offset,
                });
            }
            state.set_barcode_module_width(width);
            Ok(3)
        }
        command => Err(RenderError::UnsupportedGsCommand { command, offset }),
    }
}

fn execute_gs_k(
    data: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<usize, RenderError> {
    let Some(system) = data.get(2).copied() else {
        return Err(RenderError::TruncatedCommand {
            command: "GS k",
            offset,
        });
    };

    let (system, payload, mut command_length, is_function_a) = match system {
        function_a @ 0..=6 => {
            let barcode_system = barcode_system_from_function_b(function_a + 65)
                .expect("every Function A command maps to a known barcode system");
            state.require_barcode_system(barcode_system, true, offset)?;
            let (payload, command_length) = function_a_barcode_payload(data, function_a, offset)?;
            state.validate_command_payload_size(payload.len())?;
            (function_a + 65, payload, command_length, true)
        }
        function_b @ 65..=79 => {
            let barcode_system = barcode_system_from_function_b(function_b)
                .expect("the accepted Function B range contains known barcode systems");
            state.require_barcode_system(barcode_system, false, offset)?;
            let payload_length =
                usize::from(*data.get(3).ok_or(RenderError::TruncatedCommand {
                    command: "GS k",
                    offset,
                })?);
            state.validate_command_payload_size(payload_length)?;
            let command_length = 4usize.saturating_add(payload_length);
            let payload = data
                .get(4..command_length)
                .ok_or(RenderError::TruncatedCommand {
                    command: "GS k",
                    offset,
                })?;
            (function_b, payload, command_length, false)
        }
        _ => {
            return Err(RenderError::InvalidBarcodeData {
                system: "unknown",
                offset,
                reason: "barcode system is not supported",
            });
        }
    };

    // Function A predates the explicit byte count. Epson documents that its
    // ITF mode drops the final digit when the NUL-terminated count is odd.
    let payload = if is_function_a && system == 70 && !payload.len().is_multiple_of(2) {
        &payload[..payload.len() - 1]
    } else {
        payload
    };
    let payload = if !is_function_a && system == 69 {
        let first_possible_stop = usize::from(payload.first() == Some(&b'*'));
        let stop = payload[first_possible_stop..]
            .iter()
            .position(|character| *character == b'*')
            .map(|position| first_possible_stop + position);
        if let Some(stop) = stop.filter(|stop| stop + 1 < payload.len()) {
            // In Function B the declared byte count does not swallow bytes
            // after a Code 39 stop. The parser must return them to the main
            // ESC/POS stream as ordinary input.
            command_length = 4 + stop + 1;
            &payload[..=stop]
        } else {
            payload
        }
    } else {
        payload
    };

    let barcode = match system {
        65 => barcode::encode_upca(payload).map_err(|error| RenderError::InvalidBarcodeData {
            system: "UPC-A",
            offset,
            reason: match error {
                barcode::BarcodeError::Length => "expected 11 or 12 digits",
                barcode::BarcodeError::Character => "expected decimal digits only",
                barcode::BarcodeError::Format => "invalid UPC-A data format",
            },
        })?,
        66 => barcode::encode_upce(payload).map_err(|error| RenderError::InvalidBarcodeData {
            system: "UPC-E",
            offset,
            reason: match error {
                barcode::BarcodeError::Length => "expected 6, 7, 8, 11, or 12 digits",
                barcode::BarcodeError::Character => "expected decimal digits only",
                barcode::BarcodeError::Format => {
                    "expected number system 0 and a compressible UPC-A value"
                }
            },
        })?,
        67 => barcode::encode_ean13(payload).map_err(|error| RenderError::InvalidBarcodeData {
            system: "EAN-13",
            offset,
            reason: match error {
                barcode::BarcodeError::Length => "expected 12 or 13 digits",
                barcode::BarcodeError::Character => "expected decimal digits only",
                barcode::BarcodeError::Format => "invalid EAN-13 data format",
            },
        })?,
        68 => barcode::encode_ean8(payload).map_err(|error| RenderError::InvalidBarcodeData {
            system: "EAN-8",
            offset,
            reason: match error {
                barcode::BarcodeError::Length => "expected 7 or 8 digits",
                barcode::BarcodeError::Character => "expected decimal digits only",
                barcode::BarcodeError::Format => "invalid EAN-8 data format",
            },
        })?,
        69 => barcode::encode_code39(payload).map_err(|error| RenderError::InvalidBarcodeData {
            system: "Code 39",
            offset,
            reason: match error {
                barcode::BarcodeError::Length => "expected at least one character",
                barcode::BarcodeError::Character => "contains an unsupported character",
                barcode::BarcodeError::Format => "the stop character may appear only at the end",
            },
        })?,
        70 => barcode::encode_itf(payload).map_err(|error| RenderError::InvalidBarcodeData {
            system: "ITF",
            offset,
            reason: match error {
                barcode::BarcodeError::Length => "expected an even number of at least two digits",
                barcode::BarcodeError::Character => "expected decimal digits only",
                barcode::BarcodeError::Format => "invalid ITF data format",
            },
        })?,
        71 => {
            barcode::encode_codabar(payload).map_err(|error| RenderError::InvalidBarcodeData {
                system: "Codabar",
                offset,
                reason: match error {
                    barcode::BarcodeError::Length => "expected start and stop characters",
                    barcode::BarcodeError::Character => "contains an unsupported character",
                    barcode::BarcodeError::Format => {
                        "expected A through D start and stop characters"
                    }
                },
            })?
        }
        72 => barcode::encode_code93(payload).map_err(|error| RenderError::InvalidBarcodeData {
            system: "Code 93",
            offset,
            reason: match error {
                barcode::BarcodeError::Length => "expected at least one character",
                barcode::BarcodeError::Character => "expected bytes 00h through 7Fh",
                barcode::BarcodeError::Format => "invalid Code 93 data format",
            },
        })?,
        73 => {
            barcode::encode_code128(payload).map_err(|error| RenderError::InvalidBarcodeData {
                system: "Code 128",
                offset,
                reason: match error {
                    barcode::BarcodeError::Length => {
                        "expected an explicit {A, {B, or {C start sequence"
                    }
                    barcode::BarcodeError::Character => {
                        "character is not valid in the selected code set"
                    }
                    barcode::BarcodeError::Format => "invalid Code 128 code-set data",
                },
            })?
        }
        74 => {
            barcode::encode_gs1_128(payload).map_err(|error| RenderError::InvalidBarcodeData {
                system: "GS1-128",
                offset,
                reason: match error {
                    barcode::BarcodeError::Length => "expected 2 through 255 bytes",
                    barcode::BarcodeError::Character => "expected bytes 00h through 7Fh",
                    barcode::BarcodeError::Format => "invalid GS1-128 data structure",
                },
            })?
        }
        75 => barcode::encode_gs1_databar_omnidirectional(payload).map_err(|error| {
            RenderError::InvalidBarcodeData {
                system: "GS1 DataBar Omnidirectional",
                offset,
                reason: match error {
                    barcode::BarcodeError::Length => "expected exactly 13 digits",
                    barcode::BarcodeError::Character => "expected decimal digits only",
                    barcode::BarcodeError::Format => {
                        "could not encode the GS1 DataBar Omnidirectional value"
                    }
                },
            }
        })?,
        76 => barcode::encode_gs1_databar_truncated(payload).map_err(|error| {
            RenderError::InvalidBarcodeData {
                system: "GS1 DataBar Truncated",
                offset,
                reason: match error {
                    barcode::BarcodeError::Length => "expected exactly 13 digits",
                    barcode::BarcodeError::Character => "expected decimal digits only",
                    barcode::BarcodeError::Format => {
                        "could not encode the GS1 DataBar Truncated value"
                    }
                },
            }
        })?,
        77 => barcode::encode_gs1_databar_limited(payload).map_err(|error| {
            RenderError::InvalidBarcodeData {
                system: "GS1 DataBar Limited",
                offset,
                reason: match error {
                    barcode::BarcodeError::Length => "expected exactly 13 digits",
                    barcode::BarcodeError::Character => "expected decimal digits only",
                    barcode::BarcodeError::Format => {
                        "expected a value between 0000000000000 and 1999999999999"
                    }
                },
            }
        })?,
        78 => barcode::encode_gs1_databar_expanded(payload).map_err(|error| {
            RenderError::InvalidBarcodeData {
                system: "GS1 DataBar Expanded",
                offset,
                reason: match error {
                    barcode::BarcodeError::Length => "expected at least two bytes",
                    barcode::BarcodeError::Character => {
                        "contains a character outside the GS1 encodable set"
                    }
                    barcode::BarcodeError::Format => "invalid GS1 DataBar Expanded data structure",
                },
            }
        })?,
        79 => barcode::encode_code128_auto(payload).map_err(|error| {
            RenderError::InvalidBarcodeData {
                system: "Code 128 auto",
                offset,
                reason: match error {
                    barcode::BarcodeError::Length => "expected at least one byte",
                    barcode::BarcodeError::Character => "could not encode byte in Code 128",
                    barcode::BarcodeError::Format => "could not plan automatic Code 128 data",
                },
            }
        })?,
        _ => {
            return Err(RenderError::InvalidBarcodeData {
                system: "unknown",
                offset,
                reason: "barcode system is not implemented yet",
            });
        }
    };
    state.print_barcode(&barcode, offset)?;
    Ok(command_length)
}

fn barcode_system_from_function_b(system: u8) -> Option<BarcodeSystem> {
    Some(match system {
        65 => BarcodeSystem::UpcA,
        66 => BarcodeSystem::UpcE,
        67 => BarcodeSystem::Ean13,
        68 => BarcodeSystem::Ean8,
        69 => BarcodeSystem::Code39,
        70 => BarcodeSystem::Itf,
        71 => BarcodeSystem::Codabar,
        72 => BarcodeSystem::Code93,
        73 => BarcodeSystem::Code128,
        74 => BarcodeSystem::Gs1_128,
        75 => BarcodeSystem::Gs1DataBarOmnidirectional,
        76 => BarcodeSystem::Gs1DataBarTruncated,
        77 => BarcodeSystem::Gs1DataBarLimited,
        78 => BarcodeSystem::Gs1DataBarExpanded,
        79 => BarcodeSystem::Code128Auto,
        _ => return None,
    })
}

fn function_a_barcode_payload(
    data: &[u8],
    system: u8,
    offset: usize,
) -> Result<(&[u8], usize), RenderError> {
    let remaining = data.get(3..).ok_or(RenderError::TruncatedCommand {
        command: "GS k",
        offset,
    })?;
    let nul = remaining.iter().position(|byte| *byte == 0);

    if system == 4 {
        // A leading '*' is Code 39's start character. A later '*' is the stop
        // character and ends command processing immediately, even before the
        // NUL that normally frames Function A.
        let first_possible_stop = usize::from(remaining.first() == Some(&b'*'));
        let stop = remaining[first_possible_stop..]
            .iter()
            .position(|character| *character == b'*')
            .map(|position| first_possible_stop + position);
        if let Some(stop) = stop.filter(|stop| nul.is_none_or(|nul| *stop < nul)) {
            let payload_length = stop + 1;
            return Ok((&remaining[..payload_length], 3 + payload_length));
        }
    }

    let payload_length = nul.ok_or(RenderError::TruncatedCommand {
        command: "GS k",
        offset,
    })?;
    Ok((
        &remaining[..payload_length],
        4usize.saturating_add(payload_length),
    ))
}

fn execute_gs_parenthesized_l(
    data: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<usize, RenderError> {
    if data.len() < 5 || data[2] != b'L' {
        return Err(RenderError::TruncatedCommand {
            command: "GS ( L",
            offset,
        });
    }

    let parameter_length = usize::from(u16::from_le_bytes([data[3], data[4]]));
    state.validate_command_payload_size(parameter_length)?;
    let command_length = 5 + parameter_length;
    let Some(parameters) = data.get(5..command_length) else {
        return Err(RenderError::TruncatedCommand {
            command: "GS ( L",
            offset,
        });
    };
    execute_graphics_function(parameters, false, offset, state)?;
    Ok(command_length)
}

fn execute_gs_parenthesized_k(
    data: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<usize, RenderError> {
    if data.len() < 5 || data[2] != b'k' {
        return Err(RenderError::TruncatedCommand {
            command: "GS ( k",
            offset,
        });
    }

    let parameter_length = usize::from(u16::from_le_bytes([data[3], data[4]]));
    state.validate_command_payload_size(parameter_length)?;
    let command_length = 5usize.saturating_add(parameter_length);
    let Some(parameters) = data.get(5..command_length) else {
        return Err(RenderError::TruncatedCommand {
            command: "GS ( k",
            offset,
        });
    };
    execute_qr_function(parameters, offset, state)?;
    Ok(command_length)
}

fn execute_qr_function(
    parameters: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<(), RenderError> {
    let Some((&code_type, &function)) = parameters.first().zip(parameters.get(1)) else {
        return Err(RenderError::TruncatedCommand {
            command: "GS ( k",
            offset,
        });
    };
    if code_type != 49 {
        return Err(RenderError::InvalidQrParameter {
            parameter: "cn",
            value: code_type,
            offset,
        });
    }

    match function {
        65 => {
            let Some((&model, &reserved)) = parameters.get(2).zip(parameters.get(3)) else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS ( k Function 165",
                    offset,
                });
            };
            if parameters.len() != 4 || reserved != 0 {
                return Err(RenderError::InvalidQrParameter {
                    parameter: "n2",
                    value: reserved,
                    offset,
                });
            }
            if model != 50 {
                return Err(RenderError::UnsupportedQrModel { model, offset });
            }
            state.select_qr_model_2(offset)
        }
        67 => {
            let Some(&module_size) = parameters.get(2) else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS ( k Function 167",
                    offset,
                });
            };
            if parameters.len() != 3 || !(1..=16).contains(&module_size) {
                return Err(RenderError::InvalidQrParameter {
                    parameter: "module_size",
                    value: module_size,
                    offset,
                });
            }
            state.set_qr_module_size(module_size, offset)
        }
        69 => {
            let Some(&level) = parameters.get(2) else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS ( k Function 169",
                    offset,
                });
            };
            if parameters.len() != 3 {
                return Err(RenderError::InvalidQrParameter {
                    parameter: "error_correction",
                    value: level,
                    offset,
                });
            }
            let level = match level {
                48 => qr::ErrorCorrection::Low,
                49 => qr::ErrorCorrection::Medium,
                50 => qr::ErrorCorrection::Quartile,
                51 => qr::ErrorCorrection::High,
                value => {
                    return Err(RenderError::InvalidQrParameter {
                        parameter: "error_correction",
                        value,
                        offset,
                    });
                }
            };
            state.set_qr_error_correction(level, offset)
        }
        80 => {
            if parameters.len() > MAX_QR_STORE_PARAMETER_BYTES {
                return Err(RenderError::InvalidQrData {
                    offset,
                    reason: "store command exceeds the 7092-byte parameter limit",
                });
            }
            let Some((&mode, data)) = parameters.get(2).zip(parameters.get(3..)) else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS ( k Function 180",
                    offset,
                });
            };
            if mode != 48 {
                return Err(RenderError::InvalidQrParameter {
                    parameter: "m",
                    value: mode,
                    offset,
                });
            }
            state.store_qr_data(data, offset)
        }
        81 => {
            let Some(&mode) = parameters.get(2) else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS ( k Function 181",
                    offset,
                });
            };
            if parameters.len() != 3 || mode != 48 {
                return Err(RenderError::InvalidQrParameter {
                    parameter: "m",
                    value: mode,
                    offset,
                });
            }
            state.print_qr(offset)
        }
        function => Err(RenderError::UnsupportedQrFunction { function, offset }),
    }
}

fn execute_gs_8_l(
    data: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<usize, RenderError> {
    if data.len() < 7 || data[2] != b'L' {
        return Err(RenderError::TruncatedCommand {
            command: "GS 8 L",
            offset,
        });
    }

    let parameter_length = u32::from_le_bytes([data[3], data[4], data[5], data[6]]) as usize;
    state.validate_command_payload_size(parameter_length)?;
    let command_length = 7usize.saturating_add(parameter_length);
    let Some(parameters) = data.get(7..command_length) else {
        return Err(RenderError::TruncatedCommand {
            command: "GS 8 L",
            offset,
        });
    };
    execute_graphics_function(parameters, true, offset, state)?;
    Ok(command_length)
}

fn execute_graphics_function(
    parameters: &[u8],
    extended_length: bool,
    offset: usize,
    state: &mut PrinterState,
) -> Result<(), RenderError> {
    let Some((&mode, &function)) = parameters.first().zip(parameters.get(1)) else {
        return Err(RenderError::TruncatedCommand {
            command: if extended_length { "GS 8 L" } else { "GS ( L" },
            offset,
        });
    };
    if mode != 48 {
        return Err(RenderError::InvalidGraphicsParameter {
            parameter: "m",
            value: u64::from(mode),
            offset,
        });
    }

    match function {
        2 | 50 if !extended_length => {
            if parameters.len() != 2 {
                return Err(RenderError::InvalidGraphicsPayloadLength {
                    expected: 2,
                    actual: parameters.len(),
                    offset,
                });
            }
            state.print_buffered_graphics(offset)
        }
        112 => store_raster_graphics(parameters, offset, state),
        function => Err(RenderError::UnsupportedGraphicsFunction { function, offset }),
    }
}

fn store_raster_graphics(
    parameters: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<(), RenderError> {
    if parameters.len() < 10 {
        return Err(RenderError::InvalidGraphicsPayloadLength {
            expected: 10,
            actual: parameters.len(),
            offset,
        });
    }
    let tone = parameters[2];
    let scale_x = parameters[3];
    let scale_y = parameters[4];
    let color = parameters[5];
    validate_graphics_parameter(tone == 48, "a", tone, offset)?;
    validate_graphics_parameter(matches!(scale_x, 1 | 2), "bx", scale_x, offset)?;
    validate_graphics_parameter(matches!(scale_y, 1 | 2), "by", scale_y, offset)?;
    validate_graphics_parameter(color == 49, "c", color, offset)?;

    let width_dots = usize::from(u16::from_le_bytes([parameters[6], parameters[7]]));
    let height_dots = usize::from(u16::from_le_bytes([parameters[8], parameters[9]]));
    if width_dots == 0 || height_dots == 0 {
        return Err(RenderError::InvalidGraphicsDimensions {
            width_dots,
            height_dots,
            offset,
        });
    }
    let row_bytes = width_dots.div_ceil(8);
    let expected_payload = row_bytes.saturating_mul(height_dots);
    let payload = &parameters[10..];
    if payload.len() != expected_payload {
        return Err(RenderError::InvalidGraphicsPayloadLength {
            expected: 10 + expected_payload,
            actual: parameters.len(),
            offset,
        });
    }

    state.store_raster_graphics(
        BufferedGraphics {
            payload: payload.to_vec(),
            row_bytes,
            width_dots,
            height_dots,
            horizontal_scale: u32::from(scale_x),
            vertical_scale: u32::from(scale_y),
        },
        offset,
    )
}

fn validate_graphics_parameter(
    valid: bool,
    parameter: &'static str,
    value: u8,
    offset: usize,
) -> Result<(), RenderError> {
    if valid {
        return Ok(());
    }
    Err(RenderError::InvalidGraphicsParameter {
        parameter,
        value: u64::from(value),
        offset,
    })
}

fn execute_gs_v(
    data: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<usize, RenderError> {
    let Some(mode) = data.get(2).copied() else {
        return Err(RenderError::TruncatedCommand {
            command: "GS V",
            offset,
        });
    };

    match mode {
        0 | 48 => {
            state.cut(false, offset)?;
            Ok(3)
        }
        1 | 49 => {
            state.cut(true, offset)?;
            Ok(3)
        }
        mode @ (65 | 66) => {
            let Some(feed) = data.get(3).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "GS V",
                    offset,
                });
            };
            state.feed_to_cut_position_and_cut(mode, feed, offset)?;
            Ok(4)
        }
        97 | 98 | 103 | 104 => {
            if data.get(3).is_none() {
                return Err(RenderError::TruncatedCommand {
                    command: "GS V",
                    offset,
                });
            }
            Err(RenderError::UnsupportedCutMode { mode, offset })
        }
        mode => Err(RenderError::UnsupportedCutMode { mode, offset }),
    }
}

fn execute_gs_v0(
    data: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<usize, RenderError> {
    if data.len() < 3 {
        return Err(RenderError::TruncatedCommand {
            command: "GS v 0",
            offset,
        });
    }

    if data[2] != 0x30 {
        return Err(RenderError::UnsupportedGsCommand {
            command: data[1],
            offset,
        });
    }

    if !state.at_beginning_of_line {
        // In Standard mode Epson consumes only the GS v 0 prefix when the
        // line has started. The outer parser must see m and every later byte
        // as normal input instead of trusting the raster length fields.
        return Ok(3);
    }
    state.require_raster_bit_image(offset)?;

    if data.len() < 8 {
        return Err(RenderError::TruncatedCommand {
            command: "GS v 0",
            offset,
        });
    }

    let mode = data[3];
    let (horizontal_scale, vertical_scale) = match mode {
        0 | 48 => (1, 1),
        1 | 49 => (2, 1),
        2 | 50 => (1, 2),
        3 | 51 => (2, 2),
        mode => return Err(RenderError::UnsupportedRasterBitImageMode { mode, offset }),
    };

    let width_bytes = usize::from(data[4]) + usize::from(data[5]) * 256;
    let height_dots = usize::from(data[6]) + usize::from(data[7]) * 256;
    if width_bytes == 0 || height_dots == 0 {
        return Err(RenderError::InvalidRasterBitImageDimensions {
            width_bytes,
            height_dots,
            offset,
        });
    }

    let payload_length = width_bytes.saturating_mul(height_dots);
    state.validate_command_payload_size(payload_length)?;
    let command_length = 8 + payload_length;
    let Some(payload) = data.get(8..command_length) else {
        return Err(RenderError::TruncatedCommand {
            command: "GS v 0",
            offset,
        });
    };

    state.print_raster_image(
        payload,
        width_bytes,
        width_bytes.saturating_mul(8) as u32,
        height_dots,
        horizontal_scale,
        vertical_scale,
    )?;
    Ok(command_length)
}

#[derive(Debug, Clone)]
struct BufferedGraphics {
    payload: Vec<u8>,
    row_bytes: usize,
    width_dots: usize,
    height_dots: usize,
    horizontal_scale: u32,
    vertical_scale: u32,
}

#[derive(Debug)]
struct PrinterState {
    profile_id: String,
    limits: RenderLimits,
    device_events: Vec<DeviceEvent>,
    completed_sheets: Vec<MonoSurface>,
    roll: MonoSurface,
    // Text and ESC * data are composed on a line first because ESC a applies
    // justification when the printer receives the line feed, not per glyph.
    line: MonoSurface,
    print_area_left: u32,
    print_area_width: u32,
    line_top: u32,
    print_x: u32,
    line_used_width: u32,
    // Some commands are deliberately ignored after printable data or a
    // position command has moved the printer away from the line origin.
    at_beginning_of_line: bool,
    line_spacing: u32,
    default_line_spacing: u32,
    horizontal_dpi: u32,
    default_horizontal_motion_units_per_inch: u32,
    horizontal_motion_units_per_inch: u32,
    vertical_dpi: u32,
    default_vertical_motion_units_per_inch: u32,
    vertical_motion_units_per_inch: u32,
    font_a: ProfileFont,
    font_b: ProfileFont,
    active_font: ProfileFont,
    code_pages: BTreeMap<u8, String>,
    default_code_page: u8,
    active_code_page: u8,
    default_international_character_set: u8,
    active_international_character_set: u8,
    carriage_return_mode: CarriageReturnMode,
    right_side_character_spacing: u32,
    default_tab_positions: Vec<u32>,
    tab_positions: Vec<u32>,
    character_width_multiplier: u32,
    character_height_multiplier: u32,
    emphasized: bool,
    underline_thickness: u32,
    reversed: bool,
    justification: Justification,
    line_height: u32,
    buffered_graphics: Option<BufferedGraphics>,
    stored_qr_data: Option<Vec<u8>>,
    qr_module_size: u32,
    qr_error_correction: qr::ErrorCorrection,
    barcode_height: u32,
    barcode_module_width: u32,
    hri_position: HriPosition,
    hri_font: ProfileFont,
    function_a_barcodes: BTreeSet<BarcodeSystem>,
    function_b_barcodes: BTreeSet<BarcodeSystem>,
    supports_qr: bool,
    supports_column_bit_image: bool,
    supports_raster_bit_image: bool,
    supports_graphics: bool,
    supports_full_cut: bool,
    supports_partial_cut: bool,
    supports_standard_drawer_pulse: bool,
}

impl PrinterState {
    fn new(profile: &PrinterProfile, limits: RenderLimits) -> Self {
        let width = profile.geometry.printable_width_dots;
        let default_line_spacing = profile.defaults.line_spacing_dots;
        let font_a = profile.fonts.a.clone();
        let font_b = profile.fonts.b.clone();
        let hri_font = font_a.clone();
        // ESC/POS defaults to columns 8, 16, ... 248 measured with the
        // power-on font and size.
        let default_tab_positions = (1..=31)
            .map(|index| index * 8 * font_a.cell_width_dots)
            .collect::<Vec<_>>();

        Self {
            profile_id: profile.id.clone(),
            limits,
            device_events: Vec::new(),
            completed_sheets: Vec::new(),
            roll: MonoSurface::new(width),
            line: MonoSurface::new(width),
            print_area_left: 0,
            print_area_width: width,
            line_top: 0,
            print_x: 0,
            line_used_width: 0,
            at_beginning_of_line: true,
            line_spacing: default_line_spacing,
            default_line_spacing,
            horizontal_dpi: profile.geometry.dpi_x,
            default_horizontal_motion_units_per_inch: profile.motion.horizontal_units_per_inch,
            horizontal_motion_units_per_inch: profile.motion.horizontal_units_per_inch,
            vertical_dpi: profile.geometry.dpi_y,
            default_vertical_motion_units_per_inch: profile.motion.vertical_units_per_inch,
            vertical_motion_units_per_inch: profile.motion.vertical_units_per_inch,
            active_font: font_a.clone(),
            font_a,
            font_b,
            code_pages: profile.code_pages.clone(),
            default_code_page: profile.defaults.code_page,
            active_code_page: profile.defaults.code_page,
            default_international_character_set: profile.defaults.international_character_set,
            active_international_character_set: profile.defaults.international_character_set,
            carriage_return_mode: profile.defaults.carriage_return,
            right_side_character_spacing: 0,
            tab_positions: default_tab_positions.clone(),
            default_tab_positions,
            character_width_multiplier: 1,
            character_height_multiplier: 1,
            emphasized: false,
            underline_thickness: 0,
            reversed: false,
            justification: Justification::Left,
            line_height: 0,
            buffered_graphics: None,
            stored_qr_data: None,
            qr_module_size: DEFAULT_QR_MODULE_SIZE_DOTS,
            qr_error_correction: qr::ErrorCorrection::Low,
            barcode_height: DEFAULT_BARCODE_HEIGHT_DOTS,
            barcode_module_width: DEFAULT_BARCODE_MODULE_WIDTH_DOTS,
            hri_position: HriPosition::None,
            hri_font,
            function_a_barcodes: profile.features.barcodes.function_a.clone(),
            function_b_barcodes: profile.features.barcodes.function_b.clone(),
            supports_qr: profile.features.qr_code,
            supports_column_bit_image: profile.features.bit_image_column,
            supports_raster_bit_image: profile.features.bit_image_raster,
            supports_graphics: profile.features.graphics,
            supports_full_cut: profile.features.paper_full_cut,
            supports_partial_cut: profile.features.paper_part_cut,
            supports_standard_drawer_pulse: profile.features.pulse_standard,
        }
    }

    fn initialize(&mut self) {
        // Epson defines ESC @ as clearing the print buffer before restoring
        // modes. Already committed rows on `roll` represent fed paper and stay.
        self.print_area_left = 0;
        self.print_area_width = self.roll.width;
        self.line = MonoSurface::new(self.print_area_width);
        self.print_x = 0;
        self.line_used_width = 0;
        self.at_beginning_of_line = true;
        self.line_spacing = self.default_line_spacing;
        self.horizontal_motion_units_per_inch = self.default_horizontal_motion_units_per_inch;
        self.vertical_motion_units_per_inch = self.default_vertical_motion_units_per_inch;
        self.active_font = self.font_a.clone();
        self.active_code_page = self.default_code_page;
        self.active_international_character_set = self.default_international_character_set;
        self.right_side_character_spacing = 0;
        self.tab_positions.clone_from(&self.default_tab_positions);
        self.character_width_multiplier = 1;
        self.character_height_multiplier = 1;
        self.emphasized = false;
        self.underline_thickness = 0;
        self.reversed = false;
        self.justification = Justification::Left;
        self.line_height = 0;
        self.buffered_graphics = None;
        self.stored_qr_data = None;
        self.qr_module_size = DEFAULT_QR_MODULE_SIZE_DOTS;
        self.qr_error_correction = qr::ErrorCorrection::Low;
        self.barcode_height = DEFAULT_BARCODE_HEIGHT_DOTS;
        self.barcode_module_width = DEFAULT_BARCODE_MODULE_WIDTH_DOTS;
        self.hri_position = HriPosition::None;
        self.hri_font = self.font_a.clone();
    }

    fn set_print_mode(&mut self, mode: u8) {
        if mode & 0x01 == 0 {
            self.select_font_a();
        } else {
            self.select_font_b();
        }
        self.character_height_multiplier = if mode & 0x10 == 0 { 1 } else { 2 };
        self.character_width_multiplier = if mode & 0x20 == 0 { 1 } else { 2 };
        self.emphasized = mode & 0x08 != 0;
        self.underline_thickness = u32::from(mode & 0x80 != 0);
    }

    fn set_character_size(&mut self, size: u8) {
        // GS ! stores height minus one in bits 0–2 and width minus one in
        // bits 4–6. Bits 3 and 7 are reserved and do not affect either value.
        self.character_height_multiplier = u32::from(size & 0x07) + 1;
        self.character_width_multiplier = u32::from((size >> 4) & 0x07) + 1;
    }

    fn set_barcode_height(&mut self, height: u8) {
        self.barcode_height = u32::from(height);
    }

    fn set_barcode_module_width(&mut self, width: u8) {
        self.barcode_module_width = u32::from(width);
    }

    fn set_hri_position(&mut self, position: HriPosition) {
        self.hri_position = position;
    }

    fn select_hri_font_a(&mut self) {
        self.hri_font = self.font_a.clone();
    }

    fn select_hri_font_b(&mut self) {
        self.hri_font = self.font_b.clone();
    }

    fn set_absolute_print_position(&mut self, motion_units: u16) {
        let position = self.horizontal_motion_units_to_dots(motion_units);
        // Epson specifies that out-of-area settings are ignored, leaving the
        // previous cursor untouched.
        if position <= self.line.width {
            self.print_x = position;
        }
        self.at_beginning_of_line = false;
    }

    fn set_right_side_character_spacing(&mut self, motion_units: u8) {
        self.right_side_character_spacing =
            self.horizontal_motion_units_to_dots(u16::from(motion_units));
    }

    fn set_motion_units(&mut self, horizontal: u8, vertical: u8) {
        self.horizontal_motion_units_per_inch = match horizontal {
            0 => self.default_horizontal_motion_units_per_inch,
            horizontal => u32::from(horizontal),
        };
        self.vertical_motion_units_per_inch = match vertical {
            0 => self.default_vertical_motion_units_per_inch,
            vertical => u32::from(vertical),
        };
    }

    fn set_left_margin(&mut self, motion_units: u16) {
        if !self.at_beginning_of_line {
            return;
        }

        let margin = self
            .horizontal_motion_units_to_dots(motion_units)
            .min(self.roll.width);
        self.print_area_left = margin;
        self.print_area_width = self
            .print_area_width
            .min(self.roll.width.saturating_sub(margin));
        // Line coordinates are relative to the active print area. Rebuilding
        // is safe here because GS L is honored only at the beginning of a line.
        self.line = MonoSurface::new(self.print_area_width);
        self.print_x = 0;
        self.line_used_width = 0;
    }

    fn set_print_area_width(&mut self, motion_units: u16) {
        if !self.at_beginning_of_line {
            return;
        }

        let available_width = self.roll.width.saturating_sub(self.print_area_left);
        self.print_area_width = self
            .horizontal_motion_units_to_dots(motion_units)
            .min(available_width);
        // Keeping the line buffer print-area-sized makes wrapping and
        // justification independent of the physical left margin.
        self.line = MonoSurface::new(self.print_area_width);
        self.print_x = 0;
        self.line_used_width = 0;
    }

    fn horizontal_tab(&mut self) -> Result<(), RenderError> {
        let next_position = self
            .tab_positions
            .iter()
            .copied()
            .find(|&position| position > self.print_x);

        match next_position {
            Some(position) if position <= self.line.width => {
                self.print_x = position;
                self.line_used_width = self.line_used_width.max(position);
            }
            Some(_) => {
                // Epson performs buffer-full printing and applies HT again
                // from the next line when the next stop is outside the area.
                self.line_feed()?;
                if let Some(position) = self
                    .tab_positions
                    .iter()
                    .copied()
                    .find(|&position| position <= self.line.width)
                {
                    self.print_x = position;
                    self.line_used_width = position;
                }
            }
            None => {}
        }

        self.at_beginning_of_line = false;
        Ok(())
    }

    fn set_tab_positions(&mut self, columns: &[u8]) {
        let character_advance = self.current_character_advance_width();
        self.tab_positions = columns
            .iter()
            .map(|&column| u32::from(column).saturating_mul(character_advance))
            .collect();
    }

    fn set_relative_print_position(&mut self, motion_units: i16) {
        let distance = self.horizontal_motion_units_to_dots(motion_units.unsigned_abs());
        let position = if motion_units.is_negative() {
            self.print_x.checked_sub(distance)
        } else {
            self.print_x.checked_add(distance)
        };

        // Moving left of the print-area origin or right of its edge is an
        // ignored setting, not a clamped position.
        if let Some(position) = position.filter(|&position| position <= self.line.width) {
            self.print_x = position;
        }
        self.at_beginning_of_line = false;
    }

    fn horizontal_motion_units_to_dots(&self, motion_units: u16) -> u32 {
        // ESC/POS applies the current motion unit when it receives the
        // command. Store the resulting dot coordinate so later GS P changes
        // cannot move content that was already positioned.
        (u64::from(motion_units) * u64::from(self.horizontal_dpi)
            / u64::from(self.horizontal_motion_units_per_inch)) as u32
    }

    fn set_line_spacing(&mut self, motion_units: u8) {
        // ESC 3 uses the printer's vertical motion unit, which is not
        // necessarily one dot. Integer truncation matches dot-grid hardware.
        self.line_spacing = (u64::from(motion_units) * u64::from(self.vertical_dpi)
            / u64::from(self.vertical_motion_units_per_inch)) as u32;
    }

    fn restore_default_line_spacing(&mut self) {
        self.line_spacing = self.default_line_spacing;
    }

    fn print_and_feed_motion_units(&mut self, motion_units: u8) -> Result<(), RenderError> {
        let feed_dots = (u64::from(motion_units) * u64::from(self.vertical_dpi)
            / u64::from(self.vertical_motion_units_per_inch)) as u32;
        // ESC J is a one-off feed. Reuse the normal line commit so tall data
        // cannot overlap, then restore the persistent ESC 2/ESC 3 spacing.
        let line_spacing = self.line_spacing;
        self.line_spacing = feed_dots;
        self.feed_lines(1)?;
        self.line_spacing = line_spacing;
        Ok(())
    }

    fn set_emphasis(&mut self, emphasized: bool) {
        self.emphasized = emphasized;
    }

    fn set_underline(&mut self, thickness: u32) {
        self.underline_thickness = thickness;
    }

    fn set_reverse(&mut self, reversed: bool) {
        self.reversed = reversed;
    }

    fn set_justification(&mut self, justification: Justification) {
        if self.at_beginning_of_line {
            self.justification = justification;
        }
    }

    fn select_font_a(&mut self) {
        self.active_font = self.font_a.clone();
    }

    fn select_font_b(&mut self) {
        self.active_font = self.font_b.clone();
    }

    fn code_page_encoding(&self, code_page: u8) -> Option<&str> {
        self.code_pages.get(&code_page).map(String::as_str)
    }

    fn select_code_page(&mut self, code_page: u8) {
        self.active_code_page = code_page;
    }

    fn select_international_character_set(&mut self, character_set: u8) {
        self.active_international_character_set = character_set;
    }

    fn feed_lines(&mut self, lines: u8) -> Result<(), RenderError> {
        let remaining_width = self.line.width.saturating_sub(self.line_used_width);
        // Track logical data width rather than scanning black dots. This keeps
        // spaces significant and preserves far-right data after ESC $ or
        // ESC \ moves the cursor back to an earlier position.
        let line_left = match self.justification {
            Justification::Left => 0,
            Justification::Center => remaining_width / 2,
            Justification::Right => remaining_width,
        };
        let feed = match lines {
            0 => 0,
            lines => self.line_spacing.max(self.line_height).saturating_add(
                self.line_spacing
                    .saturating_mul(u32::from(lines).saturating_sub(1)),
            ),
        };
        let required_height = self
            .line_top
            .saturating_add(feed.max(self.line.height).max(self.line_height));
        self.validate_roll_height(required_height)?;

        self.roll.composite_at(
            &self.line,
            self.print_area_left.saturating_add(line_left),
            self.line_top,
        );
        // Epson expands the feed for tall characters, but ESC * graphics keep
        // the selected line spacing. This permits the intentional overlap used
        // by column-image streams whose rows are advanced separately.
        self.line_top = self.line_top.saturating_add(feed);
        self.roll.ensure_height(self.line_top);
        self.line.clear();
        self.print_x = 0;
        self.line_used_width = 0;
        self.at_beginning_of_line = true;
        self.line_height = 0;
        Ok(())
    }

    fn print_byte(&mut self, byte: u8, offset: usize) -> Result<(), RenderError> {
        // The ESC t operand is printer-specific. The profile translates that
        // numeric slot into a stable encoding name before we decode the byte.
        let encoding = self
            .code_page_encoding(self.active_code_page)
            .unwrap_or("<not present in printer profile>");

        // ESC R replaces a small set of ASCII positions independently of the
        // active ESC t table. Printable ASCII is common to every table,
        // including multibyte tables whose extended ranges remain post-v1.
        let character = international::substitution(self.active_international_character_set, byte)
            .or_else(|| byte.is_ascii().then(|| char::from(byte)))
            .or_else(|| {
                is_supported_code_page_encoding(encoding)
                    .then(|| decode_printable_byte(byte, encoding))
                    .flatten()
            });
        if character.is_none() && !is_supported_code_page_encoding(encoding) {
            return Err(RenderError::UnsupportedCodePage {
                code_page: self.active_code_page,
                encoding: encoding.to_owned(),
                offset,
            });
        }
        let Some(character) = character else {
            return Err(RenderError::UndefinedCodePageByte {
                byte,
                code_page: self.active_code_page,
                encoding: encoding.to_owned(),
                offset,
            });
        };
        // fontdue uses glyph index zero for the font's generic .notdef box.
        // Report it instead of silently making unrelated scripts look equal.
        if default_font().lookup_glyph_index(character) == 0 {
            return Err(RenderError::MissingGlyph { character, offset });
        }

        self.print_character(character)
    }

    fn print_character(&mut self, character: char) -> Result<(), RenderError> {
        let cell_width = self.current_character_advance_width();
        let cell_height = self
            .active_font
            .cell_height_dots
            .saturating_mul(self.character_height_multiplier);
        if self.print_x.saturating_add(cell_width) > self.line.width {
            self.line_feed()?;
        }
        self.line_height = self.line_height.max(cell_height);
        if self.reversed {
            for x in self.print_x..self.print_x.saturating_add(cell_width) {
                for y in 0..cell_height {
                    self.line.print_dot(x, y);
                }
            }
        }

        let font_size = self.active_font.cell_height_dots as f32;
        let (metrics, bitmap) = default_font().rasterize(character, font_size);
        // The bundled font supplies stable glyph shapes, while printer profile
        // cells remain authoritative for clipping, centering, and advancement.
        let horizontal_crop = metrics
            .width
            .saturating_sub(self.active_font.cell_width_dots as usize)
            / 2;
        let horizontal_padding =
            (self.active_font.cell_width_dots as usize).saturating_sub(metrics.width) / 2;
        let glyph_top =
            self.active_font.baseline_dots as i32 - (metrics.ymin + metrics.height as i32);

        for source_y in 0..metrics.height {
            let destination_y = glyph_top + source_y as i32;
            if destination_y < 0 || destination_y >= self.active_font.cell_height_dots as i32 {
                continue;
            }

            for source_x in horizontal_crop..metrics.width {
                let destination_x = horizontal_padding + source_x - horizontal_crop;
                if destination_x >= self.active_font.cell_width_dots as usize {
                    break;
                }
                if bitmap[source_y * metrics.width + source_x] < GLYPH_ALPHA_THRESHOLD {
                    continue;
                }

                let left = self.print_x + destination_x as u32 * self.character_width_multiplier;
                let top = destination_y as u32 * self.character_height_multiplier;
                for x in left..left + self.character_width_multiplier {
                    for y in top..top + self.character_height_multiplier {
                        if self.reversed {
                            self.line.clear_dot(x, y);
                        } else {
                            self.line.print_dot(x, y);
                        }
                        if self.emphasized && x + 1 < self.print_x.saturating_add(cell_width) {
                            if self.reversed {
                                self.line.clear_dot(x + 1, y);
                            } else {
                                self.line.print_dot(x + 1, y);
                            }
                        }
                    }
                }
            }
        }

        if !self.reversed {
            let underline_top = cell_height.saturating_sub(self.underline_thickness);
            for x in self.print_x..self.print_x.saturating_add(cell_width) {
                for y in underline_top..cell_height {
                    self.line.print_dot(x, y);
                }
            }
        }

        self.print_x = self.print_x.saturating_add(cell_width);
        self.line_used_width = self.line_used_width.max(self.print_x);
        self.at_beginning_of_line = false;
        Ok(())
    }

    fn current_character_advance_width(&self) -> u32 {
        self.active_font
            .cell_width_dots
            .saturating_add(self.right_side_character_spacing)
            .saturating_mul(self.character_width_multiplier)
    }

    fn paint_bit_image(
        &mut self,
        payload: &[u8],
        bytes_per_column: usize,
        horizontal_scale: u32,
        vertical_scale: u32,
    ) {
        // ESC * is column-major: each group describes one x coordinate and
        // contains either 8 or 24 vertical source dots.
        for (column_index, column) in payload.chunks_exact(bytes_per_column).enumerate() {
            let x = self.print_x + column_index as u32 * horizontal_scale;
            for (byte_index, byte) in column.iter().copied().enumerate() {
                for bit in 0..8 {
                    if byte & (0x80 >> bit) == 0 {
                        continue;
                    }

                    let source_y = byte_index as u32 * 8 + bit;
                    let top = source_y * vertical_scale;
                    for destination_x in x..x + horizontal_scale {
                        for y in top..top + vertical_scale {
                            self.line.print_dot(destination_x, y);
                        }
                    }
                }
            }
        }

        let columns = payload.len() / bytes_per_column;
        self.print_x = self
            .print_x
            .saturating_add(columns as u32 * horizontal_scale);
        self.line_used_width = self.line_used_width.max(self.print_x);
        if columns > 0 {
            self.at_beginning_of_line = false;
        }
    }

    fn print_raster_image(
        &mut self,
        payload: &[u8],
        width_bytes: usize,
        width_dots: u32,
        height_dots: usize,
        horizontal_scale: u32,
        vertical_scale: u32,
    ) -> Result<(), RenderError> {
        // GS v 0 is row-major, unlike ESC *. Its image is printed immediately
        // and advances the paper by the rendered height.
        let image_width = width_dots.saturating_mul(horizontal_scale);
        let remaining_width = self.print_area_width.saturating_sub(image_width);
        let image_left = match self.justification {
            Justification::Left => 0,
            Justification::Center => remaining_width / 2,
            Justification::Right => remaining_width,
        };
        let physical_left = self.print_area_left.saturating_add(image_left);
        let print_area_right = self.print_area_left.saturating_add(self.print_area_width);

        let next_line_top = self
            .line_top
            .saturating_add(height_dots as u32 * vertical_scale);
        self.validate_roll_height(next_line_top)?;

        for (source_y, row) in payload.chunks_exact(width_bytes).enumerate() {
            for (byte_index, byte) in row.iter().copied().enumerate() {
                for bit in 0..8 {
                    if byte & (0x80 >> bit) == 0 {
                        continue;
                    }

                    let source_x = byte_index as u32 * 8 + bit;
                    if source_x >= width_dots {
                        continue;
                    }
                    let left =
                        physical_left.saturating_add(source_x.saturating_mul(horizontal_scale));
                    let top = self.line_top + source_y as u32 * vertical_scale;
                    for x in left..left + horizontal_scale {
                        // The printer drops image dots beyond the active print
                        // area instead of letting them spill into its margins.
                        if x >= print_area_right {
                            continue;
                        }
                        for y in top..top + vertical_scale {
                            self.roll.print_dot(x, y);
                        }
                    }
                }
            }
        }

        self.line_top = next_line_top;
        self.roll.ensure_height(self.line_top);
        self.print_x = 0;
        self.line_used_width = 0;
        self.at_beginning_of_line = true;
        Ok(())
    }

    fn print_barcode(
        &mut self,
        barcode: &barcode::EncodedBarcode,
        offset: usize,
    ) -> Result<(), RenderError> {
        if !self.at_beginning_of_line {
            return Err(RenderError::CommandRequiresBeginningOfLine {
                command: "GS k",
                offset,
            });
        }

        let barcode_width = barcode
            .bars
            .iter()
            .map(|bar| self.bar_element_width(bar.width))
            .fold(0_u32, u32::saturating_add);
        if barcode_width > self.print_area_width {
            return Err(RenderError::InvalidBarcodeData {
                system: "one-dimensional",
                offset,
                reason: "symbol is wider than the active print area",
            });
        }
        let hri = (self.hri_position != HriPosition::None)
            .then(|| render_hri(&barcode.hri, &self.hri_font));
        let content_width = hri
            .as_ref()
            .map_or(barcode_width, |surface| barcode_width.max(surface.width));
        let remaining_width = self.print_area_width.saturating_sub(content_width);
        let content_left = match self.justification {
            Justification::Left => 0,
            Justification::Center => remaining_width / 2,
            Justification::Right => remaining_width,
        };
        let physical_content_left = self.print_area_left.saturating_add(content_left);
        let physical_barcode_left =
            physical_content_left.saturating_add(content_width.saturating_sub(barcode_width) / 2);
        let barcode_height = self.barcode_height.max(
            u32::from(barcode.minimum_height_modules).saturating_mul(self.barcode_module_width),
        );
        let hri_height = hri.as_ref().map_or(0, |surface| surface.height);
        let hri_rows = u32::from(matches!(
            self.hri_position,
            HriPosition::Above | HriPosition::Below
        )) + 2 * u32::from(self.hri_position == HriPosition::AboveAndBelow);
        let barcode_top = self.line_top
            + if matches!(
                self.hri_position,
                HriPosition::Above | HriPosition::AboveAndBelow
            ) {
                hri_height
            } else {
                0
            };
        let next_line_top = self
            .line_top
            .saturating_add(barcode_height)
            .saturating_add(hri_height.saturating_mul(hri_rows));
        self.validate_roll_height(next_line_top)?;

        if let Some(hri) = hri.as_ref() {
            let hri_left =
                physical_content_left.saturating_add(content_width.saturating_sub(hri.width) / 2);
            if matches!(
                self.hri_position,
                HriPosition::Above | HriPosition::AboveAndBelow
            ) {
                self.roll.composite_at(hri, hri_left, self.line_top);
            }
            if matches!(
                self.hri_position,
                HriPosition::Below | HriPosition::AboveAndBelow
            ) {
                self.roll
                    .composite_at(hri, hri_left, barcode_top.saturating_add(barcode_height));
            }
        }

        let mut element_left = physical_barcode_left;
        for bar in &barcode.bars {
            let width = self.bar_element_width(bar.width);
            if bar.dark {
                for x in element_left..element_left + width {
                    for y in barcode_top..barcode_top + barcode_height {
                        self.roll.print_dot(x, y);
                    }
                }
            }
            element_left = element_left.saturating_add(width);
        }

        // GS k prints immediately and feeds exactly enough paper for the
        // barcode, independently of the selected text line spacing.
        self.line_top = next_line_top;
        self.roll.ensure_height(self.line_top);
        self.print_x = 0;
        self.line_used_width = 0;
        self.at_beginning_of_line = true;
        Ok(())
    }

    fn bar_element_width(&self, width: barcode::BarWidth) -> u32 {
        match width {
            barcode::BarWidth::Modules(modules) => {
                u32::from(modules).saturating_mul(self.barcode_module_width)
            }
            barcode::BarWidth::Narrow => self.barcode_module_width,
            barcode::BarWidth::Wide => match self.barcode_module_width {
                2 => 5,
                3 => 8,
                4 => 10,
                5 => 13,
                6 => 16,
                _ => unreachable!("GS w accepts only module widths 2 through 6"),
            },
        }
    }

    fn store_qr_data(&mut self, data: &[u8], offset: usize) -> Result<(), RenderError> {
        self.require_qr(offset)?;
        if data.is_empty() {
            return Err(RenderError::InvalidQrData {
                offset,
                reason: "expected at least one data byte",
            });
        }

        // Epson keeps the stored bytes after printing. Only another store
        // command or ESC @ replaces them.
        self.stored_qr_data = Some(data.to_vec());
        Ok(())
    }

    fn set_qr_module_size(&mut self, module_size: u8, offset: usize) -> Result<(), RenderError> {
        self.require_qr(offset)?;
        self.qr_module_size = u32::from(module_size);
        Ok(())
    }

    fn select_qr_model_2(&self, offset: usize) -> Result<(), RenderError> {
        // The matrix adapter generates ISO/IEC 18004 Model 2 symbols. Keeping
        // this explicit prevents Model 1 or Micro QR input from looking valid.
        self.require_qr(offset)
    }

    fn set_qr_error_correction(
        &mut self,
        error_correction: qr::ErrorCorrection,
        offset: usize,
    ) -> Result<(), RenderError> {
        self.require_qr(offset)?;
        self.qr_error_correction = error_correction;
        Ok(())
    }

    fn print_qr(&mut self, offset: usize) -> Result<(), RenderError> {
        self.require_qr(offset)?;
        if !self.at_beginning_of_line {
            return Err(RenderError::CommandRequiresBeginningOfLine {
                command: "GS ( k Function 181",
                offset,
            });
        }

        let data = self
            .stored_qr_data
            .as_deref()
            .ok_or(RenderError::QrDataEmpty { offset })?;
        let encoded = qr::encode(data, self.qr_error_correction).map_err(|error| match error {
            qr::QrError::DataTooLong => RenderError::InvalidQrData {
                offset,
                reason: "data does not fit in a QR symbol",
            },
        })?;
        let module_count = encoded.width as u32;
        let symbol_size = module_count.saturating_mul(self.qr_module_size);
        if symbol_size > self.print_area_width {
            return Err(RenderError::InvalidQrData {
                offset,
                reason: "symbol is wider than the active print area",
            });
        }

        let remaining_width = self.print_area_width.saturating_sub(symbol_size);
        let symbol_left = match self.justification {
            Justification::Left => 0,
            Justification::Center => remaining_width / 2,
            Justification::Right => remaining_width,
        };
        let physical_left = self.print_area_left.saturating_add(symbol_left);
        let next_line_top = self.line_top.saturating_add(symbol_size);
        self.validate_roll_height(next_line_top)?;

        for (index, dark) in encoded.modules.into_iter().enumerate() {
            if !dark {
                continue;
            }
            let module_x = index as u32 % module_count;
            let module_y = index as u32 / module_count;
            let left = physical_left + module_x * self.qr_module_size;
            let top = self.line_top + module_y * self.qr_module_size;
            for x in left..left + self.qr_module_size {
                for y in top..top + self.qr_module_size {
                    self.roll.print_dot(x, y);
                }
            }
        }

        // Function 181 prints immediately and advances by exactly the symbol
        // height. The stored data remains available for another print command.
        self.line_top = next_line_top;
        self.roll.ensure_height(self.line_top);
        self.print_x = 0;
        self.line_used_width = 0;
        self.at_beginning_of_line = true;
        Ok(())
    }

    fn store_raster_graphics(
        &mut self,
        graphics: BufferedGraphics,
        offset: usize,
    ) -> Result<(), RenderError> {
        self.require_graphics(offset)?;
        if !self.at_beginning_of_line {
            return Err(RenderError::CommandRequiresBeginningOfLine {
                command: "GS ( L Function 112",
                offset,
            });
        }

        // Function 112 stores a complete raster plane. The later Function 50
        // applies current layout state, so retain source dots and scale only.
        self.buffered_graphics = Some(graphics);
        Ok(())
    }

    fn print_buffered_graphics(&mut self, offset: usize) -> Result<(), RenderError> {
        self.require_graphics(offset)?;
        if !self.at_beginning_of_line {
            return Err(RenderError::CommandRequiresBeginningOfLine {
                command: "GS ( L Function 50",
                offset,
            });
        }
        let graphics = self
            .buffered_graphics
            .clone()
            .ok_or(RenderError::GraphicsBufferEmpty { offset })?;
        self.print_raster_image(
            &graphics.payload,
            graphics.row_bytes,
            graphics.width_dots as u32,
            graphics.height_dots,
            graphics.horizontal_scale,
            graphics.vertical_scale,
        )?;
        // Function 50 consumes the print-buffer image. A second Function 50
        // therefore cannot invent another copy without a new store command.
        self.buffered_graphics = None;
        Ok(())
    }

    fn feed_to_cut_position_and_cut(
        &mut self,
        mode: u8,
        feed: u8,
        offset: usize,
    ) -> Result<(), RenderError> {
        if !self.at_beginning_of_line {
            return Ok(());
        }

        if !self.supports_full_cut && !self.supports_partial_cut {
            // Epson Function B remains useful on mechanisms without a cutter:
            // they perform only the explicit n-unit feed.
            return self.print_and_feed_motion_units(feed);
        }

        // A cutter-equipped profile also needs the model-specific distance
        // from the print position to the blade. Keep that path explicit until
        // the profile can supply it.
        Err(RenderError::UnsupportedCutMode { mode, offset })
    }

    fn cut(&mut self, partial: bool, offset: usize) -> Result<(), RenderError> {
        if !self.at_beginning_of_line {
            return Ok(());
        }

        let supported = if partial {
            self.supports_partial_cut
        } else {
            self.supports_full_cut
        };
        if !supported {
            return Err(RenderError::CommandUnsupportedByProfile {
                command: if partial {
                    "GS V partial cut"
                } else {
                    "GS V full cut"
                },
                profile: self.profile_id.clone(),
                offset,
            });
        }

        let sheet_count = self.completed_sheets.len().saturating_add(1);
        if sheet_count > self.limits.max_sheets {
            return Err(RenderError::LimitExceeded {
                kind: LimitKind::Sheets,
                value: sheet_count as u64,
                limit: self.limits.max_sheets as u64,
            });
        }

        // Function A cuts at the current paper position; it does not add a
        // model-dependent feed-to-cutter distance.
        let next_roll = MonoSurface::new(self.roll.width);
        self.completed_sheets
            .push(std::mem::replace(&mut self.roll, next_roll));
        self.line_top = 0;
        Ok(())
    }

    fn require_column_bit_image(&self, offset: usize) -> Result<(), RenderError> {
        self.require_profile_feature(
            self.supports_column_bit_image,
            "ESC * column bit image",
            offset,
        )
    }

    fn require_raster_bit_image(&self, offset: usize) -> Result<(), RenderError> {
        self.require_profile_feature(
            self.supports_raster_bit_image,
            "GS v 0 raster bit image",
            offset,
        )
    }

    fn require_graphics(&self, offset: usize) -> Result<(), RenderError> {
        self.require_profile_feature(self.supports_graphics, "GS ( L graphics", offset)
    }

    fn require_barcode_system(
        &self,
        system: BarcodeSystem,
        is_function_a: bool,
        offset: usize,
    ) -> Result<(), RenderError> {
        let supported = if is_function_a {
            self.function_a_barcodes.contains(&system)
        } else {
            self.function_b_barcodes.contains(&system)
        };
        self.require_profile_feature(supported, barcode_system_command_name(system), offset)
    }

    fn require_qr(&self, offset: usize) -> Result<(), RenderError> {
        self.require_profile_feature(self.supports_qr, "GS ( k QR code", offset)
    }

    fn require_profile_feature(
        &self,
        supported: bool,
        command: &'static str,
        offset: usize,
    ) -> Result<(), RenderError> {
        if supported {
            return Ok(());
        }

        Err(RenderError::CommandUnsupportedByProfile {
            command,
            profile: self.profile_id.clone(),
            offset,
        })
    }

    fn validate_command_payload_size(&self, payload_bytes: usize) -> Result<(), RenderError> {
        if payload_bytes > self.limits.max_command_payload_bytes {
            return Err(RenderError::LimitExceeded {
                kind: LimitKind::CommandPayloadBytes,
                value: payload_bytes as u64,
                limit: self.limits.max_command_payload_bytes as u64,
            });
        }
        Ok(())
    }

    fn validate_roll_height(&self, height_dots: u32) -> Result<(), RenderError> {
        if height_dots > self.limits.max_sheet_height_dots {
            return Err(RenderError::LimitExceeded {
                kind: LimitKind::SheetHeightDots,
                value: u64::from(height_dots),
                limit: u64::from(self.limits.max_sheet_height_dots),
            });
        }

        let completed_dots = self
            .completed_sheets
            .iter()
            .map(|sheet| u64::from(sheet.width) * u64::from(sheet.height))
            .sum::<u64>();
        let current_dots = u64::from(self.roll.width) * u64::from(height_dots);
        let total_dots = completed_dots.saturating_add(current_dots);
        if total_dots > self.limits.max_total_dots {
            return Err(RenderError::LimitExceeded {
                kind: LimitKind::TotalDots,
                value: total_dots,
                limit: self.limits.max_total_dots,
            });
        }
        Ok(())
    }

    fn drawer_pulse(
        &mut self,
        connector: u8,
        on_time_units: u8,
        off_time_units: u8,
        offset: usize,
    ) -> Result<(), RenderError> {
        if !matches!(connector, 0 | 1 | 48 | 49) {
            return Err(RenderError::UnsupportedDrawerConnector { connector, offset });
        }
        if !self.supports_standard_drawer_pulse {
            return Err(RenderError::CommandUnsupportedByProfile {
                command: "ESC p drawer pulse",
                profile: self.profile_id.clone(),
                offset,
            });
        }

        // Pulse timing affects the connector only; retain it as an event
        // without inventing any paper-side marks.
        self.device_events.push(DeviceEvent::CashDrawerPulse {
            connector,
            on_time_units,
            off_time_units,
        });
        Ok(())
    }

    fn line_feed(&mut self) -> Result<(), RenderError> {
        self.feed_lines(1)
    }

    fn carriage_return(&mut self) -> Result<(), RenderError> {
        match self.carriage_return_mode {
            CarriageReturnMode::Ignored => Ok(()),
            CarriageReturnMode::LineFeed => self.line_feed(),
        }
    }

    fn into_surfaces(mut self) -> Result<Vec<MonoSurface>, RenderError> {
        // A cut already finalized the preceding roll. Do not invent a blank
        // trailing receipt when the job ends immediately after that cut.
        if self.roll.height > 0 {
            let sheet_count = self.completed_sheets.len().saturating_add(1);
            if sheet_count > self.limits.max_sheets {
                return Err(RenderError::LimitExceeded {
                    kind: LimitKind::Sheets,
                    value: sheet_count as u64,
                    limit: self.limits.max_sheets as u64,
                });
            }
            self.completed_sheets.push(self.roll);
        }
        Ok(self.completed_sheets)
    }
}

fn barcode_system_command_name(system: BarcodeSystem) -> &'static str {
    match system {
        BarcodeSystem::UpcA => "GS k UPC-A",
        BarcodeSystem::UpcE => "GS k UPC-E",
        BarcodeSystem::Ean13 => "GS k EAN-13",
        BarcodeSystem::Ean8 => "GS k EAN-8",
        BarcodeSystem::Code39 => "GS k Code 39",
        BarcodeSystem::Itf => "GS k ITF",
        BarcodeSystem::Codabar => "GS k Codabar",
        BarcodeSystem::Code93 => "GS k Code 93",
        BarcodeSystem::Code128 => "GS k Code 128",
        BarcodeSystem::Gs1_128 => "GS k GS1-128",
        BarcodeSystem::Gs1DataBarOmnidirectional => "GS k GS1 DataBar Omnidirectional",
        BarcodeSystem::Gs1DataBarTruncated => "GS k GS1 DataBar Truncated",
        BarcodeSystem::Gs1DataBarLimited => "GS k GS1 DataBar Limited",
        BarcodeSystem::Gs1DataBarExpanded => "GS k GS1 DataBar Expanded",
        BarcodeSystem::Code128Auto => "GS k Code 128 auto",
    }
}

fn render_hri(data: &[char], font: &ProfileFont) -> MonoSurface {
    let width = (data.len() as u32).saturating_mul(font.cell_width_dots);
    let mut surface = MonoSurface::new(width);
    surface.ensure_height(font.cell_height_dots);

    for (character_index, character) in data.iter().copied().enumerate() {
        let font_size = font.cell_height_dots as f32;
        let (metrics, bitmap) = default_font().rasterize(character, font_size);
        let horizontal_crop = metrics.width.saturating_sub(font.cell_width_dots as usize) / 2;
        let horizontal_padding = (font.cell_width_dots as usize).saturating_sub(metrics.width) / 2;
        let glyph_top = font.baseline_dots as i32 - (metrics.ymin + metrics.height as i32);
        let cell_left = character_index as u32 * font.cell_width_dots;

        for source_y in 0..metrics.height {
            let destination_y = glyph_top + source_y as i32;
            if destination_y < 0 || destination_y >= font.cell_height_dots as i32 {
                continue;
            }
            for source_x in horizontal_crop..metrics.width {
                let destination_x = horizontal_padding + source_x - horizontal_crop;
                if destination_x >= font.cell_width_dots as usize {
                    break;
                }
                if bitmap[source_y * metrics.width + source_x] >= GLYPH_ALPHA_THRESHOLD {
                    surface.print_dot(cell_left + destination_x as u32, destination_y as u32);
                }
            }
        }
    }
    surface
}

fn default_font() -> &'static Font {
    static DEFAULT_FONT: OnceLock<Font> = OnceLock::new();

    DEFAULT_FONT.get_or_init(|| {
        Font::from_bytes(DEFAULT_FONT_BYTES, FontSettings::default())
            .expect("the bundled Noto Sans Mono font must remain valid")
    })
}

fn is_supported_code_page_encoding(encoding: &str) -> bool {
    matches!(
        encoding,
        "CP437"
            | "CP720"
            | "CP737"
            | "CP775"
            | "CP850"
            | "CP852"
            | "CP855"
            | "CP857"
            | "CP858"
            | "CP860"
            | "CP861"
            | "CP862"
            | "CP863"
            | "CP864"
            | "CP865"
            | "CP866"
            | "CP869"
            | "CP874"
            | "CP1250"
            | "CP1251"
            | "CP1252"
            | "CP1253"
            | "CP1254"
            | "CP1255"
            | "CP1256"
            | "CP1257"
            | "CP1258"
    )
}

fn decode_printable_byte(byte: u8, encoding: &str) -> Option<char> {
    match encoding {
        "CP437" => Some(char::from(Cp437::from(byte))),
        "CP720" => Some(char::from(Cp720::from(byte))),
        "CP737" => Some(char::from(Cp737::from(byte))),
        "CP775" => Some(char::from(Cp775::from(byte))),
        "CP850" => Some(char::from(Cp850::from(byte))),
        "CP852" => Some(char::from(Cp852::from(byte))),
        "CP855" => Some(char::from(Cp855::from(byte))),
        "CP857" => Cp857::try_from(byte).ok().map(char::from),
        "CP858" => Some(char::from(Cp858::from(byte))),
        "CP860" => Some(char::from(Cp860::from(byte))),
        "CP861" => Some(char::from(Cp861::from(byte))),
        "CP862" => Some(char::from(Cp862::from(byte))),
        "CP863" => Some(char::from(Cp863::from(byte))),
        "CP864" => Cp864::try_from(byte).ok().map(char::from),
        "CP865" => Some(char::from(Cp865::from(byte))),
        "CP866" => Some(char::from(Cp866::from(byte))),
        "CP869" => Some(char::from(Cp869::from(byte))),
        "CP874" => Cp874::try_from(byte).ok().map(char::from),
        "CP1250" => decode_with_encoding_rs(byte, WINDOWS_1250),
        "CP1251" => decode_with_encoding_rs(byte, WINDOWS_1251),
        "CP1252" => decode_with_encoding_rs(byte, WINDOWS_1252),
        "CP1253" => decode_with_encoding_rs(byte, WINDOWS_1253),
        "CP1254" => decode_with_encoding_rs(byte, WINDOWS_1254),
        "CP1255" => decode_with_encoding_rs(byte, WINDOWS_1255),
        "CP1256" => decode_with_encoding_rs(byte, WINDOWS_1256),
        "CP1257" => decode_with_encoding_rs(byte, WINDOWS_1257),
        "CP1258" => decode_with_encoding_rs(byte, WINDOWS_1258),
        _ => unreachable!("code-page support is checked when ESC t is executed"),
    }
}

fn decode_with_encoding_rs(byte: u8, encoding: &'static Encoding) -> Option<char> {
    let bytes = [byte];
    let (decoded, had_errors) = encoding.decode_without_bom_handling(&bytes);

    (!had_errors).then(|| decoded.chars().next()).flatten()
}

impl MonoSurface {
    fn new(width: u32) -> Self {
        Self {
            width,
            height: 0,
            dots: Vec::new(),
        }
    }

    fn print_dot(&mut self, x: u32, y: u32) {
        if x >= self.width {
            return;
        }

        self.ensure_height(y + 1);
        self.dots[(y * self.width + x) as usize] = true;
    }

    fn clear_dot(&mut self, x: u32, y: u32) {
        if x >= self.width || y >= self.height {
            return;
        }

        self.dots[(y * self.width + x) as usize] = false;
    }

    fn composite_at(&mut self, source: &Self, left: u32, top: u32) {
        if source.height == 0 {
            return;
        }

        self.ensure_height(top.saturating_add(source.height));
        for y in 0..source.height {
            for x in 0..source.width {
                if source.is_printed(x, y) {
                    self.print_dot(left.saturating_add(x), top + y);
                }
            }
        }
    }

    fn ensure_height(&mut self, height: u32) {
        if height <= self.height {
            return;
        }

        self.dots.resize((height * self.width) as usize, false);
        self.height = height;
    }

    fn clear(&mut self) {
        self.height = 0;
        self.dots.clear();
    }
}
