# Printer Profile Enrichments

escpos2png imports the community-maintained ESC/POS printer database and adds
only the information required for faithful emulation. This document defines
the initial authoring and compilation protocol for those additions and
corrections.

The protocol is deliberately small. It provides reproducibility, validation,
upstream-drift detection, and provenance without introducing a generic patch
language or a separate evidence database.

## Data layers

```text
Pinned upstream database
        │
        ▼
Resolved upstream profile
        │
        ├── typed TOML enrichment
        ▼
Canonical profile
        │
        ├── validation report
        └── canonical SHA-256
```

The Rust renderer consumes only the resolved canonical profile. It does not
interpret TOML overlays or upstream inheritance while rendering.

## Repository layout

The intended source layout is:

```text
profiles/
├── upstream.lock.toml
├── upstream/
│   └── escpos-printer-db/
├── enrichments/
│   └── NT-5890K.toml
└── generated/
    └── profiles.json
```

The exact generated-artifact location may change with the Rust workspace
layout. The separation between source data, enrichments, and generated output
does not.

## Upstream lock

One repository-level lock identifies the complete upstream snapshot:

```toml
schema_version = 1
repository = "https://github.com/receipt-print-hq/escpos-printer-db.git"
commit = "e3bf6056ee75cf70ffaccb925081fffa7ad6ced5"
license = "CC-BY-4.0"
```

The Git commit applies to every imported printer. Updating it is an explicit,
reviewable dependency change.

The build does not fetch the repository. It verifies that the local upstream
checkout matches the lock.

## Profile-specific upstream hash

Each enrichment records the SHA-256 of its one fully resolved upstream
profile. This value differs by printer.

For `NT-5890K`, the importer first resolves its inheritance from `POS-5890`
and any further ancestors. It then hashes the effective profile rather than
the original YAML bytes.

The initial normalization algorithm is:

1. Resolve upstream inheritance using the pinned database's semantics.
2. Preserve the resulting values, including explicit `Unknown` strings.
3. Serialize the resolved profile as UTF-8 JSON.
4. Sort every object by key recursively.
5. Emit no insignificant whitespace or trailing newline.
6. Calculate SHA-256 over those exact bytes.

The normalization algorithm is part of the enrichment schema and requires test
fixtures. It cannot change silently.

The global Git commit and per-profile SHA-256 serve different purposes:

- the commit identifies the complete database snapshot;
- the resolved hash detects changes relevant to one enriched printer.

An upstream update that changes only an unrelated profile therefore does not
invalidate `NT-5890K`. A change to `NT-5890K`, `POS-5890`, or another ancestor
changes its resolved hash and requires review.

## Enrichment document

Enrichments are partial, typed TOML documents:

```toml
schema_version = 1
profile = "NT-5890K"
revision = 1
upstream_profile_sha256 = "<resolved-profile-sha256>"

sources = [
    "upstream:escpos-printer-db/NT-5890K",
    "case:tests/cases/geometry/printable-width",
    "case:tests/cases/text/default-font",
]

[geometry]
printable_width_dots = 384
dpi_x = 203
dpi_y = 203

[fonts.a]
columns = 32

[fonts.b]
columns = 42

[[approximations]]
field = "fonts.resident_glyph_shapes"
reason = "Representative glyphs are used instead of printer ROM glyphs"
```

The document follows escpos2png's typed canonical structure, not the raw
upstream YAML structure. This insulates rendering behavior from upstream
schema changes.

Only fields needed by the current implementation should be introduced. The
schema grows vertically with implemented behavior rather than attempting to
describe the full ESC/POS command set before the first render works.

## Automatic change classification

Authors do not label values as additions, confirmations, or corrections. The
compiler compares each overlay value with the canonical draft imported from
upstream:

| Upstream state | Overlay state | Classification |
|---|---|---|
| Missing or `Unknown` | Value | Added |
| Same value | Same value | Confirmed |
| Different value | Value | Corrected |

The compiler emits this classification in its validation report. The overlay
itself remains a straightforward typed document.

An upstream-profile hash mismatch stops compilation before applying the
overlay. A review command should show the old and new resolved profiles,
classifications, and proposed new hash.

## Sources and physical evidence

Version 1 uses simple source references rather than separate evidence files or
per-field provenance wrappers.

Useful source forms include:

```text
upstream:escpos-printer-db/NT-5890K
manual:<document-id>#<section-or-page>
case:tests/cases/<case-directory>
observation:<short-identifier>
```

Detailed physical evidence belongs in the conformance case's `notes.md` and
hardware-validation report as defined by `TESTING.md`. The input hash in the
case manifest binds that evidence to exact ESC/POS bytes.

A source reference can support a group of related profile values. Field-level
evidence may be added in a future schema if real review ambiguity demonstrates
the need.

## Approximations

Known approximations are explicit:

```toml
[[approximations]]
field = "fonts.resident_glyph_shapes"
reason = "Exact Netum ROM glyph bitmaps are unavailable"
source = "decision:DD-007"
```

An approximation identifies a canonical field or field group and explains the
fidelity boundary. It is not assigned a numeric confidence score.

The generated profile retains applicable approximations so the renderer can
produce honest completeness metadata or diagnostics.

## Validation

The compiler rejects:

- an upstream checkout that does not match the global commit;
- a profile that does not exist upstream;
- a resolved upstream hash mismatch;
- unknown TOML fields;
- invalid types or units;
- non-positive DPI, dimensions, or character counts;
- geometry inconsistent with the selected color or surface model;
- references to conformance cases that do not exist;
- approximation paths that do not identify canonical fields; and
- a canonical profile that violates renderer invariants.

Unknown fields are errors rather than ignored forward compatibility. A newer
schema version must be selected explicitly when new syntax is needed.

Validation rules should be added as actual implemented behavior requires them.
Speculative restrictions must not block legitimate printer variants.

## Generated canonical profile

Compilation produces deterministic JSON suitable for embedding in Rust. A
generated profile contains:

```text
identity and revision
resolved geometry and defaults
font metrics and code-page mappings
capabilities and quirks
known approximations
upstream Git commit
resolved upstream profile SHA-256
enrichment SHA-256
canonical profile SHA-256
```

The enrichment and canonical hashes are generated automatically. Developers
maintain only the upstream lock, expected resolved-profile hash, enrichment
values, revision, and source references.

The canonical hash covers behaviorally relevant generated content. Metadata
that would make identical builds differ, such as build timestamps or absolute
paths, is prohibited.

## Versioning

`schema_version` identifies how to interpret the enrichment document.

`revision` is the human-facing version of one printer profile. Increment it
when generated rendering behavior can change, including when:

- a canonical value is added or corrected;
- an upstream update changes effective imported behavior;
- a capability or model-specific quirk changes; or
- an approximation becomes exact in a way that changes output.

Source wording, additional evidence, or notes that do not change the canonical
rendering input do not require a revision increment.

The canonical hash is the final machine-verifiable identity. Until automated
revision-policy enforcement proves necessary, review and tests enforce the
human revision rule.

## Deferred complexity

Version 1 deliberately does not include:

- a generic JSON Patch or dotted-path mutation language;
- explicit `add`, `confirm`, or `replace` operations;
- separate evidence documents;
- per-field evidence objects;
- numeric confidence scores;
- arbitrary runtime mutation of built-in profiles;
- complex firmware-variant inheritance; or
- a custom binary profile-pack format.

These features should be introduced only in response to demonstrated profile
maintenance needs.
