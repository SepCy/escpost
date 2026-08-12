use escpost_profiles::compile_profile;
use escpost_render::{RenderError, render};

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
const GS: u8 = 0x1d;

#[test]
fn stores_and_prints_a_qr_code_with_reset_defaults() {
    let mut profile = test_profile();
    // Keep the command test explicit about its required profile capability.
    profile.features.qr_code = true;
    let input = [
        // Function 180 stores one raw data byte. pL/pH count cn, fn, m, and data.
        GS, b'(', b'k', 4, 0, 49, 80, 48, b'A',
        // Function 181 prints the stored symbol without consuming its data.
        GS, b'(', b'k', 3, 0, 49, 81, 48,
    ];

    let rendered = render(&input, &profile).expect("GS ( k should render a QR code");
    let surface = &rendered.sheets[0].surface;

    // Model 2 starts at 21 modules. Epson's reset-default module size is three
    // dots, and the command does not add the QR quiet zone.
    assert_eq!((surface.width(), surface.height()), (384, 63));
    assert_finder_pattern(surface, 0, 0, 3);
    assert_finder_pattern(surface, 14, 0, 3);
    assert_finder_pattern(surface, 0, 14, 3);
}

#[test]
fn function_167_sets_the_qr_module_size_in_dots() {
    let mut profile = test_profile();
    profile.features.qr_code = true;
    let input = [
        GS, b'(', b'k', 3, 0, 49, 67, 2, // Two dots per module.
        GS, b'(', b'k', 4, 0, 49, 80, 48, b'A', GS, b'(', b'k', 3, 0, 49, 81, 48,
    ];

    let rendered = render(&input, &profile).expect("Function 167 should set the module size");
    let surface = &rendered.sheets[0].surface;

    assert_eq!(surface.height(), 42);
    assert_finder_pattern(surface, 0, 0, 2);
}

#[test]
fn function_169_sets_the_qr_error_correction_level() {
    let mut profile = test_profile();
    profile.features.qr_code = true;
    let mut input = vec![
        GS, b'(', b'k', 3, 0, 49, 69, 51, // Error correction H.
        GS, b'(', b'k', 18, 0, 49, 80, 48,
    ];
    input.extend_from_slice(b"ABCDEFGHIJKLMNO");
    input.extend_from_slice(&[GS, b'(', b'k', 3, 0, 49, 81, 48]);

    let rendered = render(&input, &profile).expect("Function 169 should select level H");

    // Fifteen alphanumeric characters fit in Version 1 at level L but need
    // Version 2's 25 modules at level H.
    assert_eq!(rendered.sheets[0].surface.height(), 75);
}

#[test]
fn function_165_accepts_the_supported_model_2_setting() {
    let mut profile = test_profile();
    profile.features.qr_code = true;
    let input = [
        GS, b'(', b'k', 4, 0, 49, 65, 50, 0, // QR Model 2.
        GS, b'(', b'k', 4, 0, 49, 80, 48, b'A', GS, b'(', b'k', 3, 0, 49, 81, 48,
    ];

    let rendered = render(&input, &profile).expect("Function 165 should accept QR Model 2");

    assert_eq!(rendered.sheets[0].surface.height(), 63);
}

#[test]
fn rejects_qr_store_commands_above_the_epson_protocol_limit() {
    let mut profile = test_profile();
    profile.features.qr_code = true;
    // Function 180 permits pL + pH*256 up to 7092. The three fixed bytes
    // leave at most 7089 bytes for symbol data.
    let parameter_length = 7093_u16;
    let [low, high] = parameter_length.to_le_bytes();
    let mut input = vec![GS, b'(', b'k', low, high, 49, 80, 48];
    input.extend(std::iter::repeat_n(b'A', 7090));

    let error = render(&input, &profile).expect_err("an oversized store command must be rejected");

    assert!(matches!(
        error,
        RenderError::InvalidQrData {
            reason: "store command exceeds the 7092-byte parameter limit",
            ..
        }
    ));
}

#[test]
fn qr_data_remains_stored_after_printing() {
    let mut profile = test_profile();
    profile.features.qr_code = true;
    let input = [
        GS, b'(', b'k', 4, 0, 49, 80, 48, b'A', GS, b'(', b'k', 3, 0, 49, 81, 48, GS, b'(', b'k',
        3, 0, 49, 81, 48,
    ];

    let rendered = render(&input, &profile).expect("stored QR data should print twice");
    let surface = &rendered.sheets[0].surface;

    assert_eq!(surface.height(), 126);
    assert!(surface.is_printed(0, 0));
    assert!(surface.is_printed(0, 63));
}

#[test]
fn qr_store_treats_control_bytes_as_atomic_symbol_data() {
    let mut profile = test_profile();
    profile.features.qr_code = true;
    let input = [
        GS, b'(', b'k', 7, 0, 49, 80, 48, 0x00, 0x1b, 0x1d, 0xff, GS, b'(', b'k', 3, 0, 49, 81, 48,
    ];

    let rendered =
        render(&input, &profile).expect("binary QR data should stay inside Function 180");

    assert_eq!(rendered.sheets[0].surface.height(), 63);
}

#[test]
fn rejects_qr_commands_when_the_profile_does_not_support_them() {
    let mut profile = test_profile();
    // Capability gating remains important even though the physical Netum
    // probe corrected this particular profile to support native QR.
    profile.features.qr_code = false;
    let input = [GS, b'(', b'k', 4, 0, 49, 80, 48, b'A'];

    let error = render(&input, &profile).expect_err("the synthetic profile disables native QR");

    assert!(matches!(
        error,
        RenderError::CommandUnsupportedByProfile {
            command: "GS ( k QR code",
            ..
        }
    ));
}

#[test]
fn reports_unsupported_qr_models_instead_of_rendering_model_2() {
    let mut profile = test_profile();
    profile.features.qr_code = true;
    let input = [GS, b'(', b'k', 4, 0, 49, 65, 49, 0];

    let error = render(&input, &profile).expect_err("QR Model 1 must not look supported");

    assert!(matches!(
        error,
        RenderError::UnsupportedQrModel { model: 49, .. }
    ));
}

#[test]
fn reports_an_empty_qr_store_when_print_is_requested() {
    let mut profile = test_profile();
    profile.features.qr_code = true;
    let input = [GS, b'(', b'k', 3, 0, 49, 81, 48];

    let error = render(&input, &profile).expect_err("print requires stored QR data");

    assert!(matches!(error, RenderError::QrDataEmpty { .. }));
}

fn test_profile() -> escpost_profiles::PrinterProfile {
    compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML).expect("the test profile should compile")
}

fn assert_finder_pattern(
    surface: &escpost_render::MonoSurface,
    module_left: u32,
    module_top: u32,
    module_size: u32,
) {
    for y in 0..7 {
        for x in 0..7 {
            let expected = x == 0
                || x == 6
                || y == 0
                || y == 6
                || ((2..=4).contains(&x) && (2..=4).contains(&y));
            for dot_y in 0..module_size {
                for dot_x in 0..module_size {
                    let actual = surface.is_printed(
                        (module_left + x) * module_size + dot_x,
                        (module_top + y) * module_size + dot_y,
                    );
                    assert_eq!(
                        actual, expected,
                        "unexpected finder dot in module ({x}, {y})"
                    );
                }
            }
        }
    }
}
