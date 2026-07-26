# ESC * density modes

This case resets the printer and draws one X with each Epson-defined `ESC *`
mode, in ascending mode order:

1. mode 0 — 8-dot single-density;
2. mode 1 — 8-dot double-density;
3. mode 32 — 24-dot single-density; and
4. mode 33 — 24-dot double-density.

The 8-dot specimens have eight source columns. The 24-dot specimens have 24
source columns so that every source specimen starts as a square X.

On a 203 DPI Epson printer, single-density modes use one source column per two
printer dots and double-density modes use one source column per printer dot.
Epson places 8-dot-mode rows on a three-printer-dot vertical pitch.

The connected NT-5890K differs in one material way: modes 0 and 1 paint their
eight source rows adjacently. Its typed profile therefore uses a one-dot pitch.
For this profile the expected specimen dimensions are:

1. mode 0 — 16 × 8 dots;
2. mode 1 — 8 × 8 dots;
3. mode 32 — 48 × 24 dots; and
4. mode 33 — 24 × 24 dots.

Each LF commits one specimen at the profile's 30-dot default line spacing.
The line origins remain 30 dots apart independently of the visible image
height.

The geometry is derived from Epson's `ESC * — Select bit-image mode`
documentation and the connected-printer density probe.

## Physical observation

On 2026-07-26, USB printer `0416:5011` with serial `B120300001` printed:

- mode 0 twice as wide as mode 1;
- eight adjacent rows for the 8-dot modes;
- 24 adjacent rows for mode 33; and
- an additional faint line below each 8-dot image.

The first three observations determine the modeled geometry. The final line is
an incidental firmware artifact and is deliberately absent from the PNG.
