//! Printer profile import, enrichment, and validation.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

const ENRICHMENT_SCHEMA_VERSION: u32 = 2;
const CANONICAL_PROFILE_SCHEMA_VERSION: u32 = 2;
const UPSTREAM_LOCK_SCHEMA_VERSION: u32 = 1;
const BUNDLED_UPSTREAM_LOCK_TOML: &str = include_str!("../../../profiles/upstream.lock.toml");
const LEGACY_FUNCTION_A_SYSTEMS: [BarcodeSystem; 7] = [
    BarcodeSystem::UpcA,
    BarcodeSystem::UpcE,
    BarcodeSystem::Ean13,
    BarcodeSystem::Ean8,
    BarcodeSystem::Code39,
    BarcodeSystem::Itf,
    BarcodeSystem::Codabar,
];
const LEGACY_FUNCTION_B_SYSTEMS: [BarcodeSystem; 9] = [
    BarcodeSystem::UpcA,
    BarcodeSystem::UpcE,
    BarcodeSystem::Ean13,
    BarcodeSystem::Ean8,
    BarcodeSystem::Code39,
    BarcodeSystem::Itf,
    BarcodeSystem::Codabar,
    BarcodeSystem::Code93,
    BarcodeSystem::Code128,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompiledProfile {
    pub profile: PrinterProfile,
    pub source: ProfileSource,
    pub changes: Vec<ProfileChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrinterProfile {
    pub schema_version: u32,
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
    pub source: ProfileSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePack {
    schema_version: u32,
    profiles: BTreeMap<String, PrinterProfile>,
}

impl ProfilePack {
    pub fn get(&self, profile_id: &str) -> Option<&PrinterProfile> {
        self.profiles.get(profile_id)
    }

    pub fn profiles(&self) -> impl Iterator<Item = &PrinterProfile> {
        self.profiles.values()
    }
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
    /// International character set active at power-on and after `ESC @`.
    pub international_character_set: u8,
    /// Whether CR is ignored or prints and feeds like LF.
    pub carriage_return: CarriageReturnMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarriageReturnMode {
    Ignored,
    LineFeed,
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
    pub barcodes: BarcodeCapabilities,
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
pub struct BarcodeCapabilities {
    pub function_a: BTreeSet<BarcodeSystem>,
    pub function_b: BTreeSet<BarcodeSystem>,
}

/// A logical symbol system supported by one of the two `GS k` wire formats.
///
/// The enum is ordered in ESC/POS command-number order. A `BTreeSet` therefore
/// produces deterministic canonical JSON without a separate sorting step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BarcodeSystem {
    UpcA,
    UpcE,
    #[serde(rename = "ean_13")]
    Ean13,
    #[serde(rename = "ean_8")]
    Ean8,
    #[serde(rename = "code_39")]
    Code39,
    Itf,
    Codabar,
    #[serde(rename = "code_93")]
    Code93,
    #[serde(rename = "code_128")]
    Code128,
    Gs1_128,
    #[serde(rename = "gs1_databar_omnidirectional")]
    Gs1DataBarOmnidirectional,
    #[serde(rename = "gs1_databar_truncated")]
    Gs1DataBarTruncated,
    #[serde(rename = "gs1_databar_limited")]
    Gs1DataBarLimited,
    #[serde(rename = "gs1_databar_expanded")]
    Gs1DataBarExpanded,
    #[serde(rename = "code_128_auto")]
    Code128Auto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approximation {
    pub field: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileSource {
    pub upstream_repository: String,
    pub upstream_commit: String,
    pub upstream_license: String,
    pub upstream_profile_sha256: String,
    pub enrichment_sha256: String,
    pub canonical_profile_sha256: String,
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

    #[error("invalid upstream lock TOML")]
    InvalidUpstreamLock(#[source] toml::de::Error),

    #[error("unsupported profile enrichment schema version {found}")]
    UnsupportedSchemaVersion { found: u32 },

    #[error("unsupported upstream lock schema version {found}")]
    UnsupportedUpstreamLockSchemaVersion { found: u32 },

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

    #[error("unsupported default international character set {character_set}")]
    UnsupportedDefaultInternationalCharacterSet { character_set: u8 },

    #[error("barcode system {system:?} has no GS k Function {function} command number")]
    InvalidBarcodeSystemForFunction {
        function: &'static str,
        system: BarcodeSystem,
    },

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

    #[error("could not normalize profile enrichment")]
    NormalizeEnrichment(#[source] serde_json::Error),

    #[error("could not normalize canonical profile content")]
    NormalizeCanonicalProfile(#[source] serde_json::Error),

    #[error("could not import resolved upstream profile {profile:?}")]
    ImportUpstreamProfile {
        profile: String,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Error)]
pub enum CanonicalProfileError {
    #[error("invalid canonical profile JSON")]
    InvalidJson(#[source] serde_json::Error),

    #[error("unsupported canonical profile schema version {found}")]
    UnsupportedSchemaVersion { found: u32 },

    #[error("canonical profile is invalid: {message}")]
    InvalidProfile { message: String },

    #[error("could not normalize canonical profile content")]
    Normalize(#[source] serde_json::Error),

    #[error("could not serialize canonical profile")]
    Serialize(#[source] serde_json::Error),

    #[error("canonical profile hash mismatch: expected {expected}, got {actual}")]
    CanonicalHashMismatch { expected: String, actual: String },

    #[error("profile pack contains key {key:?} for profile {profile_id:?}")]
    ProfileIdMismatch { key: String, profile_id: String },

    #[error("profile pack contains duplicate profile id {profile_id:?}")]
    DuplicateProfile { profile_id: String },
}

#[derive(Debug, Serialize, Deserialize)]
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
    #[serde(default)]
    features: FeatureOverrides,
    approximations: Vec<Approximation>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FeatureOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    barcodes: Option<BarcodeCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bit_image_column: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bit_image_raster: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    graphics: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    high_density: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    paper_full_cut: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    paper_part_cut: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pdf417_code: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pulse_bel: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pulse_standard: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    qr_code: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    star_commands: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamLock {
    schema_version: u32,
    repository: String,
    commit: String,
    license: String,
}

#[derive(Debug, Deserialize)]
struct UpstreamCapabilities {
    profiles: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct ImportedProfile {
    media: ImportedMedia,
    fonts: BTreeMap<String, ImportedFont>,
    features: ImportedFeatures,
    #[serde(rename = "codePages")]
    code_pages: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportedFeatures {
    barcode_a: bool,
    barcode_b: bool,
    bit_image_column: bool,
    bit_image_raster: bool,
    graphics: bool,
    high_density: bool,
    paper_full_cut: bool,
    paper_part_cut: bool,
    pdf417_code: bool,
    pulse_bel: bool,
    pulse_standard: bool,
    qr_code: bool,
    star_commands: bool,
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

#[derive(Serialize)]
struct CanonicalProfileContent<'a> {
    schema_version: u32,
    id: &'a str,
    revision: u32,
    geometry: &'a Geometry,
    motion: &'a MotionUnits,
    defaults: &'a PrinterDefaults,
    fonts: &'a Fonts,
    features: &'a Features,
    code_pages: &'a BTreeMap<u8, String>,
    approximations: &'a [Approximation],
}

pub fn compile_profile(
    capabilities_json: &[u8],
    enrichment_toml: &str,
) -> Result<CompiledProfile, CompileProfileError> {
    compile_profile_with_lock(
        capabilities_json,
        enrichment_toml,
        BUNDLED_UPSTREAM_LOCK_TOML,
    )
}

pub fn compile_profile_with_lock(
    capabilities_json: &[u8],
    enrichment_toml: &str,
    upstream_lock_toml: &str,
) -> Result<CompiledProfile, CompileProfileError> {
    let capabilities: UpstreamCapabilities = serde_json::from_slice(capabilities_json)
        .map_err(CompileProfileError::InvalidCapabilities)?;
    let enrichment: Enrichment =
        toml::from_str(enrichment_toml).map_err(CompileProfileError::InvalidEnrichment)?;
    let upstream_lock: UpstreamLock =
        toml::from_str(upstream_lock_toml).map_err(CompileProfileError::InvalidUpstreamLock)?;

    validate_schema_version(enrichment.schema_version)?;
    validate_upstream_lock_schema_version(upstream_lock.schema_version)?;
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
    let features = apply_feature_overrides(imported.features, &enrichment.features);

    let enrichment_sha256 = hash_enrichment(&enrichment)?;
    let mut profile = PrinterProfile {
        schema_version: CANONICAL_PROFILE_SCHEMA_VERSION,
        id: enrichment.profile,
        revision: enrichment.revision,
        geometry: enrichment.geometry,
        motion: enrichment.motion,
        defaults: enrichment.defaults,
        fonts: enrichment.fonts,
        features,
        code_pages,
        sources: enrichment.sources,
        approximations: enrichment.approximations,
        source: ProfileSource {
            upstream_repository: upstream_lock.repository,
            upstream_commit: upstream_lock.commit,
            upstream_license: upstream_lock.license,
            upstream_profile_sha256: actual_hash,
            enrichment_sha256,
            canonical_profile_sha256: String::new(),
        },
    };
    profile.source.canonical_profile_sha256 =
        hash_canonical_profile(&profile).map_err(CompileProfileError::NormalizeCanonicalProfile)?;

    Ok(CompiledProfile {
        changes,
        source: profile.source.clone(),
        profile,
    })
}

pub fn to_canonical_json(profile: &PrinterProfile) -> Result<Vec<u8>, CanonicalProfileError> {
    verify_canonical_profile(profile)?;
    serde_json::to_vec(profile).map_err(CanonicalProfileError::Serialize)
}

pub fn from_canonical_json(json: &[u8]) -> Result<PrinterProfile, CanonicalProfileError> {
    let profile: PrinterProfile =
        serde_json::from_slice(json).map_err(CanonicalProfileError::InvalidJson)?;
    verify_canonical_profile(&profile)?;
    Ok(profile)
}

pub fn to_canonical_profile_pack_json(
    profiles: impl IntoIterator<Item = PrinterProfile>,
) -> Result<Vec<u8>, CanonicalProfileError> {
    let mut profiles_by_id = BTreeMap::new();
    for profile in profiles {
        verify_canonical_profile(&profile)?;
        let profile_id = profile.id.clone();
        if profiles_by_id.insert(profile_id.clone(), profile).is_some() {
            return Err(CanonicalProfileError::DuplicateProfile { profile_id });
        }
    }
    let pack = ProfilePack {
        schema_version: CANONICAL_PROFILE_SCHEMA_VERSION,
        profiles: profiles_by_id,
    };
    serde_json::to_vec_pretty(&pack).map_err(CanonicalProfileError::Serialize)
}

pub fn from_canonical_profile_pack_json(json: &[u8]) -> Result<ProfilePack, CanonicalProfileError> {
    let pack: ProfilePack =
        serde_json::from_slice(json).map_err(CanonicalProfileError::InvalidJson)?;
    if pack.schema_version != CANONICAL_PROFILE_SCHEMA_VERSION {
        return Err(CanonicalProfileError::UnsupportedSchemaVersion {
            found: pack.schema_version,
        });
    }
    for (key, profile) in &pack.profiles {
        if key != &profile.id {
            return Err(CanonicalProfileError::ProfileIdMismatch {
                key: key.clone(),
                profile_id: profile.id.clone(),
            });
        }
        verify_canonical_profile(profile)?;
    }
    Ok(pack)
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

    let mut changes = vec![
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
            "defaults.international_character_set",
            None,
            u32::from(enrichment.defaults.international_character_set),
        ),
        ProfileChange {
            field: "defaults.carriage_return".to_owned(),
            kind: ProfileChangeKind::Added,
        },
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
    ];
    classify_feature_changes(&mut changes, &imported.features, &enrichment.features);
    changes
}

fn apply_feature_overrides(imported: ImportedFeatures, overrides: &FeatureOverrides) -> Features {
    let mut features = imported.into_canonical();
    if let Some(barcodes) = &overrides.barcodes {
        features.barcodes.clone_from(barcodes);
    }
    apply_bool_override(&mut features.bit_image_column, overrides.bit_image_column);
    apply_bool_override(&mut features.bit_image_raster, overrides.bit_image_raster);
    apply_bool_override(&mut features.graphics, overrides.graphics);
    apply_bool_override(&mut features.high_density, overrides.high_density);
    apply_bool_override(&mut features.paper_full_cut, overrides.paper_full_cut);
    apply_bool_override(&mut features.paper_part_cut, overrides.paper_part_cut);
    apply_bool_override(&mut features.pdf417_code, overrides.pdf417_code);
    apply_bool_override(&mut features.pulse_bel, overrides.pulse_bel);
    apply_bool_override(&mut features.pulse_standard, overrides.pulse_standard);
    apply_bool_override(&mut features.qr_code, overrides.qr_code);
    apply_bool_override(&mut features.star_commands, overrides.star_commands);
    features
}

impl ImportedFeatures {
    fn into_canonical(self) -> Features {
        let function_a = if self.barcode_a {
            BTreeSet::from(LEGACY_FUNCTION_A_SYSTEMS)
        } else {
            BTreeSet::new()
        };
        let function_b = if self.barcode_b {
            BTreeSet::from(LEGACY_FUNCTION_B_SYSTEMS)
        } else {
            BTreeSet::new()
        };

        Features {
            barcodes: BarcodeCapabilities {
                function_a,
                function_b,
            },
            bit_image_column: self.bit_image_column,
            bit_image_raster: self.bit_image_raster,
            graphics: self.graphics,
            high_density: self.high_density,
            paper_full_cut: self.paper_full_cut,
            paper_part_cut: self.paper_part_cut,
            pdf417_code: self.pdf417_code,
            pulse_bel: self.pulse_bel,
            pulse_standard: self.pulse_standard,
            qr_code: self.qr_code,
            star_commands: self.star_commands,
        }
    }
}

fn apply_bool_override(target: &mut bool, replacement: Option<bool>) {
    if let Some(replacement) = replacement {
        *target = replacement;
    }
}

fn classify_feature_changes(
    changes: &mut Vec<ProfileChange>,
    imported: &ImportedFeatures,
    overrides: &FeatureOverrides,
) {
    if let Some(enriched) = &overrides.barcodes {
        let imported = imported.clone().into_canonical().barcodes;
        push_barcode_capability_change(
            changes,
            "features.barcodes.function_a",
            &imported.function_a,
            &enriched.function_a,
        );
        push_barcode_capability_change(
            changes,
            "features.barcodes.function_b",
            &imported.function_b,
            &enriched.function_b,
        );
    }
    push_feature_change(
        changes,
        "features.bit_image_column",
        imported.bit_image_column,
        overrides.bit_image_column,
    );
    push_feature_change(
        changes,
        "features.bit_image_raster",
        imported.bit_image_raster,
        overrides.bit_image_raster,
    );
    push_feature_change(
        changes,
        "features.graphics",
        imported.graphics,
        overrides.graphics,
    );
    push_feature_change(
        changes,
        "features.high_density",
        imported.high_density,
        overrides.high_density,
    );
    push_feature_change(
        changes,
        "features.paper_full_cut",
        imported.paper_full_cut,
        overrides.paper_full_cut,
    );
    push_feature_change(
        changes,
        "features.paper_part_cut",
        imported.paper_part_cut,
        overrides.paper_part_cut,
    );
    push_feature_change(
        changes,
        "features.pdf417_code",
        imported.pdf417_code,
        overrides.pdf417_code,
    );
    push_feature_change(
        changes,
        "features.pulse_bel",
        imported.pulse_bel,
        overrides.pulse_bel,
    );
    push_feature_change(
        changes,
        "features.pulse_standard",
        imported.pulse_standard,
        overrides.pulse_standard,
    );
    push_feature_change(
        changes,
        "features.qr_code",
        imported.qr_code,
        overrides.qr_code,
    );
    push_feature_change(
        changes,
        "features.star_commands",
        imported.star_commands,
        overrides.star_commands,
    );
}

fn push_barcode_capability_change(
    changes: &mut Vec<ProfileChange>,
    field: &str,
    imported: &BTreeSet<BarcodeSystem>,
    enriched: &BTreeSet<BarcodeSystem>,
) {
    changes.push(ProfileChange {
        field: field.to_owned(),
        kind: if imported == enriched {
            ProfileChangeKind::Confirmed
        } else {
            ProfileChangeKind::Corrected
        },
    });
}

fn push_feature_change(
    changes: &mut Vec<ProfileChange>,
    field: &str,
    imported: bool,
    enriched: Option<bool>,
) {
    let Some(enriched) = enriched else {
        return;
    };
    changes.push(ProfileChange {
        field: field.to_owned(),
        kind: if imported == enriched {
            ProfileChangeKind::Confirmed
        } else {
            ProfileChangeKind::Corrected
        },
    });
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

fn validate_upstream_lock_schema_version(schema_version: u32) -> Result<(), CompileProfileError> {
    if schema_version == UPSTREAM_LOCK_SCHEMA_VERSION {
        return Ok(());
    }

    Err(CompileProfileError::UnsupportedUpstreamLockSchemaVersion {
        found: schema_version,
    })
}

fn validate_profile_values(enrichment: &Enrichment) -> Result<(), CompileProfileError> {
    validate_runtime_values(
        &enrichment.geometry,
        &enrichment.motion,
        &enrichment.defaults,
        &enrichment.fonts,
        None,
    )?;
    if let Some(barcodes) = &enrichment.features.barcodes {
        validate_barcode_capabilities(barcodes)?;
    }
    Ok(())
}

fn validate_runtime_profile(profile: &PrinterProfile) -> Result<(), CompileProfileError> {
    validate_runtime_values(
        &profile.geometry,
        &profile.motion,
        &profile.defaults,
        &profile.fonts,
        Some(&profile.code_pages),
    )?;
    validate_barcode_capabilities(&profile.features.barcodes)
}

fn validate_barcode_capabilities(
    barcodes: &BarcodeCapabilities,
) -> Result<(), CompileProfileError> {
    for system in &barcodes.function_a {
        if !LEGACY_FUNCTION_A_SYSTEMS.contains(system) {
            return Err(CompileProfileError::InvalidBarcodeSystemForFunction {
                function: "A",
                system: *system,
            });
        }
    }
    Ok(())
}

fn validate_runtime_values(
    geometry: &Geometry,
    motion: &MotionUnits,
    defaults: &PrinterDefaults,
    fonts: &Fonts,
    code_pages: Option<&BTreeMap<u8, String>>,
) -> Result<(), CompileProfileError> {
    validate_positive(
        "geometry.printable_width_dots",
        geometry.printable_width_dots,
    )?;
    validate_positive("geometry.dpi_x", geometry.dpi_x)?;
    validate_positive("geometry.dpi_y", geometry.dpi_y)?;
    validate_positive(
        "motion.horizontal_units_per_inch",
        motion.horizontal_units_per_inch,
    )?;
    validate_positive(
        "motion.vertical_units_per_inch",
        motion.vertical_units_per_inch,
    )?;
    validate_positive("fonts.a.columns", fonts.a.columns)?;
    validate_positive("fonts.a.cell_width_dots", fonts.a.cell_width_dots)?;
    validate_positive("fonts.a.cell_height_dots", fonts.a.cell_height_dots)?;
    validate_positive("fonts.b.columns", fonts.b.columns)?;
    validate_positive("fonts.b.cell_width_dots", fonts.b.cell_width_dots)?;
    validate_positive("fonts.b.cell_height_dots", fonts.b.cell_height_dots)?;
    validate_font("fonts.a", &fonts.a)?;
    validate_font("fonts.b", &fonts.b)?;
    validate_default_international_character_set(defaults.international_character_set)?;
    if let Some(code_pages) = code_pages {
        validate_default_code_page(defaults.code_page, code_pages)?;
    }
    Ok(())
}

fn validate_default_international_character_set(
    character_set: u8,
) -> Result<(), CompileProfileError> {
    // Version 1 renders Epson's common 0–17 sets. The additional Indic sets
    // need matching glyph coverage before profiles may select them by default.
    if character_set <= 17 {
        return Ok(());
    }

    Err(CompileProfileError::UnsupportedDefaultInternationalCharacterSet { character_set })
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
    Ok(hash_bytes(&normalized))
}

fn hash_enrichment(enrichment: &Enrichment) -> Result<String, CompileProfileError> {
    // Hash parsed values rather than TOML bytes so comments and whitespace do
    // not create a new provenance identity for the same enrichment.
    let normalized =
        serde_json::to_vec(enrichment).map_err(CompileProfileError::NormalizeEnrichment)?;
    Ok(hash_bytes(&normalized))
}

fn hash_canonical_profile(profile: &PrinterProfile) -> Result<String, serde_json::Error> {
    // Evidence labels and repository provenance do not affect rendering.
    // Excluding them keeps the hash stable when source notes are clarified,
    // while every runtime value and diagnostic approximation remains covered.
    let content = CanonicalProfileContent {
        schema_version: profile.schema_version,
        id: &profile.id,
        revision: profile.revision,
        geometry: &profile.geometry,
        motion: &profile.motion,
        defaults: &profile.defaults,
        fonts: &profile.fonts,
        features: &profile.features,
        code_pages: &profile.code_pages,
        approximations: &profile.approximations,
    };
    let normalized = serde_json::to_vec(&content)?;
    Ok(hash_bytes(&normalized))
}

fn verify_canonical_profile(profile: &PrinterProfile) -> Result<(), CanonicalProfileError> {
    if profile.schema_version != CANONICAL_PROFILE_SCHEMA_VERSION {
        return Err(CanonicalProfileError::UnsupportedSchemaVersion {
            found: profile.schema_version,
        });
    }
    validate_runtime_profile(profile).map_err(|error| CanonicalProfileError::InvalidProfile {
        message: error.to_string(),
    })?;

    let actual = hash_canonical_profile(profile).map_err(CanonicalProfileError::Normalize)?;
    if actual == profile.source.canonical_profile_sha256 {
        return Ok(());
    }

    Err(CanonicalProfileError::CanonicalHashMismatch {
        expected: profile.source.canonical_profile_sha256.clone(),
        actual,
    })
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
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
