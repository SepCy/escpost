# Design Decisions

This is the project's durable decision log. It records both the selected
design and the reasoning that future changes must consider.

Each decision has a status:

- **Accepted**: current design; implementation should follow it.
- **Provisional**: current direction, deliberately easy to revisit.
- **Superseded**: retained for history and replaced by a later decision.

When a decision changes, add a new entry that names the superseded decision
instead of rewriting the old rationale.

## DD-001 — Develop as a standalone project

**Status:** Accepted

### Context

The renderer is initially developed inside the Receiptful repository but is a
generally useful ESC/POS capability that may be open-sourced independently.

### Decision

Develop it under the root `escpos2png/` directory as an isolated Python
project. It must not import Receiptful application modules or rely on
Receiptful's database models.

### Consequences

- Extraction into a separate repository remains straightforward.
- Shared behavior must cross the boundary through serialized profiles, public
  APIs, or fixtures rather than application imports.
- Packaging metadata and dependencies belong to this project once selected.

## DD-002 — Promise geometry fidelity, not photographic fidelity

**Status:** Accepted

### Context

An exact physical reproduction would require proprietary font ROMs, firmware
details, paper chemistry, print density, print-head condition, and mechanical
tolerances. The product need is an accurate preview of layout and printed
elements.

### Decision

Target near dot-perfect geometry for the selected printer profile: positions,
dimensions, cell advancement, wrapping, print areas, buffers, feeds, and cuts.

Exact proprietary glyph shapes and physical print artifacts are outside the
initial fidelity contract.

### Consequences

- The project can use redistributable, deterministic glyph sources.
- Character-cell metrics remain part of the fidelity promise.
- Documentation must not market the result as a photographic printer
  simulation.

## DD-003 — Use a dot-addressed raster as the canonical result

**Status:** Accepted

### Context

ESC/POS printers ultimately energize or strike dots. Using HTML, CSS, SVG text,
or a browser layout engine would delegate important geometry to a renderer
whose behavior is not controlled by the printer profile.

### Decision

The virtual printer renders to one or more dot-addressed sheet surfaces.
Output encoders consume those surfaces.

### Consequences

- Browser layout and font metrics cannot move receipt elements.
- Raster graphics map naturally to the result.
- Memory and dimension limits are required.
- Alternate encoders remain possible without changing printer emulation.

## DD-004 — Make PNG the primary output

**Status:** Accepted

### Context

The canonical result is already a raster. PNG is lossless, broadly supported,
compact for typical receipt imagery, and easy to display on Android and the
web.

### Decision

PNG is the primary public rendering format. Monochrome output should use a
one-bit representation where practical. Integer scaling may be offered for
high-density displays.

### Consequences

- No headless browser is required.
- PNG compression bytes are not suitable as golden fixtures; decoded pixels
  are.
- A future SVG encoder is optional rather than architectural.

## DD-005 — Keep the coordinate system fixed in printer dots

**Status:** Accepted

### Context

Printer commands can change motion units, margins, active print areas, and Page
mode directions. These change the interpretation of command parameters, not
the physical resolution of the print head.

### Decision

Use immutable horizontal and vertical printer-dot coordinates supplied by the
profile. Convert command motion units into that coordinate system using
model-specific truncation and mechanical-pitch rules.

### Consequences

- `GS P` and related commands mutate state without resizing the surface width.
- Horizontal and vertical DPI and motion defaults must be independently
  representable.
- Rounding behavior belongs to command/profile semantics.

## DD-006 — Treat printer profiles as versioned behavioral inputs

**Status:** Accepted

### Context

ESC/POS command support, parameter ranges, defaults, print geometry, code-page
mappings, font metrics, storage, and quirks differ by model and sometimes by
firmware or configured compatibility mode.

### Decision

Rendering always uses an explicit, versioned printer profile. A profile covers
behavior as well as geometry.

### Consequences

- There is no unqualified, universally accurate "ESC/POS default printer."
- Callers can reproduce historical previews by retaining profile identity and
  version.
- Profile validation and conformance fixtures are first-class project work.

## DD-007 — Match character metrics without cloning resident glyphs

**Status:** Accepted

### Context

Exact printer ROM glyphs are unnecessary for the layout-preview goal and may
not be available as redistributable assets. Font engine metrics must still not
control ESC/POS advancement.

### Decision

Decode characters according to printer state, rasterize deterministic
representative glyphs, and fit them into profile-defined cells and baselines.
Advance using ESC/POS metrics only.

### Consequences

- Text layout can be geometry-faithful even when glyph shapes differ.
- Host-installed fonts are unsuitable.
- The glyph provider is replaceable so profiles may later supply exact bitmap
  atlases.
- Broad script coverage and asset licensing remain implementation concerns.

## DD-008 — Target the full documented Epson ESC/POS set incrementally

