# ESCPost

**The ESC/POS Tools and Workbench**

`escpost` is a standalone ESC/POS developer toolkit built around a
dot-accurate Rust renderer. It currently renders ESC/POS byte streams as PNG
receipt previews, provides hardware calibration tools, and exposes the engine
to applications through a thin Python package.

The project aims to emulate printer layout on a dot-addressed surface. Its
fidelity target is the placement and sizing of printed elements—not a
photographic simulation of thermal paper or an exact copy of every printer's
resident font glyphs.

## Status

The renderer currently supports profile-driven Font A/B text with common OEM
and Windows single-byte code pages, wrapping, character sizing up to 8×,
Epson international character sets 0–17, profile-selected carriage-return
behavior, emphasis, underline, reverse printing, justification, line spacing
and feeds, all four `ESC *` bit-image modes, all `GS v 0` raster-image scaling
modes, absolute and relative horizontal positioning, motion units, margins,
and print-area widths. It also renders the common native one-dimensional
barcode systems, GS1-128, automatic Code 128, and Model 2 QR symbols, including
their persistent size, HRI, error-correction, alignment, print-area, and reset
state. Barcode availability is gated per logical system by the selected
printer profile. Supported `GS V` cuts create separate output sheets. It emits
one-bit grayscale PNG.

The bundled representative font currently covers Latin, Greek, and Cyrillic.
A decoded character outside that asset returns an error rather than a
misleading replacement glyph.

The Rust `escpost render` command accepts raw bytes, readable hexadecimal
input, stdin, or a conformance-case directory. It can write PNGs, stream one
PNG to stdout, or host an embedded browser workbench. The Rust `print` command
sends the same source types unchanged to an explicitly addressed USB printer.
The Python binding remains available to applications, while the existing
Python hardware commands continue to provide USB discovery and higher-level
physical calibration during their migration to Rust.

The virtual `REFERENCE` profile enables every capability currently represented
by the renderer without inheriting limitations or quirks from a physical
printer. It does not imply that post-v1 command families are already
implemented.

Use `REFERENCE` for generic previews and tests when no target printer is known:

```python
from escpost import render

sheets = render(escpos_bytes, profile="REFERENCE")
```

When the target printer is known, select its physical profile instead so the
preview includes that device's geometry, capabilities, and calibrated quirks.

The Rust result includes bounded rendering, device events, profile
approximations, and reproducible renderer/profile identity. Canonical profile
JSON verifies its content hash when loaded.

This is not yet a general-purpose ESC/POS renderer. Unsupported data and
commands return errors while command coverage grows one conformance case at a
time.

## Development

All build and test commands run in the project container:

```bash
docker compose build
./escpost render --help
docker compose run --rm test cargo test --workspace
docker compose run --rm test .venv/bin/python -m unittest discover -s python/tests
```

Regenerate the canonical runtime profile pack after changing an enrichment or
the pinned upstream source:

```bash
docker compose run --rm test cargo run --quiet \
  -p escpost-profiles --bin compile-profile-pack -- \
  profiles/.escpos-printer-db/dist/capabilities.json \
  profiles profiles/.generated/profiles.json
```

`./escpost` is the stable development entry point. It runs `render` and
`print` through the Rust CLI and keeps the existing Python discovery and
calibration commands reachable during migration. The CLI service has USB
access for physical workflows; its Python environment lives in a named Docker
volume and is created or updated only when a legacy command needs it.

List connected USB printer-class devices:

```bash
./escpost printers discover
```

Save one selected device to the ignored local configuration:

```bash
./escpost printers discover \
  --serial B120300001 \
  --name netum-usb \
  --profile NT-5890K
```

The Compose service joins host group GID `7`, the conventional `lp` group on
Debian-derived systems. Set `USB_GROUP_ID` when the USB printer device belongs
to a different host group.

Send a raw or hexadecimal ESC/POS stream unchanged to a USB printer:

```bash
./escpost print receipt.hex \
  --usb-vendor-id 0x0416 \
  --usb-product-id 0x5011 \
  --usb-interface 0 \
  --usb-out-endpoint 0x01 \
  --non-interactive
```

All four USB values are required. `print` does not read a printer alias,
infer values from a profile, or discover an interface or endpoint. Use
`printers discover` to inspect a connected device, then pass the selected
values explicitly. The invocation itself authorizes the physical write.

Render a raw byte stream to one PNG:

```bash
./escpost render receipt.bin \
  --profile REFERENCE \
  --output receipt.png \
  --non-interactive
```

Render every sheet of a conformance case. Case metadata supplies its profile:

