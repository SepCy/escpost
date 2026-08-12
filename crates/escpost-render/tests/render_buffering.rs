mod support;

use escpost_render::render;
use support::test_profile;

const GS: u8 = 0x1d;
const LF: u8 = 0x0a;

#[test]
fn gs_l_and_gs_w_are_ignored_after_the_line_has_started() {
    let profile = test_profile();
    let input = [
        GS, b'B', 1, b' ', GS, b'L', 24, 0, GS, b'W', 120, 0, LF, b' ', LF,
    ];

    let rendered = render(&input, &profile).expect("mid-line print-area commands are ignored");
    let surface = &rendered.sheets[0].surface;

    // Epson enables GS L and GS W only at the beginning of a line. Both
    // reversed cells therefore remain at the default physical x=0.
    assert_eq!(count_printed_dots(surface, 0, 12, 24), 12 * 24);
    assert_eq!(count_printed_dots(surface, 0, 12, 54), 12 * 48);
}

#[test]
fn gs_v0_processes_its_parameters_as_normal_data_after_the_line_has_started() {
    let profile = test_profile();
    let input = [
        // Reverse mode makes otherwise blank spaces fully measurable.
        GS, b'B', 1, b' ', GS, b'v', b'0', b' ', b' ', LF,
    ];

    let rendered = render(&input, &profile).expect("mid-line GS v 0 should become normal data");
    let surface = &rendered.sheets[0].surface;

    // Epson consumes the three-byte GS v 0 prefix, then processes m and the
    // following bytes normally. All three spaces therefore remain visible.
    assert_eq!((surface.width(), surface.height()), (384, 30));
    assert_eq!(count_printed_dots(surface, 0, 36, 24), 36 * 24);
    assert_eq!(count_printed_dots(surface, 36, 348, 24), 0);
}

#[test]
fn esc_a_is_ignored_after_the_line_has_started() {
    let profile = test_profile();
    let input = [GS, b'B', 1, b' ', 0x1b, b'a', 2, LF, b' ', LF];

    let rendered = render(&input, &profile).expect("mid-line ESC a should be ignored");
    let surface = &rendered.sheets[0].surface;

    // The ignored right-justification setting must not leak into either the
    // active line or the following line.
    assert_eq!(count_printed_dots(surface, 0, 12, 54), 12 * 48);
    assert_eq!(count_printed_dots(surface, 12, 372, 54), 0);
}

#[test]
fn gs_v_is_ignored_after_the_line_has_started() {
    let profile = test_profile();
    let input = [GS, b'B', 1, b' ', GS, b'V', 0, LF, b' ', LF];

    let rendered = render(&input, &profile).expect("mid-line GS V should be ignored");

    // The fictional test profile does not support cutting. Epson's beginning-of-line rule
    // takes precedence, so this command is ignored without a capability error.
    assert_eq!(rendered.sheets.len(), 1);
    let surface = &rendered.sheets[0].surface;
    assert_eq!((surface.width(), surface.height()), (384, 60));
    assert_eq!(count_printed_dots(surface, 0, 12, 54), 12 * 48);
}

#[test]
fn gs_v_function_b_consumes_its_feed_operand_when_ignored_mid_line() {
    let profile = test_profile();
    let input = [GS, b'B', 1, b' ', GS, b'V', 65, b' ', LF];

    let rendered = render(&input, &profile).expect("mid-line Function B should be ignored");
    let surface = &rendered.sheets[0].surface;

    // The beginning-of-line rule suppresses the feed, but the complete
    // four-byte command is still consumed. Its n operand is not printable.
    assert_eq!((surface.width(), surface.height()), (384, 30));
    assert_eq!(count_printed_dots(surface, 0, 12, 24), 12 * 24);
    assert_eq!(count_printed_dots(surface, 12, 372, 24), 0);
}

fn count_printed_dots(
    surface: &escpost_render::MonoSurface,
    left: u32,
    width: u32,
    height: u32,
) -> usize {
    (left..left + width)
        .flat_map(|x| (0..height).map(move |y| (x, y)))
        .filter(|&(x, y)| surface.is_printed(x, y))
        .count()
}
