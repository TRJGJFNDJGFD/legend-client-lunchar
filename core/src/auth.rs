//! Real Microsoft OAuth device-code flow → Xbox Live → XSTS → Minecraft
//! Services, per Microsoft's documented flow. No password ever touches
//! this code — the device-code flow only ever sees short-lived tokens.
//!
//! Reachability note (see README): `login.microsoftonline.com` is reachable
//! from the dev sandbox this was written in, but `user.auth.xboxlive.com`,
//! `xsts.auth.xboxlive.com`, and `api.minecraftservices.com` are blocked by
//! that sandbox's network policy. Steps 1-2 below are verified against the
//! real endpoint with a real client ID; steps 3-5 are written to the same
//! documented API contract but could not be exercised end-to-end here —
//! verify on a machine with normal internet access before shipping.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBOX_LIVE_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MINECRAFT_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("network request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("authorization pending — keep polling")]
    Pending,
    #[error("the device code expired before the user signed in")]
    Expired,
    #[error("Microsoft account has no Xbox profile, or Minecraft is not owned on this account")]
    NoMinecraftOwnership,
    #[error("unexpected response from {0}: {1}")]
    UnexpectedResponse(&'static str, String),
}

#[derive(Deserialize, Debug, Clone)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub struct MsTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Deserialize, Debug)]
struct MsTokenErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct XblAuthRequest<'a> {
    #[serde(rename = "Properties")]
    properties: XblAuthProperties<'a>,
    #[serde(rename = "RelyingParty")]
    relying_party: &'static str,
    #[serde(rename = "TokenType")]
    token_type: &'static str,
}

#[derive(Serialize)]
struct XblAuthProperties<'a> {
    #[serde(rename = "AuthMethod")]
    auth_method: &'static str,
    #[serde(rename = "SiteName")]
    site_name: &'static str,
    #[serde(rename = "RpsTicket")]
    rps_ticket: String,
    #[serde(skip)]
    _marker: std::marker::PhantomData<&'a ()>,
}

#[derive(Deserialize, Debug)]
struct XblAuthResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XblDisplayClaims,
}

#[derive(Deserialize, Debug)]
struct XblDisplayClaims {
    xui: Vec<std::collections::HashMap<String, String>>,
}

#[derive(Serialize)]
struct XstsAuthRequest<'a> {
    #[serde(rename = "Properties")]
    properties: XstsProperties<'a>,
    #[serde(rename = "RelyingParty")]
    relying_party: &'static str,
    #[serde(rename = "TokenType")]
    token_type: &'static str,
}

#[derive(Serialize)]
struct XstsProperties<'a> {
    #[serde(rename = "SandboxId")]
    sandbox_id: &'static str,
    #[serde(rename = "UserTokens")]
    user_tokens: Vec<&'a str>,
}

#[derive(Serialize)]
struct MinecraftLoginRequest {
    identity_token: String,
}

#[derive(Deserialize, Debug)]
struct MinecraftLoginResponse {
    access_token: String,
    #[allow(dead_code)] // part of the documented response shape; not needed yet
    expires_in: u64,
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
    pub skins: Vec<MinecraftSkin>,
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct MinecraftSkin {
    pub id: String,
    pub url: String,
    pub state: String,
}

/// The full result of a successful login: the account's public profile
/// plus the tokens needed to launch the game and to refresh later.
/// `minecraft_access_token` and `ms_refresh_token` are secrets — callers
/// must hand these to `secrets::store`, never `accounts.json`.
pub struct LoginResult {
    pub profile: MinecraftProfile,
    pub minecraft_access_token: String,
    pub ms_refresh_token: String,
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent("LegendClient/0.1")
        .build()
        .expect("reqwest client should always build")
}

/// Step 1: ask Microsoft for a device code. Show `user_code` and
/// `verification_uri` to the user (e.g. "go to microsoft.com/link and
/// enter ABCD-1234").
pub fn request_device_code(client_id: &str) -> Result<DeviceCodeResponse, AuthError> {
    let response = client()
        .post(DEVICE_CODE_URL)
        .form(&[
            ("client_id", client_id),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()?;

    let status = response.status();
    let text = response.text()?;
    if !status.is_success() {
        return Err(AuthError::UnexpectedResponse("devicecode", text));
    }

    serde_json::from_str(&text).map_err(|_| AuthError::UnexpectedResponse("devicecode", text))
}

/// Step 2: poll for the user having completed sign-in in their browser.
/// Callers should sleep `device_code.interval` seconds between calls to
/// this, and stop after `device_code.expires_in` seconds total.
fn poll_once(client_id: &str, device_code: &str) -> Result<MsTokenResponse, AuthError> {
    let response = client()
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", device_code),
        ])
        .send()?;

    if response.status().is_success() {
        return Ok(response.json()?);
    }

    let text = response.text()?;
    if let Ok(err) = serde_json::from_str::<MsTokenErrorResponse>(&text) {
        match err.error.as_str() {
            "authorization_pending" | "slow_down" => return Err(AuthError::Pending),
            "expired_token" | "code_expired" => return Err(AuthError::Expired),
            _ => {}
        }
    }
    Err(AuthError::UnexpectedResponse("token", text))
}

/// Blocks the calling thread, polling until the user finishes sign-in or
/// the code expires. Intended to run on a background thread (Tauri
/// `spawn_blocking`), same pattern as `launch::spawn_and_stream`.
pub fn wait_for_token(device: &DeviceCodeResponse, client_id: &str) -> Result<MsTokenResponse, AuthError> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(device.expires_in);
    loop {
        if std::time::Instant::now() > deadline {
            return Err(AuthError::Expired);
        }
        match poll_once(client_id, &device.device_code) {
            Ok(token) => return Ok(token),
            Err(AuthError::Pending) => {
                std::thread::sleep(std::time::Duration::from_secs(device.interval));
            }
            Err(other) => return Err(other),
        }
    }
}

