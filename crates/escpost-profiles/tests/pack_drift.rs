//! Guards against a stale committed profile pack (DD-031/DD-032): the
//! committed `profiles/.generated/profiles.json` must always equal a fresh
//! compile of the same enrichment TOMLs and upstream capabilities snapshot.

use escpost_profiles::{compile_all, to_canonical_profile_pack_json};
use std::fs;
use std::path::Path;

const CAPABILITIES: &[u8] =
    include_bytes!("../../../profiles/.escpos-printer-db/dist/capabilities.json");

fn enrichment_tomls() -> Vec<String> {
    // Mirror the bin's find_profile_files: visible dirs containing profile.toml.
    let profiles_dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../profiles"));
    let mut paths: Vec<_> = fs::read_dir(profiles_dir)
        .expect("profiles dir")
        .map(|e| e.expect("entry").path())
        .filter(|p| {
            p.is_dir()
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'))
                && p.join("profile.toml").is_file()
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| fs::read_to_string(p.join("profile.toml")).expect("read toml"))
        .collect()
}

#[test]
fn committed_pack_equals_a_fresh_compile() {
    let out = compile_all(CAPABILITIES, &enrichment_tomls()).expect("compile");
    let fresh = to_canonical_profile_pack_json(out.profiles).expect("serialize");
    let committed = include_bytes!("../../../profiles/.generated/profiles.json");
    assert_eq!(
        fresh, committed,
        "regenerate profiles/.generated/profiles.json (cargo run -p escpost-profiles --bin compile-profile-pack -- ...)"
    );
}
