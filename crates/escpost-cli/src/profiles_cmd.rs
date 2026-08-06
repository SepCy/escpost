//! `ProfileView` data, rendering, and command handlers for the `escpost
//! profiles` catalog commands (`list`, and `show`/`find` in later tasks).
//! `run` dispatches each `ProfilesCommand`; each handler gathers profiles via
//! `escpost_profiles::resolver`, projects them through `ProfileView::from_profile`,
//! and prints either `render_table`/`render_detail` or `--json`.

use std::collections::BTreeSet;
use std::io::IsTerminal;

use escpost_profiles::resolver::{self, ResolveError};
use escpost_profiles::{BarcodeSystem, Font, PrinterProfile, ProfileSource};
use inquire::Select;
use serde::Serialize;

use crate::cli::{
    FindProfileArgs, ListProfilesArgs, ProfilesArgs, ProfilesCommand, ShowProfileArgs, SourceFilter,
};
use crate::error::CliError;

/// Dispatches `escpost profiles <subcommand>`.
pub(crate) fn run(arguments: ProfilesArgs, non_interactive: bool) -> Result<(), CliError> {
    match arguments.command {
        ProfilesCommand::List(list_arguments) => run_list(list_arguments),
        ProfilesCommand::Show(show_arguments) => run_show(show_arguments),
        ProfilesCommand::Find(find_arguments) => run_find(find_arguments, non_interactive),
    }
}

/// Handles `escpost profiles list`: gathers every embedded profile, applies
/// the `--vendor`/`--source`/`--search` filters (AND), then prints either the
/// compact table or `--json`.
fn run_list(arguments: ListProfilesArgs) -> Result<(), CliError> {
    let ids = resolver::available_ids().map_err(map_resolve_error)?;
    let mut views = ids
        .iter()
        .map(|id| {
            resolver::resolve(id)
                .map(ProfileView::from_profile)
                .map_err(map_resolve_error)
        })
        .collect::<Result<Vec<ProfileView>, CliError>>()?;
    views.sort_by(|left, right| left.id.cmp(&right.id));

    let filtered: Vec<ProfileView> = views
        .into_iter()
        .filter(|view| matches_filters(view, &arguments))
        .collect();

    if filtered.is_empty() {
        eprintln!("no profiles match");
        return Ok(());
    }

    if arguments.json {
        println!("{}", serde_json::to_string_pretty(&filtered)?);
    } else {
        println!("{}", render_table(&filtered));
    }
    Ok(())
}

/// Handles `escpost profiles show <id>`: resolves the profile, then prints
/// either the detail view or `--json`.
fn run_show(arguments: ShowProfileArgs) -> Result<(), CliError> {
    let profile = resolver::resolve(&arguments.id).map_err(map_resolve_error)?;
    let view = ProfileView::from_profile(profile);

    if arguments.json {
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        println!("{}", render_detail(&view));
    }
    Ok(())
}

/// Handles `escpost profiles find`: an interactive substring picker over
/// every embedded profile. Refuses to run without a usable terminal (honoring
/// the global `--non-interactive` flag), pointing at `profiles list --search`
/// instead.
fn run_find(_arguments: FindProfileArgs, non_interactive: bool) -> Result<(), CliError> {
    let can_prompt =
        !non_interactive && std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
    if !can_prompt {
        return Err(CliError::InteractiveFindUnavailable);
    }

    let ids = resolver::available_ids().map_err(map_resolve_error)?;
    let mut views = ids
        .iter()
        .map(|id| {
            resolver::resolve(id)
                .map(ProfileView::from_profile)
                .map_err(map_resolve_error)
        })
        .collect::<Result<Vec<ProfileView>, CliError>>()?;
    views.sort_by(|left, right| left.id.cmp(&right.id));

    let options: Vec<(String, String)> = views
        .into_iter()
        .map(|view| {
            (
                format!("{} — {} · {}", view.id, view.vendor, view.model),
                view.id,
            )
        })
        .collect();
    let labels: Vec<String> = options.iter().map(|(label, _)| label.clone()).collect();

    let chosen = Select::new("Find a printer profile", labels)
        .with_page_size(10)
        .prompt()
        .map_err(|error| CliError::ProfilePrompt(error.to_string()))?;

    let id = options
        .into_iter()
        .find(|(label, _)| *label == chosen)
        .map(|(_, id)| id)
        .unwrap_or(chosen);
    println!("{id}");
    Ok(())
}

