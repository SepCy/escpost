# GS V Function B feed without an autocutter

This case draws three 8 × 3-dot horizontal markers. Their top edges should be
at printer-dot rows 0, 50, and 100.

1. The first marker is committed by LF and advances by the 30-dot default line
   spacing.
2. `GS P 203 101` makes each vertical motion unit approximately two dots.
3. `GS V 65 10` requests a full cut after ten units. The NT-5890K has no
   autocutter, so Epson specifies a 20-dot feed without a cut.
4. The second marker is committed at row 50 and advances by 30 dots.
5. `GS V 66 10` exercises the equivalent partial-cut form. It feeds another
   20 dots without a cut, placing the third marker at row 100.
6. `ESC d 3` provides blank paper after the final marker for physical review.

The one-bit markers avoid making printer-font shapes part of this geometry
test. A physical run should produce one uncut receipt with two equal extra
gaps. Measure marker-to-marker distances rather than the trailing manual-tear
margin.

References: Epson `GS V — Select cut mode and cut paper`, Function B; `GS P —
Set horizontal and vertical motion units`.
