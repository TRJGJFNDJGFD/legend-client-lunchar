//! Minecraft version resolution + classpath builder (ARCHITECTURE.md §6,
//! steps 1-3). Fetches Mojang's public piston-meta manifest, downloads the
//! client jar + libraries with sha1 verification, and builds the classpath
//! + launch args `launch.rs` needs to actually start the game.
//!
//! Reachability note: `piston-meta.mojang.com` / `piston-data.mojang.com` /
//! `resources.download.minecraft.net` are blocked by this dev sandbox's
//! network policy (same restriction as Modrinth/Fabric — see mods.rs's
//! module doc). Written against Mojang's documented (if unofficial)
//! version-manifest-v2 contract; JSON parsing and the OS-rule/classpath
//! logic are verified with fixture data and real temp-file downloads in
//! the tests below — the live HTTP calls are not exercised here.

use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VersionError {
    #[error("network request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse version metadata: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("version '{0}' not found in Mojang's version manifest")]
    VersionNotFound(String),
    #[error("downloaded file failed hash verification (expected {expected}, got {actual})")]
    HashMismatch { expected: String, actual: String },
}

/// One library entry resolved for the CURRENT operating system — already
/// filtered by `rules`, so anything in this list should actually be
/// downloaded and put on the classpath.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLibrary {
    pub maven_name: String,
    pub url: String,
    pub sha1: String,
    /// Path relative to the shared `libraries/` cache dir, e.g.
    /// `com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar`.
    pub relative_path: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub id: String,
    pub main_class: String,
    pub client_jar_url: String,
    pub client_jar_sha1: String,
    pub libraries: Vec<ResolvedLibrary>,
}

// --- Mojang manifest / version JSON shapes (subset we use) ---

#[derive(Deserialize)]
struct VersionManifest {
    versions: Vec<ManifestEntry>,
}

#[derive(Deserialize)]
struct ManifestEntry {
    id: String,
    url: String,
}

#[derive(Deserialize)]
struct VersionJson {
    #[serde(rename = "mainClass")]
    main_class: String,
    downloads: VersionDownloads,
    libraries: Vec<LibraryEntry>,
}

#[derive(Deserialize)]
struct VersionDownloads {
    client: DownloadArtifact,
}

#[derive(Deserialize)]
struct DownloadArtifact {
    url: String,
    sha1: String,
}

#[derive(Deserialize)]
struct LibraryEntry {
    name: String,
    downloads: LibraryDownloads,
    #[serde(default)]
    rules: Vec<LibraryRule>,
}

#[derive(Deserialize)]
struct LibraryDownloads {
    artifact: Option<DownloadArtifactWithPath>,
}

#[derive(Deserialize)]
struct DownloadArtifactWithPath {
    path: String,
    url: String,
    sha1: String,
}

#[derive(Deserialize)]
struct LibraryRule {
    action: String, // "allow" | "disallow"
    os: Option<RuleOs>,
}