fn authenticate_xbox_live(ms_access_token: &str) -> Result<XblAuthResponse, AuthError> {
    let body = XblAuthRequest {
        properties: XblAuthProperties {
            auth_method: "RPS",
            site_name: "user.auth.xboxlive.com",
            rps_ticket: format!("d={}", ms_access_token),
            _marker: std::marker::PhantomData,
        },
        relying_party: "http://auth.xboxlive.com",
        token_type: "JWT",
    };

    let response = client().post(XBOX_LIVE_AUTH_URL).json(&body).send()?;
    if !response.status().is_success() {
        return Err(AuthError::UnexpectedResponse("xbl authenticate", response.text()?));
    }
    Ok(response.json()?)
}

fn authenticate_xsts(xbl_token: &str) -> Result<(String, String), AuthError> {
    let body = XstsAuthRequest {
        properties: XstsProperties {
            sandbox_id: "RETAIL",
            user_tokens: vec![xbl_token],
        },
        relying_party: "rp://api.minecraftservices.com/",
        token_type: "JWT",
    };

    let response = client().post(XSTS_AUTH_URL).json(&body).send()?;
    if !response.status().is_success() {
        return Err(AuthError::UnexpectedResponse("xsts authorize", response.text()?));
    }
    let parsed: XblAuthResponse = response.json()?;
    let user_hash = parsed
        .display_claims
        .xui
        .first()
        .and_then(|claims| claims.get("uhs"))
        .cloned()
        .ok_or_else(|| AuthError::UnexpectedResponse("xsts authorize", "missing uhs claim".into()))?;
    Ok((parsed.token, user_hash))
}

fn login_with_xbox(xsts_token: &str, user_hash: &str) -> Result<MinecraftLoginResponse, AuthError> {
    let body = MinecraftLoginRequest {
        identity_token: format!("XBL3.0 x={};{}", user_hash, xsts_token),
    };

    let response = client().post(MINECRAFT_LOGIN_URL).json(&body).send()?;
    if !response.status().is_success() {
        return Err(AuthError::UnexpectedResponse("minecraft login", response.text()?));
    }
    Ok(response.json()?)
}

fn fetch_profile(minecraft_access_token: &str) -> Result<MinecraftProfile, AuthError> {
    let response = client()
        .get(MINECRAFT_PROFILE_URL)
        .bearer_auth(minecraft_access_token)
        .send()?;

    match response.status().as_u16() {
        200 => Ok(response.json()?),
        404 => Err(AuthError::NoMinecraftOwnership),
        _ => Err(AuthError::UnexpectedResponse("minecraft profile", response.text()?)),
    }
}

/// Runs the full chain after the Microsoft device-code step already
/// produced an access token: Xbox Live -> XSTS -> Minecraft Services ->
/// profile. This is the part that could not be exercised in the dev
/// sandbox (see module doc) — the shape matches Microsoft/Mojang's
/// documented contract but treat it as unverified until run for real.
pub fn complete_minecraft_login(ms_access_token: &str, ms_refresh_token: &str) -> Result<LoginResult, AuthError> {
    let xbl = authenticate_xbox_live(ms_access_token)?;
    let (xsts_token, user_hash) = authenticate_xsts(&xbl.token)?;
    let mc_login = login_with_xbox(&xsts_token, &user_hash)?;
    let profile = fetch_profile(&mc_login.access_token)?;

    Ok(LoginResult {
        profile,
        minecraft_access_token: mc_login.access_token,
        ms_refresh_token: ms_refresh_token.to_string(),
    })
}

/// Full login: request a device code, hand it to `on_code` so the caller
/// can show it to the user, block until sign-in completes, then run the
/// Xbox Live/XSTS/Minecraft chain. This is what the `login_start` Tauri
/// command calls on a background thread.
pub fn login_via_device_code(
    client_id: &str,
    on_code: impl FnOnce(&DeviceCodeResponse),
) -> Result<LoginResult, AuthError> {
    let device = request_device_code(client_id)?;
    on_code(&device);
    let ms_token = wait_for_token(&device, client_id)?;
    complete_minecraft_login(&ms_token.access_token, &ms_token.refresh_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one thing about this module that IS verifiable in the sandbox
    /// this was written in: a real device-code request against the real
    /// Microsoft endpoint, with a real client ID. Ignored by default so
    /// `cargo test` doesn't require network/a live client ID in CI; run
    /// explicitly with `cargo test -- --ignored` and
    /// LEGEND_TEST_CLIENT_ID set.
    #[test]
    #[ignore]
    fn requests_a_real_device_code() {
        let client_id = std::env::var("LEGEND_TEST_CLIENT_ID")
            .expect("set LEGEND_TEST_CLIENT_ID to run this test");
        let device = request_device_code(&client_id).expect("device code request should succeed");
        assert!(!device.user_code.is_empty());
        assert!(device.verification_uri.contains("microsoft.com"));
    }
}