**Status:** Accepted

### Context

Receiptful initially needs a limited command subset, but an open-source
renderer should not be architecturally restricted to commands emitted by one
encoder.

### Decision

The long-term protocol target is the full Epson-documented ESC/POS command set,
including model-specific behavior, Standard mode, Page mode, downloaded and NV
resources, native symbols, color/tone graphics, and non-visual device actions.

Implementation and release coverage may grow incrementally. Non-Epson
extensions use dialect or profile extension points and are not implied by the
initial completeness claim.

### Consequences

- Parser framing and state abstractions must be designed for commands not yet
  rendered.
- A support matrix is required.
- "Full support" is evaluated for a selected profile, because individual
  printers intentionally support only subsets.

## DD-009 — Model color as bitplanes; implement monochrome first

**Status:** Accepted

### Context

Most thermal receipt printing is one bit per dot. The full Epson graphics
functions also include multiple-tone data with four weighted planes, and some
models support spot colors such as black and red.

### Decision

The surface abstraction supports one or more bitplanes and an explicit color
model. Implement `MONO1` first, while reserving `TONE4` and indexed spot-color
composition in the architecture.

### Consequences

- The common case remains compact and simple.
- Multiple-tone and two-color commands do not require replacing the canonical
  surface later.
- Profiles map logical planes to preview palette or tone values.

## DD-010 — Emulate buffers and state instead of translating commands directly

**Status:** Accepted

### Context

ESC/POS commands form a stateful instruction stream. Alignment can apply to a
composed line, Page mode buffers data before printing, and resources can be
defined in one command and printed later.

### Decision

Interpret commands through a virtual-printer state machine with Standard-mode
line buffers, Page-mode surfaces, and model-aware resource stores. Do not
translate each command independently into final pixels.

### Consequences

- Command ordering and reset behavior can be represented correctly.
- Rendering an isolated job may require an initial device-resource snapshot.
- State transitions need extensive unit tests.

## DD-011 — Represent cuts as sheet boundaries

**Status:** Accepted

### Context

A byte stream may feed and cut paper multiple times. A single unbounded bitmap
does not represent the physical result cleanly.

### Decision

The render result is a sequence of sheets. A cut finalizes the active sheet and
starts the next one when subsequent printable output appears.

### Consequences

- One job may produce multiple PNG files.
- Feed-to-cutter behavior affects sheet height.
- Non-cut jobs finalize at the final content/feed position.

## DD-012 — Produce structured diagnostics and never guess unsafe framing

**Status:** Accepted

### Context

Unknown, unsupported, malformed, or truncated commands are unavoidable,
especially with vendor extensions. Guessing where a binary payload ends can
desynchronize the remainder of the stream and create a misleading preview.

### Decision

Known unsupported commands with reliable framing may be skipped with a
diagnostic. If framing cannot be determined safely, stop interpretation at the
offending byte and mark the result incomplete.

### Consequences

- Partial previews can still be useful without pretending to be complete.
- Every diagnostic retains a byte offset and command identity when known.
- The tokenizer needs exact length rules independently of rendering support.

## DD-013 — Enforce explicit resource limits

**Status:** Accepted

### Context

ESC/POS inputs can declare large raster dimensions, retain resources, execute
macros, create long feeds, and otherwise cause excessive CPU or memory use.

### Decision

Apply configurable limits with conservative defaults to input size, payload
size, rendered dots, sheet dimensions/count, stored resources, symbol data,
and repeated execution.

### Consequences

- Limit violations are controlled diagnostics, not crashes or unbounded
  allocation.
- Applications may select stricter limits for untrusted public input.
- Tests must include adversarial streams.

## DD-014 — Separate living architecture from decision history

**Status:** Accepted

### Context

A single document that mixes current design, alternatives, and historical
rationale becomes difficult to read. One ADR file per early decision would add
unhelpful navigation overhead while the project is still small.

### Decision

Keep:

- `README.md` as the project entry point;
- `ARCHITECTURE.md` as the coherent current design; and
- this single numbered decision log for rationale and history.

Split decisions into individual ADR files only when this log becomes difficult
to maintain.

### Consequences

- Readers can understand the current architecture without reconstructing it
  from decisions.
- Decision rationale remains easy to review in one place.
- Superseded decisions stay in this file unless and until the ADR migration
  occurs.

## DD-015 — Make rendering assumptions reproducible

**Status:** Accepted

### Context

The same byte stream can render differently after a profile correction,
renderer behavior change, or change in device-resident resources. Raw streams
may also omit `ESC @` and rely on state established before the job.

### Decision

A render result records the renderer version, profile identity and version, and
initial-state assumption. Callers may provide an explicit state/resource
snapshot. Without one, rendering starts from documented profile reset defaults
and reports that assumption.

### Consequences

- Reproducing a historical preview requires more than retaining its ESC/POS
  bytes.
