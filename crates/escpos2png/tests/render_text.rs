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
        .expect("the test profile should compile");
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
        .expect("the test profile should compile");
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
        .expect("the test profile should compile");
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
        .expect("the test profile should compile");
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
fn gs_bang_scales_character_width_and_height_independently_up_to_eight_times() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [GS, b'!', 0x23, GS, b'B', 1, b' ', LF];

    let rendered = render(&input, &profile).expect("GS ! should scale the character");
    let surface = &rendered.sheets[0].surface;

    // 23h selects width x3 (bits 4–6 = 2) and height x4 (bits 0–2 = 3).
    // A reversed space exposes the complete scaled 12×24-dot Font A cell.
    assert_eq!((surface.width(), surface.height()), (384, 96));
    assert_eq!(count_printed_dots(surface, 0, 36, 96), 36 * 96);
}

#[test]
fn esc_e_emphasizes_text_without_changing_cell_advance() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
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
        .expect("the test profile should compile");
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
        .expect("the test profile should compile");
    let input = [GS, b'B', 1, b' ', LF];

    let rendered = render(&input, &profile).expect("GS B should reverse text");
    let surface = &rendered.sheets[0].surface;

    assert_eq!(count_printed_dots(surface, 0, 12, 24), 12 * 24);
}

#[test]
fn esc_t_decodes_extended_bytes_with_the_profile_cp437_mapping() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
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
        .expect("the test profile should compile");

    // The cent sign is byte 9Bh in CP437 but byte BDh in CP850. Rendering
    // both through their profile slots must therefore produce the same dots.
    let cp437 =
        render(&[ESC, b't', 0, 0x9b, LF], &profile).expect("profile slot 0 should decode CP437");
    let cp850 =
        render(&[ESC, b't', 2, 0xbd, LF], &profile).expect("profile slot 2 should decode CP850");

    assert_eq!(cp437.sheets[0].surface, cp850.sheets[0].surface);
}

#[test]
fn esc_t_decodes_a_windows_code_page_selected_by_the_profile() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");

    // The cent sign is byte 9Bh in CP437 and A2h in Windows-1252.
    let cp437 =
        render(&[ESC, b't', 0, 0x9b, LF], &profile).expect("profile slot 0 should decode CP437");
    let cp1252 =
        render(&[ESC, b't', 16, 0xa2, LF], &profile).expect("profile slot 16 should decode CP1252");

    assert_eq!(cp437.sheets[0].surface, cp1252.sheets[0].surface);
}

#[test]
fn esc_t_accepts_the_profiles_supported_single_byte_code_pages() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let supported_slots = [
        0,  // CP437
        2,  // CP850
        3,  // CP860
        4,  // CP863
        5,  // CP865
        16, // CP1252
        17, // CP866
        18, // CP852
        19, // CP858
        25, // CP1257
        27, // CP1258
        28, // CP864
        32, // CP1255
        56, // CP861
        60, // CP855
        61, // CP857
        62, // CP862
        64, // CP737
        66, // CP869
        72, // CP1250
        73, // CP1251
        90, // CP1253
        91, // CP1254
        92, // CP1256
        93, // CP720
        95, // CP775
    ];

    for code_page in supported_slots {
        render(&[ESC, b't', code_page, b'A', LF], &profile)
            .unwrap_or_else(|error| panic!("profile slot {code_page} should render: {error}"));
    }
}

#[test]
fn undefined_code_page_bytes_return_an_error_instead_of_a_replacement_glyph() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");

    let error = render(&[ESC, b't', 28, 0xa6], &profile)
        .expect_err("A6h is undefined in the profile's CP864 table");

    assert!(
        matches!(
            error,
            RenderError::UndefinedCodePageByte {
                byte: 0xa6,
                code_page: 28,
                ref encoding,
                offset: 3,
            } if encoding == "CP864"
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
fn characters_missing_from_the_bundled_font_return_an_error() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");

    let error = render(&[ESC, b't', 62, 0x80], &profile)
        .expect_err("CP862 80h is Hebrew alef, which is not in the current font asset");

    assert!(matches!(
        error,
        RenderError::MissingGlyph {
            character: 'א',
            offset: 3,
        }
    ));
}

#[test]
fn rendering_reports_an_unsupported_default_code_page_without_panicking() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
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
