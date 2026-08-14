//! Bundled font provider and the geometry derived from its metrics.
//!
//! DD-023 keeps the glyph-provider boundary replaceable. This module owns the
//! embedded representative font, the one-time measurement of its metrics, and
//! the mapping of those metrics onto a printer profile's character cell —
//! horizontal condensation and the effective baseline. Glyph placement asks for
//! a [`GlyphGeometry`] and rasterizes through [`default_font`]; a future
//! per-profile bitmap atlas could replace this module without touching the
//! placement code.

use fontdue::{Font, FontSettings};
use std::sync::OnceLock;

const DEFAULT_FONT_BYTES: &[u8] =
    include_bytes!("../assets/fonts/noto-sans-mono/NotoSansMono-Regular.ttf");

/// Alpha value at or above which a glyph dot's mean coverage becomes ink.
pub(crate) const GLYPH_ALPHA_THRESHOLD: u8 = 80;

/// Linear supersampling factor for glyph rasterization. Glyphs are rasterized at
/// `N ×` the cell resolution and each output dot is decided from the mean
/// coverage of its `N × N` samples, so horizontal condensation integrates the
/// glyph over each dot's area instead of picking one nearest column. Output
/// stays 1-bit on the profile's dot grid; higher `N` only refines which dots the
/// condensed glyph lights. Two already resolves the stem-placement artefacts of
/// nearest-neighbour condensation; three was visually indistinguishable at this
/// cell size.
const GLYPH_SUPERSAMPLE: u32 = 2;

/// How the bundled font maps onto one profile character cell.
pub(crate) struct GlyphGeometry {
    /// Pixel size to rasterize glyphs at. Normally the cell height so glyphs fill
    /// the cell vertically, reduced when the font's ink extends past its em so the
    /// ink box still fits the cell without clipping.
    pub font_size: f32,
    /// Horizontal scale applied to every glyph so its advance box fills the cell
    /// width instead of overflowing into the next cell.
    pub condense: f32,
    /// Baseline row (dots from the top of the cell) used for vertical placement.
    /// Equals the profile baseline unless the font's descenders would clip, in
    /// which case it is lowered just enough to admit them.
    pub baseline_dots: u32,
}

/// Resolve the geometry for one profile font cell.
pub(crate) fn glyph_geometry(
    cell_width_dots: u32,
    cell_height_dots: u32,
    baseline_dots: u32,
) -> GlyphGeometry {
    let font_size = fitted_font_size(cell_height_dots);
    GlyphGeometry {
        font_size,
        condense: condense_factor(cell_width_dots, font_size),
        baseline_dots: effective_baseline(cell_height_dots, baseline_dots, font_size),
    }
}

/// Rasterize `character` into an 8-bit coverage buffer covering one profile
/// cell at `out_scale ×` the dot resolution (255 = full ink), row-major. The
/// caller thresholds it at [`GLYPH_ALPHA_THRESHOLD`] for the faithful 1-bit path
/// or keeps the coverage for the anti-aliased preview. Placement (styling,
/// multipliers) stays with the caller.
pub(crate) fn glyph_cell_coverage(
    character: char,
    cell_width_dots: u32,
    cell_height_dots: u32,
    geometry: &GlyphGeometry,
    out_scale: u32,
) -> Vec<u8> {
    sample_cell(
        character,
        cell_width_dots,
        cell_height_dots,
        geometry,
        out_scale,
    )
}

