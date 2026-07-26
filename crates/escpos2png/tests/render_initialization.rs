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

#[test]
fn esc_at_restores_barcode_defaults() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    profile.features.barcode_b = true;
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'H', 2, GS, b'f', 1, ESC, b'@', GS, b'k', 67, 12, b'5',
        b'9', b'0', b'1', b'2', b'3', b'4', b'1', b'2', b'3', b'4', b'5',
    ];

    let rendered = render(&input, &profile).expect("ESC @ should restore barcode defaults");
    let surface = &rendered.sheets[0].surface;

    // Reset removes HRI and restores the common 162-dot height and three-dot
    // module width. The first EAN-13 guard module is therefore three dots.
    assert_eq!(surface.height(), 162);
    assert!((0..3).all(|x| surface.is_printed(x, 0)));
    assert!(!surface.is_printed(3, 0));
}

#[test]
fn esc_at_clears_qr_data_and_restores_qr_defaults() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    profile.features.qr_code = true;
    let input = [
        GS, b'(', b'k', 3, 0, 49, 67, 2, GS, b'(', b'k', 3, 0, 49, 69, 51, GS, b'(', b'k', 4, 0,
        49, 80, 48, b'X', ESC, b'@',
        // A new store is required because reset discarded X. Defaults make
        // this single-byte symbol 21 modules at three dots per module.
        GS, b'(', b'k', 4, 0, 49, 80, 48, b'A', GS, b'(', b'k', 3, 0, 49, 81, 48,
    ];

    let rendered = render(&input, &profile).expect("ESC @ should restore QR defaults");

    assert_eq!(rendered.sheets[0].surface.height(), 63);
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
