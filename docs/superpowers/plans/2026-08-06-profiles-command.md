# `escpost profiles` Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an `escpost profiles` subcommand (`list` / `show` / `find`) to discover supported printer profiles from the embedded pack, honest about calibrated-vs-synthesized.

**Architecture:** A new `crates/escpost-cli/src/profiles_cmd.rs` module holds a `ProfileView` (built from `&PrinterProfile`) plus table/detail/JSON rendering; three thin clap sub-subcommands call it, mirroring the existing `printers` subcommand. All read `escpost_profiles::resolver`. A prerequisite adds a required `paper_width_mm` to the profile (same pattern as `vendor`/`model`).

**Tech Stack:** Rust; `clap` (derive), `inquire` 0.9.4 (already deps), `serde`/`serde_json`; `escpost-profiles`.

## Global Constraints

- Design authority: `docs/superpowers/specs/2026-08-06-profiles-command-design.md`.
- **Run the entire toolchain through docker compose — never host `cargo`/`python3`.** Wrap every `cargo …` as:
  `docker compose -f /home/lars/projects/duala-digital/escpost/compose.yaml --project-directory /home/lars/projects/duala-digital/escpost run --rm -w /workspace/.claude/worktrees/profiles-command test cargo …`
  (`git` runs on the host from the worktree dir.)
- Reuse `escpost_profiles::resolver` (`available_ids()`, `resolve(id)`); do not embed the pack again.
- Reuse `CliError`; add exactly one variant for non-interactive `find`.
- Mirror the existing `Printers` clap sub-subcommand structure (see `crates/escpost-cli/src/cli.rs` and `printers.rs`).
- Calibration labels: `ProfileSource::Upstream → "calibrated"`, `UpstreamDefault → "synthesized"`, `Reference → "virtual"`.
- `printable_width_mm = round(printable_width_dots / dpi_x * 25.4)` (integer). Authoritative stored width stays `printable_width_dots`.
- Commit after each task (`feat`/`test`/`docs`), body ending `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

## File Structure

- `crates/escpost-profiles/src/lib.rs` — add `paper_width_mm` (Task 1).
- `profiles/REFERENCE/profile.toml` — `paper_width_mm = 80` (Task 1).
- `profiles/.generated/profiles.json` — regenerated (Task 1).
- `crates/escpost-cli/src/profiles_cmd.rs` — create: `ProfileView`, rendering, handlers (Tasks 2-5).
- `crates/escpost-cli/src/cli.rs` — add `Profiles` subcommand + `ProfilesCommand` enum (Tasks 3-5).
- `crates/escpost-cli/src/error.rs` — one new `CliError` variant (Task 5).
- CLI dispatch site (the `match` over `Command` — find it, likely `main.rs`/`lib.rs`) — add the `Profiles` arm (Task 3).
- `crates/escpost-cli/tests/` — CLI integration tests per task.
- `CLI.md` — document the command (Task 6).

---

### Task 1: Prerequisite — required `paper_width_mm`

**Files:** Modify `crates/escpost-profiles/src/lib.rs`, `profiles/REFERENCE/profile.toml`, regenerate `profiles/.generated/profiles.json`; Test in `crates/escpost-profiles/tests/synthesize_upstream.rs`.

**Interfaces:** Produces `PrinterProfile.paper_width_mm: u32`.

- [ ] **Step 1: Write the failing test**

Add to `synthesize_upstream.rs`:
```rust
#[test]
fn synthesizes_nominal_paper_width_mm() {
    let p = synthesize_profile(CAPABILITIES, "TM-T88III").unwrap().unwrap();
    assert_eq!(p.paper_width_mm, 80);
}
```

- [ ] **Step 2: Run → RED**

`docker compose … test cargo test -p escpost-profiles synthesizes_nominal_paper_width_mm` → FAIL (no field).

- [ ] **Step 3: Implement**

Mirror exactly how `vendor`/`model` were added (search the file for `vendor` to find every site): add `paper_width_mm: u32` to `PrinterProfile` and `CanonicalProfileContent` (hashed); read upstream `media.width.mm` in the import (it lives next to `pixels` in `ImportedMedia` — add `width_mm: Option<u32>` with the same lenient `"Unknown"`→None parse); add optional `paper_width_mm: Option<u32>` to `Enrichment`; fill `enrichment ?? upstream.width_mm ?? Err(MissingPaperWidth{profile})` (add the error variant, modeled on `MissingVendor`). Set `paper_width_mm = 80` in `profiles/REFERENCE/profile.toml`.

- [ ] **Step 4: Regenerate the pack + run GREEN**

Regenerate:
`docker compose … test cargo run -p escpost-profiles --bin compile-profile-pack -- profiles/.escpos-printer-db/dist/capabilities.json profiles profiles/.generated/profiles.json`
Then `docker compose … test cargo test --workspace --exclude escpost-python` all green (incl. `committed_pack_equals_a_fresh_compile`); `… test cargo fmt --check` clean.

- [ ] **Step 5: Commit**
```bash
git add -A && git commit -m "feat(profiles): carry required nominal paper_width_mm"
```

---

### Task 2: `ProfileView` data + rendering module

**Files:** Create `crates/escpost-cli/src/profiles_cmd.rs`; register `mod profiles_cmd;` in the crate root (`main.rs`/`lib.rs`). Test: `crates/escpost-cli/tests/profiles_view.rs`.

**Interfaces:**
- Produces `pub struct ProfileView` (Serialize) with the spec's JSON fields, `pub fn ProfileView::from_profile(&PrinterProfile) -> Self`, `pub fn source_label(&ProfileSource) -> &'static str` (`"calibrated"|"synthesized"|"virtual"`), and rendering fns `render_table(&[ProfileView]) -> String`, `render_detail(&ProfileView) -> String`.
- Consumes: `escpost_profiles::{PrinterProfile, ProfileSource, BarcodeSystem}`.

