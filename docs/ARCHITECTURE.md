# Architecture

## Purpose

`escpost-render` interprets one complete ESC/POS byte stream as an isolated print
job and returns one or more PNG receipt sheets.

The renderer works in printer dots. HTML, CSS, SVG text, host fonts, and browser
layout do not participate in positioning.

The public Rust entry point is:

```rust
pub fn render(
    data: &[u8],
    profile: &PrinterProfile,
) -> Result<RenderResult, RenderError>
```

Every render starts from the selected profile's reset defaults. State from a
physical printer before the submitted byte stream is outside the v1 input
model.

## Workspace boundaries

The Rust workspace contains four crates:

- `escpost-profiles` imports, enriches, validates, and loads printer
  profiles.
- `escpost-render` parses ESC/POS, applies printer state, rasterizes content, and
  encodes PNG.
- `escpost-cli` provides the native `escpost` executable, PNG destinations,
  embedded local web viewer, named USB and RAW TCP output, passive printer
  inventory, and platform-native machine configuration.
- `escpost-python` exposes coarse-grained rendering functions through PyO3.

The rendering crate performs pure computation and depends on no
operating-system interface — no networking, hardware, filesystem, or clock
access. This keeps it deterministic and embeddable in any host, including
WebAssembly targets, and is a deliberate boundary documented in
`DESIGN_DECISIONS.md`.

Python calls into Rust once per job. The binding releases the Python
interpreter lock while Rust renders.

The Python package is only the render binding; it contains no CLI. The root
development wrapper routes every command to the Rust executable. Hardware
inventory and printing live in `escpost-cli`, not the Rust rendering library.

## Rust named-printer output

`escpost-cli` loads a `print` source through the same immutable source loader
as `render`, resolves one configured printer name, then hands those decoded
bytes directly to its USB or RAW TCP transport. It does not invoke the renderer
or require a printer profile.

```text
Known ESC/POS source → decode once → configured printer name
                                         ├── nusb bulk OUT
                                         └── RAW TCP socket
```

Transport details live only in `printers.toml`. This keeps `print` independent
of transport-specific flags and gives calibration, inventory, and direct output
the same printer identity. Interactive output selection may call the shared
add-printer workflow; configuration is reloaded before the new name is used.

The USB implementation uses `nusb`. On Linux it detaches a kernel driver such
as `usblp` only while claiming the configured interface and reattaches it when
the interface is released. The optional configured serial number distinguishes
devices with equal VID/PID values. A buffered bulk writer waits for every
submitted transfer to complete and applies a ten-second timeout to each
blocking transfer.

The RAW TCP implementation connects directly to the configured host and port,
writes the source bytes once, and shuts down the connection without a separate
probe or protocol framing. Connection and write timeouts keep failures bounded.

Automated tests replace only the `UsbTransport` boundary and use loopback
listeners for network output. Source loading, name resolution, target
validation, and byte preservation remain real; ordinary tests cannot open or
write to configured physical hardware.

`printers list` uses the same `nusb` dependency but remains a separate passive
inventory path. It first selects USB printer-class devices, opens them only to
read their cached active configuration descriptors, and reports every
alternate-setting-zero printer interface with at least one bulk OUT endpoint.
It never claims an interface, detaches a kernel driver, or sends a USB
transfer. It reads `printers.toml` only to identify matching configured names.
An explicit `--config` file takes precedence over `ESCPOST_CONFIG_DIR`, which
takes precedence over the platform user-configuration directory resolved by
Rust's `directories` crate. A missing implicit file is an empty configuration,
and this read-only path never creates a directory.

Inventory merges discovered interfaces with saved configuration by USB
identity. A matched printer is one connected named entry; unmatched discovered
interfaces remain connected unnamed entries; unmatched configuration becomes
unavailable entries. Connected entries sort first, then unavailable entries,
and each group sorts case-insensitively by display name with stable USB
tie-breakers. Descriptor parsing, configuration matching, merging, ordering,
and human output are tested behind the USB inventory boundary.

The interactive add workflow reuses that passive inventory. It removes
already configured identities, presents each bulk OUT endpoint explicitly,
and stores stable descriptor coordinates. Bus and address are diagnostic
selection labels only because operating systems may change them after a
reconnection. A serial number is stored when available; without one,
simultaneously connected devices with equal VID/PID cannot be distinguished
reliably and are reported as ambiguous.

The Docker wrapper creates and mounts `local/config` at the container user's
normal ESCPost configuration path. This isolates configuration used by a
checkout from an independently installed binary while keeping Docker-specific
paths out of the Rust implementation.

## Rust render command

