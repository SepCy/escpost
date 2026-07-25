use escpos2png::render;
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");

#[test]
fn esc_3_sets_line_spacing_in_profile_motion_units() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [0x1b, 0x33, 7, 0x0a];

    let rendered = render(&input, &profile).expect("ESC 3 should set line spacing");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 7));
}

#[test]
fn esc_2_restores_the_profile_default_line_spacing() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [0x1b, 0x33, 7, 0x0a, 0x1b, 0x32, 0x0a];

    let rendered = render(&input, &profile).expect("ESC 2 should restore line spacing");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 37));
}

#[test]
fn esc_d_feeds_n_current_lines_without_changing_line_spacing() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [0x1b, 0x33, 7, 0x1b, 0x64, 3, 0x0a];

    let rendered = render(&input, &profile).expect("ESC d should feed whole lines");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 28));
}
