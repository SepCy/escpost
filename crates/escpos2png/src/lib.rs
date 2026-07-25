//! Dot-accurate ESC/POS rendering.

use encoding_rs::{
    Encoding, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252, WINDOWS_1253, WINDOWS_1254, WINDOWS_1255,
    WINDOWS_1256, WINDOWS_1257, WINDOWS_1258,
};
use escpos2png_profiles::{Font as ProfileFont, PrinterProfile};
use fontdue::{Font, FontSettings};
use oem_cp::{
    Cp437, Cp720, Cp737, Cp775, Cp850, Cp852, Cp855, Cp857, Cp858, Cp860, Cp861, Cp862, Cp863,
    Cp864, Cp865, Cp866, Cp869, Cp874,
};
use std::collections::BTreeMap;
use std::sync::OnceLock;
use thiserror::Error;

const DEFAULT_FONT_BYTES: &[u8] =
    include_bytes!("../../../assets/fonts/noto-sans-mono/NotoSansMono-Regular.ttf");
const GLYPH_ALPHA_THRESHOLD: u8 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Justification {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderResult {
    pub sheets: Vec<RenderedSheet>,
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

    #[error("could not encode the rendered sheet as PNG")]
    EncodePng(#[from] png::EncodingError),
}

pub fn render(data: &[u8], profile: &PrinterProfile) -> Result<RenderResult, RenderError> {
    let mut state = PrinterState::new(profile);
    let mut offset = 0;

    while offset < data.len() {
        offset += match data[offset] {
            0x0a => {
                state.line_feed();
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

    let png = encode_png(&state.roll)?;

    Ok(RenderResult {
        sheets: vec![RenderedSheet {
            surface: state.roll,
            png,
        }],
    })
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
            state.feed_lines(lines);
            Ok(3)
        }
        0x74 => {
            let Some(code_page) = data.get(2).copied() else {
                return Err(RenderError::TruncatedCommand {
                    command: "ESC t",
                    offset,
                });
            };
            let encoding = state
                .code_page_encoding(code_page)
                .unwrap_or("<not present in printer profile>");
            if !is_supported_code_page_encoding(encoding) {
                return Err(RenderError::UnsupportedCodePage {
                    code_page,
                    encoding: encoding.to_owned(),
                    offset,
                });
            }

            state.select_code_page(code_page);
            Ok(3)
        }
        command => Err(RenderError::UnsupportedEscCommand { command, offset }),
    }
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
    let command_length = 5 + columns * bytes_per_column;
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
        0x76 => execute_gs_v0(data, offset, state),
        command => Err(RenderError::UnsupportedGsCommand { command, offset }),
    }
}

fn execute_gs_v0(
    data: &[u8],
    offset: usize,
    state: &mut PrinterState,
) -> Result<usize, RenderError> {
    if data.len() < 8 {
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

    let command_length = 8 + width_bytes * height_dots;
    let Some(payload) = data.get(8..command_length) else {
        return Err(RenderError::TruncatedCommand {
            command: "GS v 0",
            offset,
        });
    };

    state.print_raster_image(
        payload,
        width_bytes,
        height_dots,
        horizontal_scale,
        vertical_scale,
    );
    Ok(command_length)
}

#[derive(Debug)]
struct PrinterState {
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
    right_side_character_spacing: u32,
    character_width_multiplier: u32,
    character_height_multiplier: u32,
    emphasized: bool,
    underline_thickness: u32,
    reversed: bool,
    justification: Justification,
    line_height: u32,
}

impl PrinterState {
    fn new(profile: &PrinterProfile) -> Self {
        let width = profile.geometry.printable_width_dots;
        let default_line_spacing = profile.defaults.line_spacing_dots;
        let font_a = profile.fonts.a.clone();
        let font_b = profile.fonts.b.clone();

        Self {
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
            right_side_character_spacing: 0,
            character_width_multiplier: 1,
            character_height_multiplier: 1,
            emphasized: false,
            underline_thickness: 0,
            reversed: false,
            justification: Justification::Left,
            line_height: 0,
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
        self.right_side_character_spacing = 0;
        self.character_width_multiplier = 1;
        self.character_height_multiplier = 1;
        self.emphasized = false;
        self.underline_thickness = 0;
        self.reversed = false;
        self.justification = Justification::Left;
        self.line_height = 0;
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

    fn feed_lines(&mut self, lines: u8) {
        let remaining_width = self.line.width.saturating_sub(self.line_used_width);
        // Track logical data width rather than scanning black dots. This keeps
        // spaces significant and preserves far-right data after ESC $ or
        // ESC \ moves the cursor back to an earlier position.
        let line_left = match self.justification {
            Justification::Left => 0,
            Justification::Center => remaining_width / 2,
            Justification::Right => remaining_width,
        };
        self.roll.composite_at(
            &self.line,
            self.print_area_left.saturating_add(line_left),
            self.line_top,
        );
        // Oversized text and bit images must not overlap the next line even
        // when the configured line spacing is smaller than their dot height.
        let feed = match lines {
            0 => 0,
            lines => self
                .line_spacing
                .max(self.line.height.max(self.line_height))
                .saturating_add(
                    self.line_spacing
                        .saturating_mul(u32::from(lines).saturating_sub(1)),
                ),
        };
        self.line_top = self.line_top.saturating_add(feed);
        self.roll.ensure_height(self.line_top);
        self.line.clear();
        self.print_x = 0;
        self.line_used_width = 0;
        self.at_beginning_of_line = true;
        self.line_height = 0;
    }

    fn print_byte(&mut self, byte: u8, offset: usize) -> Result<(), RenderError> {
        // The ESC t operand is printer-specific. The profile translates that
        // numeric slot into a stable encoding name before we decode the byte.
        let encoding = self
            .code_page_encoding(self.active_code_page)
            .unwrap_or("<not present in printer profile>");
        if !is_supported_code_page_encoding(encoding) {
            return Err(RenderError::UnsupportedCodePage {
                code_page: self.active_code_page,
                encoding: encoding.to_owned(),
                offset,
            });
        }

        let Some(character) = decode_printable_byte(byte, encoding) else {
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

        self.print_character(character);
        Ok(())
    }

    fn print_character(&mut self, character: char) {
        let cell_width = self
            .active_font
            .cell_width_dots
            .saturating_add(self.right_side_character_spacing)
            .saturating_mul(self.character_width_multiplier);
        let cell_height = self
            .active_font
            .cell_height_dots
            .saturating_mul(self.character_height_multiplier);
        if self.print_x.saturating_add(cell_width) > self.line.width {
            self.line_feed();
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
        height_dots: usize,
        horizontal_scale: u32,
        vertical_scale: u32,
    ) {
        // GS v 0 is row-major, unlike ESC *. Its image is printed immediately
        // from the left edge and advances the paper by the rendered height.
        for (source_y, row) in payload.chunks_exact(width_bytes).enumerate() {
            for (byte_index, byte) in row.iter().copied().enumerate() {
                for bit in 0..8 {
                    if byte & (0x80 >> bit) == 0 {
                        continue;
                    }

                    let source_x = byte_index as u32 * 8 + bit;
                    let left = source_x * horizontal_scale;
                    let top = self.line_top + source_y as u32 * vertical_scale;
                    for x in left..left + horizontal_scale {
                        for y in top..top + vertical_scale {
                            self.roll.print_dot(x, y);
                        }
                    }
                }
            }
        }

        self.line_top = self
            .line_top
            .saturating_add(height_dots as u32 * vertical_scale);
        self.roll.ensure_height(self.line_top);
        self.print_x = 0;
        self.line_used_width = 0;
        self.at_beginning_of_line = true;
    }

    fn line_feed(&mut self) {
        self.feed_lines(1);
    }
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