/// Sample the condensed, baseline-placed glyph into a coverage buffer of
/// `(cell_width_dots × out_scale) × (cell_height_dots × out_scale)` bytes.
///
/// Each output sample averages `GLYPH_SUPERSAMPLE²` sub-samples of the glyph
/// rasterized at `out_scale × GLYPH_SUPERSAMPLE ×` the cell resolution, so the
/// horizontal condensation is area-correct. At `out_scale = 1` each sample is
/// one dot; larger scales carry the sub-dot detail the AA preview needs.
fn sample_cell(
    character: char,
    cell_width_dots: u32,
    cell_height_dots: u32,
    geometry: &GlyphGeometry,
    out_scale: u32,
) -> Vec<u8> {
    let out_width = (cell_width_dots * out_scale) as usize;
    let out_height = (cell_height_dots * out_scale) as usize;
    let mut out = vec![0u8; out_width * out_height];

    let render_super = (out_scale * GLYPH_SUPERSAMPLE) as f32;
    let (metrics, coverage) =
        default_font().rasterize(character, geometry.font_size * render_super);
    if metrics.width == 0 || metrics.height == 0 {
        return out;
    }

    // Work in cell-dot space; the rasterized bitmap has `render_super` samples
    // per dot. Centre the condensed ink horizontally, place it on the baseline.
    let ink_width_dots = metrics.width as f32 / render_super;
    let horizontal_padding = (cell_width_dots as f32 - ink_width_dots * geometry.condense) / 2.0;
    let glyph_top = geometry.baseline_dots as f32
        - (metrics.ymin as f32 + metrics.height as f32) / render_super;

    let out_scale_f = out_scale as f32;
    let sub = GLYPH_SUPERSAMPLE as f32;
    let denom = GLYPH_SUPERSAMPLE * GLYPH_SUPERSAMPLE;
    for oy in 0..out_height {
        for ox in 0..out_width {
            let mut coverage_sum = 0u32;
            for sub_y in 0..GLYPH_SUPERSAMPLE {
                let out_y_dot = (oy as f32 + (sub_y as f32 + 0.5) / sub) / out_scale_f;
                let source_y = ((out_y_dot - glyph_top) * render_super) as i32;
                if source_y < 0 || source_y as usize >= metrics.height {
                    continue;
                }
                let row = source_y as usize * metrics.width;
                for sub_x in 0..GLYPH_SUPERSAMPLE {
                    let out_x_dot = (ox as f32 + (sub_x as f32 + 0.5) / sub) / out_scale_f;
                    let source_dot = (out_x_dot - horizontal_padding) / geometry.condense;
                    let source_x = (source_dot * render_super) as i32;
                    if source_x < 0 || source_x as usize >= metrics.width {
                        continue;
                    }
                    coverage_sum += coverage[row + source_x as usize] as u32;
                }
            }
            out[oy * out_width + ox] = (coverage_sum / denom) as u8;
        }
    }
    out
}

/// Pixel size that rasterizes glyphs as tall as the cell without clipping.
///
/// `font_size` sets the em, but glyph ink (ascenders, descenders, accents) can
/// extend past the em, so rasterizing at the cell height can still overflow the
/// cell by that excess. When the font's ink is taller than its em, scale the
/// size down so the *ink* box — not just the em — matches the cell height; the
/// glyph then fills the cell exactly and nothing clips, for any embedded font.
/// When the ink already fits inside the em, rasterize at the full cell height.
fn fitted_font_size(cell_height_dots: u32) -> f32 {
    let (ascent_ratio, descent_ratio) = ink_extent_ratios();
    let ink_ratio = ascent_ratio + descent_ratio;
    let cell_height = cell_height_dots as f32;
    if ink_ratio > 1.0 {
        cell_height / ink_ratio
    } else {
        cell_height
    }
}

/// Horizontal condensation factor that maps the font's natural advance onto the
/// profile cell width.
///
/// The font is rasterized at `font_size = cell_height_dots` so glyphs fill the
/// cell vertically. At that size the font's monospace advance is wider than the
/// profile cell (Noto Sans Mono advances 0.6 em, i.e. 14.4 dots in a 12-dot
/// cell), so drawn one-to-one, wide glyphs overflow their cell and touch their
/// neighbours. Condensing every glyph by this factor makes the advance box
/// coincide with the cell: the font then fills the cell in both axes and keeps
/// its designed side bearings, so glyphs no longer collide.
///
/// The same factor applies to every glyph (never a per-glyph measurement), so
/// the monospace grid stays regular, and it derives from the font's own advance
/// metric so replacing the bundled font needs no code change.
fn condense_factor(cell_width_dots: u32, font_size: f32) -> f32 {
    let natural_advance = advance_ratio() * font_size;
    if natural_advance <= f32::EPSILON {
        return 1.0;
    }
    cell_width_dots as f32 / natural_advance
}

