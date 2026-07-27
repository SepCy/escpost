# ESC/POS interaction coverage

ESC/POS commands share printer state. This inventory records the interactions
that need explicit tests so command support does not look complete while a
combined print job still renders incorrectly.

The Epson ESC/POS reference determines which interactions belong here. This is
a focused checklist, not a request to test every possible command ordering.

## Status

- **Covered** means an automated public-rendering test protects the behavior.
- **Partial** means some affected output types or state transitions are tested.
- **Planned** means the required commands or interaction test are not complete.

## Interaction matrix

| Governing state or event | Affected behavior | Required coverage | Status |
|---|---|---|---|
| `ESC a` justification | Text and `ESC *` graphics | Left, center, and right placement inside the active print area | Covered |
| `ESC a` justification | `GS v 0` raster graphics | Left, center, and right placement inside the active print area | Covered |
| `ESC a` justification | `GS ( L` buffered graphics | Left, center, and right placement inside the active print area | Covered |
| `ESC a` justification | Barcodes and two-dimensional symbols | Left, center, and right placement inside the active print area | Covered |
| `GS L` and `GS W` print area | Text and `ESC *` graphics | Origin, width, justification, clipping, and oversized data | Covered |
| `GS L` and `GS W` print area | `GS v 0` raster graphics | Origin, width, justification, clipping, and oversized data | Covered |
| `GS L` and `GS W` print area | `GS ( L` buffered graphics | Origin, width, justification, clipping, and oversized data | Covered |
| `GS L` and `GS W` print area | Barcodes and two-dimensional symbols | Origin, width, justification, clipping, and oversized data | Covered |
| `ESC !`, `ESC M`, and `GS !` text metrics | Tabs, wrapping, spacing, and absolute or relative positioning | Cursor movement uses the active character cell and size at the documented time | Covered |
| `ESC 2` and `ESC 3` line spacing | Text, `ESC *`, and raster graphics | Character-height clearance, explicit `ESC *` row advance, and commands that feed independently | Covered |
| Profile `CR` mode | Line buffering and line spacing | Ignored CR and auto-line-feed CR behavior | Covered |
| `ESC R` and `ESC t` character state | Printable ASCII substitutions and code-page decoding | Substitution precedes glyph lookup; `ESC @` restores both profile defaults | Covered |
| Beginning-of-line state | Justification, print-area, raster, and cut commands | Commands are accepted, ignored, or treated as data exactly as documented | Covered |
| `ESC @` initialization | Persistent modes, motion units, tabs, print area, and pending data | Defaults are restored, pending data is cleared, and already-fed paper remains | Covered |
| Text print modes | Raster and buffered graphics, barcodes, and two-dimensional symbols | Modes that the specification excludes do not alter graphics or symbols | Covered |
| Cuts | Buffered data, paper position, and sheet boundaries | Documented beginning-of-line behavior, Function B feed geometry, and full versus partial sheet results | Covered |

## Test-file ownership

- `render_justification.rs` owns behavior governed by `ESC a`.
- `render_print_area.rs` owns behavior governed by `GS L` and `GS W`.
- `render_initialization.rs` owns state reset and pending-buffer behavior.
- `render_international.rs` owns `ESC R` framing, substitution, and reset
  behavior with `ESC t`.
- `render_line_spacing.rs` owns profile-selected `CR` behavior in addition to
  explicit feed commands.
- `render_cut.rs` owns Function A/B feed geometry, capability gating, and sheet
  boundaries for full and partial cuts.
- `render_buffering.rs` owns beginning-of-line and command-recognition rules.
- Command-specific files continue to own framing, valid operands, scaling, and
  malformed-input behavior for that command.

Conformance cases under `tests/cases/` may combine several interactions into a
receipt suitable for physical comparison. Small dot-level interaction tests do
not each need a separate PNG fixture.
