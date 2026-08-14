//! Documented constant defaults and upstream-derivation helpers.

use crate::{
    CarriageReturnMode, CommandBehavior, FeedBehavior, Font, Fonts, PositioningBehavior,
    PrinterDefaults,
};

/// Default print media width in dots (384 @ 203 DPI ≈ 4.755").
pub const DEFAULT_WIDTH_DOTS: u32 = 384;

/// Default print media DPI (203 DPI, standard for Epson thermal printers).
pub const DEFAULT_DPI: u32 = 203;

/// Epson's documented 8-dot `ESC *` vertical pitch baseline (DD-031/DD-032):
/// three printer dots between adjacent source rows at 203 DPI.
pub const DEFAULT_EIGHT_DOT_VERTICAL_PITCH_DOTS: u32 = 3;

/// Default font metrics for Font A and Font B (REFERENCE-derived baseline).
pub fn default_fonts() -> Fonts {
    Fonts {
        a: Font {
            cell_width_dots: 12,
            cell_height_dots: 24,
            baseline_dots: 20,
        },
        b: Font {
            cell_width_dots: 9,
            cell_height_dots: 17,
            baseline_dots: 14,
        },
    }
}

/// Default command behavior (all Apply/Feed, standard Epson baseline).
pub fn default_commands() -> CommandBehavior {
    CommandBehavior {
        esc_backslash_negative: PositioningBehavior::Apply,
        esc_dollar_after_printable_data: PositioningBehavior::Apply,
        esc_j: FeedBehavior::Feed,
        gs_v_0_following_lf: FeedBehavior::Feed,
        gs_v_function_b_full: FeedBehavior::Feed,
        gs_v_function_b_partial: FeedBehavior::Feed,
    }
}

/// Default printer runtime settings (REFERENCE-derived baseline).
pub fn default_printer_defaults() -> PrinterDefaults {
    PrinterDefaults {
        line_spacing_dots: 30,
        code_page: 0,
        international_character_set: 0,
        carriage_return: CarriageReturnMode::Ignored,
    }
}

/// Derive cell width in dots from printable width and column count.
///
/// Computes the character cell width: `(width_dots / columns).max(1)` when columns > 0.
/// Guards against zero columns by returning 1 (minimum safe cell width).
pub fn derive_cell_width(width_dots: u32, columns: u32) -> u32 {
    width_dots.checked_div(columns).unwrap_or(1).max(1)
}
