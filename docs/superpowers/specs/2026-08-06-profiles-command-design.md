# `escpost profiles` command — design

## Goal

Give CLI users a way to **discover a supported printer profile**: browse the
catalog, inspect one profile in detail, and interactively pick one for
scripting. Honest about which profiles are hardware-calibrated versus
synthesized from upstream defaults.

Scope is the CLI only; it reads the existing embedded profile pack through
`escpost_profiles::resolver`. No renderer changes.

## Prerequisite: import nominal paper width

The catalog wants to show paper size in millimetres, but the canonical profile
currently stores only `printable_width_dots` + `dpi`. Import upstream
`media.width.mm` as a **required, fixed-point** field
`paper_width_tenths_mm: u32` on `PrinterProfile` (tenths of a mm — `575` = 57.5
mm), following the `vendor`/`model` pattern. Upstream mm is fractional for 9/32
entries (NT-5890K is 57.5), so fixed-point tenths keeps the value **lossless**
while keeping the canonical profile all-integer (the byte-exact drift guard
stays stable — no floats in canonical JSON).

- Enrichment authors write `paper_width_mm` as a decimal (`Option<f64>`, e.g.
  `80.0`); upstream mm and the enrichment value convert to tenths via
  `round(mm × 10)`. Fill order `enrichment ?? upstream ?? error`.
- Included in the canonical hash.
- `profiles/REFERENCE/profile.toml` sets `paper_width_mm = 80.0` (virtual).
- Regenerate `profiles/.generated/profiles.json` (drift test enforces it).

Verified safe as required: every entry with a numeric `media.width.pixels` also
has a numeric `media.width.mm` (the only `Unknown` mm belong to the widthless
generics we skip).

Neither width in mm is a rendering input — both are display metadata. The CLI
presents `paper_width_mm` as a decimal (`tenths ÷ 10`). `printable_width_mm` is
**not stored** — it is derived at display time as
`printable_width_dots ÷ dpi_x × 25.4` and shown as a decimal.

## Calibration vocabulary (the honesty signal)

Every listing marks how a profile's physical fidelity was obtained, mapped from
`ProfileSource`:

| Source            | Label         | Marker | Meaning                                                        |
|-------------------|---------------|--------|----------------------------------------------------------------|
| `Upstream`        | `calibrated`  | `✓`    | Hash-pinned, enrichment measured against real hardware.        |
| `UpstreamDefault` | `synthesized` | `~`    | Real capabilities/width from upstream; physical metrics default.|
| `Reference`       | `virtual`     | `○`    | Idealized demo/testing baseline; not a real printer.           |

## Command surface

A `profiles` clap subcommand (mirroring the existing `printers` sub-subcommand
pattern) with three sub-subcommands. Implementation lives in a new
`crates/escpost-cli/src/profiles_cmd.rs`; the existing `profiles.rs` stays the
thin resolver wrapper. All three read `escpost_profiles::resolver`
(`available_ids()` / `resolve(id)`).

### `profiles list`

Human table (default) or `--json`:

```
PROFILE      VENDOR   MODEL       CAL  PAPER  PRINT  DOTS  DPI  CUT  BC   QR
NT-5890K     Netum    NT-5890K    ✓    58     48     384   203  –    A·B  ✓
TM-T88III    Epson    TM-T88III   ~    80     72     512   180  ✓    A·B  ✓
REFERENCE    ESCPost  Reference   ○    80     72     576   203  ✓    A·B  ✓

CAL: ✓ calibrated · ~ synthesized · ○ virtual   PAPER/PRINT mm, DOTS printable
```

- Columns: `PROFILE` (the id passed to `--profile`), `VENDOR`, `MODEL`, `CAL`,
  `PAPER` (mm, stored), `PRINT` (mm, derived), `DOTS` (printable, stored),
  `DPI` (stored `dpi_x`), then compact capability flags `CUT`, `BC` (barcode:
  `A·B` / `A` / `B` / `–`), `QR`.
- Rows sorted by id.
- Filters compose with AND:
  - `--vendor <name>` — case-insensitive substring match on vendor.
  - `--source calibrated|synthesized|virtual`.
  - `--search <substr>` — case-insensitive substring over id, vendor, model.
