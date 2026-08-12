use escpost_profiles::{BarcodeSystem, compile_profile};
use escpost_render::{RenderError, render};

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
const GS: u8 = 0x1d;

#[test]
fn prints_ean13_with_the_generated_check_digit_and_default_dimensions() {
    let profile = test_profile();
    // Keep the command test explicit about its required profile capability.
    let input = [
        GS, b'k', 67, 12, b'5', b'9', b'0', b'1', b'2', b'3', b'4', b'1', b'2', b'3', b'4', b'5',
    ];

    let rendered = render(&input, &profile).expect("GS k should render an EAN-13 barcode");
    let surface = &rendered.sheets[0].surface;
    let modules = ean13_modules("5901234123457");

    // Epson's common reset defaults are a three-dot module and a 162-dot
    // barcode height. GS k advances by the symbol height without requiring LF.
    assert_eq!((surface.width(), surface.height()), (384, 162));
    for y in 0..surface.height() {
        for x in 0..surface.width() {
            let module = (x / 3) as usize;
            let expected = module < modules.len() && modules[module];
            assert_eq!(
                surface.is_printed(x, y),
                expected,
                "unexpected barcode dot at ({x}, {y})"
            );
        }
    }
}

#[test]
fn gs_h_sets_barcode_height_and_independent_paper_advance() {
    let profile = test_profile();
    let input = [
        GS, b'h', 7, GS, b'k', 67, 12, b'5', b'9', b'0', b'1', b'2', b'3', b'4', b'1', b'2', b'3',
        b'4', b'5',
    ];

    let rendered = render(&input, &profile).expect("GS h should configure the next barcode");
    let surface = &rendered.sheets[0].surface;

    assert_eq!(surface.height(), 7);
    assert!((0..7).all(|y| surface.is_printed(0, y)));
}

#[test]
fn gs_w_sets_the_multilevel_barcode_module_width() {
    let profile = test_profile();
    let input = [
        GS, b'w', 2, GS, b'h', 1, GS, b'k', 67, 12, b'5', b'9', b'0', b'1', b'2', b'3', b'4', b'1',
        b'2', b'3', b'4', b'5',
    ];

    let rendered = render(&input, &profile).expect("GS w should configure the next barcode");
    let surface = &rendered.sheets[0].surface;
    let modules = ean13_modules("5901234123457");

    for x in 0..surface.width() {
        let module = (x / 2) as usize;
        let expected = module < modules.len() && modules[module];
        assert_eq!(
            surface.is_printed(x, 0),
            expected,
            "unexpected module-width dot at ({x}, 0)"
        );
    }
}

#[test]
fn prints_upca_with_the_generated_check_digit() {
    let profile = test_profile();
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 65, 11, b'0', b'3', b'6', b'0', b'0', b'0', b'2', b'9',
        b'1', b'4', b'5',
    ];

    let rendered = render(&input, &profile).expect("GS k should render a UPC-A barcode");
    let surface = &rendered.sheets[0].surface;
    // UPC-A is the EAN-13 number-system-zero representation on the wire.
    let modules = ean13_modules("0036000291452");

    for x in 0..surface.width() {
        let module = (x / 2) as usize;
        let expected = module < modules.len() && modules[module];
        assert_eq!(
            surface.is_printed(x, 0),
            expected,
            "unexpected UPC-A dot at ({x}, 0)"
        );
    }
}

#[test]
fn prints_ean8_with_the_generated_check_digit() {
    let profile = test_profile();
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 68, 7, b'5', b'5', b'1', b'2', b'3', b'4', b'5',
    ];

    let rendered = render(&input, &profile).expect("GS k should render an EAN-8 barcode");
    let surface = &rendered.sheets[0].surface;
    let modules = ean8_modules("55123457");

    for x in 0..surface.width() {
        let module = (x / 2) as usize;
        let expected = module < modules.len() && modules[module];
        assert_eq!(
            surface.is_printed(x, 0),
            expected,
            "unexpected EAN-8 dot at ({x}, 0)"
        );
    }
}

#[test]
fn prints_upce_using_the_number_system_and_check_digit_parity() {
    let profile = test_profile();
    // This is the compressed representation of UPC-A 042100005264:
    // number system 0, data 425261, and caller-supplied check digit 4.
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 66, 8, b'0', b'4', b'2', b'5', b'2', b'6', b'1', b'4',
    ];

    let rendered = render(&input, &profile).expect("GS k should render a UPC-E barcode");
    let surface = &rendered.sheets[0].surface;
    let modules = upce_modules("04252614");

    for x in 0..surface.width() {
        let module = (x / 2) as usize;
        let expected = module < modules.len() && modules[module];
        assert_eq!(
            surface.is_printed(x, 0),
            expected,
            "unexpected UPC-E dot at ({x}, 0)"
        );
    }
}

#[test]
fn upce_compresses_the_documented_eleven_digit_upca_form() {
    let profile = test_profile();
    // UPC-A 04210000526 plus its generated check digit compresses to
    // number system 0, data 425261, and check digit 4.
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 66, 11, b'0', b'4', b'2', b'1', b'0', b'0', b'0', b'0',
        b'5', b'2', b'6',
    ];

    let rendered = render(&input, &profile).expect("UPC-E should accept the UPC-A form");
    let surface = &rendered.sheets[0].surface;
    let modules = upce_modules("04252614");

    for x in 0..surface.width() {
        let module = (x / 2) as usize;
        let expected = module < modules.len() && modules[module];
        assert_eq!(
            surface.is_printed(x, 0),
            expected,
            "unexpected compressed UPC-E dot at ({x}, 0)"
        );
    }
}

#[test]
fn function_a_and_function_b_produce_the_same_ean13_pattern() {
    let profile = test_profile();
    let function_a = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 2, b'5', b'9', b'0', b'1', b'2', b'3', b'4', b'1',
        b'2', b'3', b'4', b'5', 0,
    ];
    let function_b = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 67, 12, b'5', b'9', b'0', b'1', b'2', b'3', b'4', b'1',
        b'2', b'3', b'4', b'5',
    ];

    let rendered_a = render(&function_a, &profile).expect("GS k Function A should render");
    let rendered_b = render(&function_b, &profile).expect("GS k Function B should render");

    for y in 0..rendered_a.sheets[0].surface.height() {
        for x in 0..rendered_a.sheets[0].surface.width() {
            assert_eq!(
                rendered_a.sheets[0].surface.is_printed(x, y),
                rendered_b.sheets[0].surface.is_printed(x, y),
                "Function A and B differ at ({x}, {y})"
            );
        }
    }
}

#[test]
fn function_a_itf_ignores_the_final_digit_when_the_count_is_odd() {
    let profile = test_profile();
    let function_a = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 5, b'1', b'2', b'3', 0];
    let function_b = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 70, 2, b'1', b'2'];

    let rendered_a = render(&function_a, &profile).expect("Function A should ignore digit 3");
    let rendered_b = render(&function_b, &profile).expect("the even ITF reference should render");

    assert_eq!(rendered_a.sheets[0].surface, rendered_b.sheets[0].surface);
}

#[test]
fn prints_code39_with_printer_specific_narrow_and_wide_elements() {
    let profile = test_profile();
    let input = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 69, 1, b'A'];

    let rendered = render(&input, &profile).expect("GS k should render a Code 39 barcode");
    let surface = &rendered.sheets[0].surface;
    let expected = code39_dots("A", 2, 5);

    for x in 0..surface.width() {
        assert_eq!(
            surface.is_printed(x, 0),
            expected.get(x as usize).copied().unwrap_or(false),
            "unexpected Code 39 dot at ({x}, 0)"
        );
    }
}