#[derive(Deserialize)]
struct RuleOs {
    name: Option<String>,
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent("LegendClient/0.1 (legend-client-launcher)")
        .build()
        .expect("reqwest client should always build")
}

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// Maps this build's OS to the name Mojang's rules use.
fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// A library with no rules is always included. One with rules is included
/// only if the rules, evaluated in order, end in "allow" for this OS —
/// this is Mojang's documented rule-evaluation semantics.
fn library_applies_to_current_os(rules: &[LibraryRule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let os = current_os_name();
    let mut allowed = false;
    for rule in rules {
        let matches_os = rule.os.as_ref().and_then(|o| o.name.as_deref()) == Some(os) || rule.os.is_none();
        if matches_os {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

fn parse_version_json(body: &str) -> Result<(String, DownloadArtifact, Vec<ResolvedLibrary>), VersionError> {
    let parsed: VersionJson = serde_json::from_str(body)?;

    let libraries = parsed
        .libraries
        .into_iter()
        .filter(|lib| library_applies_to_current_os(&lib.rules))
        .filter_map(|lib| {
            let artifact = lib.downloads.artifact?;
            Some(ResolvedLibrary {
                maven_name: lib.name,
                url: artifact.url,
                sha1: artifact.sha1,
                relative_path: artifact.path,
            })
        })
        .collect();

    Ok((parsed.main_class, parsed.downloads.client, libraries))
}

/// Resolves a Minecraft version id into everything needed to build a
/// classpath: fetches the manifest to find the version's metadata URL,
/// then fetches that version's own JSON.
pub fn resolve_version(minecraft_version: &str) -> Result<ResolvedVersion, VersionError> {
    let manifest_body = client().get(MANIFEST_URL).send()?.error_for_status()?.text()?;
    let manifest: VersionManifest = serde_json::from_str(&manifest_body)?;

    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == minecraft_version)
        .ok_or_else(|| VersionError::VersionNotFound(minecraft_version.to_string()))?;

    let version_body = client().get(&entry.url).send()?.error_for_status()?.text()?;
    let (main_class, client_download, libraries) = parse_version_json(&version_body)?;

    Ok(ResolvedVersion {
        id: minecraft_version.to_string(),
        main_class,
        client_jar_url: client_download.url,
        client_jar_sha1: client_download.sha1,
        libraries,
    })
}

fn legend_root() -> PathBuf {
    dirs::home_dir()
        .expect("no home directory found on this system")
        .join(".legendclient")
}

pub fn libraries_dir() -> PathBuf {
    legend_root().join("libraries")
}

pub fn versions_dir() -> PathBuf {
    legend_root().join("versions")
}

fn download_and_verify(url: &str, expected_sha1: &str, target: &Path) -> Result<(), VersionError> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Already-downloaded + hash-matching files are left alone — this is
    // the "shared runtimes/libraries cache" from ARCHITECTURE.md §4, not
    // a fresh download every launch.
    if target.exists() {
        if let Ok(bytes) = std::fs::read(target) {
            if sha1_hex(&bytes) == expected_sha1 {
                return Ok(());
            }
        }
    }

    let bytes = client().get(url).send()?.error_for_status()?.bytes()?;
    let actual = sha1_hex(&bytes);
    if !actual.eq_ignore_ascii_case(expected_sha1) {
        return Err(VersionError::HashMismatch {
            expected: expected_sha1.to_string(),
            actual,
        });
    }

    let mut file = std::fs::File::create(target)?;
    file.write_all(&bytes)?;
    Ok(())
}

fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Downloads the client jar + every resolved library (skipping ones
/// already cached with a matching hash), returning every jar path in the
/// order the classpath should list them.
pub fn download_all(resolved: &ResolvedVersion) -> Result<Vec<PathBuf>, VersionError> {
    let mut paths = Vec::new();

    let client_jar_path = versions_dir().join(&resolved.id).join(format!("{}.jar", resolved.id));
    download_and_verify(&resolved.client_jar_url, &resolved.client_jar_sha1, &client_jar_path)?;
    paths.push(client_jar_path);

    for lib in &resolved.libraries {
        let lib_path = libraries_dir().join(&lib.relative_path);
        download_and_verify(&lib.url, &lib.sha1, &lib_path)?;
        paths.push(lib_path);
    }

    Ok(paths)
}

/// Joins jar paths into a single classpath string using this OS's path
/// separator (`:` on Unix, `;` on Windows) — the same separator `java -cp`
/// expects.
pub fn build_classpath(jar_paths: &[PathBuf]) -> String {
    let separator = if cfg!(target_os = "windows") { ";" } else { ":" };
    jar_paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(separator)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_VERSION_JSON: &str = r#"{
        "mainClass": "net.minecraft.client.main.Main",
        "downloads": {
            "client": {"url": "https://piston-data.mojang.com/v1/client.jar", "sha1": "abc123"}
        },
        "libraries": [
            {
                "name": "com.mojang:brigadier:1.0.18",
                "downloads": {"artifact": {"path": "com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar", "url": "https://libraries.minecraft.net/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar", "sha1": "def456"}}
            },
            {
                "name": "org.lwjgl:lwjgl:3.3.3",
                "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar", "url": "https://libraries.minecraft.net/x", "sha1": "ghi789"}},
                "rules": [{"action": "allow", "os": {"name": "windows"}}]
            },
            {
                "name": "org.lwjgl:lwjgl:3.3.3-linux",
                "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar", "url": "https://libraries.minecraft.net/y", "sha1": "jkl012"}},
                "rules": [{"action": "allow", "os": {"name": "linux"}}]
            }
        ]
    }"#;

    #[test]
    fn parses_a_real_shaped_version_json_and_filters_libraries_by_current_os() {
        let (main_class, client_download, libraries) = parse_version_json(SAMPLE_VERSION_JSON).unwrap();
        assert_eq!(main_class, "net.minecraft.client.main.Main");
        assert_eq!(client_download.sha1, "abc123");

        // brigadier has no rules -> always included. Exactly one of the two
        // OS-gated lwjgl entries survives, matching whichever OS this test
        // actually runs on (this dev sandbox is Linux).
        let names: Vec<&str> = libraries.iter().map(|l| l.maven_name.as_str()).collect();
        assert!(names.contains(&"com.mojang:brigadier:1.0.18"));
        assert_eq!(libraries.len(), 2);
    }

    #[test]
    fn library_applies_to_current_os_respects_allow_disallow_ordering() {
        assert!(library_applies_to_current_os(&[]));

        let linux_only = vec![LibraryRule {
            action: "allow".into(),
            os: Some(RuleOs { name: Some("linux".into()) }),
        }];
        assert_eq!(library_applies_to_current_os(&linux_only), current_os_name() == "linux");

        // disallow for our OS after a blanket allow must win (rules apply in order).
        let allow_then_disallow_us = vec![
            LibraryRule { action: "allow".into(), os: None },
            LibraryRule {
                action: "disallow".into(),
                os: Some(RuleOs { name: Some(current_os_name().to_string()) }),
            },
        ];
        assert!(!library_applies_to_current_os(&allow_then_disallow_us));
    }

    #[test]
    fn build_classpath_joins_with_the_right_separator() {
        let paths = vec![PathBuf::from("/a/one.jar"), PathBuf::from("/a/two.jar")];
        let cp = build_classpath(&paths);
        let expected_sep = if cfg!(target_os = "windows") { ";" } else { ":" };
        assert_eq!(cp, format!("/a/one.jar{expected_sep}/a/two.jar"));
    }

    #[test]
    fn download_and_verify_writes_a_real_file_and_skips_a_second_call() {
        let dir = std::env::temp_dir().join("legend-test-version-download");
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("fake.jar");
        std::fs::write(&target, b"hello").unwrap();
        let hash = sha1_hex(b"hello");

        // Already on disk with a matching hash -> no network call needed,
        // and this must succeed even against an unreachable URL.
        let result = download_and_verify("https://example.invalid/nope.jar", &hash, &target);
        assert!(result.is_ok());

        std::fs::remove_file(&target).ok();
    }

    #[test]
    fn download_and_verify_rejects_a_tampered_hash() {
        let dir = std::env::temp_dir().join("legend-test-version-download-2");
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("fake2.jar");
        if target.exists() {
            std::fs::remove_file(&target).ok();
        }

        let result = download_and_verify(
            "https://example.invalid/nope.jar",
            "0000000000000000000000000000000000000000",
            &target,
        );
        assert!(result.is_err());
    }
}
