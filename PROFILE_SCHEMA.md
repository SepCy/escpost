# Printer Profiles

Physical escpos2png profiles import command capabilities and code-page mappings
from `receipt-print-hq/escpos-printer-db`. A small typed enrichment supplies
the geometry, defaults, font metrics, corrections, and fidelity disclosures
needed by the renderer.

The virtual `REFERENCE` profile is self-contained. It supplies every current
capability and supported code-page mapping directly, without posing as an
upstream printer.

The renderer consumes generated canonical JSON. It never resolves upstream
inheritance or parses enrichment TOML while rendering.

## Repository layout

```text
profiles/
├── .escpos-printer-db/      # pinned upstream Git submodule
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

## Data flow

```text
Physical:
  pinned upstream profile ── SHA-256 review guard ─┐
  typed TOML enrichment ───────────────────────────┤
                                                   ▼
Virtual:                                  validated canonical profile
  self-contained typed TOML ───────────────────────┘
```

## Upstream pinning and drift detection

The upstream database is a Git submodule:

```text
profiles/.escpos-printer-db
```

The repository gitlink pins its exact commit. `.gitmodules` records the
repository URL. A second lock file would duplicate those values, so v1 does not
have one.

Each upstream source stores the SHA-256 of its fully resolved upstream profile.
This hash differs by printer and includes inherited values.

For `NT-5890K`, the importer hashes the resolved JSON object from the upstream
`capabilities.json`. `serde_json` stores object keys deterministically in this
project configuration, and the resulting compact UTF-8 JSON bytes are hashed
with SHA-256.

When an upstream update affects only another printer, the enrichment remains
valid. When it changes `NT-5890K` or an inherited ancestor, compilation stops
until the new effective profile is reviewed and its hash is accepted.

## Enrichment format

Each `profiles/<profile-id>/profile.toml` is a typed TOML document:

```toml
schema_version = 1
profile = "NT-5890K"

sources = [
    "upstream:escpos-printer-db/NT-5890K",
    "case:tests/cases/text/ascii-fonts-and-styles",
]

[source]
type = "upstream"
profile_sha256 = "<resolved-profile-sha256>"

[geometry]
printable_width_dots = 384
dpi_x = 203
dpi_y = 203

[motion]
horizontal_units_per_inch = 203
vertical_units_per_inch = 203

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

[[approximations]]
field = "fonts.resident_glyph_shapes"
reason = "Representative glyphs are used instead of printer ROM glyphs"
```

Unknown fields are errors. A misspelled correction therefore cannot silently
enter a profile.

## Enrichment fields

`schema_version` describes the TOML structure. This unreleased format remains
version 1.

`profile` is the canonical profile identifier. For an upstream source it is
also the upstream profile identifier.

`source.type = "upstream"` imports capabilities and code pages from
escpos-printer-db. Its `profile_sha256` is the review guard described above.

`source.type = "reference"` imports nothing. It requires a complete
`[features]`, `[features.barcodes]`, and `[code_pages]` definition so compiler
defaults cannot silently limit the virtual profile.

`sources` contains human-readable evidence references. Sources remain in the
authoring file and Git history; they are not copied into the runtime profile.
Detailed profile-wide physical observations belong in that profile's
`notes.md`; focused command observations may remain in a conformance case's
`notes.md`.

Geometry, motion, defaults, and fonts are complete for the runtime fields
currently needed by the renderer. Font column counts are derived from printable
width and cell width when humans need them; they are not separate runtime
values.

Feature overrides exist only for implemented command handlers. New upstream
capabilities are added to the canonical schema with the renderer behavior that
consumes them. A reference profile must explicitly fill every current feature
instead of overriding an imported baseline.

The upstream database represents each `GS k` Function A/B family with one
boolean. The canonical profile stores exact barcode systems. An upstream true
value expands only to the established legacy systems; model-dependent systems
require explicit enrichment evidence.

## Approximations

An approximation names a fidelity boundary and explains it:

```toml
[[approximations]]
field = "fonts.resident_glyph_shapes"
reason = "Exact printer ROM glyphs are unavailable"
```

The canonical profile retains approximations so every render can disclose them
directly. Approximation fields are included in the canonical hash.

## Validation

Compilation rejects:

- an unsupported enrichment schema version;
- unknown enrichment fields;
- an unknown upstream profile;
- a stale resolved-profile hash;
- an incomplete self-contained reference profile;
- local code-page replacements on an upstream source;
- zero geometry, motion, or font dimensions;
- a baseline outside its font cell;
- a default code-page slot absent from the selected source;
- a default international set the renderer does not implement;
- a barcode system without a command number in the selected Function A/B
  framing; and
- invalid canonical JSON or a canonical hash mismatch.

Validation grows with implemented behavior. The schema does not reserve fields
for future commands.

## Canonical profile pack

Compilation produces deterministic JSON containing:

```text
schema version
profile id
runtime geometry, motion, defaults, and fonts
implemented capabilities
code-page mappings
approximations
typed source and upstream SHA-256 when applicable
canonical-profile SHA-256
```

The canonical hash covers every field that can affect rendering or its fidelity
disclosure, except the hash field itself. It is the exact profile identity used
in render metadata and cache keys.

The generated pack is committed because Python wheels embed it directly. Tests
regenerate the reviewed profiles and require them to equal the committed pack.

Regenerate it with:

```bash
docker compose run --rm test cargo run --quiet \
  -p escpos2png-profiles --bin compile-profile-pack -- \
  profiles/.escpos-printer-db/dist/capabilities.json \
  profiles profiles/.generated/profiles.json
```

## REFERENCE profile

`profiles/REFERENCE/profile.toml` is a virtual standards baseline for previews,
automated tests, and integrations that must not inherit a physical printer's
missing features. It enables every capability represented by the current
canonical schema.

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
3. If the resolved-profile hash changed, inspect the effective upstream
   profile and inherited ancestors.
4. Update corrections or capabilities when the reviewed behavior changed.
5. Replace `source.profile_sha256` with the accepted resolved hash.
6. Regenerate the canonical pack.
7. Review its canonical hash and run the full test suite.

There is no manual profile revision, enrichment hash, automatic
added/confirmed/corrected report, or duplicate upstream lock in v1.
