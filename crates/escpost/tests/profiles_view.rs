use escpost::profiles_cmd::ProfileView;
use escpost_profiles::resolver;

#[test]
fn view_maps_source_and_derived_mm() {
    let p = resolver::resolve("TM-T88III").unwrap();
    let v = ProfileView::from_profile(p);
    assert_eq!(v.source, "synthesized");
    assert_eq!(v.paper_width_mm, 80.0); // f64 from tenths ÷ 10
    assert!((v.printable_width_mm - 72.2).abs() < 0.05); // 512/180*25.4
    assert_eq!(v.dpi_x, 180);
    // JSON round-trips with snake_case keys:
    let j = serde_json::to_value(&v).unwrap();
    assert_eq!(j["id"], "TM-T88III");
    assert_eq!(j["source"], "synthesized");
    assert!(j["features"]["barcodes"]["function_b"].is_array());
}
