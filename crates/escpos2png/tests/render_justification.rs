use escpos2png::render;
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const LF: u8 = 0x0a;

#[test]
fn esc_a_centers_text_by_its_profile_cell_width() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [ESC, b'a', 1, GS, b'B', 1, b' ', LF];

    let rendered = render(&input, &profile).expect("ESC a should center the line");
    let surface = &rendered.sheets[0].surface;

    // (384 - 12) / 2 = 186. A reversed space makes the complete cell
    // visible, so this assertion detects even a one-dot centering error.
    assert_eq!(count_printed_dots(surface, 186, 12, 24), 12 * 24);
    assert_eq!(count_printed_dots(surface, 0, 186, 24), 0);
    assert_eq!(count_printed_dots(surface, 198, 186, 24), 0);
}

#[test]
fn esc_a_right_justifies_text_by_its_profile_cell_width() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [ESC, b'a', 2, GS, b'B', 1, b' ', LF];

    let rendered = render(&input, &profile).expect("ESC a should right-justify the line");
    let surface = &rendered.sheets[0].surface;

    // A 12-dot reversed space occupies x=372..383 in the 384-dot print area.
    assert_eq!(count_printed_dots(surface, 372, 12, 24), 12 * 24);
    assert_eq!(count_printed_dots(surface, 0, 372, 24), 0);
}

#[test]
fn esc_a_right_justifies_column_format_graphics() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [ESC, b'a', 2, ESC, b'*', 1, 1, 0, 0b1000_0000, LF];

    let rendered = render(&input, &profile).expect("ESC * should use line justification");
    let surface = &rendered.sheets[0].surface;

    // Mode 1 produces one horizontal printer dot. Right justification moves
    // that complete one-dot image to the final x coordinate.
    assert!(surface.is_printed(383, 0));
    assert!(!surface.is_printed(383, 1));
    assert!(!surface.is_printed(0, 0));
}

#[test]
fn esc_a_centers_a_raster_image_inside_the_active_print_area() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [ESC, b'a', 1, GS, b'v', b'0', 0, 1, 0, 1, 0, 0b1000_0000];

    let rendered = render(&input, &profile).expect("the centered raster should render");
    let surface = &rendered.sheets[0].surface;

    // One source byte is an eight-dot-wide image. Centering it in 384 dots
    // moves its first printed source bit to (384 - 8) / 2 = x=188.
    assert!(surface.is_printed(188, 0));
    assert!(!surface.is_printed(0, 0));
}

#[test]
fn esc_a_places_one_dimensional_barcodes_inside_the_active_print_area() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");

    for (justification, expected_left) in [(0, 0), (1, 97), (2, 194)] {
        let input = [
            ESC,
            b'a',
            justification,
            GS,
            b'h',
            1,
            GS,
            b'w',
            2,
            GS,
            b'k',
            67,
            12,
            b'5',
            b'9',
            b'0',
            b'1',
            b'2',
            b'3',
            b'4',
            b'1',
            b'2',
            b'3',
            b'4',
            b'5',
        ];

        let rendered = render(&input, &profile).expect("ESC a should place the barcode");
        let surface = &rendered.sheets[0].surface;

        // EAN-13 is 95 modules, or 190 dots at width two. Its guard bars make
        // both outer edges directly observable.
        assert!(surface.is_printed(expected_left, 0));
        assert!(surface.is_printed(expected_left + 189, 0));
        assert_eq!(count_printed_dots(surface, 0, expected_left, 1), 0);
        assert_eq!(
            count_printed_dots(surface, expected_left + 190, 384 - expected_left - 190, 1),
            0
        );
    }
}

#[test]
fn esc_a_places_qr_symbols_inside_the_active_print_area() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    profile.features.qr_code = true;

    for (justification, expected_left) in [(0, 0), (1, 171), (2, 342)] {
        let input = [
            ESC,
            b'a',
            justification,
            GS,
            b'(',
            b'k',
            3,
            0,
            49,
            67,
            2,
            GS,
            b'(',
            b'k',
            4,
            0,
            49,
            80,
            48,
            b'A',
            GS,
            b'(',
            b'k',
            3,
            0,
            49,
            81,
            48,
        ];

        let rendered = render(&input, &profile).expect("ESC a should place the QR symbol");
        let surface = &rendered.sheets[0].surface;

        // A Version 1 symbol is 21 modules, or 42 dots at width two. The
        // top-left and top-right finder borders touch both symbol edges.
        assert!(surface.is_printed(expected_left, 0));
        assert!(surface.is_printed(expected_left + 41, 0));
        assert_eq!(count_printed_dots(surface, 0, expected_left, 42), 0);
        assert_eq!(
            count_printed_dots(surface, expected_left + 42, 384 - expected_left - 42, 42),
            0
        );
    }
}

#[test]
fn justification_uses_the_farthest_composed_dot_after_moving_backwards() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [
        ESC,
        b'a',
        1,
        ESC,
        b'$',
        100,
        0,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        ESC,
        b'$',
        20,
        0,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
    ];

    let rendered = render(&input, &profile).expect("the repositioned line should center");
    let surface = &rendered.sheets[0].surface;

    // The far marker ends at x=101, so the complete line moves right by
    // floor((384 - 101) / 2)=141. The cursor ending at x=21 must not shrink
    // the line's composed width.
    assert!(surface.is_printed(241, 0));
    assert!(surface.is_printed(161, 0));
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
