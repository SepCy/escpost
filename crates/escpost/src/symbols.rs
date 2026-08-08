//! Barcode and QR symbol painting.

use crate::state::{HriPosition, Justification, PrinterState};
use crate::surface::RenderSurface;
use crate::text::render_hri;
use crate::{RenderError, barcode, qr};
use escpost_profiles::BarcodeSystem;

impl<S: RenderSurface> PrinterState<S> {
    pub(crate) fn print_barcode(
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
        let hri: Option<S> = (self.hri_position != HriPosition::None)
            .then(|| render_hri(&barcode.hri, &self.hri_font, self.scale, self.antialias));
        let content_width = hri
            .as_ref()
            .map_or(barcode_width, |surface| barcode_width.max(surface.width()));
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
        let hri_height = hri.as_ref().map_or(0, RenderSurface::height);
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
                physical_content_left.saturating_add(content_width.saturating_sub(hri.width()) / 2);
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
        self.line_has_printable_data = false;
        self.at_beginning_of_line = true;
        Ok(())
    }

    pub(crate) fn bar_element_width(&self, width: barcode::BarWidth) -> u32 {
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

    pub(crate) fn store_qr_data(&mut self, data: &[u8], offset: usize) -> Result<(), RenderError> {
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

    pub(crate) fn set_qr_module_size(
        &mut self,
        module_size: u8,
        offset: usize,
    ) -> Result<(), RenderError> {
        self.require_qr(offset)?;
        self.qr_module_size = u32::from(module_size);
        Ok(())
    }

    pub(crate) fn select_qr_model_2(&self, offset: usize) -> Result<(), RenderError> {
        // The matrix adapter generates ISO/IEC 18004 Model 2 symbols. Keeping
        // this explicit prevents Model 1 or Micro QR input from looking valid.
        self.require_qr(offset)
    }

    pub(crate) fn set_qr_error_correction(
        &mut self,
        error_correction: qr::ErrorCorrection,
        offset: usize,
    ) -> Result<(), RenderError> {
        self.require_qr(offset)?;
        self.qr_error_correction = error_correction;
        Ok(())
    }

    pub(crate) fn print_qr(&mut self, offset: usize) -> Result<(), RenderError> {
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
        self.line_has_printable_data = false;
        self.at_beginning_of_line = true;
        Ok(())
    }
}

pub(crate) fn barcode_system_command_name(system: BarcodeSystem) -> &'static str {
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
