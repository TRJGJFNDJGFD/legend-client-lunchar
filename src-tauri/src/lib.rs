use legend_core::{accounts, auth, fabric, java, launch, mods, profiles, secrets, version};
use profiles::Profile;
use serde::Serialize;
use tauri::Emitter;

/// Public client ID from Azure App Registration ("Legend Client", personal
/// Microsoft accounts only, public client flows enabled). Client IDs for
/// native/public OAuth clients are not secrets — nothing else about the
/// login flow is embedded here.
const MS_CLIENT_ID: &str = "ccee12b7-63bc-4cd6-b602-e52c4d0660e7";

#[tauri::command]
fn detect_java() -> Vec<java::JavaInstall> {
    java::detect_all()
}

#[tauri::command]
fn list_profiles() -> Result<Vec<Profile>, String> {
    profiles::list().map_err(|e| e.to_string())
}

#[tauri::command]
fn create_profile(
    name: String,
    minecraft_version: String,
    loader: String,
    min_ram_mb: u32,
    max_ram_mb: u32,
) -> Result<Profile, String> {
    profiles::create(name, minecraft_version, loader, min_ram_mb, max_ram_mb)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_profile(id: String) -> Result<(), String> {
    profiles::delete(&id).map_err(|e| e.to_string())
}

/// Milestone 1 launch check: finds the right Java for this profile and runs
/// `java -version` through the real spawn/stream pipeline, emitting each
/// output line to the frontend as `launch-log` events live. This is the
/// exact code path the full Minecraft launch will use once the classpath
/// builder (version/asset download) lands — only the `args` vector changes.
#[tauri::command]
async fn launch_profile(app: tauri::AppHandle, profile_id: String) -> Result<launch::LaunchResult, String> {
    let profile = profiles::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| "Profile not found".to_string())?;

    let java_install = java::best_for(21)
        .or_else(|| java::detect_all().into_iter().next())
        .ok_or_else(|| "No Java installation found on this system".to_string())?;

    let app_for_events = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        launch::spawn_and_stream(&java_install, &profile, vec!["-version".to_string()], move |event| {
            let _ = app_for_events.emit("launch-log", event);
        })
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize, Clone)]
struct AuthCodeEvent {
    user_code: String,
    verification_uri: String,
    message: String,
    expires_in: u64,
}

#[derive(Serialize, Clone)]
struct AuthResultEvent {
    success: bool,
    error: Option<String>,
    account: Option<accounts::Account>,
}

#[tauri::command]
fn list_accounts() -> Result<Vec<accounts::Account>, String> {
    accounts::list().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_active_account(id: String) -> Result<(), String> {
    accounts::set_active(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_account(id: String) -> Result<(), String> {
    accounts::remove(&id).map_err(|e| e.to_string())
}

/// Starts the device-code login flow on a background thread and returns
/// immediately — the frontend listens for the `auth-device-code` event
/// (show this code/link to the user) and the terminal `auth-result` event
/// (success + account, or a real error message). This never blocks the UI
/// thread, per the "no frozen UI on long operations" rule.
#[tauri::command]
fn start_login(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let app_for_code = app.clone();
        let login_result = auth::login_via_device_code(MS_CLIENT_ID, move |device| {
            let _ = app_for_code.emit(
                "auth-device-code",
                AuthCodeEvent {
                    user_code: device.user_code.clone(),
                    verification_uri: device.verification_uri.clone(),
                    message: device.message.clone(),
                    expires_in: device.expires_in,
                },
            );
        });

        let event = match login_result {
            Ok(result) => {
                let store_result = secrets::store_refresh_token(&result.profile.id, &result.ms_refresh_token)
                    .map_err(|e| e.to_string())
                    .and_then(|_| accounts::upsert_from_profile(&result.profile).map_err(|e| e.to_string()));

                match store_result {
                    Ok(account) => AuthResultEvent {
                        success: true,
                        error: None,
                        account: Some(account),
                    },
                    Err(e) => AuthResultEvent {
                        success: false,
                        error: Some(e),
                        account: None,
                    },
                }
            }
            Err(e) => AuthResultEvent {
                success: false,
                error: Some(e.to_string()),
                account: None,
            },
        };

        let _ = app.emit("auth-result", event);
    });
}

fn find_profile(profile_id: &str) -> Result<Profile, String> {
    profiles::list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| "Profile not found".to_string())
}

