use escpos2png::render;
use escpos2png_profiles::{CarriageReturnMode, FeedBehavior, compile_profile};

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const CR: u8 = 0x0d;
const LF: u8 = 0x0a;

#[test]
fn cr_uses_the_profile_auto_line_feed_behavior() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [GS, b'B', 1, b' ', CR, b' ', LF];

    let rendered = render(&input, &profile).expect("CR should use the selected printer behavior");
    let surface = &rendered.sheets[0].surface;

    // The NT-5890K profile has auto line feed disabled, so CR is ignored.
    // Both reversed spaces stay beside one another on the same logical line.
    assert_eq!((surface.width(), surface.height()), (384, 30));
    assert_eq!(count_printed_dots(surface, 0, 24, 24), 24 * 24);
}

#[test]
fn cr_prints_and_feeds_when_the_profile_enables_auto_line_feed() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    profile.defaults.carriage_return = CarriageReturnMode::LineFeed;
    let input = [GS, b'B', 1, b' ', CR, b' ', LF];

    let rendered =
        render(&input, &profile).expect("CR should behave like LF when auto line feed is enabled");
    let surface = &rendered.sheets[0].surface;

    // CR commits the first cell and moves back to the next line's origin.
    // The following LF then commits the second cell as a separate line.
    assert_eq!((surface.width(), surface.height()), (384, 60));
    assert_eq!(count_printed_dots(surface, 0, 12, 54), 12 * 48);
    assert_eq!(count_printed_dots(surface, 12, 372, 54), 0);
}

#[test]
fn esc_3_sets_line_spacing_in_profile_motion_units() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [ESC, 0x33, 7, LF];

    let rendered = render(&input, &profile).expect("ESC 3 should set line spacing");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 7));
}

#[test]
fn esc_2_restores_the_profile_default_line_spacing() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [ESC, 0x33, 7, LF, ESC, 0x32, LF];

    let rendered = render(&input, &profile).expect("ESC 2 should restore line spacing");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 37));
}

#[test]
fn esc_d_feeds_n_current_lines_without_changing_line_spacing() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [ESC, 0x33, 7, ESC, 0x64, 3, LF];

    let rendered = render(&input, &profile).expect("ESC d should feed whole lines");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 28));
}

#[test]
fn epson_esc_j_prints_and_feeds_a_temporary_motion_unit_distance() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    profile.commands.esc_j = FeedBehavior::Feed;
    let input = [
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        ESC,
        b'J',
        10,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
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
fn nt_5890k_consumes_esc_j_without_feeding() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let raster = [GS, b'v', b'0', 0, 1, 0, 1, 0, 0b1000_0000];
    let input = [raster.as_slice(), &[ESC, b'J', 10], raster.as_slice()].concat();

    let rendered = render(&input, &profile).expect("ignored ESC J should still be consumed");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 2));
    assert!(surface.is_printed(0, 0));
    assert!(surface.is_printed(0, 1));
}

#[test]
fn gs_v0_feeds_by_image_height_instead_of_the_selected_line_spacing() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [
        ESC,
        b'3',
        50,
        GS,
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

#[test]
fn column_graphics_advance_by_the_selected_line_spacing() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [
        ESC,
        b'3',
        1,
        ESC,
        b'*',
        33,
        1,
        0,
        0,
        0,
        0b0000_0001,
        LF,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
    ];

    let rendered = render(&input, &profile).expect("ESC * should use the selected line spacing");
    let surface = &rendered.sheets[0].surface;

    // ESC * does not replace the line spacing. The first image reaches y=23,
    // while the second image starts only one dot below the first line origin.
    // Explicit 24-dot spacing is needed when callers want adjacent image rows.
    assert!(surface.is_printed(0, 23));
    assert!(surface.is_printed(0, 1));
    assert_eq!(surface.height(), 24);
}

fn count_printed_dots(
    surface: &escpos2png::MonoSurface,
    left: u32,
    width: u32,
    height: u32,
) -> usize {
    (left..left + width)
        .flat_map(|x| (0..height).map(move |y| (x, y)))
        .filter(|&(x, y)| surface.is_printed(x, y))
        .count()
}
