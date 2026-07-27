# GS v 0 scaling modes

This case resets the printer and prints the same 8 × 8 raster X with each
numeric `GS v 0` mode:

1. mode 0 — normal, expected size 8 × 8 printer dots;
2. mode 1 — double-width, expected size 16 × 8 printer dots;
3. mode 2 — double-height, expected size 8 × 16 printer dots; and
4. mode 3 — quadruple, expected size 16 × 16 printer dots.

`GS v 0` stores raster data row by row. Its horizontal size fields count bytes
while its vertical size fields count dots. Each command advances the print
position vertically by its scaled image height, independently of the current
line spacing, and returns to the beginning of the line.

Epson behavior applies an LF after the raster image has advanced by its own
height. The connected NT-5890K instead consumes exactly one immediately
following LF without another feed. A second consecutive LF feeds normally.
The case uses the NT profile, so its four images are adjacent and the expected
total surface height is 48 dots: `8 + 8 + 16 + 16`.

The Epson baseline is derived from `GS v 0 — Print raster bit image`.
The NT behavior was established with an isolated physical probe containing
zero, one, and two LFs between staggered one-row raster blocks.