#[test]
fn code39_stops_at_an_asterisk_and_returns_later_bytes_to_text_processing() {
    let profile = test_profile();
    let terminated_early = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 69, 3, b'A', b'*', b'B', 0x0a,
    ];
    let explicit_stream = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 69, 2, b'A', b'*', b'B', 0x0a,
    ];

    let actual = render(&terminated_early, &profile)
        .expect("bytes after the Code 39 stop should remain in the stream");
    let expected = render(&explicit_stream, &profile).expect("the explicit stream should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn function_a_code39_stops_at_an_asterisk_without_waiting_for_nul() {
    let profile = test_profile();
    let function_a = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 4, b'A', b'*', b'B', 0x0a,
    ];
    let equivalent_function_b = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 69, 2, b'A', b'*', b'B', 0x0a,
    ];

    let actual = render(&function_a, &profile)
        .expect("the Code 39 stop should finish Function A before its NUL terminator");
    let expected = render(&equivalent_function_b, &profile)
        .expect("the equivalent length-prefixed stream should render");

    // B follows the stop character and is therefore ordinary text in both
    // streams. This protects framing, not merely the barcode's bars.
    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn function_a_code39_does_not_treat_a_leading_start_character_as_the_stop() {
    let profile = test_profile();
    let function_a = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 4, b'*', b'A', b'*', b'B', 0x0a,
    ];
    let equivalent_function_b = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 69, 3, b'*', b'A', b'*', b'B', 0x0a,
    ];

    let actual = render(&function_a, &profile)
        .expect("the leading start character should not finish Function A");
    let expected = render(&equivalent_function_b, &profile)
        .expect("the equivalent length-prefixed stream should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn prints_itf_by_interleaving_each_pair_of_digit_patterns() {
    let profile = test_profile();
    let input = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 70, 2, b'1', b'2'];

    let rendered = render(&input, &profile).expect("GS k should render an ITF barcode");
    let surface = &rendered.sheets[0].surface;
    let expected = itf_dots("12", 2, 5);

    for x in 0..surface.width() {
        assert_eq!(
            surface.is_printed(x, 0),
            expected.get(x as usize).copied().unwrap_or(false),
            "unexpected ITF dot at ({x}, 0)"
        );
    }
}

#[test]
fn prints_codabar_with_transmitted_start_and_stop_characters() {
    let profile = test_profile();
    let input = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 71, 3, b'A', b'0', b'B'];

    let rendered = render(&input, &profile).expect("GS k should render a Codabar barcode");
    let surface = &rendered.sheets[0].surface;
    let expected = codabar_dots("A0B", 2, 5);

    for x in 0..surface.width() {
        assert_eq!(
            surface.is_printed(x, 0),
            expected.get(x as usize).copied().unwrap_or(false),
            "unexpected Codabar dot at ({x}, 0)"
        );
    }
}

#[test]
fn prints_code93_with_c_and_k_checksums_and_the_termination_bar() {
    let profile = test_profile();
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 72, 6, b'C', b'O', b'D', b'E', b'9', b'3',
    ];

    let rendered = render(&input, &profile).expect("GS k should render a Code 93 barcode");
    let surface = &rendered.sheets[0].surface;
    // The ISO worked example produces check values P (25) and V (31).
    let expected_modules = code93_modules(&[47, 12, 24, 13, 14, 9, 3, 25, 31, 47]);

    for x in 0..surface.width() {
        let module = (x / 2) as usize;
        let expected = module < expected_modules.len() && expected_modules[module];
        assert_eq!(
            surface.is_printed(x, 0),
            expected,
            "unexpected Code 93 dot at ({x}, 0)"
        );
    }
}

#[test]
fn code93_maps_the_complete_ascii_range_through_shift_characters() {
    let profile = test_profile();
    let input = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 72, 3, 0x00, b'a', 0x7f];

    let rendered = render(&input, &profile).expect("Code 93 should encode full ASCII data");
    let surface = &rendered.sheets[0].surface;
    // NUL => %U, a => +A, DEL => %T, followed by C=40 and K=1.
    let expected_modules = code93_modules(&[47, 44, 30, 46, 10, 44, 29, 40, 1, 47]);

    for x in 0..surface.width() {
        let module = (x / 2) as usize;
        let expected = module < expected_modules.len() && expected_modules[module];
        assert_eq!(
            surface.is_printed(x, 0),
            expected,
            "unexpected full-ASCII Code 93 dot at ({x}, 0)"
        );
    }
}

#[test]
fn code93_hri_includes_start_and_stop_placeholders() {
    let profile = test_profile();
    let input = [GS, b'H', 2, GS, b'h', 1, GS, b'w', 2, GS, b'k', 72, 1, b'A'];

    let rendered = render(&input, &profile).expect("Code 93 HRI should render");
    let surface = &rendered.sheets[0].surface;

    // The bars are 46 modules × two dots = 92 dots. Epson prints □A□ as HRI,
    // centered below those bars in three Font A cells.
    let hri_left = (92 - 3 * profile.fonts.a.cell_width_dots) / 2;
    for cell in 0..3 {
        assert!(cell_contains_printed_dots(
            surface,
            hri_left + cell * profile.fonts.a.cell_width_dots,
            1,
            profile.fonts.a.cell_width_dots,
            profile.fonts.a.cell_height_dots,
        ));
    }
}

#[test]
fn code93_hri_expands_shifted_control_data_to_square_and_letter() {
    let profile = test_profile();
    let input = [GS, b'H', 2, GS, b'h', 1, GS, b'w', 2, GS, b'k', 72, 1, 0x00];

    let rendered = render(&input, &profile).expect("Code 93 control HRI should render");
    let surface = &rendered.sheets[0].surface;

    // NUL is encoded as Code 93's %U pair. Epson displays □■U□: white-square
    // guards around a black-square shift placeholder and its letter.
    let hri_left = (110 - 4 * profile.fonts.a.cell_width_dots) / 2;
    for cell in 0..4 {
        assert!(cell_contains_printed_dots(
            surface,
            hri_left + cell * profile.fonts.a.cell_width_dots,
            1,
            profile.fonts.a.cell_width_dots,
            profile.fonts.a.cell_height_dots,
        ));
    }
}

#[test]
fn prints_code128_from_the_explicit_escpos_code_set() {
    let profile = test_profile();
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 4, b'{', b'B', b'H', b'i',
    ];

    let rendered = render(&input, &profile).expect("GS k should render a Code 128 barcode");
    let surface = &rendered.sheets[0].surface;
    // Start B=104, H=40, i=73, checksum=(104 + 40 + 2×73) mod 103=84.
    let expected_modules = code128_modules(&[104, 40, 73, 84, 106]);

    for x in 0..surface.width() {
        let module = (x / 2) as usize;
        let expected = module < expected_modules.len() && expected_modules[module];
        assert_eq!(
            surface.is_printed(x, 0),
            expected,
            "unexpected Code 128 dot at ({x}, 0)"
        );
    }
}

#[test]
fn gs_h_prints_hri_below_the_bars_and_includes_it_in_paper_advance() {
    let profile = test_profile();
    let input = [
        GS, b'H', 2, GS, b'h', 3, GS, b'w', 2, GS, b'k', 68, 7, b'5', b'5', b'1', b'2', b'3', b'4',
        b'5',
    ];

    let rendered = render(&input, &profile).expect("GS H should add HRI below the barcode");
    let surface = &rendered.sheets[0].surface;

    assert_eq!(surface.height(), 3 + profile.fonts.a.cell_height_dots);
    assert!((0..3).all(|y| surface.is_printed(0, y)));
    assert!(
        (3..surface.height()).any(|y| (0..surface.width()).any(|x| surface.is_printed(x, y))),
        "the HRI region should contain representative glyph dots"
    );
}

