# Descriptor/Deviation Profiles + Upstream Synthesis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every upstream printer with a known paper width resolvable and renderable by synthesizing it from the upstream entry plus a base of default values, with all profile parameters optional.

**Architecture:** Profile parameters split into *descriptors* (intrinsic facts, sourced from upstream or measured) and *deviations* (departures from the ESC/POS baseline, default conformant). Every enrichment field becomes optional; a single compile-time fill layers explicit → upstream-derived → constant. All width-bearing upstream ids are compiled into the one canonical pack. The per-field `approximations` disclosure is removed; the profile-level `source` marker (`UpstreamDefault`) signals synthesized-vs-calibrated. Resolution centralizes in `escpost-profiles`.

**Tech Stack:** Rust workspace (crates `escpost-profiles`, `escpost`, `escpost-cli`, `escpost-python`/PyO3); `serde`/`serde_json`, `toml`, `sha2`, `thiserror`. Design recorded in `DESIGN_DECISIONS.md` DD-031/DD-032.

## Global Constraints

- Design authority: `DESIGN_DECISIONS.md` DD-031 (descriptor/deviation model, omit=assumed/set=known, no inheritance, no per-field disclosure) and DD-032 (compile-time synthesis, conservative floor, 58 mm fail-safe width, distinct source marker, drift check).
- `escpost` (renderer) stays pure compute — no I/O, no new transport deps (DD-027).
- Enrichment keeps `#[serde(deny_unknown_fields)]`; optional means `Option<...>`, never a silent unknown key.
- Authoritative width unit is `printable_width_dots` (never mm). Default width = 384 dots (58 mm @ 203 dpi).
- Deviations default conformant; descriptors default from upstream, else documented constant. Descriptor constant defaults (REFERENCE-derived): `dpi = 203`; `motion units = dpi`; font A cell `12×24` baseline `20`; font B cell `9×17` baseline `14`; `cutter = None`. Deviation constant defaults: `line_spacing_dots = 30`, `code_page = 0`, `international_character_set = 0`, `carriage_return = Ignored` (REFERENCE value); `eight_dot_vertical_pitch_dots = 3`; all `commands.*` = `Apply`/`Feed`.
- Never fabricate width for a real-named printer: an upstream entry without a numeric `media.width.pixels` and no enrichment width yields **no** profile (log the skip).
- **Run the entire toolchain through docker compose — never host `cargo`/`python3`** (user rule). Wrap every `cargo …` step in this plan as:
  `docker compose -f /home/lars/projects/duala-digital/escpost/compose.yaml --project-directory /home/lars/projects/duala-digital/escpost run --rm -w /workspace/.claude/worktrees/upstream-profile-synthesis test cargo …`
  (`git` runs on the host as normal.)
- Pre-release: no published API, so removing `RenderResult.approximations` / the Python `approximations` key needs no deprecation path.
- Commit after each task with a `feat:`/`refactor:`/`test:` message.

## File Structure

- `crates/escpost-profiles/src/lib.rs` — modify: optional `Enrichment` fields; `ImportedProfile` gains `media`+`fonts`; new default-fill; `ProfileSource::UpstreamDefault`; `synthesize_profile`; remove `Approximation`.
- `crates/escpost-profiles/src/defaults.rs` — create: the documented constant defaults + upstream-derivation helpers.
- `crates/escpost-profiles/src/resolver.rs` — create: embedded-pack `resolve(id)` + `available_ids()`.
- `crates/escpost-profiles/src/bin/compile-profile-pack.rs` — modify: also iterate upstream ids without an enrichment dir; synthesize width-bearing ones; log skips.
- `crates/escpost-profiles/tests/` — add `synthesize_upstream.rs`, `pack_drift.rs`; scrub `approximations` from `compile_nt_5890k.rs`.
- `crates/escpost/src/lib.rs` + `tests/render_result.rs` — remove `approximations`.
- `crates/escpost-python/src/lib.rs` — remove `approximations`; call resolver.
- `crates/escpost-cli/src/profiles.rs` — call resolver; keep only the interactive prompt.
- `profiles/REFERENCE/profile.toml`, `profiles/NT-5890K/profile.toml` — remove `[[approximations]]`.
- `profiles/.generated/profiles.json` — regenerated (grows to all width-bearing profiles).
- `PROFILE_SCHEMA.md` — document the model.

