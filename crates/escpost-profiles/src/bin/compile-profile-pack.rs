//! Compile all profile enrichments into the canonical runtime pack.

use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use escpost_profiles::{compile_all, to_canonical_profile_pack_json};

struct Paths {
    capabilities: PathBuf,
    profiles: PathBuf,
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let paths = parse_paths()?;
    let capabilities = fs::read(&paths.capabilities)?;
    let enrichment_paths = find_profile_files(&paths.profiles)?;
    let mut enrichment_tomls = Vec::with_capacity(enrichment_paths.len());
    for enrichment_path in enrichment_paths {
        enrichment_tomls.push(fs::read_to_string(&enrichment_path)?);
    }

    let outcome = compile_all(&capabilities, &enrichment_tomls)?;
    for id in &outcome.skipped_upstream {
        eprintln!("skipping {id}: upstream media width unknown");
    }

    let json = to_canonical_profile_pack_json(outcome.profiles)?;
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
        profiles: required_argument(&mut arguments, "profiles directory")?,
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
    "usage: compile-profile-pack <capabilities.json> <profiles-directory> <output.json>"
}

fn find_profile_files(directory: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut paths = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    // Infrastructure directories are hidden for readability, but profile
    // discovery still requires the explicit profile.toml marker. This avoids
    // treating naming conventions alone as executable configuration.
    paths.retain(|path| {
        path.is_dir()
            && !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'))
            && path.join("profile.toml").is_file()
    });
    for path in &mut paths {
        path.push("profile.toml");
    }
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::find_profile_files;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn discovers_only_visible_directories_with_a_profile_file() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the system clock should follow the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "escpost-profile-discovery-{}-{unique}",
            std::process::id()
        ));
        let visible = root.join("VISIBLE");
        let hidden = root.join(".hidden");
        let incomplete = root.join("INCOMPLETE");
        fs::create_dir_all(&visible).expect("the visible profile directory should be created");
        fs::create_dir_all(&hidden).expect("the hidden infrastructure directory should be created");
        fs::create_dir_all(&incomplete).expect("the incomplete directory should be created");
        fs::write(visible.join("profile.toml"), "").expect("the profile marker should be written");
        fs::write(hidden.join("profile.toml"), "")
            .expect("the hidden profile marker should be written");

        let profiles = find_profile_files(&root).expect("profile discovery should succeed");

        fs::remove_dir_all(&root).expect("the temporary profile tree should be removable");
        assert_eq!(profiles, [visible.join("profile.toml")]);
    }
}