- Applications can include renderer and profile versions in cache keys.
- Missing device-resident resources produce incomplete-preview diagnostics
  rather than invented output.

## DD-016 — License the project under Apache-2.0

**Status:** Accepted

### Context

The project is intended for independent open-source distribution and should be
usable in commercial and closed-source applications. It may also receive
external contributions to protocol and rendering behavior.

### Decision

License project code and documentation under the Apache License, Version 2.0.
Track bundled third-party assets, including fonts, under their own compatible
licenses and preserve required attribution.

### Consequences

- Users may use, modify, and redistribute the library under permissive terms.
- Contributors provide the copyright and patent grants stated by Apache-2.0.
- The license does not provide patent rights from non-contributors such as
  printer manufacturers.
- Asset provenance and licensing must be reviewed before distribution.

## DD-017 — Implement a reusable Rust core with Python bindings

**Status:** Accepted

### Context

Full ESC/POS interpretation performs byte parsing, state-machine execution,
dot-level composition, symbol generation, and PNG compression. The renderer
will be used by a Python FastAPI application, but its core is also useful to
Android, command-line, WebAssembly, and non-Python consumers.

### Decision

Implement parsing, emulation, rasterization, profiles, and PNG encoding in a
Python-independent Rust crate. Expose a thin Python module through PyO3 and
build Python distributions with maturin.

Use coarse-grained foreign-function calls: a complete input, validated profile,
and render options enter Rust together, and a result containing PNG bytes,
events, and diagnostics returns. Release the Python interpreter lock during
Rust-only rendering.

This supersedes only the "Python project" implementation wording in DD-001;
the standalone project boundary remains accepted.

### Consequences

- CPU- and memory-sensitive loops execute in native code with Rust's checked
  ownership and type system.
- The core can later be exposed through a CLI, JNI, WebAssembly, or another
  language binding without reimplementing ESC/POS behavior.
- Python callers retain an ergonomic in-process API.
- Publishing requires a supported wheel matrix and native build tooling.
- Async services must run rendering outside their event-loop thread.
- Python callbacks inside the render loop are prohibited.

## DD-018 — Import and enrich the shared ESC/POS printer database

**Status:** Accepted

### Context

`receipt-print-hq/escpos-printer-db` is already consumed by python-escpos and
provides community-maintained profile identifiers, media dimensions, DPI,
font columns, code pages, colors, and capability flags. Recreating that catalog
would fragment identifiers and duplicate maintenance. Its schema does not,
however, describe all geometry and behavior required by an emulator.

### Decision

Pin the upstream database as a source repository and import it at build time.
Maintain escpos2png enrichment files for exact rendering metrics, behavioral
details, corrections, and explicit approximations. Resolve and validate both
sources into a canonical profile pack embedded in the Rust library.

Do not fetch profile data at installation or render time. Record the upstream
commit, enrichment revision, and canonical content hash for reproducibility.
Allow downstream projects to run custom upstream-compatible profiles through
the same importer.

Retain the upstream dataset's CC BY 4.0 license and attribution separately from
escpos2png's Apache-2.0 code license, and identify modifications to imported
data.

### Consequences

- python-escpos generators and escpos2png previews can share profile names.
- Receiptful custom profiles can feed both systems.
- Upstream updates are deliberate, reviewable dependency changes.
- The renderer is insulated from upstream schema changes by its importer and
  canonical internal schema.
- A large catalog does not imply high-fidelity support: profiles without
  sufficient enrichment must report documented approximations.
- Release artifacts need third-party attribution and license material.

## DD-019 — Version escpos2png independently from the outset

**Status:** Accepted

### Context

The project starts inside the Receiptful working tree but is intended for its
own open-source lifecycle, releases, issues, and commit history.

### Decision

Initialize `escpos2png/` as its own Git repository and ignore that directory
from the parent Receiptful repository. Include it in Receiptful as a pinned Git
submodule once its remote repository and initial implementation are ready.

### Consequences

- Development commits stay focused on the renderer.
- The renderer can establish its own release and contribution policies.
- Receiptful's status and commits do not accidentally absorb renderer files.
- Until the submodule is added, fresh Receiptful clones and its CI do not
  contain escpos2png.
- Work in the nested repository must be committed and pushed independently;
  the parent repository cannot protect uncommitted nested work.

## Open questions

The following are intentionally not decided yet:

- minimum supported Rust and Python versions;
- crate layout, maturin packaging, wheel targets, and dependency policy;
- initial reference printer profiles;
- serialized profile schema and compatibility policy;
- default glyph provider and redistributable font assets;
- public synchronous and streaming APIs;
- PNG library or dependency-free encoder strategy;
- sparse, tiled, banded, or contiguous surface storage;
- the exact support-matrix format; and
- whether bidirectional status commands need a configurable response emulator.