fn matches_filters(view: &ProfileView, arguments: &ListProfilesArgs) -> bool {
    let vendor_matches = arguments
        .vendor
        .as_deref()
        .is_none_or(|vendor| contains_ignore_case(&view.vendor, vendor));

    let source_matches = arguments.source.as_ref().is_none_or(|source| {
        view.source
            .eq_ignore_ascii_case(source_filter_label(*source))
    });

    let search_matches = arguments.search.as_deref().is_none_or(|search| {
        contains_ignore_case(&view.id, search)
            || contains_ignore_case(&view.vendor, search)
            || contains_ignore_case(&view.model, search)
    });

    vendor_matches && source_matches && search_matches
}

fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

fn source_filter_label(filter: SourceFilter) -> &'static str {
    match filter {
        SourceFilter::Calibrated => "calibrated",
        SourceFilter::Synthesized => "synthesized",
        SourceFilter::Virtual => "virtual",
    }
}

/// Mirrors `crate::profiles::load`'s `ResolveError` → `CliError` mapping.
fn map_resolve_error(error: ResolveError) -> CliError {
    match error {
        ResolveError::UnknownProfile(id) => CliError::UnknownProfile(id),
        ResolveError::LoadPack(message) => CliError::LoadProfiles(message),
    }
}

/// Maps a profile's provenance to the catalog's calibration vocabulary: how
/// its physical fidelity was obtained (see the profiles-command design doc).
pub fn source_label(source: &ProfileSource) -> &'static str {
    match source {
        ProfileSource::Upstream { .. } => "calibrated",
        ProfileSource::UpstreamDefault { .. } => "synthesized",
        ProfileSource::Reference => "virtual",
    }
}

/// The one-character calibration marker shown in the compact table and the
/// detail view, keyed by the `source_label` string.
fn calibration_marker(source_label: &str) -> &'static str {
    match source_label {
        "calibrated" => "✓",
        "synthesized" => "~",
        "virtual" => "○",
        _ => "?",
    }
}