---

### Task 1: Import upstream media and font columns

**Files:**
- Modify: `crates/escpost-profiles/src/lib.rs` (`ImportedProfile`, `ImportedFeatures` area ~370-388)
- Test: `crates/escpost-profiles/tests/compile_nt_5890k.rs`

**Interfaces:**
- Produces: `ImportedProfile { features, code_pages, media: ImportedMedia, fonts: BTreeMap<u8, ImportedFont> }` where `ImportedMedia { width_pixels: Option<u32>, dpi: Option<u32> }` and `ImportedFont { columns: Option<u32> }`. `"Unknown"`/absent parse to `None`.

- [ ] **Step 1: Write the failing test**

Add to `crates/escpost-profiles/tests/compile_nt_5890k.rs`:

```rust
#[test]
fn upstream_media_and_font_columns_parse_with_unknown_as_absent() {
    use escpost_profiles::import_upstream_descriptors;
    let (media, fonts) = import_upstream_descriptors(CAPABILITIES_JSON, "TM-T88III")
        .expect("TM-T88III should import");
    assert_eq!(media.width_pixels, Some(512));
    assert_eq!(media.dpi, Some(180));
    assert_eq!(fonts.get(&0).and_then(|f| f.columns), Some(42));

    let (generic, _) = import_upstream_descriptors(CAPABILITIES_JSON, "default")
        .expect("default should import");
    assert_eq!(generic.width_pixels, None);
    assert_eq!(generic.dpi, None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p escpost-profiles upstream_media_and_font_columns -- --nocapture`
Expected: FAIL — `import_upstream_descriptors` not found.

- [ ] **Step 3: Implement lenient media/font import**

In `lib.rs`, add a helper that deserializes `"Unknown"`-or-int leniently and expose `pub fn import_upstream_descriptors(capabilities_json: &[u8], id: &str) -> Result<(ImportedMedia, BTreeMap<u8, ImportedFont>), CompileProfileError>`. Extend `ImportedProfile` with `media`/`fonts`. Parse `media.width.pixels`, `media.dpi`, and `fonts.<n>.columns` via `serde_json::Value`, mapping the string `"Unknown"` and missing keys to `None` (a `u32` only when the JSON value is a number).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p escpost-profiles upstream_media_and_font_columns`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/escpost-profiles/src/lib.rs crates/escpost-profiles/tests/compile_nt_5890k.rs
git commit -m "feat(profiles): import upstream media width, dpi, and font columns"
```

---

### Task 2: Documented constant defaults + upstream-derivation helpers

**Files:**
- Create: `crates/escpost-profiles/src/defaults.rs`
- Modify: `crates/escpost-profiles/src/lib.rs` (add `mod defaults;`)
- Test: `crates/escpost-profiles/tests/synthesize_upstream.rs` (create)

**Interfaces:**
- Produces: `defaults::DEFAULT_WIDTH_DOTS: u32 = 384`, `defaults::DEFAULT_DPI: u32 = 203`; `defaults::default_fonts() -> Fonts`; `defaults::default_commands() -> CommandBehavior`; `defaults::default_printer_defaults() -> PrinterDefaults`; `defaults::derive_cell_width(width_dots, columns) -> u32` = `(width_dots / columns).max(1)`.

- [ ] **Step 1: Write the failing test**

Create `crates/escpost-profiles/tests/synthesize_upstream.rs`:

