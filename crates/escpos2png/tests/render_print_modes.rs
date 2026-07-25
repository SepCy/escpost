use escpos2png::render;
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;

#[test]
fn text_print_modes_do_not_change_raster_graphics() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile;
    let input = [
        ESC,
        b'!',
        0xb8,
        GS,
        b'!',
        0x77,
        GS,
        b'B',
        1,
        GS,
        b'v',
        b'0',
        0,
        1,
        0,
        1,
        0,
        0b1000_0000,
    ];

    let rendered = render(&input, &profile).expect("text modes must not alter GS v 0");
    let surface = &rendered.sheets[0].surface;

    // Emphasis, underline, reverse, and character multipliers apply to text.
    // The one source raster dot must remain exactly one printed dot.
    assert_eq!((surface.width(), surface.height()), (384, 1));
    assert!(surface.is_printed(0, 0));
    assert_eq!(
        (0..surface.width())
            .filter(|&x| surface.is_printed(x, 0))
            .count(),
        1
    );
}
