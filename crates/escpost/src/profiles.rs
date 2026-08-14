use escpost_profiles::PrinterProfile;
use escpost_profiles::resolver::{self, ResolveError};
use inquire::Select;

use crate::error::CliError;

pub(crate) fn load(profile_id: &str) -> Result<&'static PrinterProfile, CliError> {
    resolver::resolve(profile_id).map_err(|error| match error {
        ResolveError::UnknownProfile(id) => CliError::UnknownProfile(id),
        ResolveError::LoadPack(message) => CliError::LoadProfiles(message),
    })
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

    let choices = resolver::available_ids().map_err(|error| match error {
        ResolveError::UnknownProfile(id) => CliError::UnknownProfile(id),
        ResolveError::LoadPack(message) => CliError::LoadProfiles(message),
    })?;
    Select::new("Printer profile", choices)
        .with_help_message("Use REFERENCE when no physical printer is known")
        .prompt()
        .map_err(|error| CliError::ProfilePrompt(error.to_string()))
}