```rust
use escpost_profiles::defaults;

#[test]
fn documented_constants_match_the_reference_baseline() {
    assert_eq!(defaults::DEFAULT_WIDTH_DOTS, 384);
    assert_eq!(defaults::DEFAULT_DPI, 203);
    let fonts = defaults::default_fonts();
    assert_eq!((fonts.a.cell_width_dots, fonts.a.cell_height_dots, fonts.a.baseline_dots), (12, 24, 20));
    assert_eq!((fonts.b.cell_width_dots, fonts.b.cell_height_dots, fonts.b.baseline_dots), (9, 17, 14));
    assert_eq!(defaults::derive_cell_width(512, 42), 12);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p escpost-profiles documented_constants_match`
Expected: FAIL — `defaults` module missing.

- [ ] **Step 3: Implement `defaults.rs`**

Create the module with the constants and constructors above, returning the existing `Fonts`/`CommandBehavior`/`PrinterDefaults` types. `default_commands` returns all `Apply`/`Feed`; `default_printer_defaults` returns `line_spacing_dots: 30, code_page: 0, international_character_set: 0, carriage_return: Ignored`. Add `pub mod defaults;` to `lib.rs` and re-export the module.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p escpost-profiles documented_constants_match`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/escpost-profiles/src/defaults.rs crates/escpost-profiles/src/lib.rs crates/escpost-profiles/tests/synthesize_upstream.rs
git commit -m "feat(profiles): add documented descriptor/deviation default constants"
```

---

### Task 3: Optional enrichment fields with layered fill

**Files:**
- Modify: `crates/escpost-profiles/src/lib.rs` (`Enrichment` struct, `compile_profile`)
- Test: `crates/escpost-profiles/tests/compile_nt_5890k.rs`

**Interfaces:**
- Consumes: Task 1 media/fonts import, Task 2 defaults.
- Produces: `compile_profile` accepts an enrichment that omits any of `geometry`/`cutter`/`motion`/`column_bit_image`/`commands`/`defaults`/`fonts`; missing values fill explicit → upstream-derived → constant. A fully-specified enrichment compiles identically to today (NT-5890K unchanged).

- [ ] **Step 1: Write the failing test**

Add to `compile_nt_5890k.rs`:

```rust
#[test]
fn upstream_enrichment_may_omit_descriptors_and_deviations() {
    // Minimal upstream enrichment: only identity + pinned source.
    let sha = RESOLVED_PROFILE_SHA256;
    let minimal = format!(
        "schema_version = 1\nprofile = \"NT-5890K\"\nsources = []\n\n[source]\ntype = \"upstream\"\nprofile_sha256 = \"{sha}\"\n"
    );
    let profile = compile_profile(CAPABILITIES_JSON, &minimal)
        .expect("a minimal upstream enrichment should compile via defaults");
    // Width unknown upstream for NT-5890K → 58 mm fallback; deviations conformant.
    assert_eq!(profile.geometry.printable_width_dots, 384);
    assert_eq!(profile.defaults.carriage_return, CarriageReturnMode::Ignored);
    assert_eq!(profile.commands.esc_j, FeedBehavior::Feed); // conformant default, not the measured Ignored
    assert_eq!(profile.column_bit_image.eight_dot_vertical_pitch_dots, 3); // Epson baseline default
}
```

