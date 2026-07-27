# Shared printer qualification receipt

`input.hex` is the one comprehensive ESC/POS stream used to qualify every
printer profile. Keeping the bytes shared makes printer differences visible:
the selected profile is the only rendering input that changes.

Each visible directory under `profiles/` contains the printer-specific
`expected-NNN.png` output and its qualification records. Focused, smaller
conformance cases remain under `tests/cases/` because they are easier to
diagnose when one command or interaction fails.

Do not customize this stream for one printer. Extend it when the shared
qualification receipt needs to exercise another supported command family.
