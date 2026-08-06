use escpost_profiles::defaults;

#[test]
fn documented_constants_match_the_reference_baseline() {
    assert_eq!(defaults::DEFAULT_WIDTH_DOTS, 384);
    assert_eq!(defaults::DEFAULT_DPI, 203);
    let fonts = defaults::default_fonts();
    assert_eq!(
        (
            fonts.a.cell_width_dots,
            fonts.a.cell_height_dots,
            fonts.a.baseline_dots
        ),
        (12, 24, 20)
    );
    assert_eq!(
        (
            fonts.b.cell_width_dots,
            fonts.b.cell_height_dots,
            fonts.b.baseline_dots
        ),
        (9, 17, 14)
    );
    assert_eq!(defaults::derive_cell_width(512, 42), 12);
    assert_eq!(defaults::derive_cell_width(384, 0), 1);
}
