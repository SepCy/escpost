use escpost_profiles::compile_profile;
use escpost_render::{DeviceEvent, render};

const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const ENRICHMENT_TOML: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
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
}

#[test]
fn reports_the_renderer_and_canonical_profile_identity() {
    let profile = test_profile();

    let rendered = render(&[LF], &profile).expect("the blank line should render");

    assert_eq!(
        rendered.metadata.renderer_version,
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(rendered.metadata.profile_id, "NT-5890K");
    assert_eq!(rendered.metadata.canonical_profile_sha256.len(), 64);
}

fn test_profile() -> escpost_profiles::PrinterProfile {
    compile_profile(CAPABILITIES_JSON, ENRICHMENT_TOML).expect("the test profile should compile")
}
