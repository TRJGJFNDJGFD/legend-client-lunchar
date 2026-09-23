import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface JavaInstall {
  path: string;
  version: string;
  major: number;
  is_64bit: boolean;
}

export interface Profile {
  id: string;
  name: string;
  minecraft_version: string;
  loader: string;
  min_ram_mb: number;
  max_ram_mb: number;
  java_path: string | null;
  created_at: string;
  last_played: string | null;
}

export interface LaunchEvent {
  profile_id: string;
  line: string;
  stream: "stdout" | "stderr";
}

export interface LaunchResult {
  exit_code: number | null;
  crashed: boolean;
}

export interface Account {
  id: string;
  username: string;
  skin_url: string | null;
  is_active: boolean;
}

export interface AuthCodeEvent {
  user_code: string;
  verification_uri: string;
  message: string;
  expires_in: number;
}

export interface AuthResultEvent {
  success: boolean;
  error: string | null;
  account: Account | null;
}

export interface ModSearchResult {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  icon_url: string | null;
  downloads: number;
}

export interface ModFile {
  filename: string;
  url: string;
  sha1: string;
  primary: boolean;
}

export interface ModDependency {
  project_id: string | null;
  dependency_type: string;
}

export interface ModVersion {
  version_id: string;
  project_id: string;
  version_number: string;
  game_versions: string[];
  loaders: string[];
  files: ModFile[];
  dependencies: ModDependency[];
}

export const api = {
  detectJava: () => invoke<JavaInstall[]>("detect_java"),

  listProfiles: () => invoke<Profile[]>("list_profiles"),

  createProfile: (input: {
    name: string;
    minecraft_version: string;
    loader: string;
    min_ram_mb: number;
    max_ram_mb: number;
  }) => invoke<Profile>("create_profile", input),

  deleteProfile: (id: string) => invoke<void>("delete_profile", { id }),

  launchProfile: (profileId: string) =>
    invoke<LaunchResult>("launch_profile", { profileId }),

  onLaunchLog: (callback: (event: LaunchEvent) => void) =>
    listen<LaunchEvent>("launch-log", (e) => callback(e.payload)),

  listAccounts: () => invoke<Account[]>("list_accounts"),

  setActiveAccount: (id: string) => invoke<void>("set_active_account", { id }),

  removeAccount: (id: string) => invoke<void>("remove_account", { id }),

  startLogin: () => invoke<void>("start_login"),

  onAuthDeviceCode: (callback: (event: AuthCodeEvent) => void) =>
    listen<AuthCodeEvent>("auth-device-code", (e) => callback(e.payload)),

  onAuthResult: (callback: (event: AuthResultEvent) => void) =>
    listen<AuthResultEvent>("auth-result", (e) => callback(e.payload)),

  searchMods: (query: string, minecraftVersion: string, loader: string) =>
    invoke<ModSearchResult[]>("search_mods", { query, minecraftVersion, loader }),

  getModVersions: (projectId: string, minecraftVersion: string, loader: string) =>
    invoke<ModVersion[]>("get_mod_versions", { projectId, minecraftVersion, loader }),

  listInstalledMods: (profileId: string) =>
    invoke<string[]>("list_installed_mods", { profileId }),

  removeInstalledMod: (profileId: string, filename: string) =>
    invoke<void>("remove_installed_mod", { profileId, filename }),

  installMod: (profileId: string, version: ModVersion) =>
    invoke<string[]>("install_mod", { profileId, version }),

  installModWithDependencies: (input: {
    profileId: string;
    projectId: string;
    versionId: string;
    minecraftVersion: string;
    loader: string;
  }) => invoke<string[]>("install_mod_with_dependencies", input),

  installLocalModJar: (profileId: string, sourcePath: string) =>
    invoke<string>("install_local_mod_jar", { profileId, sourcePath }),

  resolveFabricLoader: (minecraftVersion: string) =>
    invoke<ResolvedFabricLoader>("resolve_fabric_loader", { minecraftVersion }),
};

export interface FabricLibrary {
  maven_coordinate: string;
  url: string;
}

export interface ResolvedFabricLoader {
  loader_version: string;
  main_class: string;
  libraries: FabricLibrary[];
}
