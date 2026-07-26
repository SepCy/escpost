use escpos2png::{RenderError, render};
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const LF: u8 = 0x0a;

#[test]
fn gs_v_full_cut_finishes_one_sheet_and_starts_the_next() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    // The physical NT-5890K has no cutter. Enable this capability only in the
    // test profile so the generic renderer's sheet behavior can be exercised.
    profile.features.paper_full_cut = true;
    let marker_line = [ESC, b'*', 1, 1, 0, 0b1000_0000, LF];
    let input = [
        marker_line.as_slice(),
        &[GS, b'V', 0],
        marker_line.as_slice(),
    ]
    .concat();

    let rendered = render(&input, &profile).expect("GS V should create a sheet boundary");

    assert_eq!(rendered.sheets.len(), 2);
    for sheet in &rendered.sheets {
        assert_eq!((sheet.surface.width(), sheet.surface.height()), (384, 30));
        assert!(sheet.surface.is_printed(0, 0));
    }
}

#[test]
fn gs_v_reports_a_cut_that_the_profile_does_not_support() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");

    let error =
        render(&[GS, b'V', 0], &profile).expect_err("the NT-5890K has no full-cut capability");

    assert!(matches!(
        error,
        RenderError::CommandUnsupportedByProfile {
            command: "GS V full cut",
            ref profile,
            offset: 0,
        } if profile == "NT-5890K"
    ));
}
