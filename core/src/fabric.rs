//! Fabric loader resolution (ARCHITECTURE.md §6 step 2/3) — picks a loader
//! version for a Minecraft version and builds the library list the
//! classpath builder in `launch.rs` needs.
//!
//! Reachability note: `meta.fabricmc.net` and `maven.fabricmc.net` are
//! blocked by this dev sandbox's network policy (same restriction as
//! Modrinth/Xbox — see mods.rs's module doc). Written against Fabric's
//! documented Meta API v2 contract; JSON parsing is verified with fixture
//! data in the tests below, but the live HTTP calls are not exercised
//! here — verify on a machine with normal internet access.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FabricError {
    #[error("network request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("failed to parse Fabric Meta API response: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("no stable Fabric loader version is available for this Minecraft version")]
    NoStableLoader,
}

/// One Maven-coordinate library the classpath builder needs to download
/// and include, e.g. `net.fabricmc:fabric-loader:0.16.9`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FabricLibrary {
    pub maven_coordinate: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedFabricLoader {
    pub loader_version: String,
    pub main_class: String,
    pub libraries: Vec<FabricLibrary>,
}

// --- Fabric Meta API v2 response shapes (subset we use) ---
// GET https://meta.fabricmc.net/v2/versions/loader/<mc_version>

#[derive(Deserialize)]
struct LoaderEntry {
    loader: LoaderInfo,
}

#[derive(Deserialize)]
struct LoaderInfo {
    version: String,
    stable: bool,
}

// GET https://meta.fabricmc.net/v2/versions/loader/<mc_version>/<loader_version>/profile/json

#[derive(Deserialize)]
struct LauncherProfile {
    #[serde(rename = "mainClass")]
    main_class: MainClassField,
    libraries: LibrariesField,
}

// The profile JSON's mainClass can be a plain string or, on some
// Fabric versions, an object keyed by side ("client"/"server").
#[derive(Deserialize)]
#[serde(untagged)]
enum MainClassField {
    Simple(String),
    BySide { client: String },
}

#[derive(Deserialize)]
struct LibrariesField {
    common: Vec<ProfileLibrary>,
}

#[derive(Deserialize)]
struct ProfileLibrary {
    name: String,
    url: String,
}

const FABRIC_META_BASE: &str = "https://meta.fabricmc.net/v2";

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent("LegendClient/0.1 (legend-client-launcher)")
        .build()
        .expect("reqwest client should always build")
}

/// Converts a Maven coordinate (`group:artifact:version`) into the path
/// segment Maven repos serve it at: `group/with/dots/as/slashes/artifact/version/artifact-version.jar`.
fn maven_coordinate_to_path(coordinate: &str) -> Option<String> {
    let mut parts = coordinate.split(':');
    let group = parts.next()?;
    let artifact = parts.next()?;
    let version = parts.next()?;
    let group_path = group.replace('.', "/");
    Some(format!(
        "{group_path}/{artifact}/{version}/{artifact}-{version}.jar"
    ))
}

fn parse_loader_entries(body: &str) -> Result<Vec<LoaderEntry>, FabricError> {
    Ok(serde_json::from_str(body)?)
}

/// Picks the newest *stable* loader version for a Minecraft version. The
/// Meta API already returns entries newest-first, so the first stable one
/// wins.
fn pick_stable_loader_version(entries: &[LoaderEntry]) -> Option<&str> {
    entries
        .iter()
        .find(|e| e.loader.stable)
        .map(|e| e.loader.version.as_str())
}

fn parse_profile(body: &str, loader_version: &str) -> ResolvedFabricLoader {
    let profile: LauncherProfile =
        serde_json::from_str(body).expect("Fabric Meta profile JSON should match the documented shape");

    let main_class = match profile.main_class {
        MainClassField::Simple(s) => s,
        MainClassField::BySide { client } => client,
    };

    let libraries = profile
        .libraries
        .common
        .into_iter()
        .filter_map(|lib| {
            let path = maven_coordinate_to_path(&lib.name)?;
            Some(FabricLibrary {
                maven_coordinate: lib.name,
                url: format!("{}{}", lib.url, path),
            })
        })
        .collect();

    ResolvedFabricLoader {
        loader_version: loader_version.to_string(),
        main_class,
        libraries,
    }
}

/// Resolves the Fabric loader + libraries for a Minecraft version, doing
/// the two real HTTP calls the Meta API requires: list loader versions,
/// then fetch that loader's launch profile.
pub fn resolve_stable_loader(minecraft_version: &str) -> Result<ResolvedFabricLoader, FabricError> {
    let list_body = client()
        .get(format!("{FABRIC_META_BASE}/versions/loader/{minecraft_version}"))
        .send()?
        .error_for_status()?
        .text()?;

    let entries = parse_loader_entries(&list_body)?;
    let loader_version = pick_stable_loader_version(&entries)
        .ok_or(FabricError::NoStableLoader)?
        .to_string();

    let profile_body = client()
        .get(format!(
            "{FABRIC_META_BASE}/versions/loader/{minecraft_version}/{loader_version}/profile/json"
        ))
        .send()?
        .error_for_status()?
        .text()?;

    Ok(parse_profile(&profile_body, &loader_version))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_LOADER_LIST: &str = r#"[
        {"loader": {"version": "0.16.9", "stable": true}},
        {"loader": {"version": "0.17.0-beta.1", "stable": false}}
    ]"#;

    const SAMPLE_PROFILE: &str = r#"{
        "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
        "libraries": {
            "common": [
                {"name": "net.fabricmc:fabric-loader:0.16.9", "url": "https://maven.fabricmc.net/"},
                {"name": "net.fabricmc:intermediary:1.21.1", "url": "https://maven.fabricmc.net/"}
            ]
        }
    }"#;

    #[test]
    fn picks_the_first_stable_loader_version() {
        let entries = parse_loader_entries(SAMPLE_LOADER_LIST).unwrap();
        assert_eq!(pick_stable_loader_version(&entries), Some("0.16.9"));
    }

    #[test]
    fn returns_none_when_nothing_is_stable() {
        let entries = parse_loader_entries(
            r#"[{"loader": {"version": "0.17.0-beta.1", "stable": false}}]"#,
        )
        .unwrap();
        assert_eq!(pick_stable_loader_version(&entries), None);
    }

    #[test]
    fn parses_a_real_shaped_profile_into_libraries_with_download_urls() {
        let resolved = parse_profile(SAMPLE_PROFILE, "0.16.9");
        assert_eq!(resolved.main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert_eq!(resolved.libraries.len(), 2);
        assert_eq!(
            resolved.libraries[0].url,
            "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar"
        );
    }

    #[test]
    fn maven_coordinate_to_path_converts_dots_to_slashes() {
        assert_eq!(
            maven_coordinate_to_path("net.fabricmc:intermediary:1.21.1").unwrap(),
            "net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar"
        );
        assert_eq!(maven_coordinate_to_path("not-a-valid-coordinate"), None);
    }
}