```bash
./escpost render \
  tests/cases/mechanism/reference-full-and-partial-cuts \
  --output-dir local/rendered \
  --non-interactive
```

The directory contains `sheet-001.png`, `sheet-002.png`, and so on, plus a
`manifest.json` that lists the current sheets in order. Existing generated
files are overwritten; unlisted stale or unrelated files are preserved.

Use `-o -` for a byte-clean single-PNG pipeline:

```bash
generate-receipt |
  ./escpost render - --format binary --profile REFERENCE \
    --output - --non-interactive > receipt.png
```

Start the embedded web workbench to inspect all sheets at 1× printer-dot
scale. The command selects the first free loopback port from 9000 through
9099, prints the URL, and remains active until Ctrl+C:

```bash
./escpost render \
  tests/cases/mechanism/reference-full-and-partial-cuts \
  --web \
  --non-interactive
```

Add `--watch` to rerender a file or case when its input changes. Add
`--browser` when running a host-native binary to open the URL automatically.
The Docker wrapper cannot open a browser on the host, so use `--web` there and
open the printed URL yourself.

For focused physical calibration, first use discovery to populate
`local/printers.toml`. The `case calibrate` command renders and sends one
loaded byte buffer.

```bash
./escpost case calibrate \
  tests/cases/graphics/esc-star-8dot-double-density \
  --printer netum-usb \
  --output-dir local/calibration
```

To calibrate a printer profile comprehensively, render and print the single
shared receipt. The configured printer supplies the profile, so developers do
not have to repeat it:

```bash
./escpost calibration calibrate \
  --printer netum-usb \
  --output-dir local/calibration
```

The shared stream lives at `calibration/input.hex`. Its profile-specific
expected PNG, verification record, notes, and any remaining hardware TODOs
live together under `profiles/<profile-id>/`.

## Fidelity contract

The renderer should reproduce, as closely as the selected printer profile
allows:

- printable geometry and printer-dot coordinates;
- command-driven margins, print areas, positioning, alignment, and line feeds;
- character-cell metrics, wrapping, scaling, and baseline placement;
- raster image, barcode, and two-dimensional-code dimensions and placement;
- implemented Standard-mode buffering semantics;
- paper feeds, cuts, and multiple sheets in one job; and
- model-specific command availability, defaults, and known quirks.

The renderer does not initially promise:

- glyph shapes identical to proprietary printer ROM fonts;
- physical effects such as paper texture, thermal energy variation, print-head
  wear, ribbon behavior, or mechanical tolerances; or
- silent best guesses for commands whose framing or behavior cannot be
  determined safely.

## Documentation

- [Architecture](ARCHITECTURE.md) describes the current coherent system design.
- [Coding style](CODING_STYLE.md) defines how code, comments, and tests should
  explain non-obvious behavior in plain language.
- [CLI contract](CLI.md) documents command behavior, inputs, output modes,
  interactive rules, and requirements for the Rust developer tool.
- [Command coverage](COMMAND_COVERAGE.md) defines the version 1 boundary and
  tracks implementation, automated coverage, and physical validation.
- [Developer-tool roadmap](TODO.md) tracks the planned virtual printer,
  inspector, proxy, Rust CLI, and web workbench.
- [Platform support](PLATFORMS.md) tracks release targets, transport backends,
  operating-system caveats, and verified compatibility.
- [Design decisions](DESIGN_DECISIONS.md) records why durable choices were
  made, their consequences, and which questions remain open.
- [Printer profile enrichments](PROFILE_SCHEMA.md) defines how upstream
  profiles are confirmed, completed, and corrected.
- [Testing](TESTING.md) defines automated conformance testing and comparison
  against physical printers.

The official
[Epson ESC/POS command reference](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/)
is the initial normative protocol reference. Model profiles remain necessary
because command support and defaults differ between printers.

## License

Project code and documentation are licensed under the
[Apache License 2.0](LICENSE). Bundled third-party assets such as fonts must
retain their own compatible licenses and attribution. Imported printer data
from `receipt-print-hq/escpos-printer-db` remains under CC BY 4.0 and must be
distributed with its attribution and license.

ESC/POS is a registered trademark of Seiko Epson Corporation. ESCPost is an
independent open-source project and is not affiliated with or endorsed by
Epson.

## Relationship to Receiptful

The project is developed as an independent Git repository inside the
Receiptful working tree. Receiptful ignores the directory until it is ready to
be included as a Git submodule.

The library must not depend on Receiptful application modules. Receiptful
integration happens through the Python package, serialized profiles, and
public APIs.