- [ ] **Step 1: Write the failing test**
```rust
use escpost_cli::profiles_cmd::{ProfileView, source_label};
use escpost_profiles::resolver;

#[test]
fn view_maps_source_and_derived_mm() {
    let p = resolver::resolve("TM-T88III").unwrap();
    let v = ProfileView::from_profile(p);
    assert_eq!(v.source, "synthesized");
    assert_eq!(v.paper_width_mm, 80);
    assert_eq!(v.printable_width_mm, 72); // round(512/180*25.4)
    assert_eq!(v.dpi_x, 180);
    // JSON round-trips with snake_case keys:
    let j = serde_json::to_value(&v).unwrap();
    assert_eq!(j["id"], "TM-T88III");
    assert_eq!(j["source"], "synthesized");
    assert!(j["features"]["barcodes"]["function_b"].is_array());
}
```
(Requires the crate to expose `profiles_cmd` publicly for the test — add `pub mod profiles_cmd;` in a `lib.rs`, or use an integration approach consistent with how the crate exposes testable code. If the crate is bin-only, put this as a `#[cfg(test)] mod tests` inside `profiles_cmd.rs` instead and adjust imports.)

- [ ] **Step 2: Run → RED.** `docker compose … test cargo test -p escpost-cli view_maps_source`.

- [ ] **Step 3: Implement `ProfileView`**

Define the Serialize struct with the spec fields (`id, vendor, model, source, paper_width_mm, printable_width_mm, printable_width_dots, dpi_x, dpi_y, fonts{a,b}, features{barcodes{function_a,function_b}, graphics, paper_full_cut, paper_part_cut, qr_code, pulse_standard}, code_page_count, canonical_profile_sha256`). `from_profile` computes `printable_width_mm` per the constraint, maps `source_label`, collects barcode system names (`BarcodeSystem` → its snake_case serde name via `serde_json`), `code_page_count = profile.code_pages.len()`. Implement `render_table` (aligned columns, the CAL legend line) and `render_detail`.

- [ ] **Step 4: Run → GREEN** + `… test cargo fmt --check`.

- [ ] **Step 5: Commit** `feat(cli): add ProfileView and rendering for the profiles catalog`.

---

### Task 3: `profiles list`

**Files:** `crates/escpost-cli/src/cli.rs` (add `Profiles(ProfilesArgs)` + `ProfilesCommand::List(ListArgs)`), `profiles_cmd.rs` (list handler), the `Command` dispatch site. Test: `crates/escpost-cli/tests/profiles_list.rs`.

**Interfaces:** Consumes `resolver::available_ids()` + `resolve`. Produces `ProfilesCommand` enum (Show/Find added in later tasks).

