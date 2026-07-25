//! Compile all profile enrichments into the canonical runtime pack.

use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use escpos2png_profiles::{compile_profile_with_lock, to_canonical_profile_pack_json};

struct Paths {
    capabilities: PathBuf,
    upstream_lock: PathBuf,
    enrichments: PathBuf,
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let paths = parse_paths()?;
    let capabilities = fs::read(&paths.capabilities)?;
    let upstream_lock = fs::read_to_string(&paths.upstream_lock)?;
    let enrichment_paths = find_enrichments(&paths.enrichments)?;
    let mut profiles = Vec::with_capacity(enrichment_paths.len());

    for enrichment_path in enrichment_paths {
        let enrichment = fs::read_to_string(&enrichment_path)?;
        let compiled = compile_profile_with_lock(&capabilities, &enrichment, &upstream_lock)?;
        profiles.push(compiled.profile);
    }

    let json = to_canonical_profile_pack_json(profiles)?;
    if let Some(parent) = paths.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&paths.output, json)?;
    println!("wrote {}", paths.output.display());
    Ok(())
}

fn parse_paths() -> Result<Paths, Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1).map(PathBuf::from);
    let paths = Paths {
        capabilities: required_argument(&mut arguments, "capabilities JSON")?,
        upstream_lock: required_argument(&mut arguments, "upstream lock")?,
        enrichments: required_argument(&mut arguments, "enrichments directory")?,
        output: required_argument(&mut arguments, "output profile pack")?,
    };
    if arguments.next().is_some() {
        return Err(usage().into());
    }
    Ok(paths)
}

fn required_argument(
    arguments: &mut impl Iterator<Item = PathBuf>,
    name: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    arguments
        .next()
        .ok_or_else(|| format!("missing {name}\n{}", usage()).into())
}

fn usage() -> &'static str {
    "usage: compile-profile-pack <capabilities.json> <upstream.lock.toml> \
     <enrichments-directory> <output.json>"
}

fn find_enrichments(directory: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut paths = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "toml")
    });
    paths.sort();
    Ok(paths)
}