- No matches → exit 0, message "no profiles match" on stderr.
- No `--supports` capability filter in v1 (YAGNI; `--search`/`--vendor` cover
  real discovery).

### `profiles show <id>`

Detailed single-profile view (human or `--json`):

- Identity: id, vendor, model.
- Provenance: source label + `✓/~/○`, `canonical_profile_sha256`.
- Geometry: `paper_width_mm`, printable mm (derived), `printable_width_dots`,
  `dpi_x`/`dpi_y`.
- Fonts A and B: `cell_width_dots × cell_height_dots`, baseline.
- Code pages: count.
- Features: barcode systems (Function A / Function B lists), graphics, cut
  (full/partial), QR, drawer pulse.

Unknown id → nonzero exit via `CliError::UnknownProfile`.

### `profiles find`

Interactive substring picker built on `inquire::Select` (already a dependency,
already used by the profile prompt at `profiles.rs:30`):

- Options are labels `"<id> — <vendor> · <model>"`, id-sorted, so typing filters
  on id **or** vendor **or** model.
- `.with_page_size(10)`; the default case-insensitive substring filter shows the
  top matches as the user types; Enter selects one.
- Prints the selected **id** to stdout (nothing else), for
  `escpost render … --profile "$(escpost profiles find)"`.
- Honors the global `--non-interactive` flag and a non-TTY stdin: errors with a
  nonzero exit pointing at `profiles list --search`.
- Relevance-ranked fuzzy ordering is out of scope for v1 (substring is enough
  for a ~32-entry catalog).

## JSON output (`--json`)

Deterministic, `snake_case`. Both `list` (an array) and `show` (a single
object) emit the **same full object shape** below; the human `list` table is a
compact projection of it.

```json
{
  "id": "TM-T88III",
  "vendor": "Epson",
  "model": "TM-T88III",
  "source": "synthesized",
  "paper_width_mm": 80.0,
  "printable_width_mm": 72.2,
  "printable_width_dots": 512,
  "dpi_x": 180,
  "dpi_y": 180,
  "fonts": {
    "a": { "cell_width_dots": 12, "cell_height_dots": 24, "baseline_dots": 20 },
    "b": { "cell_width_dots": 9,  "cell_height_dots": 17, "baseline_dots": 14 }
  },
  "features": {
    "barcodes": { "function_a": ["upc_a", "…"], "function_b": ["code_128", "…"] },
    "graphics": true,
    "paper_full_cut": true,
    "paper_part_cut": true,
    "qr_code": true,
    "pulse_standard": true
  },
  "code_page_count": 12,
  "canonical_profile_sha256": "…"
}
```

`paper_width_mm` and `printable_width_mm` are decimals (one place); the compact
`list` table rounds them to whole mm, while `show`/`--json` keep the decimal.
Barcode systems use the canonical snake_case names (`upc_a`, `code_128`, …).

## Errors & exit codes

- Reuse `CliError`. Add one variant for `find` in a non-interactive/non-TTY
  context (message directs the user to `profiles list --search`).
- `show <unknown>` → `CliError::UnknownProfile`, nonzero exit.
- `list` with zero matches → exit 0 (an empty result is not an error).

## Testing

CLI integration tests (the crate already has this style):

- `list` default output includes known ids and the calibration markers.
- Each filter (`--vendor`, `--source`, `--search`) narrows as expected; unknown
  filter value for `--source` is a clap parse error.
- `list --json` parses and carries the documented fields; `list` with a
  no-match filter exits 0 with the stderr note.
- `show <known>` renders the detail; `show <known> --json` matches the schema;
  `show <unknown>` exits nonzero.
- `find` with `--non-interactive` (or non-TTY) exits nonzero with the guidance
  message.
- The interactive picker UI itself is not unit-tested; its data source (the
  id-sorted label list) is exercised via `list`.

## Non-goals (v1)

- `profiles diff <a> <b>`.
- `--supports` capability filter.
- Relevance-ranked fuzzy `find`.
- Vendor-based grouping/sectioning in `list` output.