#[test]
fn gs_f_selects_the_hri_font_without_changing_normal_text_state() {
    let profile = test_profile();
    let input = [
        GS, b'H', 2, GS, b'f', 1, GS, b'h', 3, GS, b'w', 2, GS, b'k', 68, 7, b'5', b'5', b'1',
        b'2', b'3', b'4', b'5',
    ];

    let rendered = render(&input, &profile).expect("GS f should select HRI Font B");
    let surface = &rendered.sheets[0].surface;

    assert_eq!(surface.height(), 3 + profile.fonts.b.cell_height_dots);
}

#[test]
fn code128_switches_code_sets_using_escpos_control_sequences() {
    let profile = test_profile();
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 8, b'{', b'B', b'A', b'B', b'{', b'C', b'1', b'2',
    ];

    let rendered = render(&input, &profile).expect("Code 128 should switch from B to C");
    let surface = &rendered.sheets[0].surface;
    // Start B, A, B, Code C, 12, checksum 35, stop.
    let expected_modules = code128_modules(&[104, 33, 34, 99, 12, 35, 106]);

    for x in 0..surface.width() {
        let module = (x / 2) as usize;
        let expected = module < expected_modules.len() && expected_modules[module];
        assert_eq!(
            surface.is_printed(x, 0),
            expected,
            "unexpected switched Code 128 dot at ({x}, 0)"
        );
    }
}

#[test]
fn code128_shift_uses_the_other_code_set_for_one_character() {
    let profile = test_profile();
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 7, b'{', b'B', b'A', b'{', b'S', 0x01, b'B',
    ];

    let rendered = render(&input, &profile).expect("Code 128 SHIFT should render");
    let surface = &rendered.sheets[0].surface;
    // Start B, A, SHIFT, SOH encoded through Code A, B, checksum 46, stop.
    let expected_modules = code128_modules(&[104, 33, 98, 65, 34, 46, 106]);

    for x in 0..surface.width() {
        let module = (x / 2) as usize;
        let expected = module < expected_modules.len() && expected_modules[module];
        assert_eq!(
            surface.is_printed(x, 0),
            expected,
            "unexpected shifted Code 128 dot at ({x}, 0)"
        );
    }
}

#[test]
fn gs1_128_adds_fnc1_and_the_requested_modulus_10_check_digit() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1_128);
    let gs1 = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 74, 18, b'(', b'0', b'1', b')', b'9', b'5', b'0', b'1',
        b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'*',
    ];
    let explicit_code128 = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 20, b'{', b'C', b'{', b'1', b'0', b'1', b'9', b'5',
        b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'3',
    ];

    let actual = render(&gs1, &profile).expect("the Epson GS1-128 example should render");
    let expected =
        render(&explicit_code128, &profile).expect("the equivalent Code 128 should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn gs1_128_encodes_an_explicit_fnc1_between_concatenated_fields() {
    let mut profile = test_profile_with_function_b(BarcodeSystem::Gs1_128);
    // The complete Epson concatenation example is wider than 58 mm even at
    // the minimum module width, so use a synthetic 80 mm print area.
    profile.geometry.printable_width_dots = 576;
    let gs1 = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 74, 33, b'(', b'0', b'1', b')', b'9', b'5', b'0', b'1',
        b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'*', b' ', b'{', b'1', b'(', b'3',
        b'1', b'0', b'2', b')', b'0', b'0', b'0', b'4', b'0', b'0',
    ];
    let explicit_code128 = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 32, b'{', b'C', b'{', b'1', b'0', b'1', b'9', b'5',
        b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'3', b'{', b'1', b'3',
        b'1', b'0', b'2', b'0', b'0', b'0', b'4', b'0', b'0',
    ];

    let actual = render(&gs1, &profile).expect("concatenated GS1-128 fields should render");
    let expected =
        render(&explicit_code128, &profile).expect("the equivalent Code 128 should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn gs1_128_hri_keeps_parentheses_and_shows_the_inserted_check_digit() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1_128);
    let input = [
        GS, b'H', 2, GS, b'h', 1, GS, b'w', 2, GS, b'k', 74, 18, b'(', b'0', b'1', b')', b'9',
        b'5', b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'*',
    ];

    let actual = render(&input, &profile).expect("GS1-128 HRI should render");
    let expected_text =
        render(b"(01)95012345678903\n", &profile).expect("the HRI reference text should render");
    // Eleven ordinary Code 128 symbols use 11 modules each; stop uses 13.
    let barcode_width = (11 * 11 + 13) * 2;
    let hri_width = 18 * profile.fonts.a.cell_width_dots;
    let hri_left = (barcode_width - hri_width) / 2;

    for y in 0..profile.fonts.a.cell_height_dots {
        for x in 0..hri_width {
            assert_eq!(
                actual.sheets[0].surface.is_printed(hri_left + x, 1 + y),
                expected_text.sheets[0].surface.is_printed(x, y),
                "unexpected GS1-128 HRI dot at ({x}, {y})"
            );
        }
    }
}

#[test]
fn gs1_128_escapes_literal_special_characters_and_encodes_fnc3() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1_128);
    let gs1 = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 74, 11, b'{', b'(', b'{', b')', b'{', b'*', b'{', b'{',
        b'{', b'3', b'A',
    ];
    let explicit_code128 = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 12, b'{', b'B', b'{', b'1', b'(', b')', b'*', b'{',
        b'{', b'{', b'3', b'A',
    ];

    let actual = render(&gs1, &profile).expect("escaped GS1-128 data should render");
    let expected =
        render(&explicit_code128, &profile).expect("the equivalent Code 128 should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn gs1_128_hri_shows_escaped_literals_and_spaces_for_controls() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1_128);
    let input = [
        GS, b'H', 2, GS, b'h', 1, GS, b'w', 2, GS, b'k', 74, 11, b'{', b'(', b'{', b')', b'{',
        b'*', b'{', b'{', b'{', b'3', b'A',
    ];

    let actual = render(&input, &profile).expect("GS1-128 special-character HRI should render");
    let expected_text =
        render(b"()*{ A\n", &profile).expect("the HRI reference text should render");
    let barcode_width = (9 * 11 + 13) * 2;
    let hri_width = 6 * profile.fonts.a.cell_width_dots;
    let hri_left = (barcode_width - hri_width) / 2;

    for y in 0..profile.fonts.a.cell_height_dots {
        for x in 0..hri_width {
            assert_eq!(
                actual.sheets[0].surface.is_printed(hri_left + x, 1 + y),
                expected_text.sheets[0].surface.is_printed(x, y),
                "unexpected special-character HRI dot at ({x}, {y})"
            );
        }
    }
}

#[test]
fn gs1_128_requires_its_exact_profile_capability() {
    let profile = test_profile();
    let input = [GS, b'k', 74, 2, b'0', b'1'];

    let error =
        render(&input, &profile).expect_err("legacy Function B support must not imply GS1-128");

    assert!(matches!(
        error,
        RenderError::CommandUnsupportedByProfile {
            command: "GS k GS1-128",
            ..
        }
    ));
}

#[test]
fn gs1_128_rejects_a_payload_shorter_than_two_bytes() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1_128);
    let input = [GS, b'k', 74, 1, b'A'];

    let error = render(&input, &profile).expect_err("GS1-128 requires at least two source bytes");

    assert!(matches!(
        error,
        RenderError::InvalidBarcodeData {
            system: "GS1-128",
            reason: "expected 2 through 255 bytes",
            ..
        }
    ));
}

#[test]
fn gs1_128_rejects_an_undefined_brace_escape() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1_128);
    let input = [GS, b'k', 74, 2, b'{', b'A'];

    let error =
        render(&input, &profile).expect_err("GS1-128 accepts only its documented brace escapes");

    assert!(matches!(
        error,
        RenderError::InvalidBarcodeData {
            system: "GS1-128",
            reason: "invalid GS1-128 data structure",
            ..
        }
    ));
}

