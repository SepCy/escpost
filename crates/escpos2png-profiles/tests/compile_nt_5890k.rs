use escpos2png_profiles::{
    CanonicalProfileError, CarriageReturnMode, CompileProfileError, ProfileChange,
    ProfileChangeKind, compile_profile, compile_profile_with_lock, from_canonical_json,
    from_canonical_profile_pack_json, to_canonical_json, to_canonical_profile_pack_json,
};
use serde_json::Value;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const UPSTREAM_LOCK_TOML: &str = include_str!("../../../profiles/upstream.lock.toml");
const GENERATED_PROFILE_PACK: &[u8] = include_bytes!("../../../profiles/generated/profiles.json");
const RESOLVED_PROFILE_SHA256: &str =
    "2e471a3f255d2dc85988d350754023a107a882d33504dd2df5e9f3c8d4d79b0b";
const UPSTREAM_COMMIT: &str = "e3bf6056ee75cf70ffaccb925081fffa7ad6ced5";

#[test]
fn nt_5890k_compiles_to_rendering_geometry() {
    let compiled = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the pinned NT-5890K profile should compile");

    assert_eq!(
        (
            compiled.profile.id.as_str(),
            compiled.profile.revision,
            compiled.profile.geometry.printable_width_dots,
            compiled.profile.geometry.dpi_x,
            compiled.profile.geometry.dpi_y,
            compiled.profile.motion.horizontal_units_per_inch,
            compiled.profile.motion.vertical_units_per_inch,
            compiled.source.upstream_profile_sha256.as_str(),
            compiled.source.upstream_commit.as_str(),
        ),
        (
            "NT-5890K",
            7,
            384,
            203,
            203,
            203,
            203,
            RESOLVED_PROFILE_SHA256,
            UPSTREAM_COMMIT,
        )
    );
    assert_eq!(
        (
            compiled.profile.defaults.line_spacing_dots,
            compiled.profile.defaults.code_page,
            compiled.profile.defaults.international_character_set,
            compiled.profile.defaults.carriage_return,
        ),
        (30, 0, 0, CarriageReturnMode::Ignored)
    );
    assert_eq!(compiled.profile.source, compiled.source);
    assert_eq!(compiled.profile.schema_version, 1);
    assert_eq!(compiled.source.enrichment_sha256.len(), 64);
    assert_eq!(compiled.source.canonical_profile_sha256.len(), 64);
    assert_eq!(
        (
            compiled.profile.fonts.a.columns,
            compiled.profile.fonts.a.cell_width_dots,
            compiled.profile.fonts.a.cell_height_dots,
            compiled.profile.fonts.a.baseline_dots,
        ),
        (32, 12, 24, 20)
    );
    assert_eq!(
        (
            compiled.profile.fonts.b.columns,
            compiled.profile.fonts.b.cell_width_dots,
            compiled.profile.fonts.b.cell_height_dots,
            compiled.profile.fonts.b.baseline_dots,
        ),
        (42, 9, 17, 14)
    );
    assert_eq!(
        compiled.profile.code_pages.get(&0).map(String::as_str),
        Some("CP437")
    );
    assert_eq!(
        (
            compiled.profile.features.barcode_a,
            compiled.profile.features.barcode_b,
            compiled.profile.features.bit_image_column,
            compiled.profile.features.bit_image_raster,
            compiled.profile.features.paper_full_cut,
            compiled.profile.features.paper_part_cut,
            compiled.profile.features.qr_code,
        ),
        (true, true, true, true, false, false, true)
    );
}

#[test]
fn canonical_profile_json_round_trips_and_verifies_its_hash() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the pinned NT-5890K profile should compile")
        .profile;

    let json = to_canonical_json(&profile).expect("the compiled profile should serialize");
    let loaded = from_canonical_json(&json).expect("the canonical profile should verify");

    assert_eq!(loaded, profile);
    assert_eq!(
        to_canonical_json(&loaded).expect("serialization should be deterministic"),
        json
    );
}

