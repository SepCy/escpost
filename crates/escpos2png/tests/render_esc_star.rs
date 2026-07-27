use escpos2png::{RenderError, render};
use escpos2png_profiles::compile_profile;
use std::io::Cursor;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
const INPUT_HEX: &str =
    include_str!("../../../tests/cases/graphics/esc-star-8dot-double-density/input.hex");

#[test]
fn renders_nt_5890k_8_dot_mode_with_adjacent_vertical_rows() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = decode_hex(INPUT_HEX);

    let rendered = render(&input, &profile).expect("the conformance case should render");

    assert_eq!(rendered.sheets.len(), 1);
    let surface = &rendered.sheets[0].surface;
    assert_eq!((surface.width(), surface.height()), (384, 30));

    let columns = [0x81_u8, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81];
    for y in 0..surface.height() {
        for x in 0..surface.width() {
            let expected = x < 8 && y < 8 && columns[x as usize] & (0x80 >> y) != 0;
            assert_eq!(
                surface.is_printed(x, y),
                expected,
                "unexpected dot at ({x}, {y})"
            );
        }
    }
}

#[test]
fn renders_epson_8_dot_geometry_as_one_dot_on_a_three_dot_vertical_pitch() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    profile.column_bit_image.eight_dot_vertical_pitch_dots = 3;
    let input = [0x1b, 0x40, 0x1b, 0x2a, 0, 1, 0, 0x81, 0x0a];

    let rendered = render(&input, &profile).expect("single-density ESC * should render");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 30));
    for y in 0..surface.height() {
        for x in 0..surface.width() {
            let expected = x < 2 && (y == 0 || y == 21);
            assert_eq!(
                surface.is_printed(x, y),
                expected,
                "unexpected dot at ({x}, {y})"
            );
        }
    }
}

#[test]
fn renders_esc_star_24_dot_single_density_at_two_by_one_printer_dots() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [0x1b, 0x40, 0x1b, 0x2a, 32, 1, 0, 0x80, 0x00, 0x01, 0x0a];

    let rendered = render(&input, &profile).expect("24-dot ESC * should render");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 30));
    for y in 0..surface.height() {
        for x in 0..surface.width() {
            let expected = x < 2 && (y == 0 || y == 23);
            assert_eq!(
                surface.is_printed(x, y),
                expected,
                "unexpected dot at ({x}, {y})"
            );
        }
    }
}

#[test]
fn renders_esc_star_24_dot_double_density_at_one_printer_dot_per_source_dot() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [0x1b, 0x40, 0x1b, 0x2a, 33, 1, 0, 0x80, 0x00, 0x01, 0x0a];

    let rendered = render(&input, &profile).expect("24-dot ESC * should render");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 30));
    for y in 0..surface.height() {
        for x in 0..surface.width() {
            let expected = x == 0 && (y == 0 || y == 23);
            assert_eq!(
                surface.is_printed(x, y),
                expected,
                "unexpected dot at ({x}, {y})"
            );
        }
    }
}

#[test]
fn encodes_the_rendered_sheet_as_one_bit_grayscale_png() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = decode_hex(INPUT_HEX);

    let rendered = render(&input, &profile).expect("the conformance case should render");
    let png = &rendered.sheets[0].png;
    let decoder = png::Decoder::new(Cursor::new(png));
    let mut reader = decoder.read_info().expect("the output should be valid PNG");
    let mut pixels = vec![
        0;
        reader
            .output_buffer_size()
            .expect("the decoded image should fit in memory")
    ];
    let info = reader
        .next_frame(&mut pixels)
        .expect("the PNG should contain one complete frame");

    assert_eq!((info.width, info.height), (384, 30));
    assert_eq!(info.color_type, png::ColorType::Grayscale);
    assert_eq!(info.bit_depth, png::BitDepth::One);

    let columns = [0x81_u8, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81];
    let row_bytes = 384 / 8;
    for y in 0..info.height {
        for x in 0..info.width {
            let byte = pixels[(y * row_bytes + x / 8) as usize];
            let is_black = byte & (0x80 >> (x % 8)) == 0;
            let expected = x < 8 && y < 8 && columns[x as usize] & (0x80 >> y) != 0;
            assert_eq!(is_black, expected, "unexpected PNG pixel at ({x}, {y})");
        }
    }
}

#[test]
fn reports_column_bit_images_that_the_profile_does_not_support() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    profile.features.bit_image_column = false;

    let error = render(&[0x1b, b'*', 1, 1, 0, 0x80], &profile)
        .expect_err("ESC * should be gated by the selected profile");

    assert!(matches!(
        error,
        RenderError::CommandUnsupportedByProfile {
            command: "ESC * column bit image",
            ref profile,
            offset: 0,
        } if profile == "NT-5890K"
    ));
}

fn decode_hex(encoded: &str) -> Vec<u8> {
    encoded
        .split_ascii_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("fixture contains valid byte hex"))
        .collect()
}