#[test]
fn gs1_128_requires_an_ai_data_delimiter_before_a_check_placeholder() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1_128);
    let input = [GS, b'k', 74, 3, b'0', b'1', b'*'];

    let error =
        render(&input, &profile).expect_err("the printer cannot infer the AI boundary by itself");

    assert!(matches!(
        error,
        RenderError::InvalidBarcodeData {
            system: "GS1-128",
            reason: "invalid GS1-128 data structure",
            ..
        }
    ));
}

#[test]
fn gs1_databar_omnidirectional_matches_the_bwipp_module_vector_and_minimum_height() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarOmnidirectional);
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 75, 13, b'2', b'0', b'0', b'1', b'2', b'3', b'4', b'5',
        b'6', b'7', b'8', b'9', b'0',
    ];

    let rendered = render(&input, &profile).expect("GS1 DataBar Omnidirectional should render");
    let surface = &rendered.sheets[0].surface;
    // This 96-module vector was independently generated by BWIPP. Keeping the
    // expected geometry outside the encoder catches adapter and scaling errors.
    let modules = modules(
        "010100011101000001001111111000010100110110111110110000010010100101100000000111000110110110001101",
    );

    // Epson overrides GS h when it is below 33 times the module width.
    assert_eq!(surface.height(), 66);
    assert_module_pattern(surface, &modules, 2, 66);
}

#[test]
fn gs1_databar_omnidirectional_matches_bwipp_vectors_across_character_groups() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarOmnidirectional);
    // These independent vectors exercise different data-character groups and
    // finder checksums; one happy-path value would not protect those branches.
    let cases = [
        (
            b"0000000000000".as_slice(),
            "010101001000000001000111111110010111111100101010101010110000000101111111110111011111111011010101",
        ),
        (
            b"0095011015300".as_slice(),
            "010100100000001001000100000000010100111011110110110100110111100101111111000111011110111101101101",
        ),
    ];

    for (payload, expected) in cases {
        let mut input = vec![GS, b'h', 1, GS, b'w', 2, GS, b'k', 75, 13];
        input.extend_from_slice(payload);

        let rendered = render(&input, &profile).expect("the BWIPP reference value should render");
        assert_module_pattern(&rendered.sheets[0].surface, &modules(expected), 2, 66);
    }
}

#[test]
fn gs1_databar_truncated_reuses_the_omnidirectional_pattern_at_its_own_minimum_height() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarTruncated);
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 76, 13, b'2', b'0', b'0', b'1', b'2', b'3', b'4', b'5',
        b'6', b'7', b'8', b'9', b'0',
    ];

    let rendered = render(&input, &profile).expect("GS1 DataBar Truncated should render");
    let surface = &rendered.sheets[0].surface;
    let modules = modules(
        "010100011101000001001111111000010100110110111110110000010010100101100000000111000110110110001101",
    );

    // Truncated changes only the minimum printed height: its logical bars are
    // identical to the Omnidirectional form.
    assert_eq!(surface.height(), 26);
    assert_module_pattern(surface, &modules, 2, 26);
}

#[test]
fn gs1_databar_limited_matches_the_iso_figure_7_vector_and_minimum_height() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarLimited);
    let input = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 77, 13, b'1', b'5', b'0', b'1', b'2', b'3', b'4', b'5',
        b'6', b'7', b'8', b'9', b'0',
    ];

    let rendered = render(&input, &profile).expect("GS1 DataBar Limited should render");
    let surface = &rendered.sheets[0].surface;
    // ISO/IEC 24724:2011 Figure 7, independently reproduced by Zint. The
    // final five light modules are the revision's right-side clear area.
    let modules =
        modules("0100011001100011011010100111010010101101001101001001011000110111001100110100000");

    // Epson overrides GS h when it is below 10 times the module width.
    assert_eq!(surface.height(), 20);
    assert_module_pattern(surface, &modules, 2, 20);
}

#[test]
fn gs1_databar_limited_matches_zint_vectors_across_all_reachable_pair_groups() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarLimited);
    // These vectors cover every right-pair group plus the two non-zero
    // left-pair groups reachable within Limited's permitted numeric range.
    let cases = [
        (
            b"0000000000000".as_slice(),
            "0101010101010000001000000111010111010100100101010101010100000010000001110100000",
        ),
        (
            b"0000000183064".as_slice(),
            "0101010101010000001000000111001010101011100101010101010100011110000011110100000",
        ),
        (
            b"0000000820064".as_slice(),
            "0101010101010000001000000111011010110100100101010101010101111110001111110100000",
        ),
        (
            b"0000001000776".as_slice(),
            "0101010101010000001000000111010111010101000101010101010100000110000011110100000",
        ),
        (
            b"0000001491021".as_slice(),
            "0101010101010000001000000111010100101011001101010101010100111110000111110100000",
        ),
        (
            b"0000001979845".as_slice(),
            "0101010101010000001000000111010111010010100101010101010100000010000000010100000",
        ),
        (
            b"0000001996939".as_slice(),
            "0101010101010000001000000111011011010101000101010101010101111110111111110100000",
        ),
        (
            b"0368610347973".as_slice(),
            "0100000011100000010101010101001010101110100101010101010100000010000001110100000",
        ),
        (
            b"1651255079973".as_slice(),
            "0100000111100011110101010101010101101100100101010110010010001001100000010100000",
        ),
        (
            b"1999999999999".as_slice(),
            "0100111100110110101101111101010101101011000101010000101110001101011110010100000",
        ),
    ];

    for (payload, expected) in cases {
        let mut input = vec![GS, b'h', 1, GS, b'w', 2, GS, b'k', 77, 13];
        input.extend_from_slice(payload);

        let rendered = render(&input, &profile).expect("the Zint reference value should render");
        assert_module_pattern(&rendered.sheets[0].surface, &modules(expected), 2, 20);
    }
}

#[test]
fn gs1_databar_expanded_matches_the_iso_figure_10_vector_and_minimum_height() {
    let mut profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarExpanded);
    profile.geometry.printable_width_dots = 576;
    let payload = b"(01)98898765432106(3202)012345(15)991231";
    let mut input = vec![GS, b'h', 1, GS, b'w', 2, GS, b'k', 78, payload.len() as u8];
    input.extend_from_slice(payload);

    let rendered = render(&input, &profile).expect("GS1 DataBar Expanded should render");
    let surface = &rendered.sheets[0].surface;
    // ISO/IEC 24724:2011 Figure 10, independently reproduced by Zint.
    let modules = modules(
        "01001000011000110110111111110000101110000110010100011010000001100010101111110000111010011100000010010100111110111001100011111100001011101100000100100100011110010110001011111111001110001101111010000101",
    );

    // Epson overrides GS h when it is below 34 times the module width.
    assert_eq!(surface.height(), 68);
    assert_module_pattern(surface, &modules, 2, 68);
}

#[test]
fn gs1_databar_expanded_compacts_a_general_purpose_field() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarExpanded);
    let payload = b"(10)12A";
    let mut input = vec![GS, b'h', 1, GS, b'w', 2, GS, b'k', 78, payload.len() as u8];
    input.extend_from_slice(payload);

    let rendered = render(&input, &profile).expect("the general-purpose field should render");
    // ISO/IEC 24724:2011 Figure F.3 starts in the general-purpose field and
    // exercises its Numeric-to-Alphanumeric transition.
    let modules = modules(
        "010100000110100000101111111100001010001000000010110101111100100111001011110000000010011101111111010101",
    );

    assert_module_pattern(&rendered.sheets[0].surface, &modules, 2, 68);
}

