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
fn gs_v_partial_cut_uses_its_own_profile_capability() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    profile.features.paper_part_cut = true;
    let marker_line = [ESC, b'*', 1, 1, 0, 0b1000_0000, LF];
    let input = [
        marker_line.as_slice(),
        &[GS, b'V', b'1'],
        marker_line.as_slice(),
    ]
    .concat();

    let rendered = render(&input, &profile).expect("a supported partial cut should split sheets");

    // A partial cut still separates the preview into physical receipt sheets.
    // The uncut bridge is a mechanism detail, not printable content.
    assert_eq!(rendered.sheets.len(), 2);
    assert!(
        rendered
            .sheets
            .iter()
            .all(|sheet| sheet.surface.is_printed(0, 0))
    );
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

#[test]
fn gs_v_function_b_feeds_without_cutting_on_a_printer_without_an_autocutter() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [
        // At 203 dpi, 101 vertical units per inch make each unit two dots.
        GS, b'P', 203, 101, GS, b'V', 65, 5, GS, b'V', 66, 5,
    ];

    let rendered =
        render(&input, &profile).expect("Function B should remain a feed on this printer");

    // Epson specifies that a printer without an autocutter performs only the
    // requested n-unit feed. Full/partial selection cannot create a cut that
    // the mechanism does not have.
    assert_eq!(rendered.sheets.len(), 1);
    assert_eq!(
        (
            rendered.sheets[0].surface.width(),
            rendered.sheets[0].surface.height(),
        ),
        (384, 20)
    );
}
