use escpost_profiles::compile_profile;
use escpost_render::{RenderError, render};

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
const ESC: u8 = 0x1b;
const LF: u8 = 0x0a;

#[test]
fn esc_r_substitutes_the_documented_ascii_positions() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let german = [
        ESC, b'R', 2, b'@', b'[', b'\\', b']', b'{', b'|', b'}', b'~', LF,
    ];
    let same_characters_in_cp1252 = [
        ESC, b't', 16, 0xa7, 0xc4, 0xd6, 0xdc, 0xe4, 0xf6, 0xfc, 0xdf, LF,
    ];

    let international =
        render(&german, &profile).expect("ESC R should select the German substitutions");
    let encoded = render(&same_characters_in_cp1252, &profile)
        .expect("the same Unicode characters should render through CP1252");

    // Comparing complete dot surfaces proves that ESC R substitutes before
    // glyph lookup without coupling this test to compressed PNG bytes.
    assert_eq!(international.sheets[0].surface, encoded.sheets[0].surface);
}

#[test]
fn esc_at_restores_the_default_international_character_set() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let selected_then_reset = [ESC, b'R', 1, b'@', LF, ESC, b'@', b'@', LF];
    let direct_characters = [ESC, b't', 16, 0xe0, LF, ESC, b'@', b'@', LF];

    let reset = render(&selected_then_reset, &profile)
        .expect("ESC @ should restore the profile's international set");
    let expected =
        render(&direct_characters, &profile).expect("the equivalent characters should render");

    assert_eq!(reset.sheets[0].surface, expected.sheets[0].surface);
}

#[test]
fn esc_r_rejects_a_set_outside_the_version_one_table() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");

    let error = render(&[ESC, b'R', 18], &profile)
        .expect_err("an unsupported set must not silently select different glyphs");

    assert!(matches!(
        error,
        RenderError::UnsupportedInternationalCharacterSet {
            character_set: 18,
            offset: 0,
        }
    ));
}

#[test]
fn truncated_esc_r_reports_its_command_boundary() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");

    let error =
        render(&[ESC, b'R'], &profile).expect_err("a missing operand must stop interpretation");

    assert!(matches!(
        error,
        RenderError::TruncatedCommand {
            command: "ESC R",
            offset: 0,
        }
    ));
}
