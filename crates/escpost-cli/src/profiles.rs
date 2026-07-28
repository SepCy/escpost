use std::sync::OnceLock;

use escpost_profiles::{PrinterProfile, ProfilePack, from_canonical_profile_pack_json};
use inquire::Select;

use crate::error::CliError;

const PROFILE_PACK_JSON: &[u8] = include_bytes!("../../../profiles/.generated/profiles.json");
static PROFILE_PACK: OnceLock<ProfilePack> = OnceLock::new();

pub(crate) fn load(profile_id: &str) -> Result<&'static PrinterProfile, CliError> {
    let profiles = profile_pack()?;
    profiles
        .get(profile_id)
        .ok_or_else(|| CliError::UnknownProfile(profile_id.to_owned()))
}

pub(crate) fn resolve(
    explicit: Option<String>,
    source_profile: Option<String>,
    can_prompt: bool,
) -> Result<String, CliError> {
    if let Some(profile) = explicit.or(source_profile) {
        return Ok(profile);
    }
    if !can_prompt {
        return Err(CliError::MissingProfile);
    }

    let choices = profile_pack()?
        .profiles()
        .map(|profile| profile.id.clone())
        .collect();
    Select::new("Printer profile", choices)
        .with_help_message("Use REFERENCE when no physical printer is known")
        .prompt()
        .map_err(|error| CliError::ProfilePrompt(error.to_string()))
}

fn profile_pack() -> Result<&'static ProfilePack, CliError> {
    if PROFILE_PACK.get().is_none() {
        let profiles = from_canonical_profile_pack_json(PROFILE_PACK_JSON)
            .map_err(|error| CliError::LoadProfiles(error.to_string()))?;
        // Another thread may initialize the same verified embedded bytes
        // first. In that case both packs have identical canonical contents.
        let _ = PROFILE_PACK.set(profiles);
    }

    PROFILE_PACK
        .get()
        .ok_or_else(|| CliError::LoadProfiles("initialization did not complete".to_owned()))
}
