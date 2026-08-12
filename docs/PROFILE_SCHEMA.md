# Printer Profiles

A printer profile states two unlike things: intrinsic physical facts
(*descriptors*) and confirmed departures from documented ESC/POS behavior
(*deviations*) (DD-031). Physical ESCPost profiles import capabilities and
code-page mappings from `receipt-print-hq/escpos-printer-db` and layer a
small typed enrichment on top to state the descriptors and deviations the
renderer needs. Every parameter, imported or enriched, is optional.

The virtual `REFERENCE` profile is self-contained. It supplies every current
capability and supported code-page mapping directly, without posing as an
upstream printer, and turns on no deviations — it is the zero-deviation
baseline.

The renderer consumes generated canonical JSON. It never resolves upstream
data or parses enrichment TOML while rendering.

## Repository layout

```text
crates/escpost-profiles/profiles/
├── .escpos-printer-db/      # upstream Git submodule
├── .generated/
│   └── profiles.json        # canonical runtime pack
└── <profile-id>/
    ├── profile.toml         # typed renderer enrichment
    ├── expected-001.png     # physical profiles: calibration output
    ├── verification.toml    # physical profiles: last paper comparison
    ├── notes.md             # physical evidence and fidelity context
    └── TODO.md              # optional deferred hardware work
```

The compiler discovers visible directories containing `profile.toml`.
Non-profile infrastructure starts with a dot and is never mistaken for a
printer profile.

