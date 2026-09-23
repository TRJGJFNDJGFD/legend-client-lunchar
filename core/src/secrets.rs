//! OS keychain access for tokens — Windows Credential Manager / macOS
//! Keychain / Linux Secret Service, via the `keyring` crate. Nothing
//! secret ever gets written to a plain file (see ARCHITECTURE.md §7).
//!
//! Reachability note: this needs a real desktop session with a secret
//! service running (e.g. gnome-keyring on Linux). It could not be
//! exercised in the headless dev sandbox this was written in — verify on
//! a real desktop before relying on it.

const SERVICE_NAME: &str = "legend-client";

#[derive(thiserror::Error, Debug)]
pub enum SecretError {
    #[error("keychain error: {0}")]
    Keyring(#[from] keyring::Error),
}

fn entry_for(account_id: &str) -> Result<keyring::Entry, SecretError> {
    Ok(keyring::Entry::new(SERVICE_NAME, account_id)?)
}

/// Stores the Microsoft refresh token for this account, overwriting any
/// previous value.
pub fn store_refresh_token(account_id: &str, refresh_token: &str) -> Result<(), SecretError> {
    entry_for(account_id)?.set_password(refresh_token)?;
    Ok(())
}

pub fn get_refresh_token(account_id: &str) -> Result<Option<String>, SecretError> {
    match entry_for(account_id)?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(other) => Err(other.into()),
    }
}

pub fn delete_refresh_token(account_id: &str) -> Result<(), SecretError> {
    match entry_for(account_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(other) => Err(other.into()),
    }
}
