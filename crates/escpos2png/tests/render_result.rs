use escpos2png::{
    Completeness, DeviceEvent, DiagnosticEffect, DiagnosticSeverity, InitialStateAssumption, render,
};
use escpos2png_profiles::compile_profile;

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/upstream/escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/enrichments/NT-5890K.toml");
const ESC: u8 = 0x1b;
const LF: u8 = 0x0a;

#[test]
fn reports_nonprinting_device_actions_as_structured_events() {
    let profile = test_profile();
    let input = [ESC, b'p', 0, 50, 100, LF];

    let rendered = render(&input, &profile).expect("the drawer pulse should render");

    assert_eq!(
        rendered.device_events,
        vec![DeviceEvent::CashDrawerPulse {
            connector: 0,
            on_time_units: 50,
            off_time_units: 100,
        }]
    );
    assert_eq!(
        rendered.completeness,
        Completeness::CompleteWithNonVisualEvents
    );
}

#[test]
fn reports_the_renderer_profile_and_initial_state_assumption() {
    let profile = test_profile();

    let rendered = render(&[LF], &profile).expect("the blank line should render");

    assert_eq!(
        rendered.metadata.renderer_version,
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(rendered.metadata.profile_id, "NT-5890K");
    assert_eq!(rendered.metadata.profile_revision, 8);
    assert_eq!(rendered.metadata.canonical_profile_sha256.len(), 64);
    assert_eq!(
        rendered.metadata.upstream_repository,
        "https://github.com/receipt-print-hq/escpos-printer-db.git"
    );
    assert_eq!(
        rendered.metadata.upstream_commit,
        "e3bf6056ee75cf70ffaccb925081fffa7ad6ced5"
    );
    assert_eq!(
        rendered.metadata.upstream_profile_sha256,
        "2e471a3f255d2dc85988d350754023a107a882d33504dd2df5e9f3c8d4d79b0b"
    );
    assert_eq!(rendered.metadata.enrichment_sha256.len(), 64);
    assert_eq!(
        rendered.metadata.initial_state,
        InitialStateAssumption::ProfileResetDefaults
    );
}

#[test]
fn exposes_profile_approximations_without_marking_pixels_incomplete() {
    let profile = test_profile();

    let rendered = render(&[LF], &profile).expect("the blank line should render");

    assert_eq!(rendered.completeness, Completeness::Complete);
    assert!(rendered.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == DiagnosticSeverity::Information
            && diagnostic.byte_offset.is_none()
            && diagnostic.command.is_none()
            && diagnostic.effect == DiagnosticEffect::None
            && diagnostic.message.contains("fonts.resident_glyph_shapes")
    }));
}

fn test_profile() -> escpos2png_profiles::PrinterProfile {
    compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML)
        .expect("the test profile should compile")
        .profile
}