`escpost-cli` is an application boundary around the renderer. It embeds the
canonical profile pack and resolves a profile from an explicit argument,
recognized source metadata, or an interactive selection. Non-interactive
operation fails instead of silently choosing a physical printer.

The command accepts raw files, readable `.hex` files, stdin, and recognized
conformance-case directories. Output adapters consume one completed
`RenderResult`:

```text
Known ESC/POS source
        │
        ▼
Profile resolution → escpost_render::render
                           │
             ┌─────────────┼─────────────┐
             ▼             ▼             ▼
        one PNG       sheet directory   in-memory job
        or stdout     plus manifest     and web viewer
```

Single-PNG output never drops later sheets. Directory output publishes its
manifest only after all current sheets are complete. An explicit file and the
web viewer may consume the same render without parsing or rendering twice.

The web application, CSS, and JavaScript are embedded in the executable.
Rendered PNGs live in a shared in-memory job store, which is also the intended
handoff point for the future virtual printer. HTTP binds to loopback by
default. The viewer reports ordered sheet names and printer-dot dimensions,
uses one screen pixel per dot initially, and offers only integer,
nearest-neighbor zoom.

Watch mode polls the selected filesystem input and performs each rerender away
from the asynchronous HTTP task. A successful result atomically replaces the
visible job. A parse or render failure is reported by the page while the last
complete sheets remain available.

## Rendering pipeline

```text
ESC/POS bytes
      │
      ▼
Sequential command parser
      │
      ▼
Profile-aware printer state
      │
      ▼
Standard-mode line composition
      │
      ▼
Monochrome dot surfaces
      │
      ▼
One-bit PNG sheets
```

The parser consumes the submitted byte slice from left to right. Commands with
binary data use their documented length fields, so payload bytes are never
searched for command prefixes.

Malformed, truncated, unknown, or unsupported input returns a `RenderError`.
V1 does not return a speculative partial preview after a parser error.

### Renderer modules

Each rendering domain owns one module in `crates/escpost-render/src/`:

```text
lib.rs            public API types and the render entry point
command.rs        sequential ESC/POS parsing and dispatch
state.rs          printer state, line composition, cuts, and limits
text.rs           code-page decoding and glyph rasterization
graphics.rs       bit-image and raster graphics painting
symbols.rs        barcode and QR placement and painting
barcode.rs        one-dimensional barcode encoders
databar.rs        GS1 DataBar encoding
qr.rs             QR matrix adapter
international.rs  ESC R character substitutions
surface/          rendering contract, monochrome raster, and tracing decorator
error.rs          renderer error types
```

`PrinterState` and its lifecycle live in `state.rs`; the text, graphics, and
symbols modules extend it with their own `impl` blocks so each painting
domain stays readable on its own. The public API is re-exported from the
crate root, so module boundaries are not visible to embedders.

The private `RenderSurface` contract keeps command interpretation independent
from raster storage. `MonoSurface` is the ordinary bitmap implementation; the
experimental tracing decorator retains command provenance without duplicating
the interpreter. See [`TRACING.md`](TRACING.md) for the current vertical slice
and intended trace semantics.

## Printer state

The mutable state contains only behavior required by implemented commands:

- active print area and horizontal position;
- motion units, line spacing, and tab stops;
- justification and text modes;
- selected font, code page, and international character set;
- barcode and QR settings;
- stored QR data and buffered graphics;
- the current Standard-mode line;
- completed and active roll surfaces; and
- non-printing device events such as a cash-drawer pulse.

`ESC @` restores implemented settings to the selected profile's defaults and
clears volatile data according to covered command behavior.

New state is added with the command that needs it. V1 does not reserve runtime
models for Page mode, macros, non-volatile resources, or printer state supplied
from outside the job.

## Printer profiles

Profiles provide behavior that cannot be derived from ESC/POS bytes:

