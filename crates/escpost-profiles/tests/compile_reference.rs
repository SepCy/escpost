use escpost_profiles::{BarcodeSystem, CompileProfileError, ProfileSource, compile_profile};

const REFERENCE_TOML: &str = include_str!("../../../profiles/REFERENCE/profile.toml");

#[test]
fn reference_profile_compiles_without_an_upstream_database() {
    let profile = compile_profile(b"", REFERENCE_TOML)
        .expect("the virtual reference profile should be self-contained");

    assert_eq!(profile.id, "REFERENCE");
    assert_eq!(profile.source, ProfileSource::Reference);
    assert_eq!(profile.geometry.printable_width_dots, 576);
    assert_eq!(
        profile
            .cutter
            .expect("the reference mechanism should define its virtual cutter")
            .print_head_to_cutter_dots,
        80
    );
}

#[test]
fn reference_profile_enables_every_current_rendering_capability() {
    let profile = compile_profile(b"", REFERENCE_TOML)
        .expect("the virtual reference profile should be self-contained");

    assert!(profile.features.bit_image_column);
    assert!(profile.features.bit_image_raster);
    assert!(profile.features.graphics);
    assert!(profile.features.paper_full_cut);
    assert!(profile.features.paper_part_cut);
    assert!(profile.features.pulse_standard);
    assert!(profile.features.qr_code);
    assert_eq!(profile.features.barcodes.function_a.len(), 7);
    assert_eq!(profile.features.barcodes.function_b.len(), 15);
    assert!(
        profile
            .features
            .barcodes
            .function_b
            .contains(&BarcodeSystem::Code128Auto)
    );
    assert_eq!(profile.code_pages.len(), 27);
}

#[test]
fn reference_profile_rejects_an_implicit_capability_default() {
    let incomplete = REFERENCE_TOML.replace("graphics = true\n", "");

    let error = compile_profile(b"", &incomplete)
        .expect_err("REFERENCE must make every current capability explicit");

    assert!(matches!(
        error,
        CompileProfileError::MissingReferenceField {
            field: "features.graphics"
        }
    ));
}
