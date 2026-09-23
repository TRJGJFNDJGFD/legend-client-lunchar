//! Profile persistence — real reads/writes to `~/.legendclient/profiles/`.
//! One profile = one directory with a `profile.json` plus its own
//! mods/resourcepacks/shaderpacks/screenshots/logs subfolders, matching the
//! architecture doc.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: String, // "vanilla" | "fabric" | "forge" | "neoforge"
    pub min_ram_mb: u32,
    pub max_ram_mb: u32,
    pub java_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_played: Option<DateTime<Utc>>,
}

fn legend_root() -> PathBuf {
    dirs::home_dir()
        .expect("no home directory found on this system")
        .join(".legendclient")
}

fn profiles_root() -> PathBuf {
    legend_root().join("profiles")
}

fn profile_dir(id: &str) -> PathBuf {
    profiles_root().join(id)
}

/// Every profile subfolder from the architecture doc, created up front so
/// nothing downstream has to remember to `mkdir` before writing into it.
fn ensure_profile_dirs(id: &str) -> std::io::Result<()> {
    let base = profile_dir(id);
    for sub in ["mods", "resourcepacks", "shaderpacks", "screenshots", "logs"] {
        fs::create_dir_all(base.join(sub))?;
    }
    Ok(())
}

pub fn create(
    name: String,
    minecraft_version: String,
    loader: String,
    min_ram_mb: u32,
    max_ram_mb: u32,
) -> std::io::Result<Profile> {
    let profile = Profile {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        minecraft_version,
        loader,
        min_ram_mb,
        max_ram_mb,
        java_path: None,
        created_at: Utc::now(),
        last_played: None,
    };

    ensure_profile_dirs(&profile.id)?;
    save(&profile)?;
    Ok(profile)
}

pub fn save(profile: &Profile) -> std::io::Result<()> {
    let path = profile_dir(&profile.id).join("profile.json");
    let json = serde_json::to_string_pretty(profile).expect("Profile always serializes");
    fs::write(path, json)
}

pub fn list() -> std::io::Result<Vec<Profile>> {
    let root = profiles_root();
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut profiles = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let profile_json = entry.path().join("profile.json");
        if profile_json.exists() {
            if let Ok(contents) = fs::read_to_string(&profile_json) {
                if let Ok(profile) = serde_json::from_str::<Profile>(&contents) {
                    profiles.push(profile);
                }
            }
        }
    }

    profiles.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(profiles)
}

pub fn delete(id: &str) -> std::io::Result<()> {
    let dir = profile_dir(id);
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}

pub fn mods_dir(id: &str) -> PathBuf {
    profile_dir(id).join("mods")
}

pub fn logs_dir(id: &str) -> PathBuf {
    profile_dir(id).join("logs")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each test uses its own throwaway profile id so they don't collide,
    // and cleans up after itself.

    #[test]
    fn create_then_list_then_delete_roundtrip() {
        let created = create(
            "Test Profile".into(),
            "1.21.1".into(),
            "fabric".into(),
            2048,
            4096,
        )
        .expect("create should succeed");

        let all = list().expect("list should succeed");
        assert!(all.iter().any(|p| p.id == created.id));

        assert!(mods_dir(&created.id).exists());

        delete(&created.id).expect("delete should succeed");
        let all_after = list().expect("list should succeed");
        assert!(!all_after.iter().any(|p| p.id == created.id));
    }
}
