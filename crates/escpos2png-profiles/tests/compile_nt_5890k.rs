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
            compiled.profile.defaults.code_page,
            compiled.source.upstream_profile_sha256.as_str(),
        ),
        (
            "NT-5890K",
            4,
            384,
            203,
            203,
            203,
            203,
            30,
            0,
            RESOLVED_PROFILE_SHA256,
        )
    );
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
            "defaults.code_page",
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
                || (field.starts_with("fonts.")
                    && field != "fonts.a.columns"
                    && field != "fonts.b.columns")
            {
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
