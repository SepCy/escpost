# Testing escpos2png

escpos2png uses deterministic automated tests and opt-in physical-printer
calibration. Both paths consume the same version-controlled ESC/POS byte
streams.

The automated suite protects behavior on every development machine and in CI.
Physical printing establishes and checks model-specific behavior that cannot
be proven from documentation alone.

## Principles

1. Test observable behavior through the public rendering interface.
2. Use one immutable ESC/POS stream for both rendering and physical printing.
3. Compare decoded pixels or logical dot surfaces, never compressed PNG bytes.
4. Keep physical-printer tests explicit; ordinary test commands must never
   print paper.
5. Treat hardware observations as evidence for a selected profile, not as
   universal ESC/POS behavior.
6. Never accept a changed golden image only because the implementation
   produced it.

## Test layers

### Public behavior tests

Most regression tests call the public Rust rendering API with:

- ESC/POS input bytes;
- a resolved printer profile;
- an explicit initial-state assumption; and
- explicit resource limits and render options where relevant.

They assert observable results:

- sheet count and dimensions;
- decoded output pixels or logical dots;
- feeds and cuts;
- device events;
- diagnostics and completeness; and
- reproducibility metadata.

These tests should survive refactoring of tokenizers, command handlers,
buffers, and surface storage.

Focused parser or state tests are appropriate for framing and transition
invariants, but they supplement rather than replace public behavior tests.

### Python binding tests

Binding tests prove that Python callers receive the same behavior as Rust
callers. They cover:

- byte input and profile selection;
- PNG and diagnostic results;
- Rust error conversion to documented Python exceptions; and
- repeated or concurrent calls.

They should not duplicate the complete Rust conformance suite.

### Robustness tests

Malformed, truncated, adversarial, and resource-intensive streams verify that
the renderer returns controlled errors or incomplete results instead of
panicking, hanging, or allocating without bounds.

Fuzzing targets command framing and state-machine execution. A discovered
failure becomes a permanent minimal regression case before the implementation
is fixed.

### Physical-printer calibration

Hardware calibration sends a case's exact input bytes to a selected printer
and renders those same bytes with the matching profile.

The first physical reference profile is `NT-5890K`, a Netum 58 mm printer.
The upstream profile inherits from `POS-5890`, currently describing:

- a 384-dot printable width;
- 203 DPI;
- 32 columns for Font A; and
- 42 columns for Font B.

These values are starting hypotheses. The connected printer and its
documentation determine whether escpos2png needs profile enrichments or
corrections.

## Conformance case format

Each behavior is represented by a self-contained case directory:

```text
tests/cases/text/default-font/
├── case.toml
├── input.escpos
├── expected-001.png
└── notes.md
```

`input.escpos` is the canonical byte stream. The renderer and physical-printer
transport both read that exact file. Neither path may regenerate, normalize,
prefix, suffix, or otherwise transform it.

`case.toml` records machine-readable expectations and provenance:

```toml
schema_version = 1
name = "default Font A advances by 12 dots"
profile = "NT-5890K"
input = "input.escpos"
input_sha256 = "<sha256>"
expected_sheets = ["expected-001.png"]
expected_completeness = "complete"

[initial_state]
assumption = "profile-reset-defaults"

[[references]]
source = "printer-manual"
location = "character font section"
```

The exact manifest schema remains versioned and may grow as implementation
needs become concrete.

`expected-001.png` is a lossless, reviewable representation of expected dots.
Tests decode it and compare its pixel values and dimensions. PNG encoder output
bytes are not asserted because compression settings can change without
changing the receipt.

`notes.md` explains the behavior under test, relevant commands, manual
references, intentional approximations, and physical observations. It should
not be required for executing the test.

Cases with multiple cuts contain one expected PNG per sheet.

## Python calibration CLI

A small Python CLI orchestrates rendering and physical printing. The intended
interface is:

```text
escpos2png case render <case>
escpos2png case print <case> --printer <local-name>
escpos2png case calibrate <case> --printer <local-name>
```

`render` invokes the Rust engine through the Python binding and writes an
actual PNG plus diagnostics.

`print` sends `input.escpos` unchanged to the selected physical transport.

`calibrate` loads the stream once, reports its SHA-256 digest, renders it, and
sends the same in-memory bytes to the printer. This makes accidental divergence
between the two paths detectable.

The printing adapter may use python-escpos's USB transport, but it uses only
the raw-byte operation. It must not call high-level helpers such as `text`,
`image`, `feed`, or `cut`, because those helpers generate additional ESC/POS
bytes.