#[test]
fn gs1_databar_expanded_uses_the_compact_metric_weight_method() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarExpanded);
    let payload = b"(01)90012345678908(3103)001750";
    let mut input = vec![GS, b'h', 1, GS, b'w', 2, GS, b'k', 78, payload.len() as u8];
    input.extend_from_slice(payload);

    let rendered = render(&input, &profile).expect("the compact metric-weight form should render");
    // ISO/IEC 24724:2011 Figure 11 validates compressed method 3 rather than
    // the general-purpose field used by the previous test.
    let modules = modules(
        "0101110010000010011011111111000010111000010011000101011110111001100010111100000011100101110001110111011110101111000110001111110000101011000010011111010",
    );

    assert_module_pattern(&rendered.sheets[0].surface, &modules, 2, 68);
}

#[test]
fn gs1_databar_expanded_matches_every_specialized_compaction_method() {
    let mut profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarExpanded);
    profile.geometry.printable_width_dots = 576;
    // These ISO/BWIPP/Zint vectors cover method 1 and methods 4 through 14.
    // Method 2 is covered by Figure F.3 and method 3 by Figure 11 above.
    let cases = [
        (
            b"(01)00012345678905(10)ABC123".as_slice(),
            "0100011000001011011011111111000010110011000010111101011110011011111010111110000001100010110000110111000111101101011110001111110000101110001100100001010011101111110110101111111100111001011011111101110011011100101111100011110000001010",
        ),
        (
            b"(01)90012345678908(3202)000156".as_slice(),
            "0101001000111100001011111111000010100111000100001101011110111001100010111100000011100101110001110111011110101111000110001111110000101100001000001010010",
        ),
        (
            b"(01)90012345678908(3922)795".as_slice(),
            "010110000010001011101111111100001010011100000101100101111001101111101011111100001110001011000011011100011110110101111000111111000010100111101110100001100011011100100010111111110011101",
        ),
        (
            b"(01)90012345678908(3932)0081234".as_slice(),
            "01001110000101100010111111110000101110100000110010010111100110111110101111110000111000101100001101110001111011010111100011111100001011000011010111100100111110001011101011111111001110001101111001011101",
        ),
        (
            b"(01)90012345678908(3102)099999(11)201209".as_slice(),
            "01000101111001000010111111110000101000110111000010010111100110111110101111110000111000101100001101110001111011010111100011111100001010111100100001000100000011100101001011111111001110010000100100011101",
        ),
        (
            b"(01)90012345678908(3201)099999(11)201209".as_slice(),
            "01001000001101001110111111110000101110100011000010010111100110111110101111110000111000101100001101110001111011010111100011111100001011000100001101100111010111001110001011111111001110010000100100011101",
        ),
        (
            b"(01)90012345678908(3100)099999(13)201209".as_slice(),
            "01001000001101001110111111110000101111001010000010010111100110111110101111110000111000101100001101110001111011010111100011111100001011000111000001010111010000110110001011111111001110010000100100011101",
        ),
        (
            b"(01)90012345678908(3204)099999(13)201209".as_slice(),
            "01001000111000010110111111110000101100101000001110010111100110111110101111110000111000101100001101110001111011010111100011111100001010011101000011110101110000101111101011111111001110010000100100011101",
        ),
        (
            b"(01)90012345678908(3103)012233(15)991231".as_slice(),
            "01001100000100111010111111110000101011100100000110010111100110111110101111110000111000101100001101110001111011010111100011111100001011000011010110000111001100110001001011111111001110001101111010000101",
        ),
        (
            b"(01)90012345678908(3205)099999(15)201209".as_slice(),
            "01001110000010011010111111110000101000001101110100010111100110111110101111110000111000101100001101110001111011010111100011111100001011110011010001100101001100011000001011111111001110010000100100011101",
        ),
        (
            b"(01)90012345678908(3105)099999(17)201209".as_slice(),
            "01000111010000110010111111110000101110000100110100010111100110111110101111110000111000101100001101110001111011010111100011111100001011110011010001100101001100011000001011111111001110010000100100011101",
        ),
        (
            b"(01)90012345678908(3200)099999(17)201209".as_slice(),
            "01001110000010100110111111110000101111000100010100010111100110111110101111110000111000101100001101110001111011010111100011111100001011000111000001010111010000110110001011111111001110010000100100011101",
        ),
    ];

    for (payload, expected) in cases {
        let mut input = vec![GS, b'h', 1, GS, b'w', 2, GS, b'k', 78, payload.len() as u8];
        input.extend_from_slice(payload);

        let rendered =
            render(&input, &profile).expect("the specialized Expanded method should render");
        assert_module_pattern(&rendered.sheets[0].surface, &modules(expected), 2, 68);
    }
}

#[test]
fn gs1_databar_expanded_compacts_lowercase_and_mode_transitions() {
    let mut profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarExpanded);
    profile.geometry.printable_width_dots = 576;
    let payload = b"(91)a1234ABCDE";
    let mut input = vec![GS, b'h', 1, GS, b'w', 2, GS, b'k', 78, payload.len() as u8];
    input.extend_from_slice(payload);

    let rendered =
        render(&input, &profile).expect("ISO/IEC 646 general-purpose data should render");
    // This independent BWIPP/Zint vector crosses Numeric, Alphanumeric, and
    // ISO/IEC 646 modes, including the early latch back to Numeric.
    let modules = modules(
        "01001000011000111010111111110000101100100001000001010110111111001110101111110000111000011011010001110000111000010101100011111100001010011100001011000100000010011011001011111111001110010011100000100101",
    );

    assert_module_pattern(&rendered.sheets[0].surface, &modules, 2, 68);
}

#[test]
fn gs1_databar_expanded_encodes_fnc1_but_omits_it_from_hri() {
    let mut profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarExpanded);
    profile.geometry.printable_width_dots = 576;
    let payload = b"(01)90012345678908(3922)795{1(20)01";
    let mut input = vec![
        GS,
        b'H',
        2,
        GS,
        b'h',
        1,
        GS,
        b'w',
        2,
        GS,
        b'k',
        78,
        payload.len() as u8,
    ];
    input.extend_from_slice(payload);

    let rendered = render(&input, &profile).expect("an escaped FNC1 should render");
    // The reference vector includes the FNC1 that terminates the variable
    // (3922) field before the following (20) field.
    let modules = modules(
        "01000110110000110010111111110000101111000100001010010111100110111110101111110000111000101100001101110001111011010111100011111100001010011110111010000110001110001011001011111111001110100111110001110101",
    );
    let surface = &rendered.sheets[0].surface;

    assert_module_pattern(surface, &modules, 2, 68);
    // Epson keeps AI delimiters in HRI but gives FNC1 no visible glyph.
    assert_hri_below(
        surface,
        &profile,
        68,
        400,
        "(01)90012345678908(3922)795(20)01",
    );
}

#[test]
fn gs1_databar_expanded_escaped_parentheses_are_data_and_hri() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarExpanded);
    let escaped_payload = b"(91){({)";
    let mut escaped = vec![
        GS,
        b'H',
        2,
        GS,
        b'h',
        1,
        GS,
        b'w',
        2,
        GS,
        b'k',
        78,
        escaped_payload.len() as u8,
    ];
    escaped.extend_from_slice(escaped_payload);
    let delimiter_payload = b"(91)()";
    let mut delimiters = vec![
        GS,
        b'H',
        2,
        GS,
        b'h',
        1,
        GS,
        b'w',
        2,
        GS,
        b'k',
        78,
        delimiter_payload.len() as u8,
    ];
    delimiters.extend_from_slice(delimiter_payload);

    let escaped = render(&escaped, &profile).expect("escaped literal parentheses should render");
    let delimiters = render(&delimiters, &profile).expect("unescaped HRI delimiters should render");

    // Both forms show parentheses, but only the escaped form changes the bars.
    assert_hri_below(&escaped.sheets[0].surface, &profile, 68, 268, "(91)()");
    assert!((0..68).any(|y| {
        (0..268).any(|x| {
            escaped.sheets[0].surface.is_printed(x, y)
                != delimiters.sheets[0].surface.is_printed(x, y)
        })
    }));
}

