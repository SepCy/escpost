use escpost_profiles::resolver;

#[test]
fn resolver_finds_curated_and_synthesized_ids() {
    assert_eq!(
        resolver::resolve("REFERENCE").expect("reference").id,
        "REFERENCE"
    );
    assert_eq!(
        resolver::resolve("TM-T88III").expect("synthesized").id,
        "TM-T88III"
    );
    assert!(resolver::resolve("definitely-not-a-profile").is_err());
    let ids = resolver::available_ids().expect("ids");
    assert!(ids.iter().any(|id| id == "TM-T88III"));
    assert!(ids.iter().any(|id| id == "NT-5890K"));
}
