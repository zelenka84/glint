import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type VersionSummary = { id: string; release: boolean; releaseTime: string };
export type VersionList = { latestRelease: string; versions: VersionSummary[] };

export type Account = { name: string; uuid: string; kind: "offline" };
export type Accounts = { accounts: Account[]; active: string | null; offlineAllowed: boolean };

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
  | { type: "preparing" }
  | { type: "started"; pid: number | null }
  | { type: "log"; level: string; message: string }
  | { type: "exited"; code: number | null }
  | { type: "failed"; message: string };

export const api = {
  versions: () => invoke<VersionList>("versions"),
  accounts: () => invoke<Accounts>("accounts"),
  addOfflineAccount: (name: string) => invoke<Accounts>("add_offline_account", { name }),
  setActiveAccount: (uuid: string) => invoke<Accounts>("set_active_account", { uuid }),
  removeAccount: (uuid: string) => invoke<Accounts>("remove_account", { uuid }),
  play: (version: string) => invoke<void>("play", { version }),
  onPlayEvent: (handler: (event: PlayEvent) => void): Promise<UnlistenFn> =>
    listen<PlayEvent>("play", (e) => handler(e.payload)),
};
