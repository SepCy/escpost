# ESC * density modes

This case resets the printer and draws one X with each Epson-defined `ESC *`
mode, in ascending mode order:

1. mode 0 — 8-dot single-density;
2. mode 1 — 8-dot double-density;
3. mode 32 — 24-dot single-density; and
4. mode 33 — 24-dot double-density.

The 8-dot specimens have eight source columns. The 24-dot specimens have 24
source columns so that every source specimen starts as a square X.

On a 203 DPI Epson-compatible printer, single-density modes use one source
column per two printer dots and double-density modes use one source column per
printer dot. The 8-dot modes use one source row per three printer dots; the
24-dot modes use one source row per printer dot. Consequently, all four
specimens should be 24 printer dots tall, while their expected widths are 16,
8, 48, and 24 printer dots respectively.

Each LF commits one specimen at the profile's 30-dot default line spacing.
The output is therefore four rows high with six blank printer-dot rows between
the 24-dot specimens.

The geometry is derived from Epson's `ESC * — Select bit-image mode`
documentation. Physical observations for the NT-5890K remain to be recorded.
