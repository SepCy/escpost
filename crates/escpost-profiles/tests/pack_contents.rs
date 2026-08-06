use escpost_profiles::compile_all;

const CAPABILITIES: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");
const NT: &str = include_str!("../../../profiles/NT-5890K/profile.toml");
const REFERENCE: &str = include_str!("../../../profiles/REFERENCE/profile.toml");

#[test]
fn pack_covers_curated_and_width_bearing_upstream() {
    let out =
        compile_all(CAPABILITIES, &[NT.to_string(), REFERENCE.to_string()]).expect("compiles");
    let ids: Vec<&str> = out.profiles.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"NT-5890K")); // curated enrichment
    assert!(ids.contains(&"REFERENCE")); // curated reference
    assert!(ids.contains(&"TM-T88III")); // synthesized (has width)
    assert!(!ids.contains(&"default")); // widthless generic, skipped
    assert!(out.skipped_upstream.iter().any(|s| s == "default"));
    // curated NT-5890K is not duplicated by synthesis:
    assert_eq!(ids.iter().filter(|id| **id == "NT-5890K").count(), 1);
}
