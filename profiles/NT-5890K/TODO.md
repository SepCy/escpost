# NT-5890K calibration TODO

These checks need the reference printer and are intentionally deferred until
it is available again:

- Print the shared calibration receipt once more and compare the complete
  receipt against `expected-001.png` in a single review.
- Verify `CR` behavior explicitly instead of relying on the current
  profile/default evidence.
- Verify every advertised one-dimensional barcode system; physical tests so
  far cover EAN-13 through both `GS k` command forms.
- Verify representative imported code-page slots and international character
  sets against the printer's resident tables.
- Test the cash-drawer pulse command only with compatible drawer hardware.
- Calibrate additional post-v1 command families as they are implemented and
  advertised by this profile.

Accepted fidelity boundaries, such as representative font glyphs and
firmware-specific symbol dimensions, belong in `profile.toml` and `notes.md`,
not in this TODO list.
