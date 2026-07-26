use escpos2png_profiles::{
    BarcodeSystem, CanonicalProfileError, CarriageReturnMode, CompileProfileError, compile_profile,
    from_canonical_json, from_canonical_profile_pack_json, to_canonical_json,
    to_canonical_profile_pack_json,
};
use serde_json::Value;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const GENERATED_PROFILE_PACK: &[u8] = include_bytes!("../../../profiles/generated/profiles.json");
const RESOLVED_PROFILE_SHA256: &str =
    "2e471a3f255d2dc85988d350754023a107a882d33504dd2df5e9f3c8d4d79b0b";

#[test]
fn nt_5890k_compiles_to_rendering_geometry() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the pinned NT-5890K profile should compile");

    assert_eq!(
        (
            profile.id.as_str(),
            profile.geometry.printable_width_dots,
            profile.geometry.dpi_x,
            profile.geometry.dpi_y,
            profile.motion.horizontal_units_per_inch,
            profile.motion.vertical_units_per_inch,
            profile.upstream_profile_sha256.as_str(),
        ),
        ("NT-5890K", 384, 203, 203, 203, 203, RESOLVED_PROFILE_SHA256,)
    );
    assert_eq!(
        (
            profile.defaults.line_spacing_dots,
            profile.defaults.code_page,
            profile.defaults.international_character_set,
            profile.defaults.carriage_return,
        ),
        (30, 0, 0, CarriageReturnMode::Ignored)
    );
    assert_eq!(profile.schema_version, 1);
    assert_eq!(profile.canonical_profile_sha256.len(), 64);
    assert_eq!(
        (
            profile.fonts.a.cell_width_dots,
            profile.fonts.a.cell_height_dots,
            profile.fonts.a.baseline_dots,
        ),
        (12, 24, 20)
    );
    assert_eq!(
        (
            profile.fonts.b.cell_width_dots,
            profile.fonts.b.cell_height_dots,
            profile.fonts.b.baseline_dots,
        ),
        (9, 17, 14)
    );
    assert_eq!(
        profile.code_pages.get(&0).map(String::as_str),
        Some("CP437")
    );
    assert_eq!(
        (
            !profile.features.barcodes.function_a.is_empty(),
            !profile.features.barcodes.function_b.is_empty(),
            profile.features.bit_image_column,
            profile.features.bit_image_raster,
            profile.features.paper_full_cut,
            profile.features.paper_part_cut,
            profile.features.qr_code,
        ),
        (true, true, true, true, false, false, true)
    );
    for system in [
        BarcodeSystem::Gs1DataBarOmnidirectional,
        BarcodeSystem::Gs1DataBarTruncated,
        BarcodeSystem::Gs1DataBarLimited,
        BarcodeSystem::Gs1DataBarExpanded,
    ] {
        assert!(
            !profile.features.barcodes.function_b.contains(&system),
            "the connected NT-5890K ignored every GS1 DataBar system"
        );
    }
}

#[test]
fn imported_barcode_flags_expand_only_to_the_legacy_systems_they_describe() {
    let mut capabilities: Value =
        serde_json::from_slice(CAPABILITIES_JSON).expect("the upstream fixture should be JSON");
    capabilities["profiles"]["NT-5890K"]["features"]["barcodeA"] = Value::Bool(true);
    capabilities["profiles"]["NT-5890K"]["features"]["barcodeB"] = Value::Bool(true);
    let capabilities =
        serde_json::to_vec(&capabilities).expect("the modified fixture should serialize");

    // Remove the exact enrichment so this test exercises only the upstream
    // booleans. The synthetic profile change gets its own reviewed hash below.
    let table_start = ENRICHMENT_TOML
        .find("[features.barcodes]")
        .expect("the fixture should contain barcode enrichment");
    let table_end = ENRICHMENT_TOML[table_start..]
        .find("[[approximations]]")
        .map(|offset| table_start + offset)
        .expect("the barcode table should end before approximations");
    let without_barcode_enrichment = format!(
        "{}{}",
        &ENRICHMENT_TOML[..table_start],
        &ENRICHMENT_TOML[table_end..]
    );
    let first_error = compile_profile(&capabilities, &without_barcode_enrichment)
        .expect_err("changing the synthetic upstream profile must change its hash");
    let actual_hash = match first_error {
        CompileProfileError::UpstreamProfileHashMismatch { actual, .. } => actual,
        other => panic!("expected an upstream hash mismatch, got {other}"),
    };
    let reviewed_enrichment =
        without_barcode_enrichment.replace(RESOLVED_PROFILE_SHA256, &actual_hash);

    let profile = compile_profile(&capabilities, &reviewed_enrichment)
        .expect("reviewed upstream barcode flags should compile");

    assert_eq!(
        profile
            .features
            .barcodes
            .function_b
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![
            BarcodeSystem::UpcA,
            BarcodeSystem::UpcE,
            BarcodeSystem::Ean13,
            BarcodeSystem::Ean8,
            BarcodeSystem::Code39,
            BarcodeSystem::Itf,
            BarcodeSystem::Codabar,
            BarcodeSystem::Code93,
            BarcodeSystem::Code128,
        ]
    );
    assert!(
        !profile
            .features
            .barcodes
            .function_b
            .contains(&BarcodeSystem::Code128Auto),
        "model-dependent systems must require explicit profile evidence"
    );
}

#[test]
fn enrichment_can_advertise_model_dependent_function_b_systems_exactly() {
    let enrichment = ENRICHMENT_TOML.replace(
        "    \"code_128\",\n]",
        "    \"code_128\",\n    \"gs1_128\",\n    \"code_128_auto\",\n]",
    );

    let profile = compile_profile(CAPABILITIES_JSON, &enrichment)
        .expect("explicit model-dependent barcode systems should compile");

    assert!(
        profile
            .features
            .barcodes
            .function_b
            .contains(&BarcodeSystem::Gs1_128)
    );
    assert!(
        profile
            .features
            .barcodes
            .function_b
            .contains(&BarcodeSystem::Code128Auto)
    );
}

#[test]
fn enrichment_rejects_a_system_that_has_no_function_a_command_number() {
    let enrichment = ENRICHMENT_TOML.replace(
        "    \"codabar\",\n]\nfunction_b",
        "    \"codabar\",\n    \"code_128\",\n]\nfunction_b",
    );

    let error = compile_profile(CAPABILITIES_JSON, &enrichment)
        .expect_err("Function A cannot encode Code 128");

    assert!(matches!(
        error,
        CompileProfileError::InvalidBarcodeSystemForFunction {
            function: "A",
            system: BarcodeSystem::Code128,
        }
    ));
}

#[test]
fn canonical_profile_json_round_trips_and_verifies_its_hash() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the pinned NT-5890K profile should compile");

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
        .expect("the pinned NT-5890K profile should compile");
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
        .expect("the pinned NT-5890K profile should compile");

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
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the pinned NT-5890K profile should compile");
    let pack = from_canonical_profile_pack_json(GENERATED_PROFILE_PACK)
        .expect("the committed profile pack should verify");

    assert_eq!(pack.get("NT-5890K"), Some(&profile));
    assert_eq!(pack.profiles().count(), 1);
}

#[test]
fn canonical_profile_pack_rejects_a_key_that_disagrees_with_the_profile_id() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the pinned NT-5890K profile should compile");
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
