# ASCII fonts and styles

This automated conformance case exercises the initial deterministic text
renderer:

- profile-defined Font A at 12 × 24 dots;
- profile-defined Font B at 9 × 17 dots;
- centered double-width/double-height text with `ESC !`;
- emphasis with `ESC E`;
- one-dot underline with `ESC -`; and
- white/black reverse with `GS B`.

Noto Sans Mono 2.006 supplies representative glyph shapes. The source font's
advance width is ignored: every glyph is thresholded into a one-bit bitmap,
clipped to the selected profile cell, and advanced by that cell's ESC/POS
geometry.

The expected surface height is 198 dots. Five ordinary lines advance by 30
dots each, while the double-height Font A line advances by its 48-dot
character height.

This case is not sent to the printer during incremental development. It will
be incorporated into the consolidated v1 hardware-conformance receipt.
