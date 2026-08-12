# Shared printer calibration receipt

`input.hex` is the one comprehensive ESC/POS stream used to calibrate every
physical printer profile. Keeping the bytes shared makes printer differences
visible: the selected profile is the only rendering input that changes.

Each physical profile directory contains the printer-specific
`expected-NNN.png` output and its calibration records. Virtual profiles use
focused golden cases without claiming a paper comparison. Smaller conformance
cases remain under `crates/escpost-render/tests/cases/` because they are easier to diagnose when one
command or interaction fails.

Do not customize this stream for one printer. Extend it when the shared
calibration receipt needs to exercise another supported command family.