/// A catalog-ready projection of `PrinterProfile`: every field the `list`,
/// `show`, and `--json` outputs need, already resolved to display units
/// (millimeters, source labels, barcode system names). Both the human table
/// and the JSON output are built from this one shape (design doc: "the human
/// `list` table is a compact projection of it").
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProfileView {
    pub id: String,
    pub vendor: String,
    pub model: String,
    pub source: String,
    pub paper_width_mm: f64,
    pub printable_width_mm: f64,
    pub printable_width_dots: u32,
    pub dpi_x: u32,
    pub dpi_y: u32,
    pub fonts: FontsView,
    pub features: FeaturesView,
    pub code_page_count: usize,
    pub canonical_profile_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FontsView {
    pub a: FontView,
    pub b: FontView,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FontView {
    pub cell_width_dots: u32,
    pub cell_height_dots: u32,
    pub baseline_dots: u32,
}

impl From<&Font> for FontView {
    fn from(font: &Font) -> Self {
        FontView {
            cell_width_dots: font.cell_width_dots,
            cell_height_dots: font.cell_height_dots,
            baseline_dots: font.baseline_dots,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeaturesView {
    pub barcodes: BarcodesView,
    pub graphics: bool,
    pub paper_full_cut: bool,
    pub paper_part_cut: bool,
    pub qr_code: bool,
    pub pulse_standard: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BarcodesView {
    pub function_a: Vec<String>,
    pub function_b: Vec<String>,
}

impl ProfileView {
    pub fn from_profile(profile: &PrinterProfile) -> Self {
        let paper_width_mm = f64::from(profile.paper_width_tenths_mm) / 10.0;
        let printable_width_mm = f64::from(profile.geometry.printable_width_dots)
            / f64::from(profile.geometry.dpi_x)
            * 25.4;

        ProfileView {
            id: profile.id.clone(),
            vendor: profile.vendor.clone(),
            model: profile.model.clone(),
            source: source_label(&profile.source).to_owned(),
            paper_width_mm,
            printable_width_mm,
            printable_width_dots: profile.geometry.printable_width_dots,
            dpi_x: profile.geometry.dpi_x,
            dpi_y: profile.geometry.dpi_y,
            fonts: FontsView {
                a: FontView::from(&profile.fonts.a),
                b: FontView::from(&profile.fonts.b),
            },
            features: FeaturesView {
                barcodes: BarcodesView {
                    function_a: barcode_system_names(&profile.features.barcodes.function_a),
                    function_b: barcode_system_names(&profile.features.barcodes.function_b),
                },
                graphics: profile.features.graphics,
                paper_full_cut: profile.features.paper_full_cut,
                paper_part_cut: profile.features.paper_part_cut,
                qr_code: profile.features.qr_code,
                pulse_standard: profile.features.pulse_standard,
            },
            code_page_count: profile.code_pages.len(),
            canonical_profile_sha256: profile.canonical_profile_sha256.clone(),
        }
    }
}

/// Converts each barcode system to its canonical snake_case name (e.g.
/// `code_128`) by reusing its own `Serialize` impl, rather than duplicating
/// the naming rules embedded in `escpost_profiles::BarcodeSystem`'s
/// `#[serde(rename_all/rename)]` attributes.
fn barcode_system_names(systems: &BTreeSet<BarcodeSystem>) -> Vec<String> {
    systems
        .iter()
        .map(|system| match serde_json::to_value(system) {
            Ok(serde_json::Value::String(name)) => name,
            _ => unreachable!("BarcodeSystem always serializes to a JSON string"),
        })
        .collect()
}

const TABLE_HEADERS: [&str; 11] = [
    "PROFILE", "VENDOR", "MODEL", "CAL", "PAPER", "PRINT", "DOTS", "DPI", "CUT", "BC", "QR",
];

const TABLE_LEGEND: &str =
    "CAL: ✓ calibrated · ~ synthesized · ○ virtual   PAPER/PRINT mm, DOTS printable";

/// Renders the compact `profiles list` table: one row per view, aligned
/// columns, followed by a blank line and the CAL legend.
pub fn render_table(views: &[ProfileView]) -> String {
    let mut rows: Vec<[String; 11]> = vec![TABLE_HEADERS.map(str::to_owned)];
    rows.extend(views.iter().map(table_row));

    let mut widths = [0usize; 11];
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }

    let mut lines: Vec<String> = rows
        .iter()
        .map(|row| format_table_row(row, &widths))
        .collect();
    lines.push(String::new());
    lines.push(TABLE_LEGEND.to_owned());
    lines.join("\n")
}

fn format_table_row(cells: &[String; 11], widths: &[usize; 11]) -> String {
    cells
        .iter()
        .zip(widths.iter())
        .map(|(cell, width)| format!("{cell:<width$}"))
        .collect::<Vec<_>>()
        .join("  ")
        .trim_end()
        .to_owned()
}

fn table_row(view: &ProfileView) -> [String; 11] {
    [
        view.id.clone(),
        view.vendor.clone(),
        view.model.clone(),
        calibration_marker(&view.source).to_owned(),
        (view.paper_width_mm.round() as u32).to_string(),
        (view.printable_width_mm.round() as u32).to_string(),
        view.printable_width_dots.to_string(),
        view.dpi_x.to_string(),
        cut_marker(&view.features).to_owned(),
        barcode_marker(&view.features.barcodes).to_owned(),
        check_marker(view.features.qr_code).to_owned(),
    ]
}

fn cut_marker(features: &FeaturesView) -> &'static str {
    check_marker(features.paper_full_cut || features.paper_part_cut)
}

fn check_marker(supported: bool) -> &'static str {
    if supported { "✓" } else { "–" }
}

fn barcode_marker(barcodes: &BarcodesView) -> &'static str {
    match (
        !barcodes.function_a.is_empty(),
        !barcodes.function_b.is_empty(),
    ) {
        (true, true) => "A·B",
        (true, false) => "A",
        (false, true) => "B",
        (false, false) => "–",
    }
}

/// Renders the detailed `profiles show <id>` block: every field, one per
/// line, grouped by identity/provenance/geometry/fonts/code pages/features.
pub fn render_detail(view: &ProfileView) -> String {
    let marker = calibration_marker(&view.source);
    let lines = [
        format!("{} — {} {}", view.id, view.vendor, view.model),
        format!("source:           {} {marker}", view.source),
        format!("sha256:           {}", view.canonical_profile_sha256),
        String::new(),
        format!("paper width:      {:.1} mm", view.paper_width_mm),
        format!(
            "printable width:  {:.1} mm ({} dots)",
            view.printable_width_mm, view.printable_width_dots
        ),
        format!("resolution:       {} x {} dpi", view.dpi_x, view.dpi_y),
        String::new(),
        format!(
            "font a:           {}x{} dots, baseline {}",
            view.fonts.a.cell_width_dots, view.fonts.a.cell_height_dots, view.fonts.a.baseline_dots
        ),
        format!(
            "font b:           {}x{} dots, baseline {}",
            view.fonts.b.cell_width_dots, view.fonts.b.cell_height_dots, view.fonts.b.baseline_dots
        ),
        String::new(),
        format!("code pages:       {}", view.code_page_count),
        String::new(),
        format!("graphics:         {}", yes_no(view.features.graphics)),
        format!(
            "cut:              full {} · partial {}",
            yes_no(view.features.paper_full_cut),
            yes_no(view.features.paper_part_cut)
        ),
        format!("qr code:          {}", yes_no(view.features.qr_code)),
        format!("drawer pulse:     {}", yes_no(view.features.pulse_standard)),
        format!(
            "barcodes (a):     {}",
            barcode_list(&view.features.barcodes.function_a)
        ),
        format!(
            "barcodes (b):     {}",
            barcode_list(&view.features.barcodes.function_b)
        ),
    ];
    lines.join("\n")
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn barcode_list(systems: &[String]) -> String {
    if systems.is_empty() {
        "none".to_owned()
    } else {
        systems.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use escpost_profiles::resolver;

    #[test]
    fn source_label_maps_every_variant() {
        assert_eq!(
            source_label(&ProfileSource::Upstream {
                profile_sha256: String::new()
            }),
            "calibrated"
        );
        assert_eq!(
            source_label(&ProfileSource::UpstreamDefault {
                profile_sha256: String::new()
            }),
            "synthesized"
        );
        assert_eq!(source_label(&ProfileSource::Reference), "virtual");
    }

    #[test]
    fn table_includes_the_calibration_marker_and_legend() {
        let profile = resolver::resolve("TM-T88III").unwrap();
        let view = ProfileView::from_profile(profile);

        let table = render_table(std::slice::from_ref(&view));

        assert!(table.contains("TM-T88III"));
        assert!(table.contains('~'), "synthesized profiles show ~: {table}");
        assert!(table.contains("CAL: "));
    }

    #[test]
    fn detail_shows_decimal_millimeters_and_barcode_lists() {
        let profile = resolver::resolve("TM-T88III").unwrap();
        let view = ProfileView::from_profile(profile);

        let detail = render_detail(&view);

        assert!(detail.contains("80.0 mm"));
        assert!(detail.contains("72.2 mm"));
        assert!(detail.contains("code_128") || detail.contains("upc_a"));
    }
}
