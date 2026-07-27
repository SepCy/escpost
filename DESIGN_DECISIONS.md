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

## DD-006 — Treat printer profiles as content-addressed behavioral inputs

**Status:** Accepted

### Context

ESC/POS command support, parameter ranges, defaults, print geometry, code-page
mappings, font metrics, storage, and quirks differ by model and sometimes by
firmware or configured compatibility mode.

### Decision

Rendering always uses an explicit printer profile covering behavior as well as
geometry. The canonical content hash is the exact profile identity; no manual
profile revision is maintained.

### Consequences

- There is no unqualified, universally accurate "ESC/POS default printer."
- Callers can reproduce historical previews by retaining the profile id and
  canonical hash.
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

Implementation and release coverage grows incrementally. Non-Epson extensions
are not implied by the initial support claim.

### Consequences

- Each command family adds the smallest framing, state, and profile model needed
  by its first tested vertical slice.
- A support matrix is required.
- "Full support" is evaluated for a selected profile, because individual
  printers intentionally support only subsets.

## DD-009 — Implement monochrome before modeling additional color

**Status:** Accepted

### Context

Most thermal receipt printing is one bit per dot. The full Epson graphics
functions also include multiple-tone data with four weighted planes, and some
models support spot colors such as black and red.

### Decision

V1 uses one printed/not-printed surface. Multiple-tone or spot-color
representations will be designed with the first implemented command that needs
them instead of reserving an unused abstraction.

### Consequences

- The implemented surface matches current one-bit command semantics directly.
- Future color work may extend or replace the surface representation based on
  concrete command and printer evidence.

## DD-010 — Emulate buffers and state instead of translating commands directly

**Status:** Accepted

### Context

ESC/POS commands form a stateful instruction stream. Alignment can apply to a
composed line, Page mode buffers data before printing, and resources can be
defined in one command and printed later.

### Decision

Interpret implemented commands through a virtual-printer state machine with
Standard-mode line composition and the resource stores those commands require.
Do not translate each command independently into final pixels. Add Page mode
state when Page mode becomes an implemented vertical slice.

### Consequences

- Command ordering and reset behavior can be represented correctly.
- State that is not part of the submitted isolated job is outside v1.
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

## DD-012 — Fail explicitly and never guess unsafe framing

**Status:** Accepted

### Context

Unknown, unsupported, malformed, or truncated commands are unavoidable,
especially with vendor extensions. Guessing where a binary payload ends can
desynchronize the remainder of the stream and create a misleading preview.

### Decision

V1 returns a structured `RenderError` for malformed, truncated, unknown, or
unsupported input. It does not continue with a partial preview after an error.
Binary payloads are consumed only through documented framing.

### Consequences

- Errors retain byte offsets and command identity when known.
- A future partial-preview mode requires concrete recovery semantics and a new
  result model.

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

- Limit violations are controlled errors, not crashes or unbounded allocation.
- Applications may select stricter limits for untrusted public input.
- Tests must include adversarial streams.

## DD-015 — Make rendering assumptions reproducible

**Status:** Accepted

### Context

The same byte stream can render differently after a profile correction or
renderer behavior change. A physical printer may also have state established
before an isolated job.

### Decision

V1 defines every submitted byte stream as an isolated job starting from the
selected profile's reset defaults. A result records the renderer version,
profile id, and canonical profile hash.

### Consequences

- Reproducing a historical preview requires more than retaining its ESC/POS
  bytes.
- Applications can include renderer version and canonical profile hash in cache
  keys.
- Device-resident state is outside the v1 input model rather than represented
  by an unused snapshot abstraction.

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

Do not fetch profile data at installation or render time. The Git submodule
pins the upstream repository, and the canonical content hash identifies the
runtime profile.

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

Use the Git submodule itself as the global repository and commit pin. For each
enriched printer, store the SHA-256 of its fully resolved, deterministically
normalized upstream profile.

Express enrichments as typed TOML. Use simple source references plus explicit
approximation records, and generate deterministic canonical JSON with a
canonical profile hash.

Reject unknown enrichment fields and stale upstream-profile hashes. Defer a
generic patch language, operation declarations, separate evidence records,
per-field provenance wrappers, and numeric confidence values until real
maintenance needs require them.

### Consequences

- Upstream drift affecting an enriched printer cannot pass silently.
- Unrelated upstream profile changes do not force every enrichment to change.
- Profile authors edit ordinary typed values rather than patch operations.
- The canonical renderer input is independent of the upstream YAML schema.
- Git history records evidence and review without copying authoring provenance
  into every runtime profile.

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

## DD-025 — Do not emulate incidental firmware quirks by default

**Status:** Accepted

### Context

Low-cost ESC/POS-compatible printers sometimes deviate from Epson behavior in
small, model-specific ways. Examples include thermal after-images, slightly
different barcode dimensions, unexpected HRI behavior, resident glyph shapes,
and different valid QR compaction or mask choices.

Replicating every observed difference would turn the renderer into a collection
of fragile firmware flags. Many observations are difficult to distinguish from
paper, heat, print-head, or configuration effects and do not prevent the PNG
from communicating the receipt's content and layout.

### Decision

Use documented ESC/POS behavior plus profile geometry and capabilities as the
default model. Do not emulate a firmware quirk merely because it creates a
pixel difference.

Add a model-specific behavior only when it is reproducible and materially
affects command parsing, content meaning, positioning, wrapping, feeds, cuts,
sheet boundaries, or another behavior needed by the product. Minor native
symbol size differences, HRI deviations, thermal artifacts, resident glyph
shapes, and standards-valid QR matrix differences may remain documented
approximations when the resulting receipt layout is still useful and correct.

Record observed but unmodeled quirks with the physical test case so the
decision can be revisited if the difference later matters.

### Consequences

- Printer profiles stay focused on behavior that materially improves previews.
- The renderer follows a reviewable protocol baseline instead of reverse
  engineering every firmware revision.
- Material geometry differences remain eligible for typed corrections. For
  example, the calibrated NT-5890K paints `ESC *` 8-dot rows adjacently instead
  of using Epson's three-dot vertical pitch, ignores negative `ESC \`, ignores
  `ESC $` after printable data, ignores `ESC J`, and feeds only the full-cut
  form of `GS V` Function B. It also consumes one LF immediately following
  `GS v 0`; this is modeled because it materially changes vertical placement.
- A preview may not reproduce every dot or native-symbol implementation choice
  even when its positions and overall layout are correct.
- A previously neglected quirk can become modeled after reproducible evidence
  and a concrete fidelity need justify the added complexity.

## Open questions

The following are intentionally not decided yet:

- canonical runtime profile fields for full command coverage and their
  compatibility policy;
- whether bidirectional status commands need a configurable response emulator.
