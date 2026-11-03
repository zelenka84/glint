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
export type ContentKind = "mod" | "resourcepack" | "shader";
export type InstalledMod = {
  filename: string;
  title: string;
  version: string | null;
  projectId: string | null;
  iconUrl: string | null;
  enabled: boolean;
  dependency: boolean;
  managed: boolean;
  size: number;
};
export type ContentUpdate = { projectId: string; filename: string; title: string; current: string; latest: string };
export type InstanceChanges = { name?: string; maxMemoryMb?: number; cover?: string };

export type World = {
  folder: string;
  name: string;
  gameMode: string;
  hardcore: boolean;
  cheats: boolean;
  lastPlayed: number | null;
  version: string | null;
  seed: number | null;
  size: number;
  icon: string | null;
};
export type ServerResources = "prompt" | "enabled" | "disabled";
export type Server = { name: string; address: string; icon: string | null; resources: ServerResources };
export type MotdSpan = { text: string; color: string | null; bold: boolean; italic: boolean };
export type ServerStatus = {
  version: string;
  protocol: number;
  online: number;
  max: number;
  sample: string[];
  motd: MotdSpan[];
  favicon: string | null;
  latencyMs: number;
};
export type Screenshot = { name: string; path: string; size: number; taken: number };
export type InstanceSettings = {
  name: string;
  cover: string | null;
  maxMemoryMb: number;
  minMemoryMb: number | null;
  jvmArgs: string;
  javaPath: string | null;
  width: number | null;
  height: number | null;
  fullscreen: boolean;
  autoJoinServer: string | null;
  autoJoinWorld: string | null;
  preLaunch: string | null;
  wrapper: string | null;
  postExit: string | null;
  env: [string, string][];
};
export type JavaInstall = { path: string; version: string | null; source: string };
export type LogFile = { id: string; name: string; kind: "log" | "crash"; size: number; modified: number };
export type PlayTarget = { world?: string; server?: string };

export type AppInfo = { version: string; dataDir: string; channel: "dev" | "beta" | "public"; experimental: boolean };

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

  searchContent: (instance: string | null, kind: ContentKind, query: string, offset = 0) =>
    invoke<SearchResults>("search_content", { instance, kind, query, offset }),
  installContent: (instance: string, kind: ContentKind, projectId: string) =>
    invoke<string[]>("install_content", { instance, kind, projectId }),
  updateContent: (instance: string, kind: ContentKind, projectId: string) =>
    invoke<string[]>("update_content", { instance, kind, projectId }),
  contentUpdates: (instance: string, kind: ContentKind) => invoke<ContentUpdate[]>("content_updates", { instance, kind }),
  installedContent: (instance: string, kind: ContentKind) =>
    invoke<InstalledMod[]>("installed_content", { instance, kind }),
  addContentFiles: (instance: string, kind: ContentKind, paths: string[]) =>
    invoke<string[]>("add_content_files", { instance, kind, paths }),
  removeContent: (instance: string, kind: ContentKind, filename: string) =>
    invoke<void>("remove_content", { instance, kind, filename }),
  setContentEnabled: (instance: string, kind: ContentKind, filename: string, enabled: boolean) =>
    invoke<void>("set_content_enabled", { instance, kind, filename, enabled }),
  openContentFolder: (instance: string, kind: ContentKind) => invoke<void>("open_content_folder", { instance, kind }),

  listWorlds: (instance: string) => invoke<World[]>("list_worlds", { instance }),
  renameWorld: (instance: string, folder: string, name: string) => invoke<void>("rename_world", { instance, folder, name }),
  duplicateWorld: (instance: string, folder: string) => invoke<string>("duplicate_world", { instance, folder }),
  backupWorld: (instance: string, folder: string) => invoke<string>("backup_world", { instance, folder }),
  deleteWorld: (instance: string, folder: string) => invoke<void>("delete_world", { instance, folder }),
  openWorldFolder: (instance: string, folder?: string) => invoke<void>("open_world_folder", { instance, folder: folder ?? null }),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  listServers: (instance: string) => invoke<Server[]>("list_servers", { instance }),
  saveServers: (instance: string, servers: Server[]) => invoke<void>("save_servers", { instance, servers }),
  pingServer: (address: string) => invoke<ServerStatus>("ping_server", { address }),
  listScreenshots: (instance: string) => invoke<Screenshot[]>("list_screenshots", { instance }),
  renameScreenshot: (instance: string, name: string, newName: string) =>
    invoke<string>("rename_screenshot", { instance, name, newName }),
  deleteScreenshot: (instance: string, name: string) => invoke<void>("delete_screenshot", { instance, name }),
  openScreenshot: (instance: string, name: string) => invoke<void>("open_screenshot", { instance, name }),
  instanceSettings: (name: string) => invoke<InstanceSettings>("instance_settings", { name }),
  saveInstanceSettings: (name: string, settings: InstanceSettings) =>
    invoke<string>("save_instance_settings", { name, settings }),
  detectJava: () => invoke<JavaInstall[]>("detect_java"),
  checkJava: (path: string) => invoke<string>("check_java", { path }),
  listLogFiles: (instance: string) => invoke<LogFile[]>("list_log_files", { instance }),
  readLogFile: (instance: string, id: string) => invoke<string>("read_log_file", { instance, id }),
  deleteLogFile: (instance: string, id: string) => invoke<void>("delete_log_file", { instance, id }),
  deleteOldLogs: (instance: string) => invoke<number>("delete_old_logs", { instance }),
  readNotes: (instance: string) => invoke<string>("read_notes", { instance }),
  saveNotes: (instance: string, text: string) => invoke<void>("save_notes", { instance, text }),
  changeVersion: (name: string, gameVersion: string, loader: { kind: LoaderKind; version: string } | null) =>
    invoke<void>("change_version", { name, gameVersion, loader }),
  openLibrariesFolder: () => invoke<void>("open_libraries_folder"),
  openGameFolder: (instance: string, folder: string) => invoke<void>("open_game_subfolder", { instance, folder }),

  loadAppearance: () => invoke<{ data: unknown; mediaDir: string }>("load_appearance"),
  saveAppearance: (data: unknown) => invoke<void>("save_appearance", { data }),
  /** Copies an image into Glint's media folder; resolves to a reference like `covers/17.png`. */
  importImage: async (kind: "background" | "cover", file: Blob) =>
    invoke<string>("import_image", new Uint8Array(await file.arrayBuffer()), { headers: { "x-glint-kind": kind } }),
  deleteMedia: (image: string) => invoke<void>("delete_media", { image }),
  exportTheme: (name: string, contents: string) => invoke<string>("export_theme", { name, contents }),

  play: (instance: string, target?: PlayTarget) => invoke<void>("play", { instance, target: target ?? null }),
  killInstance: (name: string) => invoke<void>("kill_instance", { name }),
  onPlayEvent: (handler: (event: PlayEnvelope) => void): Promise<UnlistenFn> =>
    listen<PlayEnvelope>("play", (e) => handler(e.payload)),
};
