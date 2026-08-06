use escpost_profiles::{
    ProfileSource, defaults, from_canonical_json, synthesize_profile, to_canonical_json,
};

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");

#[test]
fn synthesizes_a_width_bearing_upstream_profile() {
    let profile = synthesize_profile(CAPABILITIES_JSON, "TM-T88III")
        .expect("import ok")
        .expect("TM-T88III has a width, so it synthesizes");
    assert_eq!(profile.id, "TM-T88III");
    assert_eq!(profile.geometry.printable_width_dots, 512);
    assert_eq!(profile.geometry.dpi_x, 180);
    assert_eq!(profile.fonts.a.cell_width_dots, 512 / 42);
    assert!(matches!(
        profile.source,
        ProfileSource::UpstreamDefault { .. }
    ));
    assert_eq!(
        profile.code_pages.get(&0).map(String::as_str),
        Some("CP437")
    );
    // Deviations conformant:
    assert_eq!(profile.commands.esc_j, escpost_profiles::FeedBehavior::Feed);
    // Cut capability is conservatively withheld: no descriptor backs a cutter
    // distance for a synthesized profile (DD-032).
    assert!(!profile.features.paper_full_cut);
    assert!(!profile.features.paper_part_cut);
}

#[test]
fn synthesizes_vendor_and_model_catalog_metadata() {
    let profile = synthesize_profile(CAPABILITIES_JSON, "TM-T88III")
        .expect("import ok")
        .expect("TM-T88III has a width, so it synthesizes");
    assert_eq!(profile.vendor, "Epson");
    assert_eq!(profile.model, "TM-T88III");
}

#[test]
fn synthesized_upstream_default_profile_round_trips_through_canonical_json() {
    let profile = synthesize_profile(CAPABILITIES_JSON, "TM-T88III")
        .expect("import ok")
        .expect("TM-T88III has a width, so it synthesizes");
    let json = to_canonical_json(&profile).expect("a synthesized profile should serialize");
    assert_eq!(
        from_canonical_json(&json).expect("the canonical profile should verify"),
        profile
    );
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

#[test]
fn documented_constants_match_the_reference_baseline() {
    assert_eq!(defaults::DEFAULT_WIDTH_DOTS, 384);
    assert_eq!(defaults::DEFAULT_DPI, 203);
    let fonts = defaults::default_fonts();
    assert_eq!(
        (
            fonts.a.cell_width_dots,
            fonts.a.cell_height_dots,
            fonts.a.baseline_dots
        ),
        (12, 24, 20)
    );
    assert_eq!(
        (
            fonts.b.cell_width_dots,
            fonts.b.cell_height_dots,
            fonts.b.baseline_dots
        ),
        (9, 17, 14)
    );
    assert_eq!(defaults::derive_cell_width(512, 42), 12);
    assert_eq!(defaults::derive_cell_width(384, 0), 1);
}
