//! Character decoding and glyph placement.

use crate::font;
use crate::state::PrinterState;
use crate::surface::MonoSurface;
use crate::{RenderError, international};
use encoding_rs::{
    Encoding, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252, WINDOWS_1253, WINDOWS_1254, WINDOWS_1255,
    WINDOWS_1256, WINDOWS_1257, WINDOWS_1258,
};
use escpost_profiles::Font as ProfileFont;
use oem_cp::{
    Cp437, Cp720, Cp737, Cp775, Cp850, Cp852, Cp855, Cp857, Cp858, Cp860, Cp861, Cp862, Cp863,
    Cp864, Cp865, Cp866, Cp869, Cp874,
};

impl PrinterState {
    pub(crate) fn print_byte(&mut self, byte: u8, offset: usize) -> Result<(), RenderError> {
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
        if font::default_font().lookup_glyph_index(character) == 0 {
            return Err(RenderError::MissingGlyph { character, offset });
        }

        self.print_character(character)
    }

    pub(crate) fn print_character(&mut self, character: char) -> Result<(), RenderError> {
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

        // The bundled font supplies stable glyph shapes; the profile cell stays
        // authoritative for size, spacing, and advancement. The font module maps
        // the glyph onto the cell's dot grid — fitting the size, condensing the
        // width, placing the baseline, and supersampling — and returns the inked
        // dots; placement below applies styling and multipliers.
        let cell_width_dots = self.active_font.cell_width_dots;
        let cell_height_dots = self.active_font.cell_height_dots;
        let geometry = font::glyph_geometry(
            cell_width_dots,
            cell_height_dots,
            self.active_font.baseline_dots,
        );
        let ink = font::glyph_cell_mask(character, cell_width_dots, cell_height_dots, &geometry);

        for destination_y in 0..cell_height_dots as usize {
            for destination_x in 0..cell_width_dots as usize {
                if !ink[destination_y * cell_width_dots as usize + destination_x] {
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
                        // ESC E emphasis is a firmware double-strike: the printer
                        // re-strikes the same ROM glyph one dot to the right,
                        // thickening it horizontally. Smearing the base ink by one
                        // dot models that mechanism faithfully rather than swapping
                        // in a separately-designed bold weight, which would diverge
                        // from the dots the device actually lays down. It stops at
                        // the cell's right edge so bold stays within its own cell.
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
        self.line_has_printable_data = true;
        self.at_beginning_of_line = false;
        Ok(())
    }

    pub(crate) fn current_character_advance_width(&self) -> u32 {
        self.active_font
            .cell_width_dots
            .saturating_add(self.right_side_character_spacing)
            .saturating_mul(self.character_width_multiplier)
    }
}

pub(crate) fn render_hri(data: &[char], profile_font: &ProfileFont) -> MonoSurface {
    let width = (data.len() as u32).saturating_mul(profile_font.cell_width_dots);
    let mut surface = MonoSurface::new(width);
    surface.ensure_height(profile_font.cell_height_dots);

    // Map glyphs onto the cell exactly like printed text so HRI labels share the
    // same shapes; the geometry is constant across the label.
    let geometry = font::glyph_geometry(
        profile_font.cell_width_dots,
        profile_font.cell_height_dots,
        profile_font.baseline_dots,
    );
    let cell_width_dots = profile_font.cell_width_dots;
    let cell_height_dots = profile_font.cell_height_dots;

    for (character_index, character) in data.iter().copied().enumerate() {
        let ink = font::glyph_cell_mask(character, cell_width_dots, cell_height_dots, &geometry);
        let cell_left = character_index as u32 * cell_width_dots;

        for destination_y in 0..cell_height_dots as usize {
            for destination_x in 0..cell_width_dots as usize {
                if ink[destination_y * cell_width_dots as usize + destination_x] {
                    surface.print_dot(cell_left + destination_x as u32, destination_y as u32);
                }
            }
        }
    }
    surface
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
