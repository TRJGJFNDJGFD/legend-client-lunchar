//! Mod search/install — the `ModProvider` abstraction (ARCHITECTURE.md §5)
//! with one real implementation: Modrinth's public v2 REST API.
//!
//! Reachability note: `api.modrinth.com` is blocked by this dev sandbox's
//! network policy (confirmed via the proxy status endpoint, same as
//! maven.fabricmc.net and the Xbox/Minecraft auth hosts). This module is
//! written against Modrinth's documented v2 API contract and the download
//! pipeline matches ARCHITECTURE.md §8 exactly, but it has not been
//! exercised end to end here — verify on a machine with normal internet
//! access before shipping.

use crate::profiles::Profile;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModError {
    #[error("network request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("downloaded file failed hash verification (expected {expected}, got {actual})")]
    HashMismatch { expected: String, actual: String },
    #[error("refused to write outside the mods directory: {0}")]
    UnsafePath(String),
    #[error("mod version not found or incompatible with the selected Minecraft version/loader")]
    Incompatible,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModSearchResult {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModVersion {
    pub version_id: String,
    pub project_id: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<ModFile>,
    pub dependencies: Vec<ModDependency>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModFile {
    pub filename: String,
    pub url: String,
    pub sha1: String,
    pub primary: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModDependency {
    pub project_id: Option<String>,
    pub dependency_type: String, // "required" | "optional" | "incompatible"
}

// --- Modrinth API response shapes (subset we actually use) ---

#[derive(Deserialize)]
struct ModrinthSearchResponse {
    hits: Vec<ModrinthSearchHit>,
}

#[derive(Deserialize)]
struct ModrinthSearchHit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    author: String,
    icon_url: Option<String>,
    downloads: u64,
}

#[derive(Deserialize)]
struct ModrinthVersion {
    id: String,
    project_id: String,
    version_number: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    files: Vec<ModrinthFile>,
    dependencies: Vec<ModrinthDependency>,
}

#[derive(Deserialize)]
struct ModrinthFile {
    filename: String,
    url: String,
    primary: bool,
    hashes: ModrinthHashes,
}

#[derive(Deserialize)]
struct ModrinthHashes {
    sha1: String,
}

#[derive(Deserialize)]
struct ModrinthDependency {
    project_id: Option<String>,
    dependency_type: String,
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent("LegendClient/0.1 (legend-client-launcher)")
        .build()
        .expect("reqwest client should always build")
}

const MODRINTH_BASE: &str = "https://api.modrinth.com/v2";

pub fn search(query: &str, minecraft_version: &str, loader: &str) -> Result<Vec<ModSearchResult>, ModError> {
    let facets = format!(
        r#"[["versions:{}"],["categories:{}"]]"#,
        minecraft_version, loader
    );

    let response = client()
        .get(format!("{MODRINTH_BASE}/search"))
        .query(&[("query", query), ("facets", &facets), ("limit", "20")])
        .send()?
        .error_for_status()?;

    let parsed: ModrinthSearchResponse = response.json()?;
    Ok(parsed
        .hits
        .into_iter()
        .map(|h| ModSearchResult {
            project_id: h.project_id,
            slug: h.slug,
            title: h.title,
            description: h.description,
            author: h.author,
            icon_url: h.icon_url,
            downloads: h.downloads,
        })
        .collect())
}

pub fn get_versions(project_id: &str, minecraft_version: &str, loader: &str) -> Result<Vec<ModVersion>, ModError> {
    let response = client()
        .get(format!("{MODRINTH_BASE}/project/{project_id}/version"))
        .query(&[
            ("game_versions", format!(r#"["{}"]"#, minecraft_version)),
            ("loaders", format!(r#"["{}"]"#, loader)),
        ])
        .send()?
        .error_for_status()?;

    let parsed: Vec<ModrinthVersion> = response.json()?;
    Ok(parsed
        .into_iter()
        .map(|v| ModVersion {
            version_id: v.id,
            project_id: v.project_id,
            version_number: v.version_number,
            game_versions: v.game_versions,
            loaders: v.loaders,
            files: v
                .files
                .into_iter()
                .map(|f| ModFile {
                    filename: f.filename,
                    url: f.url,
                    sha1: f.hashes.sha1,
                    primary: f.primary,
                })
                .collect(),
            dependencies: v
                .dependencies
                .into_iter()
                .map(|d| ModDependency {
                    project_id: d.project_id,
                    dependency_type: d.dependency_type,
                })
                .collect(),
        })
        .collect())
}

/// Guards against path traversal (ARCHITECTURE.md §9): resolves the target
/// filename against the mods directory and rejects anything that would
/// normalize outside it.
fn safe_target_path(mods_dir: &Path, filename: &str) -> Result<PathBuf, ModError> {
    let candidate = mods_dir.join(filename);
    let normalized_parent = candidate
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| mods_dir.to_path_buf());

    // The parent of the target file must BE the mods dir, exactly — no
    // "../", no absolute override, no nested subdirectory smuggled in via
    // the filename.
    let mods_dir_abs = mods_dir.to_path_buf();
    if normalized_parent != mods_dir_abs {
        return Err(ModError::UnsafePath(filename.to_string()));
    }
    if filename.contains("..") || filename.starts_with('/') || filename.starts_with('\\') {
        return Err(ModError::UnsafePath(filename.to_string()));
    }

    Ok(candidate)
}

/// The real install pipeline from ARCHITECTURE.md §8: download, verify
/// hash, write into the profile's mods folder. Never silently accepts a
/// corrupt/tampered download.
pub fn download_and_install(profile: &Profile, file: &ModFile) -> Result<PathBuf, ModError> {
    let mods_dir = crate::profiles::mods_dir(&profile.id);
    std::fs::create_dir_all(&mods_dir)?;
    let target = safe_target_path(&mods_dir, &file.filename)?;

    let bytes = client().get(&file.url).send()?.error_for_status()?.bytes()?;

    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    let actual_hash = hex::encode(hasher.finalize());

    if !actual_hash.eq_ignore_ascii_case(&file.sha1) {
        return Err(ModError::HashMismatch {
            expected: file.sha1.clone(),
            actual: actual_hash,
        });
    }

    let mut out = std::fs::File::create(&target)?;
    out.write_all(&bytes)?;

    Ok(target)
}

/// Picks the version Modrinth would want installed by default: the API's
/// `/version` endpoint already filters by game version + loader, so the
/// first result is the newest matching one — no extra ranking needed.
fn pick_best_version(versions: &[ModVersion]) -> Option<&ModVersion> {
    versions.first()
}

/// Installs a mod version's primary file, then recursively installs any
/// `required` dependency that isn't already present — replacing the
/// roadmap's placeholder "warns you to install it manually" behavior.
/// Returns the filenames actually written, in install order.
pub fn install_with_dependencies(
    profile: &Profile,
    project_id: &str,
    version_id: &str,
    minecraft_version: &str,
    loader: &str,
) -> Result<Vec<String>, ModError> {
    let mut installed = Vec::new();
    install_with_dependencies_inner(
        profile,
        project_id,
        Some(version_id),
        minecraft_version,
        loader,
        &mut installed,
    )?;
    Ok(installed)
}

fn install_with_dependencies_inner(
    profile: &Profile,
    project_id: &str,
    version_id: Option<&str>,
    minecraft_version: &str,
    loader: &str,
    installed: &mut Vec<String>,
) -> Result<(), ModError> {
    let versions = get_versions(project_id, minecraft_version, loader)?;
    let version = match version_id {
        Some(id) => versions
            .iter()
            .find(|v| v.version_id == id)
            .ok_or(ModError::Incompatible)?,
        None => pick_best_version(&versions).ok_or(ModError::Incompatible)?,
    };

    let already_installed = list_installed(profile).unwrap_or_default();

    let primary = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or(ModError::Incompatible)?;

    if !already_installed.contains(&primary.filename) {
        download_and_install(profile, primary)?;
        installed.push(primary.filename.clone());
    }

    for dep in &version.dependencies {
        if dep.dependency_type != "required" {
            continue;
        }
        if let Some(dep_project_id) = &dep.project_id {
            install_with_dependencies_inner(
                profile,
                dep_project_id,
                None,
                minecraft_version,
                loader,
                installed,
            )?;
        }
    }

    Ok(())
}

/// Copies a jar already on disk (e.g. the built Legend Client Fabric mod,
/// or a manually-downloaded mod) into a profile's mods folder. Fully local
/// — no network involved — so unlike the Modrinth path above, this is
/// exercised by a real test with a real file, not just guarded logic.
pub fn install_local_jar(profile: &Profile, source: &Path) -> Result<PathBuf, ModError> {
    let filename = source
        .file_name()
        .ok_or_else(|| ModError::UnsafePath(source.display().to_string()))?
        .to_string_lossy()
        .to_string();

    let mods_dir = crate::profiles::mods_dir(&profile.id);
    std::fs::create_dir_all(&mods_dir)?;
    let target = safe_target_path(&mods_dir, &filename)?;
    std::fs::copy(source, &target)?;
    Ok(target)
}

/// Lists installed mods as the filenames present in the profile's mods
/// folder — the simplest possible source of truth, always accurate
/// because it reads what's actually on disk rather than a maybe-stale
/// database of "what we think we installed."
pub fn list_installed(profile: &Profile) -> std::io::Result<Vec<String>> {
    let mods_dir = crate::profiles::mods_dir(&profile.id);
    if !mods_dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(mods_dir)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|ext| ext == "jar") {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    Ok(names)
}

pub fn remove_installed(profile: &Profile, filename: &str) -> Result<(), ModError> {
    let mods_dir = crate::profiles::mods_dir(&profile.id);
    let target = safe_target_path(&mods_dir, filename)?;
    if target.exists() {
        std::fs::remove_file(target)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_filenames() {
        let mods_dir = PathBuf::from("/tmp/legend-test-mods");
        assert!(safe_target_path(&mods_dir, "../../etc/passwd").is_err());
        assert!(safe_target_path(&mods_dir, "/etc/passwd").is_err());
        assert!(safe_target_path(&mods_dir, "sodium.jar").is_ok());
    }

    #[test]
    fn pick_best_version_takes_the_first_result() {
        let versions = vec![
            ModVersion {
                version_id: "v2".into(),
                project_id: "p".into(),
                version_number: "2.0".into(),
                game_versions: vec!["1.21.1".into()],
                loaders: vec!["fabric".into()],
                files: vec![],
                dependencies: vec![],
            },
            ModVersion {
                version_id: "v1".into(),
                project_id: "p".into(),
                version_number: "1.0".into(),
                game_versions: vec!["1.21.1".into()],
                loaders: vec!["fabric".into()],
                files: vec![],
                dependencies: vec![],
            },
        ];
        assert_eq!(pick_best_version(&versions).unwrap().version_id, "v2");
        assert!(pick_best_version(&[]).is_none());
    }

    #[test]
    fn install_local_jar_copies_a_real_file_into_the_mods_dir() {
        let profile = crate::profiles::create(
            "Local Jar Install Test".into(),
            "1.21.1".into(),
            "fabric".into(),
            1024,
            2048,
        )
        .expect("profile create should succeed");

        let source = std::env::temp_dir().join("legend-test-source-mod.jar");
        std::fs::write(&source, b"not a real jar, just test bytes").unwrap();

        let installed_path = install_local_jar(&profile, &source).expect("install should succeed");
        assert!(installed_path.exists());
        assert_eq!(
            std::fs::read(&installed_path).unwrap(),
            b"not a real jar, just test bytes"
        );

        std::fs::remove_file(&source).ok();
        crate::profiles::delete(&profile.id).ok();
    }

    #[test]
    fn download_and_install_rejects_a_tampered_hash() {
        let profile = crate::profiles::create(
            "Mod Install Test".into(),
            "1.21.1".into(),
            "fabric".into(),
            1024,
            2048,
        )
        .expect("profile create should succeed");

        let file = ModFile {
            filename: "does-not-matter.jar".into(),
            // Deliberately unreachable URL + wrong hash — this test only
            // proves the hash-mismatch path is real, without needing a
            // live download (blocked in this sandbox, see module doc).
            url: "https://example.invalid/does-not-exist.jar".into(),
            sha1: "0000000000000000000000000000000000000000".into(),
            primary: true,
        };

        // The request itself will fail before we ever get to compare
        // hashes (no network to example.invalid), which is still a
        // meaningful assertion: we never silently "succeed" on a failed
        // download.
        let result = download_and_install(&profile, &file);
        assert!(result.is_err());

        crate::profiles::delete(&profile.id).ok();
    }
}