#[test]
fn canonical_profile_json_rejects_behavior_changed_without_a_new_hash() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the pinned NT-5890K profile should compile")
        .profile;
    let json = to_canonical_json(&profile).expect("the compiled profile should serialize");
    let mut document: Value =
        serde_json::from_slice(&json).expect("the test should parse canonical JSON");
    document["geometry"]["printable_width_dots"] = Value::from(576);
    let tampered = serde_json::to_vec(&document).expect("the test should serialize tampered JSON");

    let error = from_canonical_json(&tampered)
        .expect_err("a behavior change without a new hash must be rejected");

    assert!(matches!(
        error,
        CanonicalProfileError::CanonicalHashMismatch { .. }
    ));
}

#[test]
fn canonical_profile_pack_indexes_verified_profiles_by_id() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the pinned NT-5890K profile should compile")
        .profile;

    let json = to_canonical_profile_pack_json([profile.clone()])
        .expect("the canonical profile pack should serialize");
    let pack =
        from_canonical_profile_pack_json(&json).expect("the canonical profile pack should verify");

    assert_eq!(pack.get("NT-5890K"), Some(&profile));
    assert_eq!(pack.get("unknown"), None);
    assert_eq!(
        to_canonical_profile_pack_json(pack.profiles().cloned())
            .expect("profile-pack serialization should be deterministic"),
        json
    );
}

#[test]
fn generated_profile_pack_matches_the_reviewed_sources() {
    let compiled = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the pinned NT-5890K profile should compile");
    let pack = from_canonical_profile_pack_json(GENERATED_PROFILE_PACK)
        .expect("the committed profile pack should verify");

    assert_eq!(pack.get("NT-5890K"), Some(&compiled.profile));
    assert_eq!(pack.profiles().count(), 1);
}

#[test]
fn canonical_profile_pack_rejects_a_key_that_disagrees_with_the_profile_id() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the pinned NT-5890K profile should compile")
        .profile;
    let json =
        to_canonical_profile_pack_json([profile]).expect("the profile pack should serialize");
    let mut document: Value =
        serde_json::from_slice(&json).expect("the test should parse canonical JSON");
    let profile = document["profiles"]
        .as_object_mut()
        .expect("profiles should be a JSON object")
        .remove("NT-5890K")
        .expect("the generated pack should contain NT-5890K");
    document["profiles"]["wrong-id"] = profile;
    let tampered = serde_json::to_vec(&document).expect("the test should serialize tampered JSON");

    let error = from_canonical_profile_pack_json(&tampered)
        .expect_err("a mismatched profile key must not enter the registry");

    assert!(matches!(
        error,
        CanonicalProfileError::ProfileIdMismatch {
            ref key,
            ref profile_id,
        } if key == "wrong-id" && profile_id == "NT-5890K"
    ));
}

#[test]
fn explicit_upstream_lock_is_preserved_as_profile_provenance() {
    let alternate_lock = UPSTREAM_LOCK_TOML.replace(UPSTREAM_COMMIT, &"a".repeat(40));

    let compiled = compile_profile_with_lock(CAPABILITIES_JSON, ENRICHMENT_TOML, &alternate_lock)
        .expect("an explicit pinned source should compile");

    assert_eq!(compiled.source.upstream_commit, "a".repeat(40));
    assert_eq!(
        compiled.profile.source.canonical_profile_sha256,
        compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
            .expect("the bundled source should compile")
            .source
            .canonical_profile_sha256
    );
}

#[test]
fn nt_5890k_rejects_unreviewed_upstream_changes() {
    let stale_hash = "0".repeat(64);
    let stale_enrichment = ENRICHMENT_TOML.replace(RESOLVED_PROFILE_SHA256, &stale_hash);

    let error = compile_profile(CAPABILITIES_JSON, &stale_enrichment)
        .expect_err("a stale resolved-profile hash must stop compilation");

    assert!(matches!(
        error,
        CompileProfileError::UpstreamProfileHashMismatch {
            ref profile,
            ref expected,
            ref actual,
        } if profile == "NT-5890K"
            && expected == &stale_hash
            && actual == RESOLVED_PROFILE_SHA256
    ));
}