#[test]
fn gs1_databar_expanded_rejects_malformed_epson_data_escapes() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarExpanded);

    for payload in [b"(10)A{".as_slice(), b"(10){X".as_slice()] {
        let mut input = vec![GS, b'k', 78, payload.len() as u8];
        input.extend_from_slice(payload);

        let error = render(&input, &profile).expect_err("an invalid escape must not be guessed");
        assert!(matches!(
            error,
            RenderError::InvalidBarcodeData {
                system: "GS1 DataBar Expanded",
                reason: "invalid GS1 DataBar Expanded data structure",
                ..
            }
        ));
    }
}

#[test]
fn gs1_databar_expanded_rejects_data_that_exceeds_symbol_capacity() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarExpanded);
    let mut payload = Vec::from(b"91".as_slice());
    payload.extend(std::iter::repeat_n(b'a', 76));
    let mut input = vec![GS, b'k', 78, payload.len() as u8];
    input.extend_from_slice(&payload);

    let error = render(&input, &profile).expect_err("Expanded reduced data has a 77-byte limit");

    assert!(matches!(
        error,
        RenderError::InvalidBarcodeData {
            system: "GS1 DataBar Expanded",
            reason: "invalid GS1 DataBar Expanded data structure",
            ..
        }
    ));
}

#[test]
fn gs1_databar_uses_gs_h_when_it_exceeds_the_symbol_minimum() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarOmnidirectional);
    let input = [
        GS, b'h', 80, GS, b'w', 2, GS, b'k', 75, 13, b'2', b'0', b'0', b'1', b'2', b'3', b'4',
        b'5', b'6', b'7', b'8', b'9', b'0',
    ];

    let rendered = render(&input, &profile).expect("a larger GS h value should be preserved");

    assert_eq!(rendered.sheets[0].surface.height(), 80);
}

#[test]
fn gs1_databar_omnidirectional_hri_adds_the_ai_and_check_digit() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarOmnidirectional);
    let input = [
        GS, b'H', 2, GS, b'h', 1, GS, b'w', 2, GS, b'k', 75, 13, b'2', b'0', b'0', b'1', b'2',
        b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0',
    ];

    let rendered = render(&input, &profile).expect("DataBar GTIN HRI should render");
    assert_hri_below(
        &rendered.sheets[0].surface,
        &profile,
        66,
        192,
        "(01)20012345678909",
    );
}

#[test]
fn gs1_databar_limited_hri_adds_the_ai_and_check_digit() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarLimited);
    let input = [
        GS, b'H', 2, GS, b'h', 1, GS, b'w', 2, GS, b'k', 77, 13, b'1', b'5', b'0', b'1', b'2',
        b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0',
    ];

    let rendered = render(&input, &profile).expect("DataBar Limited HRI should render");
    assert_hri_below(
        &rendered.sheets[0].surface,
        &profile,
        20,
        158,
        "(01)15012345678907",
    );
}

#[test]
fn gs1_databar_omnidirectional_requires_exactly_thirteen_digits() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarOmnidirectional);
    let input = [
        GS, b'k', 75, 12, b'2', b'0', b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9',
    ];

    let error = render(&input, &profile).expect_err("a DataBar GTIN body has exactly 13 digits");

    assert!(matches!(
        error,
        RenderError::InvalidBarcodeData {
            system: "GS1 DataBar Omnidirectional",
            reason: "expected exactly 13 digits",
            ..
        }
    ));
}

#[test]
fn gs1_databar_omnidirectional_rejects_non_decimal_data() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarOmnidirectional);
    let input = [
        GS, b'k', 75, 13, b'2', b'0', b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9',
        b'A',
    ];

    let error = render(&input, &profile).expect_err("DataBar GTIN data must be decimal");

    assert!(matches!(
        error,
        RenderError::InvalidBarcodeData {
            system: "GS1 DataBar Omnidirectional",
            reason: "expected decimal digits only",
            ..
        }
    ));
}

#[test]
fn gs1_databar_limited_rejects_values_above_1999999999999() {
    let profile = test_profile_with_function_b(BarcodeSystem::Gs1DataBarLimited);
    let input = [
        GS, b'k', 77, 13, b'2', b'0', b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9',
        b'0',
    ];

    let error =
        render(&input, &profile).expect_err("Limited permits only leading digits zero and one");

    assert!(matches!(
        error,
        RenderError::InvalidBarcodeData {
            system: "GS1 DataBar Limited",
            reason: "expected a value between 0000000000000 and 1999999999999",
            ..
        }
    ));
}

#[test]
fn gs1_databar_systems_require_their_exact_profile_capability() {
    let profile = test_profile();

    for (system, payload, command) in [
        (
            75,
            b"2001234567890".as_slice(),
            "GS k GS1 DataBar Omnidirectional",
        ),
        (
            76,
            b"2001234567890".as_slice(),
            "GS k GS1 DataBar Truncated",
        ),
        (77, b"1501234567890".as_slice(), "GS k GS1 DataBar Limited"),
        (78, b"(10)12A".as_slice(), "GS k GS1 DataBar Expanded"),
    ] {
        let mut input = vec![GS, b'k', system, payload.len() as u8];
        input.extend_from_slice(payload);

        let error = render(&input, &profile)
            .expect_err("legacy Function B support must not imply GS1 DataBar");
        assert!(matches!(
            error,
            RenderError::CommandUnsupportedByProfile {
                command: actual,
                ..
            } if actual == command
        ));
    }
}

#[test]
fn code128_auto_encodes_plain_text_without_an_explicit_code_set() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);
    let automatic = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 79, 3, b'A', b'B', b'C'];
    let explicit = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 5, b'{', b'B', b'A', b'B', b'C',
    ];

    let actual = render(&automatic, &profile)
        .expect("Code 128 auto should choose a code set for plain text");
    let expected =
        render(&explicit, &profile).expect("the explicit Code B reference should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn code128_auto_compacts_an_even_digit_run_with_code_c() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);
    let automatic = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 79, 4, b'1', b'2', b'3', b'4',
    ];
    let explicit = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 6, b'{', b'C', b'1', b'2', b'3', b'4',
    ];

    let actual =
        render(&automatic, &profile).expect("Code 128 auto should compact an even digit run");
    let expected =
        render(&explicit, &profile).expect("the explicit Code C reference should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn code128_auto_switches_into_and_out_of_code_c_for_a_numeric_run() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);
    let automatic = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 79, 10, b'a', b'b', b'1', b'2', b'3', b'4', b'5', b'6',
        b'c', b'd',
    ];
    let explicit = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 16, b'{', b'B', b'a', b'b', b'{', b'C', b'1', b'2',
        b'3', b'4', b'5', b'6', b'{', b'B', b'c', b'd',
    ];

    let actual =
        render(&automatic, &profile).expect("Code 128 auto should compact the internal digit run");
    let expected =
        render(&explicit, &profile).expect("the explicit code-set transitions should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn code128_auto_shifts_for_one_character_in_the_other_text_set() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);
    let automatic = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 79, 3, 0x01, b'a', 0x02];
    let explicit = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 7, b'{', b'A', 0x01, b'{', b'S', b'a', 0x02,
    ];

    let actual =
        render(&automatic, &profile).expect("Code 128 auto should shift for one lowercase byte");
    let expected = render(&explicit, &profile).expect("the explicit SHIFT reference should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn code128_auto_combines_fnc4_and_shift_for_one_upper_character() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);
    let automatic = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 79, 3, 0x01, 0xe1, 0x02];
    let explicit = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 9, b'{', b'A', 0x01, b'{', b'4', b'{', b'S', b'a',
        0x02,
    ];

    let actual = render(&automatic, &profile)
        .expect("Code 128 auto should upper-shift one character from Code B");
    let expected =
        render(&explicit, &profile).expect("the explicit FNC4 plus SHIFT reference should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn code128_auto_encodes_an_upper_byte_with_fnc4() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);
    let automatic = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 79, 1, 0xff];
    let explicit = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 5, b'{', b'B', b'{', b'4', 0x7f,
    ];

    let actual =
        render(&automatic, &profile).expect("Code 128 auto accepts every possible byte value");
    let expected =
        render(&explicit, &profile).expect("the explicit FNC4 upper-shift reference should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn code128_auto_hri_shows_the_source_byte_not_automatic_fnc4_symbols() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);
    let input = [GS, b'H', 2, GS, b'h', 1, GS, b'w', 2, GS, b'k', 79, 1, 0xff];

    let rendered = render(&input, &profile).expect("Code 128 auto HRI should render");
    let surface = &rendered.sheets[0].surface;

    // The barcode is 57 modules × two dots = 114 dots. Its one source HRI
    // character is centered below the bars. FNC4 was added by the printer and
    // must not create a second HRI cell or replace the source byte with spaces.
    let hri_left = (114 - profile.fonts.a.cell_width_dots) / 2;
    assert!(cell_contains_printed_dots(
        surface,
        hri_left,
        1,
        profile.fonts.a.cell_width_dots,
        profile.fonts.a.cell_height_dots,
    ));
}

