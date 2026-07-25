# ESC * 8-dot double-density X

This case resets the printer, draws an eight-column X with `ESC *` mode 1,
then commits the line with LF.

The Epson command definition makes `nL + nH × 256` the number of horizontal
dot columns and consumes one data byte per column in 8-dot modes. Bits run
from the most-significant bit at the top to the least-significant bit at the
bottom.

For a 203 DPI Epson-compatible device, mode 1 uses 203 DPI horizontally and
203/3 DPI vertically. The expected logical surface therefore contains one
printer dot per source column and three printer-dot rows per source bit. The
eight source rows occupy 24 dots; LF advances to the NT-5890K profile's
30-dot reset line spacing.

The density behavior and 30-dot reset spacing are documentation-based
hypotheses until they are calibrated on the physical NT-5890K.