#[tauri::command]
fn search_mods(query: String, minecraft_version: String, loader: String) -> Result<Vec<mods::ModSearchResult>, String> {
    mods::search(&query, &minecraft_version, &loader).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_mod_versions(project_id: String, minecraft_version: String, loader: String) -> Result<Vec<mods::ModVersion>, String> {
    mods::get_versions(&project_id, &minecraft_version, &loader).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_installed_mods(profile_id: String) -> Result<Vec<String>, String> {
    let profile = find_profile(&profile_id)?;
    mods::list_installed(&profile).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_installed_mod(profile_id: String, filename: String) -> Result<(), String> {
    let profile = find_profile(&profile_id)?;
    mods::remove_installed(&profile, &filename).map_err(|e| e.to_string())
}

/// Installs every file in a mod version (the primary jar, plus any others
/// Modrinth lists for that version) into the given profile's mods folder,
/// each one going through the real download+hash-verify pipeline.
#[tauri::command]
async fn install_mod(profile_id: String, version: mods::ModVersion) -> Result<Vec<String>, String> {
    let profile = find_profile(&profile_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut installed = Vec::new();
        for file in &version.files {
            let path = mods::download_and_install(&profile, file).map_err(|e| e.to_string())?;
            installed.push(path.file_name().unwrap().to_string_lossy().to_string());
        }
        Ok(installed)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Installs a mod version AND every `required` dependency Modrinth lists
/// for it (transitively), skipping anything already present in the mods
/// folder. Replaces the old "warn and let the user install it manually"
/// placeholder.
#[tauri::command]
async fn install_mod_with_dependencies(
    profile_id: String,
    project_id: String,
    version_id: String,
    minecraft_version: String,
    loader: String,
) -> Result<Vec<String>, String> {
    let profile = find_profile(&profile_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        mods::install_with_dependencies(&profile, &project_id, &version_id, &minecraft_version, &loader)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Copies a jar already on disk (e.g. the built Legend Client Fabric mod)
/// into a profile's mods folder.
#[tauri::command]
fn install_local_mod_jar(profile_id: String, source_path: String) -> Result<String, String> {
    let profile = find_profile(&profile_id)?;
    mods::install_local_jar(&profile, std::path::Path::new(&source_path))
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
}

/// Resolves the newest stable Fabric loader for a Minecraft version —
/// loader version, main class, and the library jars the classpath builder
/// in `launch.rs` will need to download.
#[tauri::command]
async fn resolve_fabric_loader(minecraft_version: String) -> Result<fabric::ResolvedFabricLoader, String> {
    tauri::async_runtime::spawn_blocking(move || {
        fabric::resolve_stable_loader(&minecraft_version).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Resolves a profile's Minecraft version against Mojang's manifest,
/// downloads the client jar + libraries (hash-verified, cached), and
/// returns the real `java` argument list ready to spawn — the classpath
/// half of ARCHITECTURE.md §6. Vanilla-only for now: a Fabric profile
/// still needs `resolve_fabric_loader`'s libraries merged in before this
/// is a complete Fabric launch command.
#[tauri::command]
async fn prepare_launch(profile_id: String) -> Result<Vec<String>, String> {
    let profile = find_profile(&profile_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let resolved = version::resolve_version(&profile.minecraft_version).map_err(|e| e.to_string())?;
        let jar_paths = version::download_all(&resolved).map_err(|e| e.to_string())?;
        let classpath = version::build_classpath(&jar_paths);
        Ok(launch::build_launch_args(&profile, &classpath, &resolved.main_class))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            detect_java,
            list_profiles,
            create_profile,
            delete_profile,
            launch_profile,
            list_accounts,
            set_active_account,
            remove_account,
            start_login,
            search_mods,
            get_mod_versions,
            list_installed_mods,
            remove_installed_mod,
            install_mod,
            install_mod_with_dependencies,
            install_local_mod_jar,
            resolve_fabric_loader,
            prepare_launch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
