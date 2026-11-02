import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type VersionSummary = { id: string; release: boolean; releaseTime: string };
export type VersionList = { latestRelease: string; versions: VersionSummary[] };

export type Account = { name: string; uuid: string; kind: "offline" | "ely" };
export type Accounts = { accounts: Account[]; active: string | null; offlineAllowed: boolean };

export type Instance = {
  name: string;
  gameVersion: string;
  loader: string | null;
  loaderVersion: string | null;
  extraJvmArgs: string;
  maxMemoryMb: number;
  cover: string | null;
  lastPlayed: number | null;
  running: boolean;
};

export type LoaderKind = "fabric" | "quilt" | "forge" | "neoforge";
export type LoaderVersion = { id: string; recommended: boolean };

export type NewInstance = {
  name: string;
  gameVersion: string;
  cover: string | null;
  maxMemoryMb: number;
  loader: { kind: LoaderKind; version: string } | null;
  extraJvmArgs: string;
};

export type ProjectType = "mod" | "modpack" | "resourcepack" | "shader" | "datapack";
export type ModHit = {
  projectId: string;
  slug: string | null;
  title: string;
  description: string;
  author: string;
  iconUrl: string | null;
  downloads: number;
  categories: string[];
};
export type SearchResults = { hits: ModHit[]; totalHits: number };
export type InstalledMod = {
  filename: string;
  title: string;
  version: string | null;
  projectId: string | null;
  enabled: boolean;
  dependency: boolean;
  managed: boolean;
};
export type InstanceChanges = { name?: string; maxMemoryMb?: number; cover?: string };

export type AppInfo = { version: string; dataDir: string; devBuild: boolean };

/** Mirrors `glint_launch::PlayEvent`, plus `failed` emitted by the desktop layer. */
export type PlayEvent =
  | { type: "resolving"; version: string }
  | {
      type: "downloading";
      what: "game" | "java";
      doneFiles: number;
      totalFiles: number;
      doneBytes: number;
      totalBytes: number;
    }
  | { type: "installingLoader"; name: string }
  | { type: "preparing" }
  | { type: "started"; pid: number | null }
  | { type: "log"; level: string; message: string }
  | { type: "exited"; code: number | null }
  | { type: "failed"; message: string }
  | { type: "killed" };

export type PlayEnvelope = PlayEvent & { instance: string };

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  versions: () => invoke<VersionList>("versions"),

  accounts: () => invoke<Accounts>("accounts"),
  addOfflineAccount: (name: string) => invoke<Accounts>("add_offline_account", { name }),
  setActiveAccount: (uuid: string) => invoke<Accounts>("set_active_account", { uuid }),
  removeAccount: (uuid: string) => invoke<Accounts>("remove_account", { uuid }),
  /** Rejects with "TWO_FACTOR_REQUIRED" when an authenticator code is needed. */
  elySignIn: (login: string, password: string, totp?: string) =>
    invoke<Accounts>("ely_sign_in", { login, password, totp: totp || null }),

  instances: () => invoke<Instance[]>("instances"),
  createInstance: (instance: NewInstance) => invoke<string>("create_instance", { instance }),
  updateInstance: (name: string, changes: InstanceChanges) => invoke<string>("update_instance", { name, changes }),
  deleteInstance: (name: string) => invoke<void>("delete_instance", { name }),
  openInstanceFolder: (name: string) => invoke<void>("open_instance_folder", { name }),

  loaderVersions: (kind: LoaderKind, gameVersion: string) =>
    invoke<LoaderVersion[]>("loader_versions", { kind, gameVersion }),

  searchMods: (instance: string | null, query: string, offset = 0, projectType: ProjectType = "mod") =>
    invoke<SearchResults>("search_mods", { instance, query, offset, projectType }),
  installMod: (instance: string, projectId: string) => invoke<string[]>("install_mod", { instance, projectId }),
  installedMods: (instance: string) => invoke<InstalledMod[]>("installed_mods", { instance }),
  removeMod: (instance: string, filename: string) => invoke<void>("remove_mod", { instance, filename }),
  setModEnabled: (instance: string, filename: string, enabled: boolean) =>
    invoke<void>("set_mod_enabled", { instance, filename, enabled }),

  play: (instance: string, account?: string) => invoke<void>("play", { instance, account }),
  killInstance: (name: string) => invoke<void>("kill_instance", { name }),
  onPlayEvent: (handler: (event: PlayEnvelope) => void): Promise<UnlistenFn> =>
    listen<PlayEnvelope>("play", (e) => handler(e.payload)),
};
