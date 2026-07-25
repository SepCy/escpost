use escpos2png::render;
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const HT: u8 = 0x09;
const LF: u8 = 0x0a;

#[test]
fn esc_at_clears_pending_data_but_preserves_already_fed_paper() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [
        // Commit one visible reversed cell to the roll.
        GS, b'B', 1, b' ', LF, //
        // Compose another cell, then initialize before it is fed.
        b' ', ESC, b'@', LF,
    ];

    let rendered = render(&input, &profile).expect("ESC @ should initialize the printer");
    let surface = &rendered.sheets[0].surface;

    // Initialization clears only pending print data. The first 30-dot line
    // remains, while the post-reset LF contributes one blank 30-dot line.
    assert_eq!((surface.width(), surface.height()), (384, 60));
    assert_eq!(count_printed_dots(surface, 0, 12, 24), 12 * 24);
    assert_eq!(count_printed_dots(surface, 0, 384, 60) - 12 * 24, 0);
}

#[test]
fn esc_at_restores_profile_layout_motion_and_text_defaults() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [
        GS,
        b'L',
        24,
        0,
        GS,
        b'W',
        120,
        0,
        GS,
        b'P',
        100,
        100,
        ESC,
        b'3',
        10,
        ESC,
        b'M',
        1,
        ESC,
        b' ',
        5,
        ESC,
        b'a',
        2,
        GS,
        b'B',
        1,
        GS,
        b'!',
        0x11,
        ESC,
        b'@',
        b'A',
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
    ];

    let rendered = render(&input, &profile).expect("ESC @ should restore profile defaults");
    let surface = &rendered.sheets[0].surface;

    // Reset Font A advances by 12 dots, so the marker follows at x=12.
    // Its physical origin and 30-dot feed also prove margin, justification,
    // motion-unit, line-spacing, reverse, and size state were reset.
    assert!(surface.is_printed(12, 0));
    assert!(surface.is_printed(12, 1));
    assert!(surface.is_printed(12, 2));
    assert_eq!(surface.height(), 30);
    assert!(!surface.is_printed(24, 0));
}

#[test]
fn esc_at_restores_default_code_page_and_tab_stops() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [
        ESC,
        b't',
        16,
        ESC,
        b'D',
        2,
        0,
        ESC,
        b'@',
        0x82,
        HT,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
    ];

    let rendered = render(&input, &profile).expect("ESC @ should restore table and tabs");
    let surface = &rendered.sheets[0].surface;

    // Slot 0 decodes 82h as "é". The default first tab is eight 12-dot Font A
    // cells from the origin, so the marker must start at x=96.
    assert!(cell_contains_printed_dots(surface, 0, 12, 24));
    assert!(surface.is_printed(96, 0));
    assert!(surface.is_printed(96, 1));
    assert!(surface.is_printed(96, 2));
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

fn cell_contains_printed_dots(
    surface: &escpos2png::MonoSurface,
    left: u32,
    width: u32,
    height: u32,
) -> bool {
    (left..left + width).any(|x| (0..height).any(|y| surface.is_printed(x, y)))
}
