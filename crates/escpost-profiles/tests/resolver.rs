use escpost_profiles::resolver;

#[test]
fn resolver_loads_the_embedded_pack() {
    assert_eq!(
        resolver::resolve("REFERENCE").expect("reference").id,
        "REFERENCE"
    );
    assert!(resolver::resolve("definitely-not-a-profile").is_err());
    let ids = resolver::available_ids().expect("ids");
    assert!(ids.iter().any(|id| id == "REFERENCE"));
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
}
