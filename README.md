# escpos2png

`escpos2png` is a planned standalone Rust library for rendering ESC/POS byte
streams as PNG receipt previews. A thin Python package exposes the Rust engine
to applications such as FastAPI services.

The project aims to emulate printer layout on a dot-addressed surface. Its
fidelity target is the placement and sizing of printed elements—not a
photographic simulation of thermal paper or an exact copy of every printer's
resident font glyphs.

## Status

The first vertical slice is implemented. It compiles the enriched `NT-5890K`
profile, interprets `ESC @`, `ESC *` mode 1, and LF through a Standard-mode
line buffer, and emits one-bit grayscale PNG. The Python binding and
conformance-case CLI can render, raw-print, or calibrate the same verified byte
stream.

This is not yet a general-purpose ESC/POS renderer. Unsupported data and
commands return errors while command coverage grows one conformance case at a
time.

## Development

All build and test commands run in the project container:

```bash
docker compose build test
docker compose run --rm test cargo test --workspace
docker compose run --rm test python3 -m venv .venv
docker compose run --rm -e VIRTUAL_ENV=/workspace/.venv test maturin develop
docker compose run --rm test .venv/bin/python -m unittest discover -s python/tests
```

Render the first conformance case:

```bash
docker compose run --rm test \
  .venv/bin/escpos2png case render \
  tests/cases/graphics/esc-star-8dot-double-density \
  --output-dir local/rendered
```

For physical calibration, copy `examples/printers.toml` to the ignored
`local/printers.toml`, replace the USB identifiers, install the optional
printer dependency with `maturin develop --extras printer`, and expose the USB
device to the container. The `case calibrate` command renders and sends one
loaded, hash-verified byte buffer.

```bash
docker compose run --rm -e VIRTUAL_ENV=/workspace/.venv \
  test maturin develop --extras printer

docker compose run --rm --device /dev/bus/usb:/dev/bus/usb test \
  .venv/bin/escpos2png case calibrate \
  tests/cases/graphics/esc-star-8dot-double-density \
  --printer netum-usb \
  --output-dir local/calibration
```

## Fidelity contract

The renderer should reproduce, as closely as the selected printer profile
allows:

- printable geometry and printer-dot coordinates;
- command-driven margins, print areas, positioning, alignment, and line feeds;
- character-cell metrics, wrapping, scaling, and baseline placement;
- raster image, barcode, and two-dimensional-code dimensions and placement;
- Standard mode and Page mode buffering semantics;
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

## Relationship to Receiptful

The project is developed as an independent Git repository inside the
Receiptful working tree. Receiptful ignores the directory until it is ready to
be included as a Git submodule.

The library must not depend on Receiptful application modules. Receiptful
integration happens through the Python package, serialized profiles, and
public APIs.
