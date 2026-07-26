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

## Physical run

```text
date: 2026-07-26
input SHA-256: 3910feea2720c9269ad0d1173cdeea7c5328fefb22c9556e1ec2866e6dc1175b
printer profile: NT-5890K
printer USB identity: 0416:5011
serial: B120300001
connection: USB interface 0, OUT endpoint 0x01
transport result: all 59 hash-verified bytes sent without a USB error
visual comparison: pending
```

The expected paper result is one uncut strip with three markers and equal
marker-to-marker gaps. The USB result alone cannot establish that geometry.
