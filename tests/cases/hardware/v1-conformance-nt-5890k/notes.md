# NT-5890K v1 hardware conformance receipt

This is the single consolidated physical comparison for the v1 commands that
the NT-5890K profile advertises. It combines already-isolated conformance
streams rather than introducing different test data for hardware:

1. Font A/B metrics, double size, emphasis, underline, and reverse text.
2. Absolute/relative positioning, character spacing, margins, print area, and
   centering.
3. All four `ESC *` column-image density modes.
4. All four `GS v 0` raster scaling modes.
5. EAN-13 through both `GS k` framings and a Model 2 QR symbol.
6. Full/partial `GS V` Function B forms on a printer without an autocutter.

Each section starts from `ESC @`, but initialization does not retract or erase
paper already rendered. No cut or drawer-pulse command is included. Final
feeds leave enough paper for manual tearing.

The labels aid paper/PNG comparison but their glyph outlines are not expected
to match because the renderer deliberately uses a representative bundled
font. Compare:

- line origins, wrapping, cell sizes, and vertical advancement;
- visible marker coordinates and centered print-area cell;
- the four column-graphics and four raster-graphics scales;
- barcode width, height, HRI placement, and centering;
- QR dimensions and placement; and
- the equal Function B marker gaps.

The exact PNG is served at <http://localhost:8765/tools/preview/> with integer
nearest-neighbor zoom.

Physical result is recorded here only after the exact hash-verified stream has
been sent and compared.
