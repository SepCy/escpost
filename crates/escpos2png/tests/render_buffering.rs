use escpos2png::render;
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const GS: u8 = 0x1d;
const LF: u8 = 0x0a;

#[test]
fn gs_l_and_gs_w_are_ignored_after_the_line_has_started() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [
        GS, b'B', 1, b' ', GS, b'L', 24, 0, GS, b'W', 120, 0, LF, b' ', LF,
    ];

    let rendered = render(&input, &profile).expect("mid-line print-area commands are ignored");
    let surface = &rendered.sheets[0].surface;

    // Epson enables GS L and GS W only at the beginning of a line. Both
    // reversed cells therefore remain at the default physical x=0.
    assert_eq!(count_printed_dots(surface, 0, 12, 24), 12 * 24);
    assert_eq!(count_printed_dots(surface, 0, 12, 54), 12 * 48);
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
