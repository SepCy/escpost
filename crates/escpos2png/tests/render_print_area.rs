use escpos2png::render;
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const LF: u8 = 0x0a;

#[test]
fn gs_l_and_gs_w_define_the_standard_mode_print_area() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [
        GS, b'L', 24, 0, GS, b'W', 120, 0, ESC, b'a', 1, GS, b'B', 1, b' ', LF,
    ];

    let rendered = render(&input, &profile).expect("the print area should render");
    let surface = &rendered.sheets[0].surface;

    // The visible reversed-space cell is centered inside the 120-dot area:
    // physical x = 24-dot margin + (120 - 12-dot cell) / 2 = 78.
    assert_eq!(count_printed_dots(surface, 78, 12, 24), 12 * 24);
    assert_eq!(count_printed_dots(surface, 0, 78, 24), 0);
    assert_eq!(count_printed_dots(surface, 90, 294, 24), 0);
}

#[test]
fn gs_l_and_gs_w_bound_right_justified_raster_graphics() {
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
        40,
        0,
        ESC,
        b'a',
        2,
        GS,
        b'v',
        b'0',
        0,
        1,
        0,
        1,
        0,
        0b1000_0000,
    ];

    let rendered = render(&input, &profile).expect("the bounded raster should render");
    let surface = &rendered.sheets[0].surface;

    // The image occupies its complete declared byte width of eight dots.
    // Right alignment places it at 24 + (40 - 8) = physical x=56.
    assert!(surface.is_printed(56, 0));
    assert!(!surface.is_printed(24, 0));
    assert!(!surface.is_printed(0, 0));
}

#[test]
fn gs_w_clips_raster_graphics_at_the_print_area_edge() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [
        GS, b'L', 24, 0, GS, b'W', 8, 0, GS, b'v', b'0', 0, 2, 0, 1, 0, 0xff, 0xff,
    ];

    let rendered = render(&input, &profile).expect("the oversized raster should be clipped");
    let surface = &rendered.sheets[0].surface;

    // The declared image is 16 dots wide, but only the first eight dots fit
    // inside the active area from physical x=24 through x=31.
    assert_eq!(count_printed_dots(surface, 24, 8, 1), 8);
    assert_eq!(count_printed_dots(surface, 0, 24, 1), 0);
    assert_eq!(count_printed_dots(surface, 32, 352, 1), 0);
}

#[test]
fn text_wraps_at_the_active_print_area_width() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [GS, b'W', 24, 0, GS, b'B', 1, b' ', b' ', b' ', LF];

    let rendered = render(&input, &profile).expect("text should wrap inside GS W");
    let surface = &rendered.sheets[0].surface;

    // Two 12-dot Font A cells fit on the first line. The third cell wraps to
    // the next profile-default 30-dot line.
    assert_eq!((surface.width(), surface.height()), (384, 60));
    assert_eq!(count_printed_dots(surface, 0, 24, 24), 24 * 24);
    assert_eq!(count_printed_dots(surface, 0, 12, 54) - 12 * 24, 12 * 24);
    assert_eq!(count_printed_dots(surface, 24, 360, 60), 0);
}

#[test]
fn gs_w_clips_column_format_graphics_at_the_print_area_edge() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let mut input = vec![GS, b'W', 8, 0, 0x1b, b'*', 1, 16, 0];
    input.extend([0b1000_0000; 16]);
    input.push(LF);

    let rendered = render(&input, &profile).expect("oversized ESC * should be clipped");
    let surface = &rendered.sheets[0].surface;

    // Sixteen source columns are declared, but the active area keeps only the
    // first eight printer-dot columns.
    assert_eq!(count_printed_dots(surface, 0, 8, 3), 8 * 3);
    assert_eq!(count_printed_dots(surface, 8, 376, 3), 0);
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
