use escpos2png::{RenderError, render};
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const GS: u8 = 0x1d;

#[test]
fn prints_ean13_with_the_generated_check_digit_and_default_dimensions() {
    let mut profile = test_profile();
    // Keep the command test explicit about its required profile capability.
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_a = true;
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_a = true;
    profile.features.barcode_b = true;
    let function_a = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 5, b'1', b'2', b'3', 0];
    let function_b = [GS, b'h', 1, GS, b'w', 2, GS, b'k', 70, 2, b'1', b'2'];

    let rendered_a = render(&function_a, &profile).expect("Function A should ignore digit 3");
    let rendered_b = render(&function_b, &profile).expect("the even ITF reference should render");

    assert_eq!(rendered_a.sheets[0].surface, rendered_b.sheets[0].surface);
}

#[test]
fn prints_code39_with_printer_specific_narrow_and_wide_elements() {
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_a = true;
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_a = true;
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
fn code128_auto_encodes_plain_text_without_an_explicit_code_set() {
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;

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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = true;
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
    let mut profile = test_profile();
    profile.features.barcode_b = false;
    let input = [GS, b'k', 79, 1, b'A'];

    let error = render(&input, &profile)
        .expect_err("Code 128 auto belongs to the Function B command family");

    assert!(matches!(
        error,
        RenderError::CommandUnsupportedByProfile {
            command: "GS k Function B barcode",
            ..
        }
    ));
}

#[test]
fn rejects_native_barcodes_when_the_profile_does_not_support_them() {
    let mut profile = test_profile();
    // Capability gating remains important even though the physical Netum
    // probe corrected this particular profile to support native barcodes.
    profile.features.barcode_b = false;
    let input = [
        GS, b'k', 67, 12, b'5', b'9', b'0', b'1', b'2', b'3', b'4', b'1', b'2', b'3', b'4', b'5',
    ];

    let error = render(&input, &profile).expect_err("the synthetic profile disables barcodes");

    assert!(matches!(
        error,
        RenderError::CommandUnsupportedByProfile {
            command: "GS k Function B barcode",
            ..
        }
    ));
}

fn test_profile() -> escpos2png_profiles::PrinterProfile {
    compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile
}

fn cell_contains_printed_dots(
    surface: &escpos2png::MonoSurface,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> bool {
    (left..left + width).any(|x| (top..top + height).any(|y| surface.is_printed(x, y)))
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
