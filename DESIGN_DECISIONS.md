# Design Decisions

This is the project's durable domain-decision log. It records choices about
ESC/POS interpretation, rendering, fidelity, printer behavior, safety, and
other principles whose rationale cannot be recovered from the current
architecture alone.

Current component boundaries and implementation structure belong in
`ARCHITECTURE.md`. Testing workflow belongs in `TESTING.md`; profile-format
details in `PROFILE_SCHEMA.md`; and repository, tooling, licensing, release,
and contribution process in their corresponding project documents.

Decision numbers are stable and may contain gaps when an entry is relocated
outside this document's scope.

Each decision has a status:

- **Accepted**: current design; implementation should follow it.
- **Provisional**: current direction, deliberately easy to revisit.
- **Superseded**: retained for history and replaced by a later decision.

When a decision changes, add a new entry that names the superseded decision
instead of rewriting the old rationale.

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

### Consequences

- python-escpos generators and escpos2png previews can share profile names.
- Receiptful custom profiles can feed both systems.
- Upstream updates are deliberate, reviewable dependency changes.
- The renderer is insulated from upstream schema changes by its importer and
  canonical internal schema.
- A large catalog does not imply high-fidelity support: profiles without
  sufficient enrichment must report documented approximations.

## DD-022 — Use typed, hash-guarded profile enrichments

**Status:** Accepted

### Context

escpos2png must complete and occasionally correct the shared upstream printer
database. A mature per-field evidence and patch protocol would provide strong
audit detail but would impose substantial authoring and implementation cost
before the first profile is calibrated.

Pinning only the complete upstream repository is reproducible, but it does not
distinguish an unrelated profile change from a change to the selected printer
or one of its inherited ancestors.

### Decision

Maintain one global lock containing the upstream repository and Git commit.
For each enriched printer, store the SHA-256 of its fully resolved,
deterministically normalized upstream profile.

Express enrichments as partial typed TOML in escpos2png's canonical profile
structure. The compiler automatically classifies each value as added,
confirmed, or corrected by comparing it with the imported canonical draft.

Use simple source and conformance-case references plus explicit approximation
records. Generate deterministic canonical JSON, an enrichment hash, and a
canonical profile hash.

Reject unknown enrichment fields and stale upstream-profile hashes. Defer a
generic patch language, operation declarations, separate evidence records,
per-field provenance wrappers, and numeric confidence values until real
maintenance needs require them.

### Consequences

- Upstream drift affecting an enriched printer cannot pass silently.
- Unrelated upstream profile changes do not force every enrichment to change.
- Profile authors edit ordinary typed values rather than patch operations.
- The compiler produces an auditable change classification automatically.
- The canonical renderer input is independent of the upstream YAML schema.
- Version 1 provenance is intentionally coarse and may require a future schema
  revision if profile reviews become ambiguous.

## DD-023 — Embed and pin the default representative glyph source

**Status:** Accepted

### Context

Text previews must not change with fonts installed on the host or in a
container. Exact printer ROM glyphs are outside the initial fidelity contract,
but representative glyph rasterization must still be reproducible and
license-compatible.

### Decision

Bundle Noto Sans Mono 2.006 under the SIL Open Font License 1.1 and embed its
verified bytes in the Rust renderer. Rasterize with the pinned pure-Rust
`fontdue` implementation and a fixed one-bit alpha threshold.

Printer profiles remain authoritative for cell width, cell height, baseline,
spacing, and advancement. The source font's natural metrics never control
ESC/POS layout. Keep the glyph-provider boundary replaceable so a profile can
later select a canonical bitmap atlas or printer-specific glyphs.

### Consequences

- Rendering does not read fonts from the host system.
- Font, rasterizer, or threshold changes are deliberate rendering changes that
  require pixel-fixture review.
- The font asset retains its own license and hash alongside the project.
- Model-specific atlases can improve glyph fidelity without changing layout
  semantics.

## DD-024 — Own printer semantics and isolate standards-heavy symbol generation

**Status:** Accepted

### Context

Native barcode and two-dimensional-code commands combine two different
problems. ESC/POS defines state, command framing, printer capability, layout,
paper movement, and HRI behavior. The symbol standards define checksums,
compaction, error correction, masks, and logical bars or modules.

Implementing every symbol standard locally would give complete source control,
but source ownership alone does not improve correctness. Mature,
standards-focused implementations provide useful independent coverage of rules
that are easy to implement almost correctly.

### Decision

escpos2png owns ESC/POS parsing and every printer-visible symbol behavior,
including placement and scaling. It also owns the common one-dimensional
barcode encoders, whose algorithms are small enough to review against the
printer reference and barcode standards.

Use a replaceable internal adapter around a pure-Rust QR implementation to
produce an unscaled module matrix from raw bytes. Do not expose the dependency
through the public API and do not use its image-rendering features. The
renderer remains responsible for mapping modules to printer dots.

Treat a valid QR matrix as distinct from a firmware-identical QR matrix.
Segmentation and mask selection may differ between a standards-compliant
library and a particular printer firmware. Record that difference as an
approximation until hardware evidence requires a fork or replacement.

### Consequences

- Symbol libraries cannot move, scale, clip, or feed receipt content.
- The QR dependency can be audited, pinned, fuzzed, forked, or replaced behind
  one small boundary.
- One-dimensional behavior remains directly testable without a general image
  or barcode-rendering dependency.
- Exact module equality with a selected printer requires hardware fixtures;
  successful decoding alone is insufficient evidence.
- New symbol families may use the same dependency rule when outsourcing the
  standards-heavy portion materially improves correctness.

## Open questions

The following are intentionally not decided yet:

- canonical runtime profile fields for full command coverage and their
  compatibility policy;
- whether bidirectional status commands need a configurable response emulator;
  and
- any remaining fidelity policy needed for printer behaviors that cannot be
  observed from an isolated byte stream.
