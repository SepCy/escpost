use escpos2png_profiles::{CompileProfileError, ProfileChange, ProfileChangeKind, compile_profile};

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const RESOLVED_PROFILE_SHA256: &str =
    "2e471a3f255d2dc85988d350754023a107a882d33504dd2df5e9f3c8d4d79b0b";

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
            compiled.profile.defaults.line_spacing_dots,
            compiled.profile.fonts.a.columns,
            compiled.profile.fonts.b.columns,
            compiled.source.upstream_profile_sha256.as_str(),
        ),
        (
            "NT-5890K",
            2,
            384,
            203,
            203,
            203,
            203,
            30,
            32,
            42,
            RESOLVED_PROFILE_SHA256,
        )
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

    assert_eq!(
        compiled.changes,
        [
            "geometry.printable_width_dots",
            "geometry.dpi_x",
            "geometry.dpi_y",
            "motion.horizontal_units_per_inch",
            "motion.vertical_units_per_inch",
            "defaults.line_spacing_dots",
            "fonts.a.columns",
            "fonts.b.columns",
        ]
        .map(|field| ProfileChange {
            field: field.to_owned(),
            kind: if field.starts_with("motion.") || field == "defaults.line_spacing_dots" {
                ProfileChangeKind::Added
            } else {
                ProfileChangeKind::Confirmed
            },
        })
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
