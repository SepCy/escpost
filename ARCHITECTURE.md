# Architecture

## 1. Purpose

`escpos2png` interprets an ESC/POS byte stream as a virtual printer and renders
the resulting sheets to PNG.

The renderer operates on printer dots rather than HTML, CSS, or vector text.
This prevents browser layout, font substitution, and viewport behavior from
changing receipt geometry.

Conceptually, the core operation is:

```rust
pub fn render(
    data: &[u8],
    profile: &PrinterProfile,
    initial_state: Option<&PrinterState>,
) -> Result<RenderResult, RenderError>
```

The emulation engine is implemented as a Python-independent Rust crate. A thin
PyO3 extension exposes coarse-grained operations to Python:

```python
result = escpos2png.render(data, profile="TM-T88V")
```

One render crosses the Python/Rust boundary once. Rust performs parsing,
emulation, rasterization, and PNG encoding without repeatedly calling back into
Python. Bindings release the Python interpreter lock during Rust-only work.
Applications must execute CPU-bound rendering away from an asynchronous event
loop, for example in FastAPI's worker thread pool.

The Rust and Python signatures above are architectural boundaries, not yet
committed public APIs.

## 2. Fidelity boundary

The renderer targets a geometry-faithful logical printout:

- Every visible element is positioned on the selected printer's dot grid.
- Character advancement uses profile and command metrics rather than font
  engine advance widths.
- Raster graphics preserve their encoded dots and ESC/POS scaling behavior.
- Printer buffers, active print areas, motion units, and mode transitions
  determine when and where content is committed.

Resident glyph shapes may differ from a physical printer, but they must be
rendered deterministically inside the correct character cells. Physical
appearance—including paper texture, heat, ink spread, worn print heads, and
sub-dot mechanical tolerances—is outside the initial fidelity boundary.

## 3. Processing pipeline

```text
ESC/POS bytes
      │
      ▼
Incremental tokenizer ───────────────┐
      │                              │
      ▼                              │
Command interpreter / virtual printer│
      │                 ▲            │
      │                 │            │
      ├──────── Printer profile      │
      ├──────── Mutable state        │
      └──────── Device resources     │
      │                              │
      ▼                              │
Standard/Page-mode print buffers     │
      │                              │
      ▼                              │
Dot-addressed sheet surfaces         │
      │                              │
      ├──────── Diagnostics ◄────────┘
      ▼
PNG encoder
```

Parsing, printer emulation, rasterization, and PNG encoding are separate
layers. This allows command parsing to be tested without image snapshots and
allows additional output encoders to be added without changing ESC/POS
semantics.

## 4. Incremental tokenizer

The tokenizer consumes a byte stream and emits printable-data tokens or
framed-command tokens.

It must:

- recognize control bytes and ESC, FS, GS, DLE, and other command families;
- use command-specific length rules before scanning binary payloads for more
  commands;
- support fragmented input where a command or payload spans chunks;
- distinguish known-but-unsupported commands from malformed or unknown
  framing; and
- retain byte offsets for diagnostics.

Binary image, symbol, downloaded-resource, and user-defined-character payloads
may contain arbitrary control-byte values. Their declared payload must
therefore be consumed atomically rather than searched for command prefixes.

If a known command is unsupported, the tokenizer may skip its correctly framed
payload and emit a diagnostic. If the command cannot be framed safely,
interpretation stops at that byte offset rather than treating possible payload
bytes as printable text.

## 5. Command interpreter

The interpreter applies tokens to a virtual printer. Commands may:

- mutate formatting, encoding, motion, print-area, or mode state;
- append printable data to the current buffer;
- define, select, print, or delete volatile and non-volatile resources;
- commit a line or Page mode buffer to a sheet;
- feed or cut paper;
- produce a device action with no printed pixels; or
- request status or other bidirectional behavior.

Non-visual actions such as a cash-drawer pulse, buzzer request, or status query
are represented as structured events and diagnostics. They do not paint
arbitrary marks onto the receipt.

Command handlers are selected through a registry that can account for the
active printer profile and dialect. The full Epson-documented command set is
the target; non-Epson extensions can be added without changing the tokenizer
or surface abstractions.

## 6. Printer profiles

A resolved printer profile supplies the immutable and default behavior that
cannot be recovered from a job's bytes alone.

The initial catalog is sourced from the community-maintained
`receipt-print-hq/escpos-printer-db` repository at an explicitly pinned Git
commit. Its identifiers, media geometry, code-page mappings, font columns,
colors, and capability flags are imported rather than copied into an unrelated
catalog.

