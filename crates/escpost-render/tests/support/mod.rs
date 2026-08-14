use escpost_profiles::{PrinterProfile, compile_profile};

const CAPABILITIES_JSON: &[u8] = include_bytes!("../fixtures/capabilities.json");
const PROFILE_TOML: &str = include_str!("../fixtures/profile.toml");

pub fn test_profile() -> PrinterProfile {
    compile_profile(CAPABILITIES_JSON, PROFILE_TOML)
        .expect("the fictional renderer test profile should compile")
}
