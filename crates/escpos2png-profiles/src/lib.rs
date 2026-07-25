//! Printer profile import, enrichment, and validation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

const ENRICHMENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompiledProfile {
    pub profile: PrinterProfile,
    pub source: ProfileSource,
    pub changes: Vec<ProfileChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrinterProfile {
    pub id: String,
    pub revision: u32,
    pub geometry: Geometry,
    pub motion: MotionUnits,
    pub defaults: PrinterDefaults,
    pub fonts: Fonts,
    pub features: Features,
    /// Maps each printer-specific `ESC t n` slot to a named encoding.
    pub code_pages: BTreeMap<u8, String>,
    pub sources: Vec<String>,
    pub approximations: Vec<Approximation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub printable_width_dots: u32,
    pub dpi_x: u32,
    pub dpi_y: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionUnits {
    pub horizontal_units_per_inch: u32,
    pub vertical_units_per_inch: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrinterDefaults {
    pub line_spacing_dots: u32,
    /// Character-table slot active at power-on and after `ESC @`.
    pub code_page: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fonts {
    pub a: Font,
    pub b: Font,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Font {
    pub columns: u32,
    /// Cursor advance for one unscaled character cell.
    pub cell_width_dots: u32,
    /// Maximum dot height of one unscaled character cell.
    pub cell_height_dots: u32,
    /// Baseline measured down from the top of the unscaled cell.
    pub baseline_dots: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Features {
    pub barcode_a: bool,
    pub barcode_b: bool,
    pub bit_image_column: bool,
    pub bit_image_raster: bool,
    pub graphics: bool,
    pub high_density: bool,
    pub paper_full_cut: bool,
    pub paper_part_cut: bool,
    pub pdf417_code: bool,
    pub pulse_bel: bool,
    pub pulse_standard: bool,
    pub qr_code: bool,
    pub star_commands: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approximation {
    pub field: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileSource {
    pub upstream_profile_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileChange {
    pub field: String,
    pub kind: ProfileChangeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileChangeKind {
    Added,
    Confirmed,
    Corrected,
}

#[derive(Debug, Error)]
pub enum CompileProfileError {
    #[error("invalid upstream capabilities JSON")]
    InvalidCapabilities(#[source] serde_json::Error),

    #[error("invalid profile enrichment TOML")]
    InvalidEnrichment(#[source] toml::de::Error),

    #[error("unsupported profile enrichment schema version {found}")]
    UnsupportedSchemaVersion { found: u32 },

    #[error("profile field {field} must be greater than zero")]
    NonPositiveValue { field: &'static str },

    #[error("{font} baseline {baseline_dots} must be inside its {cell_height_dots}-dot-high cell")]
    InvalidFontBaseline {
        font: &'static str,
        baseline_dots: u32,
        cell_height_dots: u32,
    },

    #[error("upstream profile {profile:?} does not exist")]
    UnknownUpstreamProfile { profile: String },

    #[error("upstream code-page identifier {identifier:?} is not an unsigned byte")]
    InvalidCodePageIdentifier { identifier: String },

    #[error("default code page {code_page} does not exist in the imported printer profile")]
    UnknownDefaultCodePage { code_page: u8 },

    #[error(
        "resolved upstream profile hash changed for {profile:?}: \
         expected {expected}, got {actual}"
    )]
    UpstreamProfileHashMismatch {
        profile: String,
        expected: String,
        actual: String,
    },

    #[error("could not normalize resolved upstream profile")]
    NormalizeUpstreamProfile(#[source] serde_json::Error),

    #[error("could not import resolved upstream profile {profile:?}")]
    ImportUpstreamProfile {
        profile: String,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Enrichment {
    schema_version: u32,
    profile: String,
    revision: u32,
    upstream_profile_sha256: String,
    sources: Vec<String>,
    geometry: Geometry,
    motion: MotionUnits,
    defaults: PrinterDefaults,
    fonts: Fonts,
    approximations: Vec<Approximation>,
}

#[derive(Debug, Deserialize)]
struct UpstreamCapabilities {
    profiles: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct ImportedProfile {
    media: ImportedMedia,
    fonts: BTreeMap<String, ImportedFont>,
    features: Features,
    #[serde(rename = "codePages")]
    code_pages: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ImportedMedia {
    dpi: Option<u32>,
    width: ImportedWidth,
}

#[derive(Debug, Deserialize)]
struct ImportedWidth {
    pixels: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ImportedFont {
    columns: Option<u32>,
}

pub fn compile_profile(
    capabilities_json: &[u8],
    enrichment_toml: &str,
) -> Result<CompiledProfile, CompileProfileError> {
    let capabilities: UpstreamCapabilities = serde_json::from_slice(capabilities_json)
        .map_err(CompileProfileError::InvalidCapabilities)?;
    let enrichment: Enrichment =
        toml::from_str(enrichment_toml).map_err(CompileProfileError::InvalidEnrichment)?;

    validate_schema_version(enrichment.schema_version)?;
    validate_profile_values(&enrichment)?;

    let upstream_profile = capabilities
        .profiles
        .get(&enrichment.profile)
        .ok_or_else(|| CompileProfileError::UnknownUpstreamProfile {
            profile: enrichment.profile.clone(),
        })?;
    let actual_hash = hash_resolved_profile(upstream_profile)?;

    validate_upstream_hash(&enrichment, &actual_hash)?;
    let imported = import_upstream_profile(upstream_profile, &enrichment.profile)?;
    let code_pages = import_code_pages(&imported)?;
    validate_default_code_page(enrichment.defaults.code_page, &code_pages)?;
    let changes = classify_changes(&imported, &enrichment);

    Ok(CompiledProfile {
        changes,
        source: ProfileSource {
            upstream_profile_sha256: actual_hash,
        },
        profile: PrinterProfile {
            id: enrichment.profile,
            revision: enrichment.revision,
            geometry: enrichment.geometry,
            motion: enrichment.motion,
            defaults: enrichment.defaults,
            fonts: enrichment.fonts,
            features: imported.features,
            code_pages,
            sources: enrichment.sources,
            approximations: enrichment.approximations,
        },
    })
}

fn import_upstream_profile(
    upstream_profile: &Value,
    profile_id: &str,
) -> Result<ImportedProfile, CompileProfileError> {
    serde_json::from_value(upstream_profile.clone()).map_err(|source| {
        CompileProfileError::ImportUpstreamProfile {
            profile: profile_id.to_owned(),
            source,
        }
    })
}

fn classify_changes(imported: &ImportedProfile, enrichment: &Enrichment) -> Vec<ProfileChange> {
    let font_a_columns = imported.fonts.get("0").and_then(|font| font.columns);
    let font_b_columns = imported.fonts.get("1").and_then(|font| font.columns);

    vec![
        classify_change(
            "geometry.printable_width_dots",
            imported.media.width.pixels,
            enrichment.geometry.printable_width_dots,
        ),
        classify_change(
            "geometry.dpi_x",
            imported.media.dpi,
            enrichment.geometry.dpi_x,
        ),
        classify_change(
            "geometry.dpi_y",
            imported.media.dpi,
            enrichment.geometry.dpi_y,
        ),
        classify_change(
            "motion.horizontal_units_per_inch",
            None,
            enrichment.motion.horizontal_units_per_inch,
        ),
        classify_change(
            "motion.vertical_units_per_inch",
            None,
            enrichment.motion.vertical_units_per_inch,
        ),
        classify_change(
            "defaults.line_spacing_dots",
            None,
            enrichment.defaults.line_spacing_dots,
        ),
        classify_change(
            "defaults.code_page",
            None,
            u32::from(enrichment.defaults.code_page),
        ),
        classify_change(
            "fonts.a.columns",
            font_a_columns,
            enrichment.fonts.a.columns,
        ),
        classify_change(
            "fonts.a.cell_width_dots",
            None,
            enrichment.fonts.a.cell_width_dots,
        ),
        classify_change(
            "fonts.a.cell_height_dots",
            None,
            enrichment.fonts.a.cell_height_dots,
        ),
        classify_change(
            "fonts.a.baseline_dots",
            None,
            enrichment.fonts.a.baseline_dots,
        ),
        classify_change(
            "fonts.b.columns",
            font_b_columns,
            enrichment.fonts.b.columns,
        ),
        classify_change(
            "fonts.b.cell_width_dots",
            None,
            enrichment.fonts.b.cell_width_dots,
        ),
        classify_change(
            "fonts.b.cell_height_dots",
            None,
            enrichment.fonts.b.cell_height_dots,
        ),
        classify_change(
            "fonts.b.baseline_dots",
            None,
            enrichment.fonts.b.baseline_dots,
        ),
    ]
}

fn import_code_pages(
    imported: &ImportedProfile,
) -> Result<BTreeMap<u8, String>, CompileProfileError> {
    imported
        .code_pages
        .iter()
        .map(|(identifier, encoding)| {
            identifier
                .parse::<u8>()
                .map(|identifier| (identifier, encoding.clone()))
                .map_err(|_| CompileProfileError::InvalidCodePageIdentifier {
                    identifier: identifier.clone(),
                })
        })
        .collect()
}

fn validate_default_code_page(
    default_code_page: u8,
    code_pages: &BTreeMap<u8, String>,
) -> Result<(), CompileProfileError> {
    // The renderer starts decoding text before it sees any ESC t command, so
    // every compiled profile must resolve its power-on code-page slot.
    if code_pages.contains_key(&default_code_page) {
        return Ok(());
    }

    Err(CompileProfileError::UnknownDefaultCodePage {
        code_page: default_code_page,
    })
}

fn classify_change(field: &str, imported: Option<u32>, enriched: u32) -> ProfileChange {
    let kind = match imported {
        None => ProfileChangeKind::Added,
        Some(value) if value == enriched => ProfileChangeKind::Confirmed,
        Some(_) => ProfileChangeKind::Corrected,
    };

    ProfileChange {
        field: field.to_owned(),
        kind,
    }
}

fn validate_schema_version(schema_version: u32) -> Result<(), CompileProfileError> {
    if schema_version == ENRICHMENT_SCHEMA_VERSION {
        return Ok(());
    }

    Err(CompileProfileError::UnsupportedSchemaVersion {
        found: schema_version,
    })
}

fn validate_profile_values(enrichment: &Enrichment) -> Result<(), CompileProfileError> {
    validate_positive(
        "motion.horizontal_units_per_inch",
        enrichment.motion.horizontal_units_per_inch,
    )?;
    validate_positive(
        "motion.vertical_units_per_inch",
        enrichment.motion.vertical_units_per_inch,
    )?;
    validate_positive("fonts.a.columns", enrichment.fonts.a.columns)?;
    validate_positive(
        "fonts.a.cell_width_dots",
        enrichment.fonts.a.cell_width_dots,
    )?;
    validate_positive(
        "fonts.a.cell_height_dots",
        enrichment.fonts.a.cell_height_dots,
    )?;
    validate_positive("fonts.b.columns", enrichment.fonts.b.columns)?;
    validate_positive(
        "fonts.b.cell_width_dots",
        enrichment.fonts.b.cell_width_dots,
    )?;
    validate_positive(
        "fonts.b.cell_height_dots",
        enrichment.fonts.b.cell_height_dots,
    )?;
    validate_font("fonts.a", &enrichment.fonts.a)?;
    validate_font("fonts.b", &enrichment.fonts.b)
}

fn validate_positive(field: &'static str, value: u32) -> Result<(), CompileProfileError> {
    if value > 0 {
        return Ok(());
    }

    Err(CompileProfileError::NonPositiveValue { field })
}

fn validate_font(name: &'static str, font: &Font) -> Result<(), CompileProfileError> {
    if font.baseline_dots < font.cell_height_dots {
        return Ok(());
    }

    Err(CompileProfileError::InvalidFontBaseline {
        font: name,
        baseline_dots: font.baseline_dots,
        cell_height_dots: font.cell_height_dots,
    })
}

fn hash_resolved_profile(profile: &Value) -> Result<String, CompileProfileError> {
    let normalized =
        serde_json::to_vec(profile).map_err(CompileProfileError::NormalizeUpstreamProfile)?;
    let digest = Sha256::digest(normalized);

    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn validate_upstream_hash(
    enrichment: &Enrichment,
    actual_hash: &str,
) -> Result<(), CompileProfileError> {
    if enrichment.upstream_profile_sha256 == actual_hash {
        return Ok(());
    }

    Err(CompileProfileError::UpstreamProfileHashMismatch {
        profile: enrichment.profile.clone(),
        expected: enrichment.upstream_profile_sha256.clone(),
        actual: actual_hash.to_owned(),
    })
}
