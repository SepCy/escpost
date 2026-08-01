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
sends the same source types unchanged to a named USB or RAW TCP printer, and
Rust `printers list` reports connected or configured targets across those
transports. Rust `printers add` registers either transport, including
descriptor-based interactive USB selection, and non-interactive selection by
USB descriptor. Rust `serve` acts as a virtual RAW TCP printer: it captures a
print job sent to it and previews the most recent one in the same web viewer.
The developer CLI is entirely Rust. The Python binding remains available to
applications for embedding the renderer; physical calibration uses the same
`render` and `print` commands against the shared calibration receipt.

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
scripts/python-binding-test
```

Regenerate the canonical runtime profile pack after changing an enrichment or
the pinned upstream source:

```bash
docker compose run --rm test cargo run --quiet \
  -p escpost-profiles --bin compile-profile-pack -- \
  profiles/.escpos-printer-db/dist/capabilities.json \
  profiles profiles/.generated/profiles.json
```

`./escpost` is the stable development entry point. It runs every command
through the Rust CLI. The CLI service has USB access for physical workflows.
The Python render binding is separate from the CLI; `scripts/python-binding-test`
builds and exercises it in the test service.

List connected USB printer-class devices:

```bash
./escpost printers list
```

The list is read-only and combines connected USB interfaces with configured
RAW TCP network printers. Network targets are marked connected when their
saved endpoint accepts a one-second TCP handshake. Probes run concurrently and
send zero bytes. Connected printers appear first; each status group is
alphabetical by display name. Every result includes either its configured
profile or `profile: unassigned`, independently of its transport. A connected
saved USB printer appears once under its configured name and includes its
model, transport, VID/PID, USB location, interface, endpoints, and cached
identity strings. Listing does not claim a USB interface, send ESC/POS data,
scan the network, or create configuration.

Native installations keep `printers.toml` in the platform user-configuration
directory. On Linux this is normally
`~/.config/escpost/printers.toml`, or
`$XDG_CONFIG_HOME/escpost/printers.toml` when that variable is set. The
development wrapper deliberately uses the separate ignored file
`local/config/printers.toml`. Set `ESCPOST_CONFIG_DIR` to deliberately share
another directory in a native installation, or pass
`printers --config <FILE>` for one invocation. With the Docker wrapper, set
`ESCPOST_CONFIG_HOST_DIR` to deliberately mount another host directory.

Register a connected USB printer interactively:

```bash
./escpost printers add
```

Choose `usb`, select one of the unconfigured printer-class devices, then give
it a local name and optionally assign a rendering profile. ESCPost reads and
stores the VID/PID, available serial number, interface, and selected bulk OUT
endpoint. USB bus and address are shown only to distinguish devices during
selection because they can change after reconnecting. If a device exposes
several bulk OUT endpoints, each route is a separate choice instead of an
implicit guess.

Register a network printer when its address is already known:

```bash
./escpost printers add kitchen \
  --transport network \
  --host 10.42.0.71
```

At a terminal, network values may also be omitted and ESCPost asks for them.
The port prompt defaults to `9100`, so Enter accepts the usual RAW TCP port;
an explicit `--port` skips that prompt. Non-interactive omission silently uses
`9100`. Pass `--non-interactive` to make missing required values fail; USB
registration is currently interactive-only because the developer must select
a concrete descriptor and endpoint. The command updates the same
developer-editable `printers.toml` used by `printers list`. If a name already
exists, an interactive command asks for another; a non-interactive command
fails without changing the file. Registration never sends print data.

The Compose service joins host group GID `7`, the conventional `lp` group on
Debian-derived systems. Set `USB_GROUP_ID` when the USB printer device belongs
to a different host group.

Send a raw or hexadecimal ESC/POS stream unchanged to a configured printer:

```bash
./escpost print receipt.hex \
  --printer netum-usb \
  --non-interactive
```

The name resolves every transport detail from `printers.toml`; `print` has no
USB, host, or port options. A USB entry supplies its VID/PID, optional serial
number, interface, and endpoint. A network entry supplies its RAW TCP host and
port. A rendering profile is not required because the bytes are already
ESC/POS. An unassigned profile does not implicitly select `REFERENCE`.

At a terminal, `--printer` may be omitted. ESCPost offers the configured
printers plus “Add a printer…”. Selecting that action runs the same workflow
as `printers add` and then prints to the new target. Without a terminal, or
with `--non-interactive`, `--printer <NAME>` is required.

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

Run `serve` to act as a virtual RAW TCP printer. Point an application at the
RAW port; each job it sends is rendered and shown in the web viewer. Both
listeners bind loopback and pick a free port automatically — the RAW printer
from 9100, the viewer from 9000 — and print the addresses they chose:

```bash
./escpost serve
```

The profile defaults to `REFERENCE`; pass `--profile` to preview through a
specific printer's profile. Pass `--listen` or `--web-listen` to pin an exact
address, which is then bound strictly. The viewer shows where to send data
until the first job arrives, then previews the most recent captured job. A job
ends when the connection closes.

For focused physical calibration, first register the printer with
`printers add` to populate `local/config/printers.toml` through the Docker
wrapper. Then render and print the same version-controlled input — a single
conformance case, or the shared calibration receipt at `calibration/input.hex`:

```bash
./escpost render calibration/input.hex \
  --profile NT-5890K \
  --output-dir local/calibration

./escpost print calibration/input.hex --printer netum-usb
```

`render` and `print` are the same primitives every job uses. The render step
names the profile explicitly; `print` sends the bytes unchanged to the named
printer.

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
