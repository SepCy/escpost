use escpos2png::{RenderError, render};
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");

#[test]
fn prints_a_normal_raster_image_and_feeds_by_its_height() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [
        0x1b, 0x40, 0x1d, 0x76, 0x30, 0, 1, 0, 8, 0, 0x81, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81,
    ];

    let rendered = render(&input, &profile).expect("GS v 0 should render");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 8));
    for y in 0..surface.height() {
        for x in 0..surface.width() {
            let expected = x < 8
                && [0x81_u8, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81][y as usize] & (0x80 >> x)
                    != 0;
            assert_eq!(
                surface.is_printed(x, y),
                expected,
                "unexpected dot at ({x}, {y})"
            );
        }
    }
}

#[test]
fn accepts_the_ascii_digit_alias_for_normal_mode() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [0x1d, 0x76, 0x30, 48, 1, 0, 1, 0, 0b1000_0001];

    let rendered = render(&input, &profile).expect("GS v 0 mode 48 should render");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 1));
    assert!(surface.is_printed(0, 0));
    assert!(surface.is_printed(7, 0));
}

#[test]
fn doubles_each_raster_dot_horizontally_in_double_width_mode() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [0x1d, 0x76, 0x30, 1, 1, 0, 1, 0, 0b1000_0001];

    let rendered = render(&input, &profile).expect("GS v 0 mode 1 should render");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 1));
    for x in 0..surface.width() {
        assert_eq!(
            surface.is_printed(x, 0),
            x < 2 || (14..16).contains(&x),
            "unexpected dot at ({x}, 0)"
        );
    }
}

#[test]
fn doubles_each_raster_row_and_the_paper_advance_in_double_height_mode() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [0x1d, 0x76, 0x30, 2, 1, 0, 1, 0, 0b1000_0000];

    let rendered = render(&input, &profile).expect("GS v 0 mode 2 should render");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 2));
    assert!(surface.is_printed(0, 0));
    assert!(surface.is_printed(0, 1));
}

#[test]
fn doubles_raster_dots_in_both_directions_in_ascii_quadruple_mode() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [0x1d, 0x76, 0x30, 51, 1, 0, 1, 0, 0b1000_0000];

    let rendered = render(&input, &profile).expect("GS v 0 mode 51 should render");
    let surface = &rendered.sheets[0].surface;

    assert_eq!((surface.width(), surface.height()), (384, 2));
    for y in 0..2 {
        for x in 0..2 {
            assert!(surface.is_printed(x, y), "missing dot at ({x}, {y})");
        }
    }
}

#[test]
fn rejects_zero_raster_dimensions_without_panicking() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [0x1d, 0x76, 0x30, 0, 0, 0, 1, 0];

    let result = render(&input, &profile);

    assert!(matches!(
        result,
        Err(RenderError::InvalidRasterBitImageDimensions {
            width_bytes: 0,
            height_dots: 1,
            offset: 0,
        })
    ));
}

#[test]
fn reports_raster_bit_images_that_the_profile_does_not_support() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    profile.features.bit_image_raster = false;

    let error = render(&[0x1d, b'v', b'0', 0, 1, 0, 1, 0, 0x80], &profile)
        .expect_err("GS v 0 should be gated by the selected profile");

    assert!(matches!(
        error,
        RenderError::CommandUnsupportedByProfile {
            command: "GS v 0 raster bit image",
            ref profile,
            offset: 0,
        } if profile == "NT-5890K"
    ));
}
