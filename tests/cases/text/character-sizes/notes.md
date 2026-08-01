# Character sizes

This automated conformance case exercises glyph rasterization across the full
`GS !` character-size range for both profile fonts, which no other test covers:
the size mechanics test (`render_text.rs`) checks one magnification on a
reversed space, not glyph shapes at size.

For Font A (12 × 24 dots) and Font B (9 × 17 dots) it renders the sample `Ag`
— a cap plus a descender, so both the ascender extent and baseline scale are
visible — at:

- square magnifications 1×, 2×, 3×, 4×, 6×, and 8×; and
- anisotropic magnifications 2×1, 1×2, 3×2, and 2×3.

Each line is prefixed with its `WxH` multiplier at normal size, then the sample
at that size, selected with `GS !` and reset afterwards. Each font's header
prints in that font (`ESC M` selects Font B), so Font B is naturally smaller —
it is a different glyph box, not a scaled Font A.

A full cut (`GS V`) between the two fonts splits the output into two sheets:
Font A on sheet 001, Font B on sheet 002.

Magnified glyphs are rasterized at their true magnified size rather than by
block-doubling the base cell, so large text stays crisp and anisotropic sizes
are condensed correctly (double-width stretches, double-height narrows). This
matches the smooth enlarged text real printers produce; a printer reaches it
through dot magnification plus firmware edge smoothing, which is not modeled
dot-for-dot here.

This case is not sent to the printer during incremental development. It will be
incorporated into the consolidated v1 hardware-conformance receipt.
