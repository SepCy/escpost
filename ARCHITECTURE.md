# Architecture

## Purpose

`escpos2png` interprets one complete ESC/POS byte stream as an isolated print
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

The Rust workspace contains three crates:

- `escpos2png-profiles` imports, enriches, validates, and loads printer
  profiles.
- `escpos2png` parses ESC/POS, applies printer state, rasterizes content, and
  encodes PNG.
- `escpos2png-python` exposes coarse-grained rendering functions through PyO3.

Python calls into Rust once per job. The binding releases the Python
interpreter lock while Rust renders.

The Python package also contains the developer CLI for conformance cases and
physical USB printers. Hardware discovery and printing are not part of the
Rust rendering library.

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
- horizontal and vertical motion units;
- `ESC *` 8-dot vertical pitch for model-specific column-image geometry;
- model-specific positioning behavior for `ESC $` and `ESC \`;
- model-specific feed behavior for `ESC J`, an LF immediately following
  `GS v 0`, and `GS V` Function B modes;
- reset line spacing, code page, international set, and carriage-return mode;
- Font A/B cell size and baseline;
- imported code-page slots;
- capabilities used by implemented command handlers;
- exact `GS k` systems supported by Function A and Function B; and
- explicit fidelity approximations.

The upstream `escpos-printer-db` repository is a Git submodule. Its gitlink pins
the complete upstream snapshot. Each enrichment also stores the SHA-256 of its
resolved upstream profile, so a change affecting that printer requires review.

The profile compiler combines the upstream capabilities with a typed TOML
enrichment and generates canonical JSON. The renderer loads only that generated
profile. It does not read the upstream database or TOML at render time.

Each canonical profile carries:

- the resolved upstream-profile SHA-256, for drift detection; and
- a canonical-profile SHA-256 covering every runtime field and approximation.

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

`MonoSurface` stores one printed/not-printed value per printer dot. All current
commands compose into this monochrome representation.

A cut finalizes the active surface. Later output starts another sheet. Without
a cut, final sheet height follows painted content and paper-feed position.

Each `RenderedSheet` contains the logical surface and its one-bit grayscale PNG.
Tests inspect surfaces for exact command behavior and decode PNGs for
end-to-end fixtures.

Additional color or tone models will be designed when an implemented command
requires them. V1 carries no unused color-plane abstraction.

## Results

Successful rendering returns:

```text
RenderResult
├── sheets
├── device_events
├── profile approximations
└── metadata
    ├── renderer version
    ├── profile id
    └── canonical profile SHA-256
```

Approximations describe known fidelity boundaries of the selected profile.
They are not parser diagnostics.

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
