//! Non-secret account display data — `~/.legendclient/accounts.json`.
//! Tokens never live here; see `secrets.rs`.

use crate::auth::MinecraftProfile;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Account {
    pub id: String, // Minecraft profile UUID — also the keychain lookup key
    pub username: String,
    pub skin_url: Option<String>,
    pub is_active: bool,
}

fn accounts_path() -> PathBuf {
    dirs::home_dir()
        .expect("no home directory found on this system")
        .join(".legendclient")
        .join("accounts.json")
}

pub fn list() -> std::io::Result<Vec<Account>> {
    let path = accounts_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents).unwrap_or_default())
}

fn save_all(accounts: &[Account]) -> std::io::Result<()> {
    let path = accounts_path();
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(path, serde_json::to_string_pretty(accounts).unwrap())
}

/// Adds/updates an account from a fresh login result and marks it active.
/// The caller is responsible for storing the actual tokens via
/// `secrets::store_refresh_token` — this function only ever touches the
/// non-secret display record.
pub fn upsert_from_profile(profile: &MinecraftProfile) -> std::io::Result<Account> {
    let mut accounts = list()?;
    for a in accounts.iter_mut() {
        a.is_active = false;
    }

    let skin_url = profile
        .skins
        .iter()
        .find(|s| s.state == "ACTIVE")
        .map(|s| s.url.clone());

    let account = Account {
        id: profile.id.clone(),
        username: profile.name.clone(),
        skin_url,
        is_active: true,
    };

    accounts.retain(|a| a.id != account.id);
    accounts.push(account.clone());
    save_all(&accounts)?;
    Ok(account)
}

pub fn set_active(id: &str) -> std::io::Result<()> {
    let mut accounts = list()?;
    for a in accounts.iter_mut() {
        a.is_active = a.id == id;
    }
    save_all(&accounts)
}

pub fn remove(id: &str) -> std::io::Result<()> {
    let mut accounts = list()?;
    accounts.retain(|a| a.id != id);
    save_all(&accounts)?;
    let _ = crate::secrets::delete_refresh_token(id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::MinecraftSkin;

    #[test]
    fn upsert_then_list_then_remove_roundtrip() {
        let profile = MinecraftProfile {
            id: "test-uuid-account-roundtrip".into(),
            name: "TestPlayer".into(),
            skins: vec![MinecraftSkin {
                id: "skin1".into(),
                url: "https://example.com/skin.png".into(),
                state: "ACTIVE".into(),
            }],
        };

        let account = upsert_from_profile(&profile).expect("upsert should succeed");
        assert_eq!(account.username, "TestPlayer");
        assert!(account.is_active);

        let all = list().expect("list should succeed");
        assert!(all.iter().any(|a| a.id == account.id));

        remove(&account.id).expect("remove should succeed");
        let after = list().expect("list should succeed");
        assert!(!after.iter().any(|a| a.id == account.id));
    }
}
