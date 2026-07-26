# GS V Function B feed without an autocutter

This case draws three solid 32 × 8-dot raster markers. Their top edges should be
at printer-dot rows 0, 50, and 100.

1. `GS v 0` prints the first marker and advances eight dots; `ESC J 22`
   completes the 30-dot baseline advance.
2. `GS P 203 101` makes each vertical motion unit approximately two dots.
3. `GS V 65 10` requests a full cut after ten units. The NT-5890K has no
   autocutter, so Epson specifies a 20-dot feed without a cut.
4. The second marker prints at row 50; `ESC J 11` advances its remaining
   22-dot baseline distance under the selected motion units.
5. `GS V 66 10` exercises the equivalent partial-cut form. It feeds another
   20 dots without a cut, placing the third marker at row 100.
6. Another `ESC J 11` completes the final baseline before `ESC d 3` provides
   blank paper for physical review.

The solid raster markers avoid printer-font shapes and the connected
NT-5890K's trailing `ESC *` line artifact. A physical run should produce one
uncut receipt with two equal extra gaps. Measure marker-to-marker distances
rather than the trailing manual-tear margin.

References: Epson `GS V — Select cut mode and cut paper`, Function B; `GS P —
Set horizontal and vertical motion units`.

## Physical run

```text
date: pending
input SHA-256: c02c4b6860f475b94a094dbc2da6d6f329d0b0b5425017f85e1d5e174c0965f8
printer profile: NT-5890K
printer USB identity: 0416:5011
serial: B120300001
connection: USB interface 0, OUT endpoint 0x01
transport result: pending for the revised raster-marker stream
visual comparison: pending
```

The expected paper result is one uncut strip with three markers and equal
marker-to-marker gaps. The USB result alone cannot establish that geometry.