- printable width and horizontal/vertical DPI;
- optional cutter geometry as the physical print-head-to-blade distance;
- horizontal and vertical motion units;
- `ESC *` 8-dot vertical pitch for model-specific column-image geometry;
- model-specific positioning behavior for `ESC $` and `ESC \`;
- model-specific feed behavior for `ESC J`, an LF immediately following
  `GS v 0`, and `GS V` Function B modes;
- reset line spacing, code page, international set, and carriage-return mode;
- Font A/B cell size and baseline;
- imported or self-contained code-page slots;
- capabilities used by implemented command handlers; and
- exact `GS k` systems supported by Function A and Function B.

Each field is a descriptor (an intrinsic physical fact) or a deviation (a
confirmed departure from documented ESC/POS baseline behavior); every field
is optional, and stating one is itself the confirmation (DD-031). See
[`PROFILE_SCHEMA.md`](PROFILE_SCHEMA.md) for the full model.

Physical profiles use the upstream `escpos-printer-db` repository as a Git
submodule. Its gitlink pins the complete upstream snapshot. Each upstream
profile source also stores the SHA-256 of its resolved profile, so a change
affecting that printer requires review.

`REFERENCE` is a separate virtual source. It imports nothing from the printer
database and explicitly supplies every current capability and code-page slot.
It represents documented baseline behavior without printer-specific
restrictions. Its 203 DPI, 576-dot paper and cutter geometry are concrete
virtual rendering parameters, not universal ESC/POS mechanism dimensions.

Profile authoring and calibration assets are collocated in visible
`profiles/<profile-id>/` directories. A physical profile also contains the
expected rendering and physical verification of `calibration/input.hex`.
Virtual profiles use focused automated golden cases instead of claiming
physical evidence. Hidden `.escpos-printer-db/` and `.generated/` directories
contain infrastructure, not profiles.

The profile compiler either combines upstream capabilities with a typed TOML
enrichment or compiles a self-contained virtual source. It generates the same
canonical JSON shape for both. The renderer loads only that generated profile;
it does not read the upstream database or TOML at render time.

A profile that advertises full- or partial-cut support must define
`cutter.print_head_to_cutter_dots`. `GS V` Function B uses that fixed distance
plus its command-supplied vertical-motion-unit feed before creating the sheet
boundary. A profile without an autocutter omits the cutter section; Function B
then applies only its profile-selected explicit feed behavior.

Each canonical profile carries:

- a typed source — `Reference`, hash-pinned `Upstream`, or synthesized
  `UpstreamDefault` — including the resolved profile SHA-256 for upstream
  sources; and
- a canonical-profile SHA-256 covering every runtime field.

The canonical hash is the profile's rendering identity. Manually maintained
profile revisions and duplicate repository provenance are intentionally absent.

## Text and symbols

Printable bytes are decoded with the profile-selected code page and Epson
international-character substitutions. The bundled Noto Sans Mono font is
rasterized deterministically into profile-defined character cells. Font engine
advance widths never move the print cursor.

One-dimensional barcode encoders return logical bar and space elements. The
printer state remains responsible for module scaling, placement, HRI, and paper
advance.

QR generation is isolated behind a small adapter around the pure-Rust `qrcode`
crate. The adapter returns an unscaled Boolean module matrix; it cannot place or
render receipt content.

## Dot surfaces and sheets

Surface code is divided into the private rendering contract, the canonical
`MonoSurface`, and an experimental tracing decorator. Ordinary rendering
selects `MonoSurface` statically and carries no trace records; traced rendering
wraps the same raster implementation and is opt-in.

`MonoSurface` stores one byte of ink coverage per scaled subpixel. Faithful
rendering thresholds glyph coverage to hard dots and encodes a one-bit
grayscale PNG. Optional antialiased preview rendering retains soft glyph
coverage and encodes an eight-bit grayscale PNG. Dot-space graphics remain
hard-edged in both modes.

A cut finalizes the active surface. Later output starts another sheet. Without
a cut, final sheet height follows painted content and paper-feed position.

Each `RenderedSheet` contains the logical surface and its encoded PNG. Tests
inspect faithful surfaces for exact command behavior and decode their one-bit
PNGs for end-to-end fixtures.

Additional color or tone models will be designed when an implemented command
requires them. V1 carries no unused color-plane abstraction.

## Results

Successful rendering returns:

```text
RenderResult
├── sheets
├── device_events
├── warnings
└── metadata
    ├── renderer version
    ├── profile id
    └── canonical profile SHA-256
```

Warnings are non-fatal diagnostics from an otherwise successful render, such
as a cut requested on a profile whose printer has no cutter. Known fidelity
boundaries of the renderer itself — representative glyphs, QR mask choice,
unmodeled thermal artifacts — are documented divergences (DD-002, DD-007,
DD-023, DD-024, DD-025) rather than a render-time channel; a profile's
`source` marker signals whether its own descriptors and deviations are
calibrated or synthesized (`PROFILE_SCHEMA.md`).

Device events describe supported non-printing commands and do not make the PNG
incomplete. Callers that care about those actions inspect the event list.

## Resource safety

Rendering limits apply before or during allocation:

- input bytes;
- declared command payload bytes;
- sheet width and height;
- sheet count; and
- total rendered dots.

Limit violations return a controlled error. These limits remain part of v1
because the future API endpoint will accept untrusted print jobs.

## Extension rule

The long-term command target remains the Epson-documented ESC/POS set, tracked
in `COMMAND_COVERAGE.md`.

New protocol families should add the smallest state and profile fields needed
by their first tested vertical slice. The architecture does not pre-model
unimplemented commands.
