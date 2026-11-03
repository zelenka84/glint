import { useCallback, useEffect, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, type Instance, type PlayTarget, type World } from "../api";
import { B3D, Icon, Modal, formatBytes } from "../ui";
import { useCopied } from "./LogView";
import { EmptyArt } from "../EmptyArt";

function timeAgo(ms: number | null): string {
  if (!ms) return "never";
  const minutes = Math.round((Date.now() - ms) / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  const days = Math.round(hours / 24);
  return days === 1 ? "yesterday" : days < 60 ? `${days} days ago` : new Date(ms).toLocaleDateString();
}

/** Quick Play into a world exists from 1.20; older versions open the title screen. */
function supportsQuickPlay(version: string): boolean {
  const m = /^1\.(\d+)/.exec(version);
  return !m || Number(m[1]) >= 20;
}

export function WorldIcon({ path, size = 64 }: { path: string | null; size?: number }) {
  const [broken, setBroken] = useState(false);
  return path && !broken ? (
    <img className="world-icon px2" style={{ width: size, height: size }} src={convertFileSrc(path)} alt="" onError={() => setBroken(true)} />
  ) : (
    <span className="world-icon placeholder px2" style={{ width: size, height: size }} aria-hidden="true">
      <Icon name="globe" size={Math.round(size / 2.2)} />
    </span>
  );
}

export function WorldsTab({ instance, canPlay, onPlay }: { instance: Instance; canPlay: boolean; onPlay: (target: PlayTarget) => void }) {
  const [worlds, setWorlds] = useState<World[] | null>(null);
  const [filter, setFilter] = useState("");
  const [busy, setBusy] = useState<Record<string, string>>({});
  const [notice, setNotice] = useState<{ tone: "ok" | "bad"; text: string; reveal?: string } | null>(null);
  const [renaming, setRenaming] = useState<{ folder: string; name: string } | null>(null);
  const [deleting, setDeleting] = useState<World | null>(null);
  const [copied, copy] = useCopied();

  const refresh = useCallback(() => {
    api.listWorlds(instance.name).then(setWorlds, (e) => setNotice({ tone: "bad", text: String(e) }));
  }, [instance.name]);
  useEffect(refresh, [refresh]);

  const shown = useMemo(
    () => (worlds ?? []).filter((w) => `${w.name} ${w.folder}`.toLowerCase().includes(filter.toLowerCase())),
    [worlds, filter],
  );
  const quickPlay = supportsQuickPlay(instance.gameVersion);

  async function run(world: World, label: string, action: () => Promise<unknown>, done?: (result: unknown) => void) {
    setBusy((b) => ({ ...b, [world.folder]: label }));
    try {
      const result = await action();
      done?.(result);
      refresh();
    } catch (e) {
      setNotice({ tone: "bad", text: String(e) });
    } finally {
      setBusy((b) => {
        const { [world.folder]: _, ...rest } = b;
        return rest;
      });
    }
  }

  return (
    <div className="tab-pane">
      <div className="pane-head">
        <h2 className="pix">Worlds</h2>
        {worlds && <span className="count px2">{worlds.length}</span>}
        <div className="grow" />
        <button className="tool-btn px2" onClick={refresh}>
          <Icon name="refresh" /> Refresh
        </button>
        <button className="tool-btn px2" onClick={() => api.openWorldFolder(instance.name)}>
          <Icon name="folder" /> Saves folder
        </button>
      </div>

      {notice && (
        <div className={`notice px2 ${notice.tone === "ok" ? "ok" : ""}`} role="status">
          <Icon name={notice.tone === "ok" ? "check" : "warn"} />
          <span className="grow">{notice.text}</span>
          {notice.reveal && (
            <button className="link" onClick={() => api.revealPath(notice.reveal!)}>
              Show
            </button>
          )}
          <button className="icon-btn" aria-label="Dismiss" onClick={() => setNotice(null)}>
            <Icon name="close" size={12} />
          </button>
        </div>
      )}

      {worlds === null ? (
        <div className="skeleton-list">{[0, 1].map((i) => <div key={i} className="skeleton tall px4" />)}</div>
      ) : worlds.length === 0 ? (
        <div className="empty px4">
          <EmptyArt art="map" />
          <p className="pix">No worlds yet</p>
          <p className="muted">Create one in game, or copy a world folder into the saves folder.</p>
        </div>
      ) : (
        <>
          {worlds.length > 4 && (
            <label className="search slim px2">
              <Icon name="search" />
              <input value={filter} placeholder={`Filter ${worlds.length} worlds`} onChange={(e) => setFilter(e.target.value)} />
            </label>
          )}
          <div className="world-list">
            {shown.map((w) => (
              <article key={w.folder} className={`world-card px4 ${busy[w.folder] ? "busy" : ""}`}>
                <WorldIcon path={w.icon} />
                <div className="world-main">
                  {renaming?.folder === w.folder ? (
                    <form
                      className="rename-row"
                      onSubmit={(e) => {
                        e.preventDefault();
                        const name = renaming.name;
                        setRenaming(null);
                        run(w, "Renaming", () => api.renameWorld(instance.name, w.folder, name));
                      }}
                    >
                      <input className="input px2" autoFocus value={renaming.name} onChange={(e) => setRenaming({ ...renaming, name: e.target.value })} />
                      <button className="tool-btn accent px2" type="submit">Save</button>
                      <button className="tool-btn px2" type="button" onClick={() => setRenaming(null)}>Cancel</button>
                    </form>
                  ) : (
                    <div className="world-title">
                      <b>{w.name}</b>
                      {w.folder !== w.name && <span className="muted small mono">{w.folder}</span>}
                    </div>
                  )}
                  <div className="world-meta">
                    <span className={`mode-tag px2 ${w.hardcore ? "hardcore" : w.gameMode.toLowerCase()}`}>
                      {w.hardcore ? "Hardcore" : w.gameMode}
                    </span>
                    {w.cheats && <span className="tag-muted px2">cheats</span>}
                    {w.version && <span className="tag-muted px2">{w.version}</span>}
                    <span className="muted small">Played {timeAgo(w.lastPlayed)}</span>
                    <span className="muted small">· {formatBytes(w.size)}</span>
                  </div>
                  <div className="world-actions">
                    <button className="tool-btn px2" disabled={!!busy[w.folder]} onClick={() => run(w, "Backing up", () => api.backupWorld(instance.name, w.folder), (p) => setNotice({ tone: "ok", text: `Backed up ${w.name}`, reveal: String(p) }))}>
                      {busy[w.folder] === "Backing up" ? <span className="spinner small" /> : <Icon name="archive" />} Backup
                    </button>
                    <button className="tool-btn px2" disabled={!!busy[w.folder] || instance.running} onClick={() => run(w, "Copying", () => api.duplicateWorld(instance.name, w.folder), () => setNotice({ tone: "ok", text: `Copied ${w.name}` }))}>
                      {busy[w.folder] === "Copying" ? <span className="spinner small" /> : <Icon name="copy" />} Duplicate
                    </button>
                    <button className="tool-btn px2" disabled={!!busy[w.folder] || instance.running} onClick={() => setRenaming({ folder: w.folder, name: w.name })}>
                      <Icon name="edit" /> Rename
                    </button>
                    {w.seed !== null && (
                      <button className="tool-btn px2" onClick={() => copy(String(w.seed))} title={String(w.seed)}>
                        <Icon name={copied ? "check" : "seed"} /> Seed
                      </button>
                    )}
                    <button className="tool-btn px2" onClick={() => api.openWorldFolder(instance.name, w.folder)}>
                      <Icon name="folder" />
                    </button>
                    <button className="tool-btn danger px2" disabled={!!busy[w.folder] || instance.running} onClick={() => setDeleting(w)} aria-label={`Delete ${w.name}`}>
                      <Icon name="trash" />
                    </button>
                  </div>
                </div>
                <B3D
                  variant="play"
                  size="sm"
                  disabled={!canPlay || instance.running || !quickPlay}
                  onClick={() => onPlay({ world: w.folder })}
                  label={quickPlay ? `Play ${w.name}` : "Opening a world directly needs Minecraft 1.20 or newer"}
                >
                  <Icon name="play" size={12} />
                  <span className="pix">Play</span>
                </B3D>
              </article>
            ))}
          </div>
        </>
      )}

      {deleting && (
        <Modal title={`Delete ${deleting.name}?`} onClose={() => setDeleting(null)}>
          <div className="form">
            <p className="warning-box px2">
              The world folder is deleted for good: buildings, inventory and progress. Make a backup first if you might
              want it back.
            </p>
            <div className="modal-actions">
              <B3D onClick={() => setDeleting(null)}>Cancel</B3D>
              <B3D
                onClick={() => {
                  const w = deleting;
                  setDeleting(null);
                  run(w, "Backing up", () => api.backupWorld(instance.name, w.folder), (p) => setNotice({ tone: "ok", text: `Backed up ${w.name}`, reveal: String(p) }));
                }}
              >
                <Icon name="archive" />
                <span className="pix">Backup instead</span>
              </B3D>
              <B3D
                variant="danger"
                onClick={() => {
                  const w = deleting;
                  setDeleting(null);
                  run(w, "Deleting", () => api.deleteWorld(instance.name, w.folder), () => setNotice({ tone: "ok", text: `Deleted ${w.name}` }));
                }}
              >
                <span className="pix">Delete</span>
              </B3D>
            </div>
          </div>
        </Modal>
      )}
    </div>
  );
}
