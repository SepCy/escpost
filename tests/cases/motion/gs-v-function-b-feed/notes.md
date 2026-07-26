# GS V Function B feed without an autocutter

This case sends three 128 × 8-dot raster images. Each contains one solid
32 × 8-dot marker at a different horizontal inset: 0, 48, or 96 dots. On the
calibrated NT-5890K profile their top edges are at rows 0, 28, and 36.

1. `GS v 0` prints the first marker and advances eight dots.
2. The connected firmware consumes `ESC J 22` without feeding.
3. `GS P 203 101` makes each vertical motion unit approximately two dots.
4. `GS V 65 10` requests a full cut after ten units. The NT-5890K has no
   autocutter; its profile records the observed 20-dot feed without a cut.
5. The second marker therefore prints at row 28.
6. This firmware ignores both `ESC J 11` and partial-cut form `GS V 66 10`.
   The right marker starts immediately below the center marker at row 36.
7. The final `ESC J 11` is also ignored before `ESC d 3` supplies the manual
   tear margin.

The horizontal staggering keeps all three command boundaries visible even
when two markers have no vertical gap. Raster graphics also avoid
printer-font shapes and the connected NT-5890K's trailing `ESC *` line
artifact. The expected physical result is one uncut receipt with three
distinct blocks; the center and right blocks have vertically adjacent rows.

References: Epson `GS V — Select cut mode and cut paper`, Function B; `GS P —
Set horizontal and vertical motion units`.

## Physical run

```text
date: pending
input SHA-256: bde163feaf4685fd4d01cceccbf26c1beb57e3cb8683e1bb5c1f95c0fd0e07e8
printer profile: NT-5890K
printer USB identity: 0416:5011
serial: B120300001
connection: USB interface 0, OUT endpoint 0x01
transport result: pending for the revised raster-marker stream
visual comparison: pending
```

The behavior was reproduced in both the consolidated receipt and a separate
402-byte motion-unit probe: `ESC J` was ignored, mode 65 fed, and mode 66 was
ignored.
