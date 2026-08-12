use escpost_profiles::{PrinterProfile, compile_profile};
use escpost_render::{LimitKind, RenderError, render};

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
const GS: u8 = 0x1d;
const ESC: u8 = 0x1b;

#[test]
fn gs_parenthesized_l_stores_then_prints_exact_width_raster_graphics() {
    let profile = graphics_profile();
    let mut input = gs_l_store(5, 2, 1, 1, &[0b1010_1000, 0b0101_0000]);
    input.extend(gs_l_print());

    let rendered = render(&input, &profile).expect("buffered graphics should print");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 2));
    for y in 0..2 {
        for x in 0..384 {
            let expected = match y {
                0 => matches!(x, 0 | 2 | 4),
                1 => matches!(x, 1 | 3),
                _ => false,
            };
            assert_eq!(surface.is_printed(x, y), expected, "dot ({x}, {y})");
        }
    }
}

#[test]
fn gs_8_l_accepts_a_32_bit_parameter_length_and_applies_scaling() {
    let profile = graphics_profile();
    let mut input = gs_8_l_store(3, 1, 2, 2, &[0b1010_0000]);
    input.extend(gs_l_print());

    let rendered = render(&input, &profile).expect("GS 8 L graphics should print");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 2));
    for y in 0..2 {
        for x in 0..384 {
            assert_eq!(
                surface.is_printed(x, y),
                x < 2 || (4..6).contains(&x),
                "dot ({x}, {y})"
            );
        }
    }
}

#[test]
fn buffered_graphics_use_justification_and_ignore_text_modes_and_line_spacing() {
    let profile = graphics_profile();
    let mut input = vec![ESC, b'3', 50, GS, b'!', 0x77, GS, b'B', 1, ESC, b'a', 2];
    input.extend(gs_l_store(3, 1, 2, 2, &[0b1010_0000]));
    input.extend(gs_l_print());

    let rendered = render(&input, &profile).expect("graphics layout state should apply");
    let surface = &rendered.sheets[0].surface;

    // The six-dot-wide scaled image is right-justified. Text size, reverse
    // mode, and the 50-dot line spacing do not change its pixels or feed.
    assert_eq!((surface.width(), surface.height()), (384, 2));
    for y in 0..2 {
        for x in 0..384 {
            let expected = (378..380).contains(&x) || (382..384).contains(&x);
            assert_eq!(surface.is_printed(x, y), expected, "dot ({x}, {y})");
        }
    }
}

#[test]
fn buffered_graphics_can_be_centered_in_the_active_print_area() {
    let profile = graphics_profile();
    let mut input = vec![ESC, b'a', 1];
    input.extend(gs_l_store(4, 1, 1, 1, &[0b1111_0000]));
    input.extend(gs_l_print());

    let rendered = render(&input, &profile).expect("graphics should center");
    let surface = &rendered.sheets[0].surface;

    for x in 0..384 {
        assert_eq!(
            surface.is_printed(x, 0),
            (190..194).contains(&x),
            "dot ({x}, 0)"
        );
    }
}

#[test]
fn buffered_graphics_are_clipped_to_the_active_print_area() {
    let profile = graphics_profile();
    let mut input = vec![GS, b'L', 10, 0, GS, b'W', 3, 0];
    input.extend(gs_l_store(5, 1, 1, 1, &[0b1111_1000]));
    input.extend(gs_l_print());

    let rendered = render(&input, &profile).expect("oversized graphics should be clipped");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 1));
    for x in 0..384 {
        assert_eq!(
            surface.is_printed(x, 0),
            (10..13).contains(&x),
            "dot ({x}, 0)"
        );
    }
}

