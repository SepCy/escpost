use escpost_profiles::compile_all;

const CAPABILITIES: &[u8] = include_bytes!("fixtures/capabilities.json");
const PROFILE: &str = include_str!("fixtures/profile.toml");

#[test]
fn curated_profiles_replace_synthesis_and_widthless_profiles_are_skipped() {
    let out = compile_all(CAPABILITIES, &[PROFILE.to_string()]).expect("compiles");
    let ids: Vec<&str> = out.profiles.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["TEST-THERMAL-80"]);
    assert_eq!(out.skipped_upstream, ["TEST-WIDTHLESS"]);
}
