//! Printer profile import, enrichment, and validation.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

const ENRICHMENT_SCHEMA_VERSION: u32 = 1;
const CANONICAL_PROFILE_SCHEMA_VERSION: u32 = 1;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrinterProfile {
    pub schema_version: u32,
    pub id: String,
    pub source: ProfileSource,
    pub geometry: Geometry,
    /// Absent on printers that use a manual tear bar.
    pub cutter: Option<CutterGeometry>,
    pub motion: MotionUnits,
    pub column_bit_image: ColumnBitImage,
    pub commands: CommandBehavior,
    pub defaults: PrinterDefaults,
    pub fonts: Fonts,
    pub features: Features,
    /// Maps each profile-specific `ESC t n` slot to a named encoding.
    pub code_pages: BTreeMap<u8, String>,
    pub approximations: Vec<Approximation>,
    pub canonical_profile_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProfileSource {
    Upstream { profile_sha256: String },
    Reference,
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
pub struct CutterGeometry {
    /// Paper-feed distance from the print head to the cutter blade.
    pub print_head_to_cutter_dots: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionUnits {
    pub horizontal_units_per_inch: u32,
    pub vertical_units_per_inch: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnBitImage {
    /// Physical row distance between adjacent source bits in ESC * modes 0/1.
    pub eight_dot_vertical_pitch_dots: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandBehavior {
    pub esc_backslash_negative: PositioningBehavior,
    pub esc_dollar_after_printable_data: PositioningBehavior,
    pub esc_j: FeedBehavior,
    pub gs_v_0_following_lf: FeedBehavior,
    pub gs_v_function_b_full: FeedBehavior,
    pub gs_v_function_b_partial: FeedBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositioningBehavior {
    Apply,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedBehavior {
    Feed,
    Ignored,
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
    pub paper_full_cut: bool,
    pub paper_part_cut: bool,
    pub pulse_standard: bool,
    pub qr_code: bool,
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

    #[error("a profile with a paper-cut capability must define cutter geometry")]
    MissingCutterGeometry,

    #[error("reference profile field {field} is required")]
    MissingReferenceField { field: &'static str },

    #[error("an upstream profile cannot replace imported code-page mappings")]
    UpstreamCodePagesOverride,

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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Enrichment {
    schema_version: u32,
    profile: String,
    source: ProfileSource,
    /// Authoring evidence is validated as strings but does not enter runtime profiles.
    #[serde(rename = "sources")]
    _sources: Vec<String>,
    geometry: Geometry,
    cutter: Option<CutterGeometry>,
    motion: MotionUnits,
    column_bit_image: ColumnBitImage,
    commands: CommandBehavior,
    defaults: PrinterDefaults,
    fonts: Fonts,
    #[serde(default)]
    features: FeatureOverrides,
    code_pages: Option<BTreeMap<u8, String>>,
    approximations: Vec<Approximation>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FeatureOverrides {
    barcodes: Option<BarcodeCapabilities>,
    bit_image_column: Option<bool>,
    bit_image_raster: Option<bool>,
    graphics: Option<bool>,
    paper_full_cut: Option<bool>,
    paper_part_cut: Option<bool>,
    pulse_standard: Option<bool>,
    qr_code: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpstreamCapabilities {
    profiles: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct ImportedProfile {
    features: ImportedFeatures,
    #[serde(rename = "codePages")]
    code_pages: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportedFeatures {
    barcode_a: bool,
    barcode_b: bool,
    bit_image_column: bool,
    bit_image_raster: bool,
    graphics: bool,
    paper_full_cut: bool,
    paper_part_cut: bool,
    pulse_standard: bool,
    qr_code: bool,
}

#[derive(Serialize)]
struct CanonicalProfileContent<'a> {
    schema_version: u32,
    id: &'a str,
    source: &'a ProfileSource,
    geometry: &'a Geometry,
    cutter: &'a Option<CutterGeometry>,
    motion: &'a MotionUnits,
    column_bit_image: &'a ColumnBitImage,
    commands: &'a CommandBehavior,
    defaults: &'a PrinterDefaults,
    fonts: &'a Fonts,
    features: &'a Features,
    code_pages: &'a BTreeMap<u8, String>,
    approximations: &'a [Approximation],
}

pub fn compile_profile(
    capabilities_json: &[u8],
    enrichment_toml: &str,
) -> Result<PrinterProfile, CompileProfileError> {
    let enrichment: Enrichment =
        toml::from_str(enrichment_toml).map_err(CompileProfileError::InvalidEnrichment)?;

    validate_schema_version(enrichment.schema_version)?;
    validate_profile_values(&enrichment)?;

    let (source, code_pages, features) = compile_profile_source(capabilities_json, &enrichment)?;
    validate_default_code_page(enrichment.defaults.code_page, &code_pages)?;
    validate_cutter_capabilities(enrichment.cutter.as_ref(), &features)?;

    let mut profile = PrinterProfile {
        schema_version: CANONICAL_PROFILE_SCHEMA_VERSION,
        id: enrichment.profile,
        source,
        geometry: enrichment.geometry,
        cutter: enrichment.cutter,
        motion: enrichment.motion,
        column_bit_image: enrichment.column_bit_image,
        commands: enrichment.commands,
        defaults: enrichment.defaults,
        fonts: enrichment.fonts,
        features,
        code_pages,
        approximations: enrichment.approximations,
        canonical_profile_sha256: String::new(),
    };
    profile.canonical_profile_sha256 =
        hash_canonical_profile(&profile).map_err(CompileProfileError::NormalizeCanonicalProfile)?;

    Ok(profile)
}

fn compile_profile_source(
    capabilities_json: &[u8],
    enrichment: &Enrichment,
) -> Result<(ProfileSource, BTreeMap<u8, String>, Features), CompileProfileError> {
    match &enrichment.source {
        ProfileSource::Upstream {
            profile_sha256: expected_hash,
        } => {
            if enrichment.code_pages.is_some() {
                return Err(CompileProfileError::UpstreamCodePagesOverride);
            }
            let capabilities: UpstreamCapabilities = serde_json::from_slice(capabilities_json)
                .map_err(CompileProfileError::InvalidCapabilities)?;
            let upstream_profile =
                capabilities
                    .profiles
                    .get(&enrichment.profile)
                    .ok_or_else(|| CompileProfileError::UnknownUpstreamProfile {
                        profile: enrichment.profile.clone(),
                    })?;
            let actual_hash = hash_resolved_profile(upstream_profile)?;
            validate_upstream_hash(&enrichment.profile, expected_hash, &actual_hash)?;
            let imported = import_upstream_profile(upstream_profile, &enrichment.profile)?;
            let code_pages = import_code_pages(&imported)?;
            let features = apply_feature_overrides(imported.features, &enrichment.features);
            Ok((
                ProfileSource::Upstream {
                    profile_sha256: actual_hash,
                },
                code_pages,
                features,
            ))
        }
        ProfileSource::Reference => {
            let code_pages = enrichment.code_pages.clone().ok_or(
                CompileProfileError::MissingReferenceField {
                    field: "code_pages",
                },
            )?;
            let features = compile_reference_features(&enrichment.features)?;
            Ok((ProfileSource::Reference, code_pages, features))
        }
    }
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

fn apply_feature_overrides(imported: ImportedFeatures, overrides: &FeatureOverrides) -> Features {
    let mut features = imported.into_canonical();
    if let Some(barcodes) = &overrides.barcodes {
        features.barcodes.clone_from(barcodes);
    }
    apply_bool_override(&mut features.bit_image_column, overrides.bit_image_column);
    apply_bool_override(&mut features.bit_image_raster, overrides.bit_image_raster);
    apply_bool_override(&mut features.graphics, overrides.graphics);
    apply_bool_override(&mut features.paper_full_cut, overrides.paper_full_cut);
    apply_bool_override(&mut features.paper_part_cut, overrides.paper_part_cut);
    apply_bool_override(&mut features.pulse_standard, overrides.pulse_standard);
    apply_bool_override(&mut features.qr_code, overrides.qr_code);
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
            paper_full_cut: self.paper_full_cut,
            paper_part_cut: self.paper_part_cut,
            pulse_standard: self.pulse_standard,
            qr_code: self.qr_code,
        }
    }
}

fn apply_bool_override(target: &mut bool, replacement: Option<bool>) {
    if let Some(replacement) = replacement {
        *target = replacement;
    }
}

fn compile_reference_features(
    features: &FeatureOverrides,
) -> Result<Features, CompileProfileError> {
    Ok(Features {
        barcodes: required_reference_field(&features.barcodes, "features.barcodes")?,
        bit_image_column: required_reference_field(
            &features.bit_image_column,
            "features.bit_image_column",
        )?,
        bit_image_raster: required_reference_field(
            &features.bit_image_raster,
            "features.bit_image_raster",
        )?,
        graphics: required_reference_field(&features.graphics, "features.graphics")?,
        paper_full_cut: required_reference_field(
            &features.paper_full_cut,
            "features.paper_full_cut",
        )?,
        paper_part_cut: required_reference_field(
            &features.paper_part_cut,
            "features.paper_part_cut",
        )?,
        pulse_standard: required_reference_field(
            &features.pulse_standard,
            "features.pulse_standard",
        )?,
        qr_code: required_reference_field(&features.qr_code, "features.qr_code")?,
    })
}

fn required_reference_field<T: Clone>(
    value: &Option<T>,
    field: &'static str,
) -> Result<T, CompileProfileError> {
    value
        .clone()
        .ok_or(CompileProfileError::MissingReferenceField { field })
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

fn validate_schema_version(schema_version: u32) -> Result<(), CompileProfileError> {
    if schema_version == ENRICHMENT_SCHEMA_VERSION {
        return Ok(());
    }

    Err(CompileProfileError::UnsupportedSchemaVersion {
        found: schema_version,
    })
}

fn validate_profile_values(enrichment: &Enrichment) -> Result<(), CompileProfileError> {
    validate_runtime_values(
        &enrichment.geometry,
        &enrichment.motion,
        &enrichment.column_bit_image,
        &enrichment.defaults,
        &enrichment.fonts,
        None,
    )?;
    if let Some(barcodes) = &enrichment.features.barcodes {
        validate_barcode_capabilities(barcodes)?;
    }
    if let Some(cutter) = &enrichment.cutter {
        validate_positive(
            "cutter.print_head_to_cutter_dots",
            cutter.print_head_to_cutter_dots,
        )?;
    }
    Ok(())
}

fn validate_runtime_profile(profile: &PrinterProfile) -> Result<(), CompileProfileError> {
    validate_runtime_values(
        &profile.geometry,
        &profile.motion,
        &profile.column_bit_image,
        &profile.defaults,
        &profile.fonts,
        Some(&profile.code_pages),
    )?;
    validate_barcode_capabilities(&profile.features.barcodes)?;
    if let Some(cutter) = &profile.cutter {
        validate_positive(
            "cutter.print_head_to_cutter_dots",
            cutter.print_head_to_cutter_dots,
        )?;
    }
    validate_cutter_capabilities(profile.cutter.as_ref(), &profile.features)
}

fn validate_cutter_capabilities(
    cutter: Option<&CutterGeometry>,
    features: &Features,
) -> Result<(), CompileProfileError> {
    if (features.paper_full_cut || features.paper_part_cut) && cutter.is_none() {
        return Err(CompileProfileError::MissingCutterGeometry);
    }
    Ok(())
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
    column_bit_image: &ColumnBitImage,
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
    validate_positive(
        "column_bit_image.eight_dot_vertical_pitch_dots",
        column_bit_image.eight_dot_vertical_pitch_dots,
    )?;
    validate_positive("fonts.a.cell_width_dots", fonts.a.cell_width_dots)?;
    validate_positive("fonts.a.cell_height_dots", fonts.a.cell_height_dots)?;
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

fn hash_canonical_profile(profile: &PrinterProfile) -> Result<String, serde_json::Error> {
    // The stored hash cannot cover itself. Every other canonical field affects
    // rendering behavior or the fidelity disclosure returned with a render.
    let content = CanonicalProfileContent {
        schema_version: profile.schema_version,
        id: &profile.id,
        source: &profile.source,
        geometry: &profile.geometry,
        cutter: &profile.cutter,
        motion: &profile.motion,
        column_bit_image: &profile.column_bit_image,
        commands: &profile.commands,
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
    if actual == profile.canonical_profile_sha256 {
        return Ok(());
    }

    Err(CanonicalProfileError::CanonicalHashMismatch {
        expected: profile.canonical_profile_sha256.clone(),
        actual,
    })
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_upstream_hash(
    profile: &str,
    expected_hash: &str,
    actual_hash: &str,
) -> Result<(), CompileProfileError> {
    if expected_hash == actual_hash {
        return Ok(());
    }

    Err(CompileProfileError::UpstreamProfileHashMismatch {
        profile: profile.to_owned(),
        expected: expected_hash.to_owned(),
        actual: actual_hash.to_owned(),
    })
}
