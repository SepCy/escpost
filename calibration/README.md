# Shared printer calibration receipt

`input.hex` is the one comprehensive ESC/POS stream used to calibrate every
printer profile. Keeping the bytes shared makes printer differences visible:
the selected profile is the only rendering input that changes.

Each visible directory under `profiles/` contains the printer-specific
`expected-NNN.png` output and its calibration records. Focused, smaller
conformance cases remain under `tests/cases/` because they are easier to
diagnose when one command or interaction fails.

Do not customize this stream for one printer. Extend it when the shared
calibration receipt needs to exercise another supported command family.