(NT-5890K upstream has no media width, so it exercises the 58 mm fallback; the full enrichment test `nt_5890k_compiles_to_rendering_geometry` still asserts the measured values.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p escpost-profiles upstream_enrichment_may_omit`
Expected: FAIL — TOML deserialize error for missing required tables.

- [ ] **Step 3: Make fields optional and fill**

Change the descriptor/deviation fields on `Enrichment` to `Option<...>`. After `validate_schema_version`, resolve each into a concrete value: geometry width = enrichment ?? upstream `media.width_pixels` ?? `DEFAULT_WIDTH_DOTS`; dpi = enrichment ?? upstream dpi ?? `DEFAULT_DPI` (both axes from the single dpi); motion = enrichment ?? dpi; fonts = enrichment ?? (cell_width from `derive_cell_width(width, columns)` with `default_fonts()` for height/baseline) ?? `default_fonts()`; `commands`/`column_bit_image`/`defaults` = enrichment ?? `defaults::*`; `cutter` stays `Option` (absent unless set or a cut feature+enrichment supplies it). Run existing validators on the filled values. Keep `validate_profile_values` operating on resolved values.

- [ ] **Step 4: Run tests to verify pass (new + regression)**

Run: `cargo test -p escpost-profiles`
Expected: PASS — new test passes; `nt_5890k_compiles_to_rendering_geometry` and all validator tests still pass (they supply full values).

- [ ] **Step 5: Commit**

```bash
git add crates/escpost-profiles/src/lib.rs crates/escpost-profiles/tests/compile_nt_5890k.rs
git commit -m "feat(profiles): make enrichment parameters optional with layered defaults"
```

---

### Task 4: `UpstreamDefault` source + `synthesize_profile`

**Files:**
- Modify: `crates/escpost-profiles/src/lib.rs` (`ProfileSource`, add `synthesize_profile`)
- Test: `crates/escpost-profiles/tests/synthesize_upstream.rs`

**Interfaces:**
- Produces: `ProfileSource::UpstreamDefault { profile_sha256: String }`; `pub fn synthesize_profile(capabilities_json: &[u8], id: &str) -> Result<Option<PrinterProfile>, CompileProfileError>` — `Ok(None)` when width is not derivable (no upstream pixels), else a profile with `source = UpstreamDefault`, descriptors filled from upstream/constants, all deviations conformant. Records the resolved upstream `profile_sha256` (drift), but performs **no** pinned-hash comparison.

- [ ] **Step 1: Write the failing tests**

Add to `synthesize_upstream.rs`:

```rust
use escpost_profiles::{synthesize_profile, ProfileSource};

#[test]
fn synthesizes_a_width_bearing_upstream_profile() {
    let profile = synthesize_profile(CAPABILITIES_JSON, "TM-T88III")
        .expect("import ok")
        .expect("TM-T88III has a width, so it synthesizes");
    assert_eq!(profile.id, "TM-T88III");
    assert_eq!(profile.geometry.printable_width_dots, 512);
    assert_eq!(profile.geometry.dpi_x, 180);
    assert_eq!(profile.fonts.a.cell_width_dots, 512 / 42);
    assert!(matches!(profile.source, ProfileSource::UpstreamDefault { .. }));
    assert_eq!(profile.code_pages.get(&0).map(String::as_str), Some("CP437"));
    // Deviations conformant:
    assert_eq!(profile.commands.esc_j, escpost_profiles::FeedBehavior::Feed);
}

#[test]
fn declines_to_synthesize_a_widthless_generic() {
    for id in ["default", "safe", "simple"] {
        assert_eq!(
            synthesize_profile(CAPABILITIES_JSON, id).expect("import ok"),
            None,
            "{id} states no width and must not synthesize"
        );
    }
}

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p escpost-profiles synthesize`
Expected: FAIL — `synthesize_profile`/`UpstreamDefault` not found.

- [ ] **Step 3: Implement**

Add the `UpstreamDefault { profile_sha256 }` variant to `ProfileSource`. Implement `synthesize_profile`: import descriptors (Task 1); if `media.width_pixels` is `None`, return `Ok(None)`; else build a `PrinterProfile` reusing the Task-3 fill with an all-`None` enrichment over the upstream entry, set `source = UpstreamDefault { profile_sha256: hash_resolved_profile(entry) }`, compute `canonical_profile_sha256`, and validate.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p escpost-profiles synthesize`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/escpost-profiles/src/lib.rs crates/escpost-profiles/tests/synthesize_upstream.rs
git commit -m "feat(profiles): synthesize width-bearing upstream profiles as UpstreamDefault"
```

---

### Task 5: Remove the `approximations` machinery

**Files:**
- Modify: `crates/escpost-profiles/src/lib.rs` (drop `Approximation`, the `approximations` field, its inclusion in `hash_canonical_profile`/`CanonicalProfileContent`)
- Modify: `crates/escpost/src/lib.rs:20,77,159`; `crates/escpost/tests/render_result.rs:42`
- Modify: `crates/escpost-python/src/lib.rs` (drop `Approximation` import, `approximations_to_python`, the `set_item("approximations", …)`)
- Modify: `profiles/REFERENCE/profile.toml`, `profiles/NT-5890K/profile.toml` (remove `[[approximations]]` tables; drop NT's now-redundant `carriage_return` note comment)
- Modify: `crates/escpost-profiles/tests/compile_nt_5890k.rs` (the `imported_barcode_flags…` test slices on `[[approximations]]` at line ~139 — reslice on end-of-string)

**Interfaces:**
- Produces: `PrinterProfile` and `RenderResult` no longer have `approximations`; canonical hashes shift (regenerated in Task 7).

- [ ] **Step 1: Delete the approximations test**

In `crates/escpost/tests/render_result.rs`, delete `exposes_profile_approximations_directly` (it asserts on the field being removed). Do **not** add a placeholder replacement — the removal is guarded by the workspace compiling (Step 4) and the grep (Step 4). The file's existing warnings/device-event tests remain the behavioral coverage.

- [ ] **Step 2: Baseline — confirm the field is still present**

Run: `docker compose … test cargo build -p escpost`
Expected: builds (field still present; removed in Step 3).

- [ ] **Step 3: Remove the field and all references**

Delete `Approximation`, the `approximations` field on `PrinterProfile`, and its entry in `CanonicalProfileContent`/`hash_canonical_profile`. Remove `approximations` from `RenderResult` and its population at `lib.rs:159`. Remove the Python import, `approximations_to_python`, and the `set_item`. Strip `[[approximations]]` from both TOMLs. Fix the `compile_nt_5890k.rs` slice to end at `ENRICHMENT_TOML.len()` instead of `find("[[approximations]]")`.

- [ ] **Step 4: Verify compiles, tests pass, and no `approximation` identifier remains**

Run: `docker compose … test cargo test -p escpost-profiles -p escpost` — pack-equality tests (`generated_profile_pack_matches_the_reviewed_sources`, and any drift test) may fail on shifted canonical hashes; that is expected and resolved in Task 7. Note it and continue.
Run on host: `grep -rni approximation crates/ profiles/REFERENCE/profile.toml profiles/NT-5890K/profile.toml` → expect **no matches**.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: remove per-field approximations; source marker carries fidelity signal"
```

---

### Task 6: Compiler emits all width-bearing upstream ids

**Files:**
- Modify: `crates/escpost-profiles/src/bin/compile-profile-pack.rs`

**Interfaces:**
- Consumes: `synthesize_profile` (Task 4), existing `compile_profile`.
- Produces: the pack includes every enrichment dir profile plus every upstream id (not covered by a dir) that synthesizes; skipped ids are logged to stderr.

- [ ] **Step 1: Write the failing test**

Add a unit test in the bin (`#[cfg(test)] mod tests`) or a new `tests/` file asserting the id set. Simplest: a new integration test `crates/escpost-profiles/tests/pack_contents.rs`:

```rust
// Builds the pack the same way the bin does and asserts coverage.
#[test]
fn pack_covers_curated_and_width_bearing_upstream() {
    let ids = escpost_profiles::compile_all_ids(
        include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json"),
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../profiles")),
    ).expect("pack builds");
    assert!(ids.contains(&"NT-5890K".to_string())); // curated
    assert!(ids.contains(&"TM-T88III".to_string())); // synthesized
    assert!(!ids.contains(&"default".to_string()));  // width-less, skipped
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p escpost-profiles pack_covers`
Expected: FAIL — `compile_all_ids` not found.

- [ ] **Step 3: Implement shared build + wire the bin**

Extract a `pub fn compile_all(capabilities_json, profiles_dir) -> Result<Vec<PrinterProfile>, …>` (and a thin `compile_all_ids` returning the ids for the test) that: compiles each enrichment dir; then for each upstream id lacking a dir, calls `synthesize_profile` and pushes `Some`, logging each `None`/width-less skip via `eprintln!`. Have `main` call `compile_all` then `to_canonical_profile_pack_json`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p escpost-profiles pack_covers`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/escpost-profiles/src/bin/compile-profile-pack.rs crates/escpost-profiles/src/lib.rs crates/escpost-profiles/tests/pack_contents.rs
git commit -m "feat(profiles): compile all width-bearing upstream profiles into the pack"
```

---

### Task 7: Regenerate the pack + drift test

**Files:**
- Modify: `profiles/.generated/profiles.json` (regenerated)
- Modify: `crates/escpost-profiles/tests/compile_nt_5890k.rs` (`generated_profile_pack_matches_the_reviewed_sources` now expects >2 profiles)
- Create: `crates/escpost-profiles/tests/pack_drift.rs`

- [ ] **Step 1: Write the drift test**

```rust
#[test]
fn committed_pack_equals_a_fresh_compile() {
    let fresh = escpost_profiles::compile_all(
        include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json"),
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../profiles")),
    ).expect("compile");
    let fresh_json = escpost_profiles::to_canonical_profile_pack_json(fresh).expect("serialize");
    let committed = include_bytes!("../../../profiles/.generated/profiles.json");
    assert_eq!(fresh_json, committed, "regenerate profiles/.generated/profiles.json");
}
```

- [ ] **Step 2: Run — expect drift**

Run: `cargo test -p escpost-profiles committed_pack_equals`
Expected: FAIL — committed pack is stale (2 profiles, old hashes).

- [ ] **Step 3: Regenerate the pack**

Run:
```bash
cargo run -p escpost-profiles --bin compile-profile-pack -- \
  profiles/.escpos-printer-db/dist/capabilities.json \
  profiles profiles/.generated/profiles.json
```
Update `generated_profile_pack_matches_the_reviewed_sources` to assert `pack.get("TM-T88III").is_some()` and `pack.profiles().count() > 2` instead of `== 2`.

- [ ] **Step 4: Run full suite**

Run: `cargo test -p escpost-profiles -p escpost`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add profiles/.generated/profiles.json crates/escpost-profiles/tests/
git commit -m "feat(profiles): regenerate pack with synthesized upstream profiles; add drift guard"
```

---

### Task 8: First-class resolver in `escpost-profiles`

**Files:**
- Create: `crates/escpost-profiles/src/resolver.rs`
- Modify: `crates/escpost-profiles/src/lib.rs` (`mod resolver; pub use`)
- Test: `crates/escpost-profiles/tests/resolver.rs` (create)

**Interfaces:**
- Produces: `resolver::resolve(id: &str) -> Result<&'static PrinterProfile, CanonicalProfileError>`; `resolver::available_ids() -> Vec<String>`. Backed by the embedded pack via `OnceLock` (the pattern currently duplicated in CLI/Python).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn resolver_finds_curated_and_synthesized_ids() {
    assert!(escpost_profiles::resolver::resolve("REFERENCE").is_ok());
    assert!(escpost_profiles::resolver::resolve("TM-T88III").is_ok());
    assert!(escpost_profiles::resolver::resolve("nope").is_err());
    let ids = escpost_profiles::resolver::available_ids();
    assert!(ids.iter().any(|id| id == "TM-T88III"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p escpost-profiles resolver_finds`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement the resolver**

Embed `include_bytes!("../../profiles/.generated/profiles.json")`, lazily `from_canonical_profile_pack_json` into a `OnceLock<ProfilePack>`, and expose `resolve`/`available_ids`. Keep it out of the pure compile core (its own module).

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p escpost-profiles resolver_finds`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/escpost-profiles/src/resolver.rs crates/escpost-profiles/src/lib.rs crates/escpost-profiles/tests/resolver.rs
git commit -m "feat(profiles): add first-class profile resolver over the embedded pack"
```

---

### Task 9: Collapse the CLI and Python leaves onto the resolver

**Files:**
- Modify: `crates/escpost-cli/src/profiles.rs`
- Modify: `crates/escpost-python/src/lib.rs:12,58-73`

**Interfaces:**
- Consumes: `resolver::resolve`, `resolver::available_ids`.

- [ ] **Step 1: Adjust CLI to delegate**

In `escpost-cli/src/profiles.rs`, replace the embedded `PROFILE_PACK_JSON`/`OnceLock`/`profile_pack()` with calls to `escpost_profiles::resolver`. `load(id)` → `resolver::resolve(id).map_err(|e| CliError::LoadProfiles(e.to_string()))` mapped to `CliError::UnknownProfile` on the not-found case; the interactive `Select` uses `resolver::available_ids()`.

- [ ] **Step 2: Run CLI tests**

Run: `cargo test -p escpost-cli`
Expected: PASS.

- [ ] **Step 3: Collapse Python binding**

In `escpost-python/src/lib.rs`, delete `PROFILE_PACK_JSON`, `PROFILE_PACK`, and the pack logic in `load_profile`; call `escpost_profiles::resolver::resolve(profile_id)` mapped to `BindingError::UnknownProfile`/`LoadProfiles`.

- [ ] **Step 4: Verify Python binding via docker**

Inspect `scripts/python-binding-test` first, then run it inside the `test` service (build the PyO3 module + exercise the binding), e.g.:
```bash
docker compose -f /home/lars/projects/duala-digital/escpost/compose.yaml --project-directory /home/lars/projects/duala-digital/escpost \
  run --rm -w /workspace/.claude/worktrees/upstream-profile-synthesis test scripts/python-binding-test
```
Expected: PASS, and the `render_result` dict no longer has an `approximations` key. Never run host `python3`.

- [ ] **Step 5: Commit**

```bash
git add crates/escpost-cli/src/profiles.rs crates/escpost-python/src/lib.rs
git commit -m "refactor: resolve profiles through escpost-profiles in CLI and Python"
```

---

### Task 10: Document the model in `PROFILE_SCHEMA.md`

**Files:**
- Modify: `PROFILE_SCHEMA.md`

- [ ] **Step 1: Write the documentation**

Add sections covering: descriptors vs deviations; every parameter optional (omit = assumed, set = known); layered fill (explicit → upstream-derived → constant); the 58 mm width default and why the authoritative unit is `printable_width_dots` (printable dots ≠ paper mm × dpi); `dpi` (raster resolution) vs `motion` (command-argument granularity, `GS P`-mutable) as distinct; width-gated synthesis and the skipped generics; the `UpstreamDefault` source marker; no inheritance. Reference DD-031/DD-032.

- [ ] **Step 2: Verify no stale `approximation` references remain**

Run: `grep -rni approximation PROFILE_SCHEMA.md README.md ARCHITECTURE.md`
Expected: no results (fix any that appear).

- [ ] **Step 3: Commit**

```bash
git add PROFILE_SCHEMA.md
git commit -m "docs: document descriptor/deviation profiles and upstream synthesis"
```

---

## Self-Review Notes

- **Spec coverage:** optional params (T3), upstream derivation (T1/T2), synthesis + width gate (T4), no fabricated width (T4 `declines_to_synthesize`), distinct source (T4), compile-time-all (T6), drift guard (T7), approximations removed (T5), resolver + leaf collapse (T8/T9), docs (T10). DD-031/032 constraints in Global Constraints.
- **Ordering:** T5 (remove approximations) precedes T7 (regenerate) so the regenerated pack already reflects the new canonical hash content.
- **Type consistency:** `synthesize_profile` and `compile_profile` share the Task-3 fill; `ProfileSource::UpstreamDefault { profile_sha256 }` mirrors `Upstream`; `resolver::resolve` returns `&'static PrinterProfile` (renderer takes `&PrinterProfile`, unchanged).
- **Open adaptation:** exact helper names inside `lib.rs` (`import_upstream_descriptors`, `compile_all`) are introduced by their first task and consumed by later ones — keep them stable.
