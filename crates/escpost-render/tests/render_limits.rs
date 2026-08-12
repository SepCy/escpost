use escpost_profiles::compile_profile;
use escpost_render::{LimitKind, RenderError, RenderLimits, RenderOptions, render_with_options};

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const LF: u8 = 0x0a;

#[test]
fn rejects_input_before_parsing_when_its_byte_limit_is_exceeded() {
    let profile = test_profile();
    let options = options_with(|limits| limits.max_input_bytes = 1);

    let result = render_with_options(b"AB", &profile, &options);

    assert_limit(result, LimitKind::InputBytes, 2, 1);
}

#[test]
fn rejects_a_declared_raster_payload_before_allocating_or_painting_it() {
    let profile = test_profile();
    let options = options_with(|limits| limits.max_command_payload_bytes = 0);
    let input = [GS, b'v', b'0', 0, 1, 0, 1, 0, 0b1000_0000];

    let result = render_with_options(&input, &profile, &options);

    assert_limit(result, LimitKind::CommandPayloadBytes, 1, 0);
}

#[test]
fn rejects_a_profile_wider_than_the_configured_surface_limit() {
    let profile = test_profile();
    let options = options_with(|limits| limits.max_sheet_width_dots = 383);

    let result = render_with_options(&[], &profile, &options);

    assert_limit(result, LimitKind::SheetWidthDots, 384, 383);
}

#[test]
fn rejects_a_feed_before_growing_a_sheet_beyond_its_height_limit() {
    let profile = test_profile();
    let options = options_with(|limits| limits.max_sheet_height_dots = 50);
    let input = [ESC, b'd', 2];

    let result = render_with_options(&input, &profile, &options);

    assert_limit(result, LimitKind::SheetHeightDots, 60, 50);
}

#[test]
fn rejects_a_surface_before_exceeding_the_total_dot_budget() {
    let profile = test_profile();
    let options = options_with(|limits| limits.max_total_dots = 384 * 29);

    let result = render_with_options(&[LF], &profile, &options);

    assert_limit(result, LimitKind::TotalDots, 384 * 30, 384 * 29);
}

#[test]
fn rejects_output_that_would_create_more_than_the_allowed_sheet_count() {
    let mut profile = test_profile();
    profile.features.paper_full_cut = true;
    let options = options_with(|limits| limits.max_sheets = 1);
    let input = [b'A', LF, GS, b'V', 0, b'B', LF];

    let result = render_with_options(&input, &profile, &options);

    assert_limit(result, LimitKind::Sheets, 2, 1);
}

fn test_profile() -> escpost_profiles::PrinterProfile {
    compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML).expect("the test profile should compile")
}

fn options_with(update: impl FnOnce(&mut RenderLimits)) -> RenderOptions {
    let mut limits = RenderLimits::default();
    update(&mut limits);
    RenderOptions {
        limits,
        ..RenderOptions::default()
    }
}

fn assert_limit(
    result: Result<escpost_render::RenderResult, RenderError>,
    expected_kind: LimitKind,
    expected_value: u64,
    expected_limit: u64,
) {
    assert!(matches!(
        result,
        Err(RenderError::LimitExceeded {
            kind,
            value,
            limit,
        }) if kind == expected_kind && value == expected_value && limit == expected_limit
    ));
}
