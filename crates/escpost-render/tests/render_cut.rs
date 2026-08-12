mod support;

use escpost_profiles::{CutterGeometry, FeedBehavior};
use escpost_render::{RenderWarning, render};
use support::test_profile;

const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const LF: u8 = 0x0a;

#[test]
fn gs_v_full_cut_finishes_one_sheet_and_starts_the_next() {
    let mut profile = test_profile();
    // The fictional test profile has no cutter. Enable this capability only in the
    // test profile so the generic renderer's sheet behavior can be exercised.
    profile.features.paper_full_cut = true;
    let marker_line = [ESC, b'*', 1, 1, 0, 0b1000_0000, LF];
    let input = [
        marker_line.as_slice(),
        &[GS, b'V', 0],
        marker_line.as_slice(),
    ]
    .concat();

    let rendered = render(&input, &profile).expect("GS V should create a sheet boundary");

    assert_eq!(rendered.sheets.len(), 2);
    for sheet in &rendered.sheets {
        assert_eq!((sheet.surface.width(), sheet.surface.height()), (384, 30));
        assert!(sheet.surface.is_printed(0, 0));
    }
}

#[test]
fn gs_v_partial_cut_uses_its_own_profile_capability() {
    let mut profile = test_profile();
    profile.features.paper_part_cut = true;
    let marker_line = [ESC, b'*', 1, 1, 0, 0b1000_0000, LF];
    let input = [
        marker_line.as_slice(),
        &[GS, b'V', b'1'],
        marker_line.as_slice(),
    ]
    .concat();

    let rendered = render(&input, &profile).expect("a supported partial cut should split sheets");

    // A partial cut still separates the preview into physical receipt sheets.
    // The uncut bridge is a mechanism detail, not printable content.
    assert_eq!(rendered.sheets.len(), 2);
    assert!(
        rendered
            .sheets
            .iter()
            .all(|sheet| sheet.surface.is_printed(0, 0))
    );
}

#[test]
fn gs_v_full_cut_splits_the_preview_and_warns_without_a_cutter() {
    let profile = test_profile();
    // The fictional test profile has no cutter, so the full cut cannot be performed. The
    // render still succeeds: the preview splits at the receipt boundary and a
    // warning records that the paper was not physically cut.
    let marker_line = [ESC, b'*', 1, 1, 0, 0b1000_0000, LF];
    let input = [
        marker_line.as_slice(),
        &[GS, b'V', 0],
        marker_line.as_slice(),
    ]
    .concat();

    let rendered =
        render(&input, &profile).expect("an unsupported cut should warn, not fail the render");

    assert_eq!(rendered.sheets.len(), 2);
    for sheet in &rendered.sheets {
        assert_eq!((sheet.surface.width(), sheet.surface.height()), (384, 30));
    }
    assert_eq!(rendered.warnings.len(), 1);
    assert!(matches!(
        rendered.warnings[0],
        RenderWarning::UncuttableCut {
            command: "GS V full cut",
            ref profile,
            // The cut follows the seven-byte marker line.
            offset: 7,
        } if profile == "TEST-RENDERER"
    ));
}

#[test]
fn epson_gs_v_function_b_splits_and_warns_for_both_modes_without_an_autocutter() {
    let mut profile = test_profile();
    profile.commands.gs_v_function_b_partial = FeedBehavior::Feed;
    let input = [
        // At 203 dpi, 101 vertical units per inch make each unit two dots.
        GS, b'P', 203, 101, GS, b'V', 65, 5, GS, b'V', 66, 5,
    ];

    let rendered =
        render(&input, &profile).expect("both Epson Function B modes should feed and split");

    // Without a cutter each fed Function B cut performs its five-unit feed, then
    // splits the preview at the boundary and warns that no physical cut happened.
    assert_eq!(rendered.sheets.len(), 2);
    for sheet in &rendered.sheets {
        assert_eq!((sheet.surface.width(), sheet.surface.height()), (384, 10));
    }
    assert_eq!(rendered.warnings.len(), 2);
    assert!(rendered.warnings.iter().all(|warning| matches!(
        warning,
        RenderWarning::UncuttableCut { profile, .. } if profile == "TEST-RENDERER"
    )));
}

#[test]
fn gs_v_function_b_feeds_to_an_autocutter_then_finishes_the_sheet() {
    let mut profile = test_profile();
    // A synthetic cutter keeps this public rendering test independent of
    // tomorrow's physical profile. The distances are deliberately distinct:
    // 12 dots from print head to blade, then 5 units × 2 dots per unit.
    profile.features.paper_full_cut = true;
    profile.cutter = Some(CutterGeometry {
        print_head_to_cutter_dots: 12,
    });
    let marker_line = [ESC, b'*', 1, 1, 0, 0b1000_0000, LF];
    let input = [
        marker_line.as_slice(),
        &[GS, b'P', 203, 101],
        &[GS, b'V', 65, 5],
        marker_line.as_slice(),
    ]
    .concat();

    let rendered = render(&input, &profile)
        .expect("Function B should feed to the cutter and create a sheet boundary");

    assert_eq!(rendered.sheets.len(), 2);
    assert_eq!(rendered.sheets[0].surface.height(), 52);
    assert!(rendered.sheets[0].surface.is_printed(0, 0));
    assert_eq!(rendered.sheets[1].surface.height(), 30);
    assert!(rendered.sheets[1].surface.is_printed(0, 0));
}

#[test]
fn gs_v_function_b_partial_cut_uses_the_same_cutter_geometry() {
    let mut profile = test_profile();
    profile.features.paper_part_cut = true;
    profile.commands.gs_v_function_b_partial = FeedBehavior::Feed;
    profile.cutter = Some(CutterGeometry {
        print_head_to_cutter_dots: 8,
    });
    let marker_line = [ESC, b'*', 1, 1, 0, 0b1000_0000, LF];
    let input = [
        marker_line.as_slice(),
        &[GS, b'P', 203, 203],
        &[GS, b'V', 66, 4],
        marker_line.as_slice(),
    ]
    .concat();

    let rendered =
        render(&input, &profile).expect("a supported Function B partial cut should split sheets");

    // Partial cuts leave a physical bridge, but output after the blade action
    // still belongs to a new receipt sheet in a preview.
    assert_eq!(rendered.sheets.len(), 2);
    assert_eq!(rendered.sheets[0].surface.height(), 42);
    assert_eq!(rendered.sheets[1].surface.height(), 30);
}

#[test]
fn test_profile_splits_for_gs_v_65_but_ignores_gs_v_66() {
    let profile = test_profile();
    let input = [
        // At 203 dpi, 101 vertical units per inch make each unit two dots.
        GS, b'P', 203, 101, GS, b'V', 65, 5, GS, b'V', 66, 5,
    ];

    let rendered = render(&input, &profile).expect("both Function B forms should be consumed");

    // This firmware acts on the full-cut form (feed then split, with a warning)
    // but consumes the partial-cut form as a no-op. The single fed sheet and the
    // lone warning show GS V 66 produced neither a boundary nor a diagnostic.
    assert_eq!(rendered.sheets.len(), 1);
    assert_eq!(
        (
            rendered.sheets[0].surface.width(),
            rendered.sheets[0].surface.height(),
        ),
        (384, 10)
    );
    assert_eq!(rendered.warnings.len(), 1);
    assert!(matches!(
        rendered.warnings[0],
        RenderWarning::UncuttableCut {
            command: "GS V full cut",
            ..
        }
    ));
}
