use escpos2png::{RenderError, render};
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const LF: u8 = 0x0a;

#[test]
fn default_font_a_uses_profile_cells_instead_of_source_font_advance() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [b'A', b'B', ESC, b'*', 1, 1, 0, 0b1000_0000, LF];

    let rendered = render(&input, &profile).expect("printable ASCII should render");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 30));
    assert!(cell_contains_printed_dots(surface, 0, 12, 24));
    assert!(cell_contains_printed_dots(surface, 12, 12, 24));
    // The three-dot ESC * marker starts immediately after two 12-dot cells.
    // Its position is independent of Noto Sans Mono's own glyph advance.
    assert!(surface.is_printed(24, 0));
    assert!(surface.is_printed(24, 1));
    assert!(surface.is_printed(24, 2));
    assert!(
        !(25..surface.width()).any(|x| { (0..surface.height()).any(|y| surface.is_printed(x, y)) })
    );
}

#[test]
fn esc_m_selects_font_b_with_its_nine_dot_profile_cells() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [
        ESC,
        b'M',
        1,
        b'A',
        b'B',
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
    ];

    let rendered = render(&input, &profile).expect("ESC M should select Font B");
    let surface = &rendered.sheets[0].surface;

    assert!(cell_contains_printed_dots(surface, 0, 9, 17));
    assert!(cell_contains_printed_dots(surface, 9, 9, 17));
    // Font B advances by 9 dots, so the marker follows at x=18.
    assert!(surface.is_printed(18, 0));
    assert!(surface.is_printed(18, 1));
    assert!(surface.is_printed(18, 2));
}

#[test]
fn text_wraps_at_the_profile_font_column_boundary() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let mut input = vec![b'A'; 33];
    input.push(LF);

    let rendered = render(&input, &profile).expect("text should wrap");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 60));
    assert!(cell_contains_printed_dots(surface, 0, 12, 24));
    // Font A has 32 columns on this profile; character 33 starts on line two.
    assert!((0..12).any(|x| (30..54).any(|y| surface.is_printed(x, y))));
}

#[test]
fn esc_bang_double_size_scales_font_a_cells_and_line_advance() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [ESC, b'!', 0x30, b'A', ESC, b'*', 1, 1, 0, 0b1000_0000, LF];

    let rendered = render(&input, &profile).expect("ESC ! should scale text");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 48));
    assert!(surface.is_printed(24, 0));
    assert!(surface.is_printed(24, 1));
    assert!(surface.is_printed(24, 2));
    assert!(cell_contains_printed_dots(surface, 0, 24, 48));
}

#[test]
fn esc_e_emphasizes_text_without_changing_cell_advance() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [b'A', ESC, b'E', 1, b'A', LF];

    let rendered = render(&input, &profile).expect("ESC E should emphasize text");
    let surface = &rendered.sheets[0].surface;

    let normal_dots = count_printed_dots(surface, 0, 12, 24);
    let emphasized_dots = count_printed_dots(surface, 12, 12, 24);
    assert!(emphasized_dots > normal_dots);
    // Emphasis adds ink but must not shift the second character's cell.
    for y in 0..24 {
        for x in 0..12 {
            if surface.is_printed(x, y) {
                assert!(surface.is_printed(x + 12, y));
            }
        }
    }
}

#[test]
fn esc_minus_underlines_spaces_across_the_profile_cell() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [ESC, b'-', 1, b' ', LF];

    let rendered = render(&input, &profile).expect("ESC - should underline text");
    let surface = &rendered.sheets[0].surface;

    for x in 0..12 {
        assert!(surface.is_printed(x, 23), "missing underline dot at x={x}");
    }
    assert_eq!(count_printed_dots(surface, 0, 12, 24), 12);
}

#[test]
fn gs_b_reverse_prints_the_background_of_a_space_cell() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [GS, b'B', 1, b' ', LF];

    let rendered = render(&input, &profile).expect("GS B should reverse text");
    let surface = &rendered.sheets[0].surface;

    assert_eq!(count_printed_dots(surface, 0, 12, 24), 12 * 24);
}

#[test]
fn esc_a_centers_the_composed_line_by_its_profile_cell_width() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
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
fn esc_t_decodes_extended_bytes_with_the_profile_cp437_mapping() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [ESC, b't', 0, 0x82, ESC, b'*', 1, 1, 0, 0b1000_0000, LF];

    let rendered = render(&input, &profile).expect("ESC t CP437 text should render");
    let surface = &rendered.sheets[0].surface;

    // CP437 82h is "é". A non-empty first cell proves it was decoded and
    // rasterized; the marker proves it retained Font A's 12-dot advance.
    assert!(cell_contains_printed_dots(surface, 0, 12, 24));
    assert!(surface.is_printed(12, 0));
    assert!(surface.is_printed(12, 1));
    assert!(surface.is_printed(12, 2));
}

#[test]
fn esc_t_uses_the_encoding_mapped_to_each_profile_slot() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;

    // The cent sign is byte 9Bh in CP437 but byte BDh in CP850. Rendering
    // both through their profile slots must therefore produce the same dots.
    let cp437 =
        render(&[ESC, b't', 0, 0x9b, LF], &profile).expect("profile slot 0 should decode CP437");
    let cp850 =
        render(&[ESC, b't', 2, 0xbd, LF], &profile).expect("profile slot 2 should decode CP850");

    assert_eq!(cp437.sheets[0].surface, cp850.sheets[0].surface);
}

#[test]
fn rendering_reports_an_unsupported_default_code_page_without_panicking() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    profile.defaults.code_page = 1;

    let error = render(b"A", &profile)
        .expect_err("CP932 needs a multi-byte decoder and is not supported yet");

    assert!(matches!(
        error,
        RenderError::UnsupportedCodePage {
            code_page: 1,
            ref encoding,
            offset: 0,
        } if encoding == "CP932"
    ));
}

fn cell_contains_printed_dots(
    surface: &escpos2png::MonoSurface,
    left: u32,
    width: u32,
    height: u32,
) -> bool {
    (left..left + width).any(|x| (0..height).any(|y| surface.is_printed(x, y)))
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
