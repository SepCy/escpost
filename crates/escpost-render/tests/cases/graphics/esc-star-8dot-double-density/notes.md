# ESC * 8-dot double-density X

This case resets the printer, draws an eight-column X with `ESC *` mode 1,
then commits the line with LF.

The Epson command definition makes `nL + nH × 256` the number of horizontal
dot columns and consumes one data byte per column in 8-dot modes. Bits run
from the most-significant bit at the top to the least-significant bit at the
bottom.

For an Epson 203 DPI device, mode 1 uses 203 DPI horizontally and 203/3 DPI
vertically. Epson therefore places adjacent source rows on a three-dot pitch.

The connected NT-5890K paints the eight source rows adjacently instead. Its
typed profile records a one-dot vertical pitch, so this expected PNG contains
an 8 × 8-dot X while LF still advances by the profile's 30-dot line spacing.
The printer also emits a faint line below the image; that incidental firmware
artifact is documented but intentionally not rendered.