#[test]
fn code128_auto_latches_upper_mode_for_a_run_of_upper_bytes() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);
    let automatic = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 79, 3, 0xe1, 0xe2, 0xe3];
    let explicit = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 9, b'{', b'B', b'{', b'4', b'{', b'4', b'a', b'b',
        b'c',
    ];

    let actual =
        render(&automatic, &profile).expect("Code 128 auto should compact an upper-byte run");
    let expected =
        render(&explicit, &profile).expect("the explicit FNC4 latch reference should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn code128_auto_unlatches_upper_mode_when_lower_bytes_resume() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);
    let automatic = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 79, 10, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, b'a', b'b', b'c',
        b'd', b'e',
    ];
    let explicit = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 20, b'{', b'B', b'{', b'4', b'{', b'4', b'a', b'b',
        b'c', b'd', b'e', b'{', b'4', b'{', b'4', b'a', b'b', b'c', b'd', b'e',
    ];

    let actual =
        render(&automatic, &profile).expect("Code 128 auto should return to lower-byte mode");
    let expected =
        render(&explicit, &profile).expect("the explicit FNC4 unlatch reference should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn code128_auto_accepts_every_byte_value() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);

    // Exercise each value in its own narrow symbol. A single 255-byte symbol
    // would exceed this 58 mm profile's print area before testing the full
    // protocol range.
    for byte in 0..=u8::MAX {
        let input = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 79, 1, byte];
        render(&input, &profile)
            .unwrap_or_else(|error| panic!("Code 128 auto rejected byte {byte:02x}: {error}"));
    }
}

#[test]
fn code128_auto_rejects_an_empty_payload() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);
    let input = [GS, b'k', 79, 0];

    let error = render(&input, &profile).expect_err("Code 128 auto requires at least one byte");

    assert!(matches!(
        error,
        RenderError::InvalidBarcodeData {
            system: "Code 128 auto",
            reason: "expected at least one byte",
            ..
        }
    ));
}

#[test]
fn code128_auto_treats_an_opening_brace_as_data() {
    let profile = test_profile_with_function_b(BarcodeSystem::Code128Auto);
    let automatic = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 79, 1, b'{'];
    let explicit = [
        GS, b'h', 1, GS, b'w', 2, GS, b'k', 73, 4, b'{', b'B', b'{', b'{',
    ];

    let actual =
        render(&automatic, &profile).expect("Code 128 auto does not use explicit code-set escapes");
    let expected =
        render(&explicit, &profile).expect("the escaped literal brace reference should render");

    assert_eq!(actual.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn code128_auto_requires_function_b_support_from_the_profile() {
    let profile = test_profile();
    let input = [GS, b'k', 79, 1, b'A'];

    let error = render(&input, &profile)
        .expect_err("Code 128 auto belongs to the Function B command family");

    assert!(matches!(
        error,
        RenderError::CommandUnsupportedByProfile {
            command: "GS k Code 128 auto",
            ..
        }
    ));
}

#[test]
fn rejects_native_barcodes_when_the_profile_does_not_support_them() {
    let mut profile = test_profile();
    // Capability gating remains important even though the physical Netum
    // probe corrected this particular profile to support native barcodes.
    profile.features.barcodes.function_b.clear();
    let input = [
        GS, b'k', 67, 12, b'5', b'9', b'0', b'1', b'2', b'3', b'4', b'1', b'2', b'3', b'4', b'5',
    ];

    let error = render(&input, &profile).expect_err("the synthetic profile disables barcodes");

    assert!(matches!(
        error,
        RenderError::CommandUnsupportedByProfile {
            command: "GS k EAN-13",
            ..
        }
    ));
}

fn test_profile() -> escpost_profiles::PrinterProfile {
    compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML).expect("the test profile should compile")
}

fn test_profile_with_function_b(system: BarcodeSystem) -> escpost_profiles::PrinterProfile {
    let mut profile = test_profile();
    profile.features.barcodes.function_b.insert(system);
    profile
}

