use escpos2png::render;
use escpos2png_profiles::{PositioningBehavior, compile_profile};

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const HT: u8 = 0x09;
const LF: u8 = 0x0a;

#[test]
fn esc_dollar_sets_an_absolute_position_in_horizontal_motion_units() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [ESC, b'$', 30, 0, ESC, b'*', 1, 1, 0, 0b1000_0000, LF];

    let rendered = render(&input, &profile).expect("ESC $ should position the marker");
    let surface = &rendered.sheets[0].surface;

    // This profile has 203 motion units and 203 dots per inch, so 30 units
    // place the one-column marker exactly at x=30.
    assert!(surface.is_printed(30, 0));
    assert!(!surface.is_printed(30, 1));
    assert!(!(0..30).any(|x| surface.is_printed(x, 0)));
}

#[test]
fn nt_5890k_ignores_esc_dollar_after_printable_data() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let marker = [ESC, b'*', 1, 1, 0, 0b1000_0000];
    let input = [
        &[ESC, b'$', 40, 0],
        marker.as_slice(),
        &[ESC, b'$', 20, 0],
        marker.as_slice(),
        &[LF],
    ]
    .concat();

    let rendered = render(&input, &profile).expect("ignored ESC $ should still be consumed");
    let surface = &rendered.sheets[0].surface;

    // The second marker continues at x=41 instead of moving back to x=20.
    assert!(surface.is_printed(40, 0));
    assert!(surface.is_printed(41, 0));
    assert!(!surface.is_printed(20, 0));
}

#[test]
fn nt_5890k_still_applies_esc_dollar_after_a_nonprinting_tab() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [HT, ESC, b'$', 20, 0, ESC, b'*', 1, 1, 0, 0b1000_0000, LF];

    let rendered = render(&input, &profile).expect("ESC $ should apply before printable data");
    let surface = &rendered.sheets[0].surface;

    assert!(surface.is_printed(20, 0));
    assert!(!surface.is_printed(96, 0));
}

#[test]
fn epson_esc_dollar_repositions_after_printable_data() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    profile.commands.esc_dollar_after_printable_data = PositioningBehavior::Apply;
    let marker = [ESC, b'*', 1, 1, 0, 0b1000_0000];
    let input = [
        &[ESC, b'$', 40, 0],
        marker.as_slice(),
        &[ESC, b'$', 20, 0],
        marker.as_slice(),
        &[LF],
    ]
    .concat();

    let rendered = render(&input, &profile).expect("Epson ESC $ should reposition");
    let surface = &rendered.sheets[0].surface;

    assert!(surface.is_printed(40, 0));
    assert!(surface.is_printed(20, 0));
    assert!(!surface.is_printed(41, 0));
}

#[test]
fn epson_esc_backslash_moves_right_or_left_from_the_current_position() {
    let mut profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    profile.commands.esc_backslash_negative = PositioningBehavior::Apply;
    let input = [
        // Start at x=30 and move ten units right.
        ESC,
        b'$',
        30,
        0,
        ESC,
        b'\\',
        10,
        0,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
        // Start at x=30 and move ten units left. -10 is FFF6h.
        ESC,
        b'$',
        30,
        0,
        ESC,
        b'\\',
        0xf6,
        0xff,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
    ];

    let rendered = render(&input, &profile).expect("ESC \\ should move in both directions");
    let surface = &rendered.sheets[0].surface;

    assert!(surface.is_printed(40, 0));
    assert!(surface.is_printed(20, 30));
}

#[test]
fn nt_5890k_ignores_negative_esc_backslash() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [
        // The physical probe showed that FFF6h is consumed but does not move
        // this firmware ten units left from x=30.
        ESC,
        b'$',
        30,
        0,
        ESC,
        b'\\',
        0xf6,
        0xff,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
    ];

    let rendered = render(&input, &profile).expect("ignored ESC \\ should still be consumed");
    let surface = &rendered.sheets[0].surface;

    assert!(surface.is_printed(30, 0));
    assert!(!surface.is_printed(20, 0));
}

#[test]
fn esc_space_adds_profile_scaled_right_side_character_spacing() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [ESC, b' ', 5, b'A', ESC, b'*', 1, 1, 0, 0b1000_0000, LF];

    let rendered = render(&input, &profile).expect("ESC SP should add character spacing");
    let surface = &rendered.sheets[0].surface;

    // Font A advances 12 dots and this profile converts five horizontal
    // motion units to five dots. The marker therefore starts at x=17.
    assert!(surface.is_printed(17, 0));
    assert!(!surface.is_printed(17, 1));
}

#[test]
fn gs_p_changes_units_for_future_positions_without_moving_the_cursor() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [
        // Resolve x=10 with the profile's default 203-units-per-inch pitch.
        ESC,
        b'$',
        10,
        0,
        // Changing to 100 units per inch must not alter the stored x=10 dots.
        GS,
        b'P',
        100,
        0,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
        // A new ten-unit position now resolves to floor(10 * 203 / 100)=20.
        ESC,
        b'$',
        10,
        0,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
    ];

    let rendered = render(&input, &profile).expect("GS P should update the motion-unit state");
    let surface = &rendered.sheets[0].surface;

    assert!(surface.is_printed(10, 0));
    assert!(surface.is_printed(20, 30));
}

#[test]
fn ht_moves_to_the_next_default_eight_character_tab_stop() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [HT, ESC, b'*', 1, 1, 0, 0b1000_0000, LF];

    let rendered = render(&input, &profile).expect("HT should use the default tab stops");
    let surface = &rendered.sheets[0].surface;

    // Default tab stops are every eight default-font cells. Font A advances
    // 12 dots, so the first marker begins at 8 × 12 = 96.
    assert!(surface.is_printed(96, 0));
    assert!(!surface.is_printed(96, 1));
}

#[test]
fn esc_d_freezes_custom_tab_stops_using_the_current_character_advance() {
    let profile = compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile");
    let input = [
        // Font B is 9 dots wide; one dot of ESC SP makes a 10-dot advance.
        ESC,
        b'M',
        1,
        ESC,
        b' ',
        1,
        // Store tab columns 3 and 7 as fixed dot positions 30 and 70.
        ESC,
        b'D',
        3,
        7,
        0,
        HT,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        HT,
        ESC,
        b'*',
        1,
        1,
        0,
        0b1000_0000,
        LF,
    ];

    let rendered = render(&input, &profile).expect("ESC D should define both tab stops");
    let surface = &rendered.sheets[0].surface;

    assert!(surface.is_printed(30, 0));
    assert!(surface.is_printed(70, 0));
}
