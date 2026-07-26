# Printer Profile Enrichments

escpos2png imports command capabilities and code-page mappings from
`receipt-print-hq/escpos-printer-db`. A small typed enrichment supplies the
geometry, defaults, font metrics, corrections, and fidelity disclosures needed
by the renderer.

The renderer consumes generated canonical JSON. It never resolves upstream
inheritance or parses enrichment TOML while rendering.

## Data flow

```text
Pinned upstream Git submodule
        │
        ▼
Resolved upstream profile ── SHA-256 review guard
        │
        ├── typed TOML enrichment
        ▼
Validated canonical profile ── canonical SHA-256
```

## Upstream pinning and drift detection

The upstream database is a Git submodule:

```text
profiles/upstream/escpos-printer-db
```

The repository gitlink pins its exact commit. `.gitmodules` records the
repository URL. A second lock file would duplicate those values, so v1 does not
have one.

Each enrichment stores the SHA-256 of its fully resolved upstream profile. This
hash differs by printer and includes inherited values.

For `NT-5890K`, the importer hashes the resolved JSON object from the upstream
`capabilities.json`. `serde_json` stores object keys deterministically in this
project configuration, and the resulting compact UTF-8 JSON bytes are hashed
with SHA-256.

When an upstream update affects only another printer, the enrichment remains
valid. When it changes `NT-5890K` or an inherited ancestor, compilation stops
until the new effective profile is reviewed and its hash is accepted.

## Enrichment format

Enrichments are typed TOML documents:

```toml
schema_version = 1
profile = "NT-5890K"
upstream_profile_sha256 = "<resolved-profile-sha256>"

sources = [
    "upstream:escpos-printer-db/NT-5890K",
    "case:tests/cases/text/ascii-fonts-and-styles",
]

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

`profile` is the shared upstream profile identifier.

`upstream_profile_sha256` is the review guard described above.

`sources` contains human-readable evidence references. Sources remain in the
authoring file and Git history; they are not copied into the runtime profile.
Detailed manual citations and physical observations belong in the applicable
case's `notes.md`.

Geometry, motion, defaults, and fonts are complete for the runtime fields
currently needed by the renderer. Font column counts are derived from printable
width and cell width when humans need them; they are not separate runtime
values.

Feature overrides exist only for implemented command handlers. New upstream
capabilities are added to the canonical schema with the renderer behavior that
consumes them.

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
- zero geometry, motion, or font dimensions;
- a baseline outside its font cell;
- a default code-page slot absent from the upstream profile;
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
resolved upstream-profile SHA-256
canonical-profile SHA-256
```

The canonical hash covers every field that can affect rendering or its fidelity
disclosure, except the hash field itself. It is the exact profile identity used
in render metadata and cache keys.

The generated pack is committed because Python wheels embed it directly. Tests
regenerate the reviewed profile and require it to equal the committed pack.

Regenerate it with:

```bash
docker compose run --rm test cargo run --quiet \
  -p escpos2png-profiles --bin compile-profile-pack -- \
  profiles/upstream/escpos-printer-db/dist/capabilities.json \
  profiles/enrichments profiles/generated/profiles.json
```

## Updating an upstream profile

1. Update the upstream submodule deliberately.
2. Run profile compilation.
3. If the resolved-profile hash changed, inspect the effective upstream
   profile and inherited ancestors.
4. Update corrections or capabilities when the reviewed behavior changed.
5. Replace `upstream_profile_sha256` with the accepted resolved hash.
6. Regenerate the canonical pack.
7. Review its canonical hash and run the full test suite.

There is no manual profile revision, enrichment hash, automatic
added/confirmed/corrected report, or duplicate upstream lock in v1.