python-escpos and its USB dependencies are development or optional hardware
dependencies. They are not dependencies of the Rust core or ordinary renderer
installations.

## Local printer configuration

Connection details belong in an ignored `local/printers.toml`:

```toml
[netum-usb]
transport = "usb"
profile = "NT-5890K"
vendor_id = "<USB vendor ID>"
product_id = "<USB product ID>"
out_endpoint = "0x01"
```

A committed example configuration may document supported fields, but real
machine configuration and local captures remain ignored.

Before sending bytes, the CLI shows:

- the selected case and input hash;
- printer profile;
- USB identity or other transport destination;
- byte count; and
- whether the stream contains a cut command.

The explicit `case print` or `case calibrate` command is the authorization to
perform the physical action. Automated tests and build scripts never invoke
these commands.

The CLI adds no implicit initialization, feed, or cut commands. A case that
requires `ESC @`, trailing feed, or a cut includes those bytes explicitly in
`input.escpos`.

## Calibration workflow

For each new visible behavior:

1. Add one conformance case that describes the public behavior.
2. Run it and observe the expected automated test failure.
3. Implement only enough behavior to make that case pass.
4. Run the complete automated suite.
5. If the behavior is model-sensitive, run the same case through
   `case calibrate` on the Netum printer.
6. Compare physical geometry with the rendered PNG.
7. Record the observation in `notes.md` and update the profile enrichment when
   the behavior is model-specific.
8. Refactor only while all automated tests remain green.

This is repeated one vertical slice at a time. Do not write a large suite of
speculative command tests before exercising the first command end to end.

## Comparing PNG and paper

Initial calibration may be visual, but test receipts should make discrepancies
easy to identify. Useful fixtures include:

- horizontal and vertical dot rulers;
- boundary marks at the printable area's edges;
- repeated characters that reveal cell width and wrapping;
- baseline and line-spacing patterns;
- aligned raster blocks with known dimensions; and
- labels containing the case name and short input hash.

Display PNGs only at integer nearest-neighbor scales so individual logical
dots remain visible.

For a more objective comparison, scan the receipt flat at a known resolution,
then deskew, crop to registration marks, resample to the printer's nominal dot
grid, and threshold it. Physical output includes feed tolerances, thermal
spread, and scanning distortion, so the digitized receipt is evaluated with
documented tolerances rather than required to equal the logical raster bit for
bit.

The unprocessed scan or photograph is evidence, not an automated golden image.
If reference captures are retained, store their printer identity, firmware or
self-test information, configuration, capture method, date, and case hash.

## What hardware observations may change

Hardware evidence can justify:

- correcting profile geometry or defaults;
- documenting a firmware or compatibility-mode variant;
- adding a model-specific command quirk;
- changing a profile's completeness level; or
- filing an upstream printer-database correction.

It does not justify changing Epson command framing or another model's behavior
without corresponding documentation or evidence.

If documentation and hardware disagree, retain both references and describe
the observed printer configuration. A new profile or firmware variant is often
safer than silently changing behavior for every device with the same marketing
name.

## Golden-image review

Golden images are updated deliberately:

1. Explain which documented behavior or physical evidence changed.
2. Render the affected case to a separate actual-output path.
3. Review dimensions, pixel differences, diagnostics, and unrelated regions.
4. Replace the golden only after the new result is accepted.
5. Commit the input, manifest, expected image, and notes together.

A bulk "regenerate all goldens" command must not overwrite expected files
without an explicit acceptance step.

## When hardware testing is required

Run applicable physical cases before accepting changes to:

- printer-profile geometry or defaults;
- text cell metrics, baselines, spacing, or wrapping;
- motion-unit conversion and rounding;
- raster, barcode, or two-dimensional-code placement;
- Standard or Page mode composition;
- feed, cutter, or sheet-boundary behavior; and
- model-specific commands or quirks.

Parser refactors, diagnostics-only changes, packaging, and equivalent PNG
compression changes normally require the automated suite but not new paper,
provided their existing conformance cases remain unchanged.

Contributors without the target hardware may still submit changes and fixtures.
They mark hardware validation as pending so a maintainer with the reference
printer can perform it.

## Reporting a physical run

A hardware-validation report includes:

```text
case:
input SHA-256:
renderer commit:
profile revision and hash:
printer model:
firmware/configuration:
connection:
result:
observations:
capture, if any:
```

This is sufficient to reproduce the comparison and prevents an unexplained
"looks correct on my printer" from becoming profile behavior.