#[test]
fn nt_5890k_reports_which_values_the_enrichment_confirms() {
    let compiled = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the pinned NT-5890K profile should compile");

    let expected_base_changes = [
        "geometry.printable_width_dots",
        "geometry.dpi_x",
        "geometry.dpi_y",
        "motion.horizontal_units_per_inch",
        "motion.vertical_units_per_inch",
        "defaults.line_spacing_dots",
        "defaults.code_page",
        "defaults.international_character_set",
        "defaults.carriage_return",
        "fonts.a.columns",
        "fonts.a.cell_width_dots",
        "fonts.a.cell_height_dots",
        "fonts.a.baseline_dots",
        "fonts.b.columns",
        "fonts.b.cell_width_dots",
        "fonts.b.cell_height_dots",
        "fonts.b.baseline_dots",
    ]
    .map(|field| ProfileChange {
        field: field.to_owned(),
        kind: if field.starts_with("motion.")
            || field == "defaults.line_spacing_dots"
            || field == "defaults.code_page"
            || field == "defaults.international_character_set"
            || field == "defaults.carriage_return"
            || (field.starts_with("fonts.")
                && field != "fonts.a.columns"
                && field != "fonts.b.columns")
        {
            ProfileChangeKind::Added
        } else {
            ProfileChangeKind::Confirmed
        },
    });
    let expected_feature_corrections =
        ["barcode_a", "barcode_b", "qr_code"].map(|feature| ProfileChange {
            field: format!("features.{feature}"),
            kind: ProfileChangeKind::Corrected,
        });

    assert_eq!(
        compiled.changes,
        expected_base_changes
            .into_iter()
            .chain(expected_feature_corrections)
            .collect::<Vec<_>>()
    );
}

#[test]
fn profiles_reject_a_zero_vertical_motion_unit_denominator() {
    let invalid_enrichment = ENRICHMENT_TOML.replace(
        "vertical_units_per_inch = 203",
        "vertical_units_per_inch = 0",
    );

    let error = compile_profile(CAPABILITIES_JSON, &invalid_enrichment)
        .expect_err("zero motion units must not reach the renderer");

    assert!(matches!(
        error,
        CompileProfileError::NonPositiveValue { field }
            if field == "motion.vertical_units_per_inch"
    ));
}

#[test]
fn profiles_reject_a_baseline_outside_its_font_cell() {
    let invalid_enrichment = ENRICHMENT_TOML.replace("baseline_dots = 20", "baseline_dots = 24");

    let error = compile_profile(CAPABILITIES_JSON, &invalid_enrichment)
        .expect_err("an out-of-cell baseline must not reach the renderer");

    assert!(matches!(
        error,
        CompileProfileError::InvalidFontBaseline {
            font,
            baseline_dots: 24,
            cell_height_dots: 24,
        } if font == "fonts.a"
    ));
}

#[test]
fn profiles_reject_a_default_code_page_missing_from_the_imported_profile() {
    let invalid_enrichment = ENRICHMENT_TOML.replace("code_page = 0", "code_page = 15");

    let error = compile_profile(CAPABILITIES_JSON, &invalid_enrichment)
        .expect_err("the renderer must always be able to resolve its default code page");

    assert!(matches!(
        error,
        CompileProfileError::UnknownDefaultCodePage { code_page: 15 }
    ));
}

#[test]
fn profiles_reject_an_international_set_outside_the_version_one_table() {
    let invalid_enrichment = ENRICHMENT_TOML.replace(
        "international_character_set = 0",
        "international_character_set = 18",
    );

    let error = compile_profile(CAPABILITIES_JSON, &invalid_enrichment)
        .expect_err("the default set must have implemented substitution semantics");

    assert!(matches!(
        error,
        CompileProfileError::UnsupportedDefaultInternationalCharacterSet { character_set: 18 }
    ));
}

#[test]
fn profiles_reject_zero_sized_font_cells() {
    let invalid_enrichment =
        ENRICHMENT_TOML.replacen("cell_width_dots = 12", "cell_width_dots = 0", 1);

    let error = compile_profile(CAPABILITIES_JSON, &invalid_enrichment)
        .expect_err("a zero-width cell cannot advance the print cursor");

    assert!(matches!(
        error,
        CompileProfileError::NonPositiveValue {
            field: "fonts.a.cell_width_dots"
        }
    ));
}

#[test]
fn profiles_reject_unknown_feature_overrides() {
    let invalid_enrichment =
        ENRICHMENT_TOML.replace("qr_code = true", "qr_code = true\ninvented_mode = true");

    let error = compile_profile(CAPABILITIES_JSON, &invalid_enrichment)
        .expect_err("a misspelled capability must not silently enter the profile");

    assert!(matches!(error, CompileProfileError::InvalidEnrichment(_)));
}
