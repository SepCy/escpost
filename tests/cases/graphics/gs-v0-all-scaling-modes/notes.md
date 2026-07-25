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

An LF after each of the first three images adds the profile's 30-dot line feed,
making the specimens visually distinct. The expected total surface height is
138 dots: `(8 + 30) + (8 + 30) + (16 + 30) + 16`.

The behavior is derived from Epson's `GS v 0 — Print raster bit image`
documentation. Physical observations for the NT-5890K remain to be recorded.
