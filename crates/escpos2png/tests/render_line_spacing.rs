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

#[test]
fn esc_j_prints_and_feeds_a_temporary_motion_unit_distance() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [
        0x1b,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        0x1b,
        b'J',
        10,
        0x1b,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        0x0a,
    ];

    let rendered = render(&input, &profile).expect("ESC J should print and feed");
    let surface = &rendered.sheets[0].surface;

    // The profile uses one dot per vertical motion unit. ESC J places the
    // second marker at y=10 without replacing the 30-dot line-spacing state.
    assert_eq!((surface.width(), surface.height()), (384, 40));
    assert!(surface.is_printed(0, 0));
    assert!(surface.is_printed(0, 10));
}

#[test]
fn gs_v0_feeds_by_image_height_instead_of_the_selected_line_spacing() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [
        0x1b,
        b'3',
        50,
        0x1d,
        b'v',
        b'0',
        0,
        1,
        0,
        2,
        0,
        0b1000_0000,
        0b1000_0000,
    ];

    let rendered = render(&input, &profile).expect("GS v 0 should ignore line spacing");
    let surface = &rendered.sheets[0].surface;

    // GS v 0 advances by its two rendered rows. ESC 3 remains active for the
    // next text line but must not add a 50-dot feed to this image.
    assert_eq!(surface.height(), 2);
    assert!(surface.is_printed(0, 0));
    assert!(surface.is_printed(0, 1));
}
