use escpos2png::render;
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const ESC: u8 = 0x1b;
const LF: u8 = 0x0a;

#[test]
fn esc_p_consumes_a_supported_drawer_pulse_without_painting_dots() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [ESC, b'p', 0, 50, 50, LF];

    let rendered = render(&input, &profile).expect("the profile supports standard drawer pulses");
    let surface = &rendered.sheets[0].surface;

    // A drawer pulse affects hardware but not paper. LF gives the otherwise
    // blank preview a 30-dot paper height.
    assert_eq!((surface.width(), surface.height()), (384, 30));
    assert!(!(0..surface.width()).any(|x| (0..surface.height()).any(|y| surface.is_printed(x, y))));
}
