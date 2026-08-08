//! Bit-image and raster graphics painting.

use crate::RenderError;
use crate::state::{BufferedGraphics, Justification, PrinterState};
use crate::surface::RenderSurface;

impl<S: RenderSurface> PrinterState<S> {
    pub(crate) fn paint_bit_image(
        &mut self,
        payload: &[u8],
        bytes_per_column: usize,
        horizontal_scale: u32,
        vertical_pitch: u32,
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
                    // This is a pitch rather than a scaling factor: a source
                    // bit prints one row and the profile decides where the
                    // next source row begins. Epson uses three dots here,
                    // while the calibrated NT-5890K uses adjacent rows.
                    let y = source_y * vertical_pitch;
                    for destination_x in x..x + horizontal_scale {
                        self.line.print_dot(destination_x, y);
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
            self.line_has_printable_data = true;
            self.at_beginning_of_line = false;
        }
    }

    pub(crate) fn print_raster_image(
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
        self.line_has_printable_data = false;
        self.at_beginning_of_line = true;
        Ok(())
    }

    pub(crate) fn store_raster_graphics(
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

    pub(crate) fn print_buffered_graphics(&mut self, offset: usize) -> Result<(), RenderError> {
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
}