That upstream schema is designed primarily for ESC/POS command generators and
does not contain every fact required for geometry-faithful emulation.
escpos2png therefore maintains versioned enrichment files containing exact
rendering behavior, corrections, and documented approximations. A build-time
importer resolves upstream inheritance, applies enrichments, validates the
result, and generates a canonical profile pack embedded in the Rust library.
No Git checkout or network access is needed at installation or render time.

Receiptful-specific profiles can pass through the same importer, preserving
one profile identifier across HTML-to-ESC/POS generation and ESC/POS preview.

`PROFILE_SCHEMA.md` defines the enrichment format, upstream drift detection,
automatic change classification, provenance, validation, and canonical-output
rules.

At minimum, a profile needs:

```text
identity
├── vendor
├── model
├── firmware/dialect variant
└── profile version

geometry
├── horizontal and vertical DPI
├── paper width
├── printable width in dots
├── printable-area offset
├── default motion units
├── default line spacing
└── cutter/feed geometry

fonts
├── cell width and height
├── baseline
├── default character spacing
├── supported code pages
└── glyph-provider selection

capabilities
├── supported commands and parameter ranges
├── Standard/Page mode restrictions
├── color model
├── storage capacities
└── model-specific quirks
```

The printable width in dots is authoritative for layout. Paper width and
printable-area offsets are used to represent non-printable margins.

Commands such as `GS P` change how subsequent parameters map to physical
motion units; they do not change the underlying printer-dot coordinate system.

Profiles should be serializable and versioned. A caller that stores rendered
jobs should retain the profile identity and version used for each job so that
later profile changes do not silently change historical previews.

Reproducibility also depends on the renderer version and initial device state.
A render result therefore records the renderer version, profile identity and
version, canonical profile hash, upstream database commit, and initial-state
assumption alongside its diagnostics.

## 7. Mutable printer state and resources

The virtual printer state includes:

- Standard or Page mode;
- current print position and active print area;
- horizontal and vertical motion units;
- line spacing and tab stops;
- alignment and character formatting;
- active font, international character set, and code page;
- active color or tone;
- line and Page mode buffers;
- user-defined characters;
- downloaded graphics and symbols;
- non-volatile graphics and configuration relevant to visible output; and
- macro-definition or execution state.

Some resources survive `ESC @`, reset, or power cycles while others do not,
depending on the command and printer. These lifetimes are part of command and
profile behavior.

Rendering an isolated job may require device-resident resources that are not
contained in its byte stream. Callers may provide an initial resource snapshot.
If required resources are absent, the result reports that the preview is
incomplete instead of fabricating their contents.

When no initial state is supplied, rendering begins from the profile's
documented reset defaults and records that assumption. A stream containing
`ESC @` establishes those defaults at that point, but it does not necessarily
clear every downloaded or non-volatile resource.

## 8. Print buffers

### Standard mode

Printable data is first composed in a line buffer. The interpreter applies
character-cell advancement, explicit positions, tabs, raster data, symbols,
and the active print area before committing the line to the roll surface.

Justification is applied to the composed line inside the active printing area,
not to each token independently.

### Page mode

Page mode uses a finite off-screen dot surface defined by its logical origin,
dimensions, and print direction. Commands paint into this buffer using the
appropriate coordinate transform. A Page mode print command composites the
buffer onto the roll according to ESC/POS semantics.

The Page mode buffer must not be approximated with ordinary line flow.

## 9. Character rendering

Byte decoding and glyph rendering are separate:

1. Decode printable bytes using the active encoding and international character
   substitutions.
2. Request a deterministic glyph bitmap for the decoded character.
3. Fit and clip the glyph inside the profile-defined cell and baseline.
4. Apply ESC/POS transformations such as width/height multiplication,
   rotation, inversion, underline, and supported emphasis behavior.
5. Advance by ESC/POS cell and spacing metrics, never by the font provider's
   natural advance width.

The default glyph provider embeds the redistributable Noto Sans Mono 2.006
font bytes in the Rust library and rasterizes them with a pinned pure-Rust
renderer and fixed one-bit threshold. Host-installed fonts are never searched
because they would make output platform-dependent.

Profiles may later provide model-specific glyph atlases without changing the
layout engine.

### Native symbol generation

The command interpreter owns all printer-visible symbol behavior: ESC/POS
framing, persistent settings, profile capability checks, active print-area
placement, justification, module scaling, HRI placement, paper advance, and
reset behavior.