An upstream identifier with no hand-authored directory here is not
undocumented — it is synthesized at compile time (DD-032). See
[Width-gated synthesis](#width-gated-synthesis-dd-032).

## Data flow

```text
Curated:
  upstream profile + typed TOML enrichment ────────┐
                                                   ▼
Synthesized:                              validated canonical profile
  upstream profile ── derivable width ─────────────┤
                                                    │
Virtual:                                           │
  self-contained typed TOML ───────────────────────┘
```

Curated (`compile_profile`) and synthesized (`synthesize_profile`) profiles
share the same layered fill; a synthesized profile simply has no enrichment
to draw its explicit layer from.

## Upstream source

The upstream database is a Git submodule:

```text
crates/escpost-profiles/profiles/.escpos-printer-db
```

The repository gitlink selects its exact commit and `.gitmodules` records the
repository URL. Profile compilation consumes the selected data directly;
profiles do not repeat or approve it through per-profile input hashes.
An upstream update therefore flows into curated and synthesized profiles the
next time the runtime pack is generated.

## Descriptors and deviations (DD-031)

**Descriptors** are intrinsic physical facts with no ESC/POS norm: printable
width, horizontal and vertical DPI, motion units, font cell metrics, cutter
distance, capabilities, and the code-page map. They are sourced from the
shared upstream database or measured from the device.

**Deviations** are departures from the documented ESC/POS baseline: the six
`commands.*` behaviors (`esc_backslash_negative`,
`esc_dollar_after_printable_data`, `esc_j`, `gs_v_0_following_lf`,
`gs_v_function_b_full`, `gs_v_function_b_partial`),
`column_bit_image.eight_dot_vertical_pitch_dots`, `defaults.carriage_return`,
and the power-on `defaults` line spacing, active code-page slot, and
international character set. Each deviation has a conformant baseline; a
profile turns one on only once it has confirmed the departure.

Both kinds share one axis, not a provenance ladder. An omitted parameter is
**assumed** — it takes its default value, or stays conformant. A stated
parameter is **known** — a measured or sourced value, or a confirmed
deviation. Stating a parameter is itself the confirmation; there is no
separate confidence level and no per-field disclosure record.

`REFERENCE` states virtual descriptors and enables every capability while
turning on no deviations. A physical profile states the descriptors it knows
and turns on the deviations it has verified — the profile is its own
calibration checklist.

There is no profile inheritance. The base an omitted parameter falls back to
is a set of default values, not an ancestor profile, so a profile stays flat,
local, and content-addressed, unlike upstream's `inherits:` graph, which is
flattened before ESCPost ever sees it.

Renderer-wide fidelity limits — representative glyphs, QR mask choice,
unmodeled thermal artifacts — are not profile parameters. They are documented
divergences that belong to the standing fidelity contract (DD-002, DD-007,
DD-023, DD-024, DD-025), and an observed but unmodeled per-printer quirk is
recorded with its physical test case rather than as a profile field.

## Layered fill

Compilation fills every omitted parameter through the same three layers, in
order:

1. **Explicit** — the value stated in the profile's own `profile.toml`.
   Curated profiles only; a synthesized profile has no enrichment.
2. **Upstream-derived** — for an upstream source, a value computed from the
   resolved upstream entry: its media width and DPI, or a font's cell width
   derived from the upstream column count divided into the resolved
   printable width.
3. **Documented constant** — a fixed default every profile falls back to once
   the layers above have nothing to offer: 58 mm width at 203 DPI, the Font
   A/B cell geometry, conformant command behavior, and the power-on defaults.

`compile_profile` (curated enrichment) and `synthesize_profile` (upstream
entries with no enrichment) call the identical fill function; only the
explicit layer differs between them.

## Width and the authoritative dot unit

The default paper width is the smaller common thermal size, 58 mm, expressed
as 384 dots at 203 DPI. It fails safe: an uncalibrated profile wraps content
early rather than overrunning a narrower physical sheet than assumed.

`printable_width_dots` — not a millimeter paper size — is the authoritative
geometry unit, because printable dots are not paper millimeters times DPI.
Print heads leave inactive margins that a paper-size label does not capture:
`TM-T88III` ships 80 mm paper at its upstream-reported 180 DPI, which a naive
mm × DPI conversion would predict as ≈567 dots (80 mm × 180 DPI ÷ 25.4). Its
actual upstream-reported printable width is only 512 dots (≈72 mm at 180
DPI) — the gap is head margin the paper-size number does not capture.
Consuming `printable_width_dots` directly is required precision, not a
simplification, and it is why the geometry table carries dots rather than
millimeters.

## `dpi` versus `motion`

`[geometry] dpi_x`/`dpi_y` and `[motion] horizontal_units_per_inch`/
`vertical_units_per_inch` are distinct fields, even though an unenriched
profile fills both from the same upstream number:

- **`dpi`** is the raster resolution: dots per inch the print head can
  physically place. It is fixed for a profile and drives pixel geometry —
  font cell size, `GS v 0` raster scaling, QR module size.
- **`motion`** is the granularity a command's numeric argument moves the
  cursor by. It starts at the profile's stated value but is mutable at
  render time by `GS P` (set horizontal/vertical motion unit), independent
  of the fixed `dpi`. The renderer converts a motion-unit argument to dots
  by scaling through the ratio of `dpi` to the current motion units, so a
  `GS P` change never rescales already-placed content.

An omitted `motion` table is filled from the resolved `dpi`, since most
printers ship with motion units equal to their raster DPI — but that is a
default fill, not an identity the two fields are required to share.

## Width-gated synthesis (DD-032)

Requiring a hand-authored enrichment before an upstream identifier is usable
would mean adopting a printer starts from nothing at all. Instead, every
upstream entry whose printable width is derivable — from its upstream media
pixels, since a human-authored enrichment may still omit width and accept
the 58 mm default — is synthesized into the pack automatically, filling every
descriptor from upstream where present and otherwise from the documented
constants, and leaving every deviation conformant.

The default capability posture stays conservative: a synthesized profile
never claims a cut capability, because cutter distance has no upstream
source and no documented constant, so nothing backs the descriptor a cut
capability would need.

Width is never fabricated for a real-named printer. The generic `default`,
`safe`, and `simple` upstream templates state no media width; they produce
no profile and are logged as skipped rather than silently omitted or given
an invented width. `REFERENCE` remains the choice when no specific printer
is known.

All curated and synthesized profiles resolve at build time into the single
canonical pack.

## Enrichment format

Each `crates/escpost-profiles/profiles/<profile-id>/profile.toml` is a typed TOML document:

```toml
schema_version = 1
profile = "NT-5890K"

sources = [
    "upstream:escpos-printer-db/NT-5890K",
    "case:tests/cases/text/ascii-fonts-and-styles",
]

[source]
type = "upstream"

[geometry]
printable_width_dots = 384
dpi_x = 203
dpi_y = 203

[motion]
horizontal_units_per_inch = 203
vertical_units_per_inch = 203

[column_bit_image]
eight_dot_vertical_pitch_dots = 1

[commands]
esc_backslash_negative = "ignored"
esc_dollar_after_printable_data = "ignored"
esc_j = "ignored"
gs_v_0_following_lf = "ignored"
gs_v_function_b_full = "feed"
gs_v_function_b_partial = "ignored"

[defaults]
line_spacing_dots = 30
code_page = 0
international_character_set = 0
carriage_return = "ignored"

[fonts.a]
cell_width_dots = 12
cell_height_dots = 24
baseline_dots = 20

[fonts.b]
cell_width_dots = 9
cell_height_dots = 17
baseline_dots = 14

[features]
qr_code = true

[features.barcodes]
function_a = [
    "upc_a",
    "upc_e",
    "ean_13",
    "ean_8",
    "code_39",
    "itf",
    "codabar",
]
function_b = [
    "upc_a",
    "upc_e",
    "ean_13",
    "ean_8",
    "code_39",
    "itf",
    "codabar",
    "code_93",
    "code_128",
]
```

Every table after `[source]` is optional; the layered fill in the previous
section supplies whatever is omitted. Unknown fields are errors, so a
misspelled correction cannot silently enter a profile.

A profile that advertises a paper-cut capability must add a `[cutter]` table
stating `print_head_to_cutter_dots`; a profile with no autocutter omits it.

## Enrichment fields

`schema_version` describes the TOML structure. This unreleased format remains
version 1.

`profile` is the canonical profile identifier. For an upstream source it is
also the upstream profile identifier.

`source.type = "upstream"` imports capabilities and code pages from
escpos-printer-db.
`source.type = "upstream_default"` is not an authorable enrichment value — it
identifies a synthesized profile and appears only in the compiled canonical
JSON.

`source.type = "reference"` imports nothing. It requires a complete
`[features]`, `[features.barcodes]`, and `[code_pages]` definition so compiler
defaults cannot silently limit the virtual profile.

`sources` contains human-readable evidence references. Sources remain in the
authoring file and Git history; they are not copied into the runtime profile.
Detailed profile-wide physical observations belong in that profile's
`notes.md`; focused command observations may remain in a conformance case's
`notes.md`.

`geometry`, `cutter`, `motion`, `column_bit_image`, `commands`, `defaults`,
and `fonts` are each entirely optional descriptor or deviation tables; an
omitted table is filled per [Layered fill](#layered-fill). Font column counts
are derived from printable width and cell width when humans need them; they
are not separate runtime values.

Feature overrides exist only for implemented command handlers. New upstream
capabilities are added to the canonical schema with the renderer behavior that
consumes them. A reference profile must explicitly fill every current feature
instead of overriding an imported baseline.

`vendor` and `model` in the enrichment are optional overrides for catalog
metadata — the manufacturer and model name, e.g. `Epson` and `TM-T88III` —
for a future `escpost profiles` command to display and search. On the
compiled profile both are required: `model` resolves as enrichment `model` →
upstream `name` → the profile id itself, so it always resolves; `vendor`
resolves as enrichment `vendor` → upstream `vendor`, with no further
fallback, so compilation rejects a `reference`-source profile that states
neither — an upstream source always supplies one, so `REFERENCE` states its
own `vendor = "ESCPost"` and `model = "Reference"` directly in its
enrichment. Both fields are display-only and never affect rendering, but are
still included in the canonical hash like every other canonical field — kept
simple, with no special exclusion (DD-031, DD-032).

The upstream database represents each `GS k` Function A/B family with one
boolean. The canonical profile stores exact barcode systems. An upstream true
value expands only to the established legacy systems; model-dependent systems
require explicit enrichment evidence.

## The `source` marker

The compiled profile's `source` field is one of three tagged variants:

- **`Reference`** — the self-contained virtual baseline; nothing is imported.
- **`Upstream`** — a curated enrichment layered onto an upstream entry.
- **`UpstreamDefault`** — a profile synthesized from an upstream entry with no
  enrichment (DD-032).

`source` is the profile-level signal for calibrated-versus-synthesized: an
`Upstream` profile has been reviewed and its stated deviations confirmed; an
`UpstreamDefault` profile rests on default descriptors and a fully conformant
deviation set that nobody has verified against the device. There is no
per-field disclosure list — the marker alone tells a caller whether to trust
the profile as calibrated.

## Validation

Compilation rejects:

- an unsupported enrichment schema version;
- unknown enrichment fields;
- an enrichment whose `source.type` is `upstream_default` (that source is
  produced only by synthesis, never authored);
- an unknown upstream profile;
- an incomplete self-contained reference profile;
- local code-page replacements on an upstream source;
- zero geometry, motion, or font dimensions;
- a baseline outside its font cell;
- a paper-cut capability without a stated `[cutter]` table;
- a default code-page slot absent from the selected source;
- a default international set the renderer does not implement;
- a barcode system without a command number in the selected Function A/B
  framing;
- a profile with no resolvable `vendor` (an upstream source always supplies
  one; only a `reference`-source profile that omits it can fail this); and
- invalid canonical JSON or a canonical hash mismatch.

Validation grows with implemented behavior. The schema does not reserve fields
for future commands.

## Canonical profile pack

Compilation produces deterministic JSON containing:

```text
schema version
profile id
typed source
runtime geometry, cutter, motion, column-image, commands, defaults, and fonts
implemented capabilities
code-page mappings
vendor and model catalog metadata
canonical-profile SHA-256
```

The canonical hash covers every field except the hash field itself, including
`vendor`/`model`, which are display-only catalog metadata and never affect
rendering — kept simple, with no special exclusion. It is the exact profile
identity used in render metadata and cache keys.

The generated pack is committed because runtime consumers embed it directly.
Regeneration is an explicit release and development step; Git records changes
to both its inputs and output.

Regenerate it with:

```bash
docker compose run --rm test cargo run --quiet \
  -p escpost-profiles --bin compile-profile-pack -- \
  crates/escpost-profiles/profiles/.escpos-printer-db/dist/capabilities.json \
  crates/escpost-profiles/profiles \
  crates/escpost-profiles/profiles/.generated/profiles.json
```

## REFERENCE profile

`crates/escpost-profiles/profiles/REFERENCE/profile.toml` is a virtual standards baseline for previews,
automated tests, and integrations that must not inherit a physical printer's
missing features. It enables every capability represented by the current
canonical schema and turns on no deviations — the zero-deviation baseline
described in [Descriptors and deviations](#descriptors-and-deviations-dd-031).

ESC/POS does not define paper width, DPI, font ROM, or cutter placement, so a
render still needs explicit geometry. REFERENCE selects deterministic 203 DPI,
576-dot paper and an 80-dot print-head-to-cutter distance. These are virtual
parameters, not claims about every compliant printer.

REFERENCE does not bypass parser coverage. Commands identified as post-v1 in
`COMMAND_COVERAGE.md` remain unsupported until their handlers are implemented.
When the canonical feature schema grows, REFERENCE must explicitly adopt the
new capability.

## Updating an upstream profile

1. Update the upstream submodule deliberately.
2. Run profile compilation.
3. Update corrections or capabilities when needed.
4. Regenerate the canonical pack.
5. Review the generated diff and run the full test suite.

There is no manual profile revision, enrichment hash, automatic
added/confirmed/corrected report, or duplicate upstream lock in v1.
