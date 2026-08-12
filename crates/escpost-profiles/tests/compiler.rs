use escpost_profiles::{
    BarcodeSystem, CanonicalProfileError, CompileProfileError, ProfileSource, compile_profile,
    from_canonical_json, from_canonical_profile_pack_json, to_canonical_json,
    to_canonical_profile_pack_json,
};
use serde_json::Value;

const CAPABILITIES: &[u8] = include_bytes!("fixtures/capabilities.json");
const PROFILE: &str = include_str!("fixtures/profile.toml");

#[test]
fn upstream_profile_compiles_from_generic_fixtures() {
    let profile = compile_profile(CAPABILITIES, PROFILE).expect("the fixture should compile");

    assert_eq!(profile.id, "TEST-THERMAL-80");
    assert_eq!(profile.source, ProfileSource::Upstream);
    assert_eq!(profile.geometry.printable_width_dots, 512);
    assert_eq!(profile.vendor, "Fixture Vendor");
    assert_eq!(profile.canonical_profile_sha256.len(), 64);
}

#[test]
fn imported_barcode_flags_expand_only_to_legacy_systems() {
    let minimal = r#"
schema_version = 1
profile = "TEST-THERMAL-80"
sources = []

[source]
type = "upstream"
"#;

    let profile = compile_profile(CAPABILITIES, minimal).expect("the fixture should compile");
    assert!(
        profile
            .features
            .barcodes
            .function_b
            .contains(&BarcodeSystem::Code128)
    );
    assert!(
        !profile
            .features
            .barcodes
            .function_b
            .contains(&BarcodeSystem::Code128Auto)
    );
}

#[test]
fn enrichment_can_advertise_model_dependent_barcodes() {
    let enrichment = PROFILE.replace(
        "    \"code_128\",\n]",
        "    \"code_128\",\n    \"gs1_128\",\n    \"code_128_auto\",\n]",
    );

    let profile = compile_profile(CAPABILITIES, &enrichment).expect("the fixture should compile");
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
fn enrichment_rejects_a_barcode_without_a_function_a_command() {
    let enrichment = PROFILE.replace(
        "    \"codabar\",\n]\nfunction_b",
        "    \"codabar\",\n    \"code_128\",\n]\nfunction_b",
    );

    let error = compile_profile(CAPABILITIES, &enrichment).expect_err("Code 128 has no Function A");
    assert!(matches!(
        error,
        CompileProfileError::InvalidBarcodeSystemForFunction {
            function: "A",
            system: BarcodeSystem::Code128,
        }
    ));
}

#[test]
fn canonical_profile_round_trips_and_detects_tampering() {
    let profile = compile_profile(CAPABILITIES, PROFILE).expect("the fixture should compile");
    let json = to_canonical_json(&profile).expect("the profile should serialize");
    assert_eq!(
        from_canonical_json(&json).expect("the profile should verify"),
        profile
    );

    let mut document: Value = serde_json::from_slice(&json).expect("valid JSON");
    document["geometry"]["printable_width_dots"] = Value::from(576);
    let tampered = serde_json::to_vec(&document).expect("valid JSON");
    assert!(matches!(
        from_canonical_json(&tampered),
        Err(CanonicalProfileError::CanonicalHashMismatch { .. })
    ));
}

#[test]
fn canonical_pack_indexes_profiles_and_rejects_a_mismatched_key() {
    let profile = compile_profile(CAPABILITIES, PROFILE).expect("the fixture should compile");
    let json = to_canonical_profile_pack_json([profile.clone()]).expect("pack should serialize");
    let pack = from_canonical_profile_pack_json(&json).expect("pack should verify");
    assert_eq!(pack.get("TEST-THERMAL-80"), Some(&profile));

    let mut document: Value = serde_json::from_slice(&json).expect("valid JSON");
    let profile = document["profiles"]
        .as_object_mut()
        .expect("profiles object")
        .remove("TEST-THERMAL-80")
        .expect("fixture profile");
    document["profiles"]["wrong-id"] = profile;
    let tampered = serde_json::to_vec(&document).expect("valid JSON");
    assert!(matches!(
        from_canonical_profile_pack_json(&tampered),
        Err(CanonicalProfileError::ProfileIdMismatch { .. })
    ));
}

#[test]
fn resolved_values_are_validated() {
    let cases = [
        (
            PROFILE.replace(
                "vertical_units_per_inch = 180",
                "vertical_units_per_inch = 0",
            ),
            "motion.vertical_units_per_inch",
        ),
        (
            PROFILE.replace(
                "eight_dot_vertical_pitch_dots = 1",
                "eight_dot_vertical_pitch_dots = 0",
            ),
            "column_bit_image.eight_dot_vertical_pitch_dots",
        ),
        (
            PROFILE.replacen("cell_width_dots = 12", "cell_width_dots = 0", 1),
            "fonts.a.cell_width_dots",
        ),
    ];

    for (enrichment, expected_field) in cases {
        assert!(matches!(
            compile_profile(CAPABILITIES, &enrichment),
            Err(CompileProfileError::NonPositiveValue { field }) if field == expected_field
        ));
    }
}

#[test]
fn cutter_capability_requires_positive_geometry() {
    let missing = PROFILE.replace("qr_code = true", "qr_code = true\npaper_full_cut = true");
    assert!(matches!(
        compile_profile(CAPABILITIES, &missing),
        Err(CompileProfileError::MissingCutterGeometry)
    ));

    let valid = missing.replace(
        "[motion]",
        "[cutter]\nprint_head_to_cutter_dots = 80\n\n[motion]",
    );
    let profile = compile_profile(CAPABILITIES, &valid).expect("valid cutter geometry");
    assert_eq!(
        profile.cutter.expect("cutter").print_head_to_cutter_dots,
        80
    );
}

#[test]
fn defaults_and_unknown_fields_are_validated() {
    let unknown_code_page = PROFILE.replace("code_page = 0", "code_page = 15");
    assert!(matches!(
        compile_profile(CAPABILITIES, &unknown_code_page),
        Err(CompileProfileError::UnknownDefaultCodePage { code_page: 15 })
    ));

    let unknown_feature = PROFILE.replace("qr_code = true", "qr_code = true\ninvented_mode = true");
    assert!(matches!(
        compile_profile(CAPABILITIES, &unknown_feature),
        Err(CompileProfileError::InvalidEnrichment(_))
    ));
}

#[test]
fn an_enrichment_cannot_claim_the_synthesis_source() {
    let enrichment = r#"
schema_version = 1
profile = "TEST-THERMAL-80"
sources = []

[source]
type = "upstream_default"
"#;

    assert!(matches!(
        compile_profile(CAPABILITIES, enrichment),
        Err(CompileProfileError::InvalidEnrichmentSource {
            kind: "upstream_default"
        })
    ));
}