#[test]
fn function_50_clears_the_graphics_buffer_after_printing() {
    let profile = graphics_profile();
    let mut input = gs_l_store(1, 1, 1, 1, &[0x80]);
    input.extend(gs_l_print());
    input.extend(gs_l_print());

    let error = render(&input, &profile)
        .expect_err("a second print must not reuse graphics that were already consumed");

    assert!(matches!(
        error,
        RenderError::GraphicsBufferEmpty { offset: 23 }
    ));
}

#[test]
fn esc_at_clears_buffered_graphics_before_restoring_defaults() {
    let profile = graphics_profile();
    let mut input = gs_l_store(1, 1, 1, 1, &[0x80]);
    input.extend([ESC, b'@']);
    input.extend(gs_l_print());

    let error = render(&input, &profile).expect_err("ESC @ should clear pending graphics");

    assert!(matches!(
        error,
        RenderError::GraphicsBufferEmpty { offset: 18 }
    ));
}

#[test]
fn modern_graphics_are_rejected_by_a_profile_without_the_graphics_feature() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = gs_l_store(1, 1, 1, 1, &[0x80]);

    let error = render(&input, &profile)
        .expect_err("the NT-5890K profile does not advertise modern graphics");

    assert!(matches!(
        error,
        RenderError::CommandUnsupportedByProfile {
            command: "GS ( L graphics",
            ref profile,
            offset: 0,
        } if profile == "NT-5890K"
    ));
}

#[test]
fn buffered_graphics_printing_requires_the_beginning_of_a_line() {
    let profile = graphics_profile();
    let mut input = gs_l_store(1, 1, 1, 1, &[0x80]);
    input.push(b'X');
    input.extend(gs_l_print());

    let error = render(&input, &profile)
        .expect_err("graphics must not bypass printable data already on the line");

    assert!(matches!(
        error,
        RenderError::CommandRequiresBeginningOfLine {
            command: "GS ( L Function 50",
            offset: 17,
        }
    ));
}

#[test]
fn gs_8_l_rejects_an_oversized_declared_payload_before_reading_it() {
    let profile = graphics_profile();
    let input = [GS, b'8', b'L', 1, 0, 0x80, 0];

    let error = render(&input, &profile)
        .expect_err("the declared 8 MiB payload should hit the default bound");

    assert!(matches!(
        error,
        RenderError::LimitExceeded {
            kind: LimitKind::CommandPayloadBytes,
            value: 8_388_609,
            limit: 8_388_608,
        }
    ));
}

fn graphics_profile() -> PrinterProfile {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    profile.features.graphics = true;
    profile
}

fn gs_l_store(
    width_dots: u16,
    height_dots: u16,
    scale_x: u8,
    scale_y: u8,
    raster: &[u8],
) -> Vec<u8> {
    let mut body = graphics_store_body(width_dots, height_dots, scale_x, scale_y, raster);
    let parameter_length = body.len() as u16;
    let mut command = vec![
        GS,
        b'(',
        b'L',
        parameter_length as u8,
        (parameter_length >> 8) as u8,
    ];
    command.append(&mut body);
    command
}

fn gs_8_l_store(
    width_dots: u16,
    height_dots: u16,
    scale_x: u8,
    scale_y: u8,
    raster: &[u8],
) -> Vec<u8> {
    let mut body = graphics_store_body(width_dots, height_dots, scale_x, scale_y, raster);
    let parameter_length = body.len() as u32;
    let mut command = vec![GS, b'8', b'L'];
    command.extend(parameter_length.to_le_bytes());
    command.append(&mut body);
    command
}

fn graphics_store_body(
    width_dots: u16,
    height_dots: u16,
    scale_x: u8,
    scale_y: u8,
    raster: &[u8],
) -> Vec<u8> {
    let mut body = vec![48, 112, 48, scale_x, scale_y, 49];
    body.extend(width_dots.to_le_bytes());
    body.extend(height_dots.to_le_bytes());
    body.extend(raster);
    body
}

fn gs_l_print() -> [u8; 7] {
    [GS, b'(', b'L', 2, 0, 48, 50]
}