/// Baseline row for vertical placement.
///
/// The profile baseline is honoured whenever the font's ink fits the cell with
/// it. If the font's descenders reach past the cell bottom — the bundled font
/// descends deeper than the ROM font a profile was measured against — the
/// baseline is raised (its row lowered towards the cell top) just enough to let
/// the descenders in, so glyphs like `g`/`y`/`p` are not clipped. When the ink
/// is taller than the whole cell, the unavoidable clip is split by the
/// ascent:descent ratio so neither end is favoured. Derived from the font's own
/// metrics, so this adapts to any embedded font.
fn effective_baseline(cell_height_dots: u32, profile_baseline: u32, font_size: f32) -> u32 {
    let (ascent_ratio, descent_ratio) = ink_extent_ratios();
    let ascent = ascent_ratio * font_size;
    let descent = descent_ratio * font_size;
    let height = cell_height_dots as f32;
    // Any baseline in [ascent, height - descent] clips neither end.
    let lowest_safe = ascent;
    let highest_safe = height - descent;
    let baseline = if lowest_safe <= highest_safe {
        // The ink fits; keep the profile baseline, nudged into the safe band.
        (profile_baseline as f32).clamp(lowest_safe, highest_safe)
    } else {
        // Ink taller than the cell: distribute the clip by the ink ratio.
        height * ascent / (ascent + descent)
    };
    baseline.round().clamp(0.0, height) as u32
}

/// The font's advance width as a fraction of its em, measured once. Advance
/// scales linearly with `font_size`, so this dimensionless ratio is
/// size-independent and cached for the whole process.
fn advance_ratio() -> f32 {
    static RATIO: OnceLock<f32> = OnceLock::new();
    *RATIO.get_or_init(|| {
        // Any glyph works for a monospace font; measure at a large size for
        // precision. `metrics` reads the precomputed advance without
        // rasterizing pixels, so this is a cheap one-time lookup.
        default_font().metrics('M', REFERENCE_EM).advance_width / REFERENCE_EM
    })
}

/// The font's ink ascent and descent, each a fraction of its em, measured once
/// as the extremes over printable ASCII. Cached for the whole process.
fn ink_extent_ratios() -> (f32, f32) {
    static RATIOS: OnceLock<(f32, f32)> = OnceLock::new();
    *RATIOS.get_or_init(|| {
        let font = default_font();
        let mut ascent = 0.0f32;
        let mut descent = 0.0f32;
        for byte in 0x21u8..=0x7e {
            // `metrics` gives each glyph's ink box relative to the baseline
            // without rasterizing: the top sits `ymin + height` above it and the
            // bottom `-ymin` below it.
            let metrics = font.metrics(byte as char, REFERENCE_EM);
            ascent = ascent.max((metrics.ymin + metrics.height as i32) as f32);
            descent = descent.max(-(metrics.ymin as f32));
        }
        (ascent / REFERENCE_EM, descent / REFERENCE_EM)
    })
}

/// Reference em size for one-time metric measurements; large for precision.
const REFERENCE_EM: f32 = 1000.0;

pub(crate) fn default_font() -> &'static Font {
    static DEFAULT_FONT: OnceLock<Font> = OnceLock::new();

    DEFAULT_FONT.get_or_init(|| {
        Font::from_bytes(DEFAULT_FONT_BYTES, FontSettings::default())
            .expect("the bundled Noto Sans Mono font must remain valid")
    })
}