- [ ] **Step 1: Write failing tests** (invoke the built binary via `assert_cmd`/`Command`, matching the crate's existing CLI test style — inspect an existing `tests/*.rs` first):
```rust
// list shows known ids + calibration markers
// `profiles list --search t88` narrows to TM-T88* ids
// `profiles list --source virtual` → only REFERENCE
// `profiles list --json` parses to a JSON array containing an object with "id"
// `profiles list --vendor nope` → exit 0, stderr contains "no profiles match"
```

- [ ] **Step 2: Run → RED.**

- [ ] **Step 3: Implement.** Add the `Profiles` subcommand mirroring `Printers`; `ListArgs { --vendor Option<String>, --source Option<SourceFilter (ValueEnum calibrated|synthesized|virtual)>, --search Option<String>, --json bool }`. Handler: gather all profiles via `available_ids()` → `resolve` → `ProfileView`, sort by id, apply filters (AND; vendor/search case-insensitive substring; source by label), then `--json` → `serde_json::to_string_pretty(&views)` else `render_table`. Empty → print "no profiles match" to stderr, return Ok. Wire the dispatch arm.

- [ ] **Step 4: Run → GREEN** (`… test cargo test -p escpost-cli`) + fmt.

- [ ] **Step 5: Commit** `feat(cli): add \`escpost profiles list\``.

---

### Task 4: `profiles show <id>`

**Files:** `cli.rs` (`ProfilesCommand::Show(ShowArgs { id, --json })`), `profiles_cmd.rs` (show handler), dispatch. Test: `crates/escpost-cli/tests/profiles_show.rs`.

- [ ] **Step 1: Write failing tests:** `show TM-T88III` output contains "Epson"/"synthesized"; `show TM-T88III --json` is one object with `"canonical_profile_sha256"`; `show nope` exits non-zero.
- [ ] **Step 2: RED.**
- [ ] **Step 3: Implement.** Handler: `resolver::resolve(id)` mapped to `CliError::UnknownProfile` on miss; build `ProfileView`; `--json` → single object, else `render_detail`.
- [ ] **Step 4: GREEN + fmt.**
- [ ] **Step 5: Commit** `feat(cli): add \`escpost profiles show\``.

---

### Task 5: `profiles find`

**Files:** `cli.rs` (`ProfilesCommand::Find(FindArgs {})`), `profiles_cmd.rs` (find handler), `error.rs` (one new variant), dispatch. Test: `crates/escpost-cli/tests/profiles_find.rs`.

- [ ] **Step 1: Write failing test:** `profiles find` with the global `--non-interactive` flag (or piped/non-TTY stdin) exits non-zero and stderr mentions `profiles list --search`.
- [ ] **Step 2: RED.**
- [ ] **Step 3: Implement.** If `--non-interactive` or stdin is not a TTY (`std::io::IsTerminal`), return the new `CliError::InteractiveFindUnavailable` (message points at `profiles list --search`). Otherwise build labels `format!("{id} — {vendor} · {model}")` (id-sorted), `inquire::Select::new("Find a printer profile", labels).with_page_size(10).prompt()`, map the chosen label back to its id, `println!("{id}")`. Map `inquire` errors to `CliError::ProfilePrompt`.
- [ ] **Step 4: GREEN + fmt.**
- [ ] **Step 5: Commit** `feat(cli): add \`escpost profiles find\``.

---

### Task 6: Document the command

**Files:** `CLI.md` (and a line in `README.md` if it enumerates subcommands).

- [ ] **Step 1:** Document `profiles list/show/find`, the filters, the calibration markers, and the `--json`/scripting (`--profile "$(escpost profiles find)"`) usage, matching the actual `--help` text.
- [ ] **Step 2:** `grep -n "profiles" CLI.md` to confirm the section exists; no stale claims.
- [ ] **Step 3: Commit** `docs: document the escpost profiles command`.

---

## Self-Review Notes

- **Spec coverage:** paper_width_mm (T1), ProfileView/JSON/labels (T2), list+filters (T3), show (T4), find+non-interactive (T5), docs (T6). Non-goals (diff, --supports, fuzzy) excluded.
- **Ordering:** T1 (data) → T2 (view) → T3–T5 (commands, each adds a `ProfilesCommand` variant + dispatch) → T6 (docs).
- **Type consistency:** `source_label` values `calibrated/synthesized/virtual` are reused by `--source`'s `ValueEnum` and the `source` JSON field. `ProfileView` is the single shape for table/detail/json.
- **Testability caveat (T2):** the crate may be bin-only; the first test step notes the fallback (in-module `#[cfg(test)]`) if `escpost_cli::profiles_cmd` isn't importable — the implementer picks based on the crate's actual `lib.rs`/`main.rs` layout.
