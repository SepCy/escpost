# Line spacing and line feed

This case draws four 8 × 1-dot horizontal markers with their top edges at
expected printer-dot rows 0, 30, 40, and 60.

1. The first marker is committed by LF using the profile's 30-dot default.
2. `ESC 3 10` selects 10 vertical motion units per line, then LF commits the
   second marker.
3. `ESC d 2` commits the third marker and feeds two current 10-unit lines.
4. `ESC 2` restores the 30-dot profile default before LF commits the fourth
   marker and advances the final paper position to row 90.

The NT-5890K profile specifies 203 vertical motion units per inch at 203 DPI,
so one default vertical motion unit maps to one printer dot. Its calibrated
`ESC *` 8-dot vertical pitch is also one printer dot.

References: Epson `ESC 2 — Select default line spacing`, `ESC 3 — Set line
spacing`, and `ESC d — Print and feed n lines`.