One-dimensional barcode encoders live inside the Rust core and return logical
bar/space elements. They do not know about printer dots or surfaces. QR
generation uses a small internal adapter around the pure-Rust `qrcode` crate.
The adapter accepts raw bytes and an error-correction level and returns only an
unscaled Boolean module matrix. It does not render an image or choose receipt
layout.

This boundary keeps the standards-heavy QR error-correction and masking
implementation replaceable. A future hardware finding can replace or fork the
adapter without changing ESC/POS parsing or the public rendering API.

## 10. Dot surfaces and color models

The canonical rendered representation is one or more dynamically sized sheet
surfaces. A surface may be implemented as bands, tiles, or sparse rows as long
as its observable behavior is a dot-addressed raster.

The color abstraction supports multiple bitplanes:

```text
MONO1
    one printed/not-printed plane

TONE4
    four weighted planes producing a 0–15 tone

INDEXED
    model-specific spot-color planes such as black and red
```

Monochrome is the first implementation target. The abstraction must still
preserve multiple-tone and spot-color commands because those are present in
the full Epson ESC/POS command set.

The printer profile maps plane combinations to preview colors or grayscale
values. Global print-density configuration may influence that mapping but does
not convert ordinary monochrome commands into per-pixel grayscale.

## 11. Sheets, feeds, and cuts

A render result contains a sequence of sheets rather than assuming one
unbounded image:

```text
RenderResult
├── sheets[]
│   ├── dot surface
│   ├── geometry
│   └── terminating paper action
├── device events[]
└── diagnostics[]
```

A cut finalizes the current sheet. Multiple cuts can therefore produce
multiple PNGs from one byte stream.

Without a cut, the sheet height is the greater of the painted content bounds
and current paper-feed position. Profile-specific cutter or feed geometry is
included when the relevant command requires it.

## 12. PNG encoding

PNG is the primary output because the logical printer result is already a
raster.

- Monochrome sheets should use a one-bit PNG where practical.
- Multiple-tone sheets may use four-bit grayscale or an equivalent lossless
  eight-bit mapping.
- Spot-color sheets should use an indexed palette where practical.
- Integer export scaling may map each printer dot to an `n × n` pixel block.
- No lossy compression or smoothing is applied.

PNG encoding is downstream of emulation. It must not influence layout or
printer state.

## 13. Diagnostics and completeness

Rendering returns structured diagnostics rather than silently hiding
differences:

```text
severity
byte offset
command identity, when known
profile identity
message
effect on preview completeness
```

Examples include an unsupported model-specific command, a missing NV logo,
invalid command parameters, truncated payload data, a profile approximation,
or a resource-limit violation.

A result can distinguish:

- complete for the selected profile;
- complete except for non-visual device behavior;
- visually incomplete but safely parsed; and
- aborted because byte framing became unreliable.

## 14. Resource safety

All externally controlled dimensions and counts require limits, including:

- input bytes;
- declared command payload size;
- sheet width and height;
- total rendered dots;
- number of sheets, commands, elements, macros, and stored resources;
- barcode and two-dimensional-code input size; and
- recursive or repeated macro execution.

Limits are explicit renderer options with conservative defaults. Exceeding a
limit produces a diagnostic and controlled failure, never an unbounded
allocation.

## 15. Verification strategy

Verification should combine:

- tokenizer tests for command framing and fragmented input;
- state-transition tests for individual commands;
- golden dot-surface fixtures for representative byte streams;
- PNG decoding tests that compare pixels, not compressed file bytes;
- model-profile tests for defaults and command availability;
- malformed and adversarial input tests;
- captures generated by independent ESC/POS encoders; and
- calibration fixtures compared with physical printers for supported reference
  profiles.

Receiptful's HTML-to-ESC/POS output can provide useful integration fixtures,
but the standalone project must not depend on Receiptful code.

The first physical reference profile is `NT-5890K`. Its upstream profile
inherits 384-dot, 203-DPI geometry from `POS-5890`; those values remain
hypotheses until checked against the connected Netum printer.

Hardware calibration uses a Python CLI and python-escpos as a raw USB
transport. A conformance case's immutable ESC/POS bytes are loaded once,
rendered by the Rust engine, and sent unchanged to the printer. High-level
python-escpos formatting methods are not used because they would generate a
different stream.

Physical printing is opt-in and never runs as part of ordinary tests or CI.
Detailed fixture, calibration, and golden-review rules live in `TESTING.md`.
