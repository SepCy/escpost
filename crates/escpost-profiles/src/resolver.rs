//! First-class resolver over the embedded profile pack.
//!
//! Centralizes what the CLI and Python binding previously each duplicated:
//! embedding the compiled profile pack JSON and lazily deserializing it into
//! a `ProfilePack` on first use. Kept out of the pure compile core (the rest
//! of this crate stays `include_bytes!`/`OnceLock`-free) so that core stays
//! trivially embeddable anywhere, including wasm; this module's use of
//! `include_bytes!` and `OnceLock` is itself wasm-safe (no filesystem access).

use std::sync::OnceLock;

use crate::{PrinterProfile, ProfilePack, from_canonical_profile_pack_json};

const PROFILE_PACK_JSON: &[u8] = include_bytes!("../profiles/.generated/profiles.json");
static PACK: OnceLock<ProfilePack> = OnceLock::new();

/// Errors from resolving a profile id against the embedded pack.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// The embedded profile pack failed to load or deserialize.
    #[error("could not load the embedded profile pack: {0}")]
    LoadPack(String),
    /// The pack loaded successfully but has no profile with this id.
    #[error("unknown printer profile {0:?}")]
    UnknownProfile(String),
}

/// Resolve a printer profile by id against the embedded pack.
pub fn resolve(id: &str) -> Result<&'static PrinterProfile, ResolveError> {
    let pack = pack()?;
    pack.get(id)
        .ok_or_else(|| ResolveError::UnknownProfile(id.to_owned()))
}

/// The sorted list of all profile ids in the embedded pack.
pub fn available_ids() -> Result<Vec<String>, ResolveError> {
    let pack = pack()?;
    let mut ids: Vec<String> = pack.profiles().map(|profile| profile.id.clone()).collect();
    ids.sort();
    Ok(ids)
}

fn pack() -> Result<&'static ProfilePack, ResolveError> {
    if PACK.get().is_none() {
        let profiles = from_canonical_profile_pack_json(PROFILE_PACK_JSON)
            .map_err(|error| ResolveError::LoadPack(error.to_string()))?;
        // Another thread may initialize the same verified embedded bytes
        // first. In that case both packs have identical canonical contents.
        let _ = PACK.set(profiles);
    }

    PACK.get()
        .ok_or_else(|| ResolveError::LoadPack("initialization did not complete".to_owned()))
}