fn cell_contains_printed_dots(
    surface: &escpost_render::MonoSurface,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> bool {
    (left..left + width).any(|x| (top..top + height).any(|y| surface.is_printed(x, y)))
}

fn modules(bits: &str) -> Vec<bool> {
    bits.bytes().map(|module| module == b'1').collect()
}

fn assert_module_pattern(
    surface: &escpost_render::MonoSurface,
    modules: &[bool],
    module_width: u32,
    bar_height: u32,
) {
    for y in 0..bar_height {
        for x in 0..surface.width() {
            let module = (x / module_width) as usize;
            let expected = module < modules.len() && modules[module];
            assert_eq!(
                surface.is_printed(x, y),
                expected,
                "unexpected barcode dot at ({x}, {y})"
            );
        }
    }
}

fn assert_hri_below(
    surface: &escpost_render::MonoSurface,
    profile: &escpost_profiles::PrinterProfile,
    barcode_height: u32,
    barcode_width: u32,
    expected: &str,
) {
    let expected_text = render(format!("{expected}\n").as_bytes(), profile)
        .expect("the HRI reference should render");
    let hri_width = expected.chars().count() as u32 * profile.fonts.a.cell_width_dots;
    let hri_left = barcode_width.max(hri_width).saturating_sub(hri_width) / 2;

    for y in 0..profile.fonts.a.cell_height_dots {
        for x in 0..hri_width {
            assert_eq!(
                surface.is_printed(hri_left + x, barcode_height + y),
                expected_text.sheets[0].surface.is_printed(x, y),
                "unexpected DataBar HRI dot at ({x}, {y})"
            );
        }
    }
}

fn ean13_modules(digits: &str) -> Vec<bool> {
    const LEFT_ODD: [&str; 10] = [
        "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
        "0110111", "0001011",
    ];
    const LEFT_EVEN: [&str; 10] = [
        "0100111", "0110011", "0011011", "0100001", "0011101", "0111001", "0000101", "0010001",
        "0001001", "0010111",
    ];
    const RIGHT: [&str; 10] = [
        "1110010", "1100110", "1101100", "1000010", "1011100", "1001110", "1010000", "1000100",
        "1001000", "1110100",
    ];
    const PARITY: [&str; 10] = [
        "LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL",
        "LGGLGL",
    ];

    let digits = digits
        .bytes()
        .map(|digit| usize::from(digit - b'0'))
        .collect::<Vec<_>>();
    let mut encoded = String::from("101");
    for (digit, parity) in digits[1..7].iter().zip(PARITY[digits[0]].bytes()) {
        encoded.push_str(if parity == b'L' {
            LEFT_ODD[*digit]
        } else {
            LEFT_EVEN[*digit]
        });
    }
    encoded.push_str("01010");
    for digit in &digits[7..] {
        encoded.push_str(RIGHT[*digit]);
    }
    encoded.push_str("101");

    encoded.bytes().map(|module| module == b'1').collect()
}

fn ean8_modules(digits: &str) -> Vec<bool> {
    const LEFT: [&str; 10] = [
        "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
        "0110111", "0001011",
    ];
    const RIGHT: [&str; 10] = [
        "1110010", "1100110", "1101100", "1000010", "1011100", "1001110", "1010000", "1000100",
        "1001000", "1110100",
    ];

    let digits = digits
        .bytes()
        .map(|digit| usize::from(digit - b'0'))
        .collect::<Vec<_>>();
    let mut encoded = String::from("101");
    for digit in &digits[..4] {
        encoded.push_str(LEFT[*digit]);
    }
    encoded.push_str("01010");
    for digit in &digits[4..] {
        encoded.push_str(RIGHT[*digit]);
    }
    encoded.push_str("101");

    encoded.bytes().map(|module| module == b'1').collect()
}

fn upce_modules(digits: &str) -> Vec<bool> {
    const LEFT_ODD: [&str; 10] = [
        "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
        "0110111", "0001011",
    ];
    const LEFT_EVEN: [&str; 10] = [
        "0100111", "0110011", "0011011", "0100001", "0011101", "0111001", "0000101", "0010001",
        "0001001", "0010111",
    ];
    const NUMBER_SYSTEM_ZERO_PARITY: [&str; 10] = [
        "GGGLLL", "GGLGLL", "GGLLGL", "GGLLLG", "GLGGLL", "GLLGGL", "GLLLGG", "GLGLGL", "GLGLLG",
        "GLLGLG",
    ];
    const NUMBER_SYSTEM_ONE_PARITY: [&str; 10] = [
        "LLLGGG", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL",
        "LGGLGL",
    ];

    let digits = digits
        .bytes()
        .map(|digit| usize::from(digit - b'0'))
        .collect::<Vec<_>>();
    let parity = if digits[0] == 0 {
        NUMBER_SYSTEM_ZERO_PARITY[digits[7]]
    } else {
        NUMBER_SYSTEM_ONE_PARITY[digits[7]]
    };
    let mut encoded = String::from("101");
    for (digit, parity) in digits[1..7].iter().zip(parity.bytes()) {
        encoded.push_str(if parity == b'L' {
            LEFT_ODD[*digit]
        } else {
            LEFT_EVEN[*digit]
        });
    }
    encoded.push_str("010101");

    encoded.bytes().map(|module| module == b'1').collect()
}

fn code39_dots(data: &str, narrow: usize, wide: usize) -> Vec<bool> {
    fn pattern(character: char) -> &'static str {
        match character {
            '*' => "nwnnwnwnn",
            'A' => "wnnnnwnnw",
            _ => panic!("the test helper only needs '*' and 'A'"),
        }
    }

    let characters = ['*']
        .into_iter()
        .chain(data.chars())
        .chain(['*'])
        .collect::<Vec<_>>();
    let mut dots = Vec::new();
    for (character_index, character) in characters.iter().enumerate() {
        for (element_index, element) in pattern(*character).bytes().enumerate() {
            let width = if element == b'w' { wide } else { narrow };
            dots.extend(std::iter::repeat_n(element_index % 2 == 0, width));
        }
        if character_index + 1 < characters.len() {
            dots.extend(std::iter::repeat_n(false, narrow));
        }
    }
    dots
}

fn itf_dots(data: &str, narrow: usize, wide: usize) -> Vec<bool> {
    fn pattern(digit: u8) -> &'static str {
        match digit {
            b'1' => "wnnnw",
            b'2' => "nwnnw",
            _ => panic!("the test helper only needs digits 1 and 2"),
        }
    }

    let mut elements = vec![
        (true, narrow),
        (false, narrow),
        (true, narrow),
        (false, narrow),
    ];
    for pair in data.as_bytes().chunks_exact(2) {
        for (bar, space) in pattern(pair[0]).bytes().zip(pattern(pair[1]).bytes()) {
            elements.push((true, if bar == b'w' { wide } else { narrow }));
            elements.push((false, if space == b'w' { wide } else { narrow }));
        }
    }
    elements.extend([(true, wide), (false, narrow), (true, narrow)]);

    let mut dots = Vec::new();
    for (dark, width) in elements {
        dots.extend(std::iter::repeat_n(dark, width));
    }
    dots
}

fn codabar_dots(data: &str, narrow: usize, wide: usize) -> Vec<bool> {
    fn pattern(character: char) -> &'static str {
        match character {
            'A' => "nnwwnwn",
            '0' => "nnnnnww",
            'B' => "nnnwnww",
            _ => panic!("the test helper only needs A, 0, and B"),
        }
    }

    let mut dots = Vec::new();
    for (character_index, character) in data.chars().enumerate() {
        for (element_index, element) in pattern(character).bytes().enumerate() {
            let width = if element == b'w' { wide } else { narrow };
            dots.extend(std::iter::repeat_n(element_index % 2 == 0, width));
        }
        if character_index + 1 < data.len() {
            dots.extend(std::iter::repeat_n(false, narrow));
        }
    }
    dots
}

fn code93_modules(values: &[usize]) -> Vec<bool> {
    const PATTERNS: [u16; 48] = [
        0b100010100,
        0b101001000,
        0b101000100,
        0b101000010,
        0b100101000,
        0b100100100,
        0b100100010,
        0b101010000,
        0b100010010,
        0b100001010,
        0b110101000,
        0b110100100,
        0b110100010,
        0b110010100,
        0b110010010,
        0b110001010,
        0b101101000,
        0b101100100,
        0b101100010,
        0b100110100,
        0b100011010,
        0b101011000,
        0b101001100,
        0b101000110,
        0b100101100,
        0b100010110,
        0b110110100,
        0b110110010,
        0b110101100,
        0b110100110,
        0b110010110,
        0b110011010,
        0b101101100,
        0b101100110,
        0b100110110,
        0b100111010,
        0b100101110,
        0b111010100,
        0b111010010,
        0b111001010,
        0b101101110,
        0b101110110,
        0b110101110,
        0b100100110,
        0b111011010,
        0b111010110,
        0b100110010,
        0b101011110,
    ];

    let mut modules = Vec::new();
    for value in values {
        modules.extend((0..9).rev().map(|bit| PATTERNS[*value] & (1 << bit) != 0));
    }
    modules.push(true);
    modules
}

fn code128_modules(values: &[usize]) -> Vec<bool> {
    fn widths(value: usize) -> &'static [usize] {
        match value {
            12 => &[1, 1, 2, 2, 3, 2],
            33 => &[1, 1, 1, 3, 2, 3],
            34 => &[1, 3, 1, 1, 2, 3],
            35 => &[1, 3, 1, 3, 2, 1],
            40 => &[2, 3, 1, 1, 1, 3],
            46 => &[1, 1, 3, 3, 2, 1],
            65 => &[1, 2, 1, 1, 2, 4],
            73 => &[1, 4, 2, 1, 1, 2],
            84 => &[1, 2, 4, 1, 1, 2],
            98 => &[4, 1, 1, 3, 1, 1],
            99 => &[1, 1, 3, 1, 4, 1],
            104 => &[2, 1, 1, 2, 1, 4],
            106 => &[2, 3, 3, 1, 1, 1, 2],
            _ => panic!("the test helper has no pattern for Code 128 value {value}"),
        }
    }

    let mut modules = Vec::new();
    for value in values {
        for (index, width) in widths(*value).iter().enumerate() {
            modules.extend(std::iter::repeat_n(index % 2 == 0, *width));
        }
    }
    modules
}
