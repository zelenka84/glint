import { useCallback, useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, type Instance, type Screenshot } from "../api";
import { Icon, formatBytes } from "../ui";
import { EmptyArt } from "../EmptyArt";

function when(seconds: number): string {
  return new Date(seconds * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

async function copyImage(path: string) {
  const blob = await (await fetch(convertFileSrc(path))).blob();
  await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
}

export function ScreenshotsTab({ instance }: { instance: Instance }) {
  const [shots, setShots] = useState<Screenshot[] | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    api.listScreenshots(instance.name).then(setShots, (e) => setError(String(e)));
  }, [instance.name]);
  useEffect(refresh, [refresh]);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 1600);
    return () => clearTimeout(t);
  }, [toast]);

  const current = open !== null && shots ? shots[open] : null;

  // Arrow keys move through the viewer, Escape closes it.
  useEffect(() => {
    if (open === null || !shots) return;
    const onKey = (e: KeyboardEvent) => {
      if (renaming !== null) return;
      if (e.key === "Escape") setOpen(null);
      if (e.key === "ArrowRight") setOpen((i) => (i === null ? i : Math.min(i + 1, shots.length - 1)));
      if (e.key === "ArrowLeft") setOpen((i) => (i === null ? i : Math.max(i - 1, 0)));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, shots, renaming]);

  async function act(action: () => Promise<unknown>, message?: string) {
    try {
      await action();
      if (message) setToast(message);
    } catch (e) {
      setError(String(e));
    }
  }

  async function remove(shot: Screenshot) {
    await act(() => api.deleteScreenshot(instance.name, shot.name), "Deleted");
    const next = (shots ?? []).filter((s) => s.name !== shot.name);
    setShots(next);
    if (open !== null) setOpen(next.length ? Math.min(open, next.length - 1) : null);
  }

  return (
    <div className="tab-pane">
      <div className="pane-head">
        <h2 className="pix">Screenshots</h2>
        {shots && <span className="count px2">{shots.length}</span>}
        <div className="grow" />
        <button className="tool-btn px2" onClick={refresh}>
          <Icon name="refresh" /> Refresh
        </button>
        <button className="tool-btn px2" onClick={() => api.openGameFolder(instance.name, "screenshots")}>
          <Icon name="folder" /> Folder
        </button>
      </div>
      {error && (
        <div className="notice px2" role="alert">
          <span className="grow">{error}</span>
          <button className="icon-btn" aria-label="Dismiss" onClick={() => setError(null)}>
            <Icon name="close" size={12} />
          </button>
        </div>
      )}

      {shots === null ? (
        <div className="shot-grid">{[0, 1, 2, 3].map((i) => <div key={i} className="skeleton shot-skeleton px4" />)}</div>
      ) : shots.length === 0 ? (
        <div className="empty px4">
          <EmptyArt art="picture" />
          <p className="pix">No screenshots yet</p>
          <p className="muted">Press F2 in game; they show up here.</p>
        </div>
      ) : (
        <div className="shot-grid">
          {shots.map((shot, i) => (
            <button key={shot.name} className="shot px4" onClick={() => setOpen(i)} aria-label={`Open ${shot.name}`}>
              <img src={convertFileSrc(shot.path)} alt="" loading="lazy" />
              <span className="shot-caption">{when(shot.taken)}</span>
            </button>
          ))}
        </div>
      )}

      {current && (
        <div className="viewer" role="dialog" aria-modal="true" aria-label={current.name} onMouseDown={(e) => e.target === e.currentTarget && setOpen(null)}>
          <button className="viewer-nav prev" aria-label="Previous" disabled={open === 0} onClick={() => setOpen((i) => (i ? i - 1 : 0))}>
            <Icon name="chevronLeft" size={24} />
          </button>
          <figure className="viewer-figure">
            <img key={current.path} src={convertFileSrc(current.path)} alt={current.name} />
            <figcaption className="viewer-bar px4">
              {renaming !== null ? (
                <form
                  className="rename-row grow"
                  onSubmit={async (e) => {
                    e.preventDefault();
                    const newName = renaming;
                    setRenaming(null);
                    await act(() => api.renameScreenshot(instance.name, current.name, newName), "Renamed");
                    refresh();
                  }}
                >
                  <input className="input px2" autoFocus value={renaming} onChange={(e) => setRenaming(e.target.value)} />
                  <button className="tool-btn accent px2" type="submit">Save</button>
                  <button className="tool-btn px2" type="button" onClick={() => setRenaming(null)}>Cancel</button>
                </form>
              ) : (
                <span className="viewer-title grow">
                  <b>{current.name}</b>
                  <span className="muted small">
                    {when(current.taken)} · {formatBytes(current.size)} · {open! + 1}/{shots!.length}
                  </span>
                </span>
              )}
              <button className="tool-btn px2" onClick={() => act(() => copyImage(current.path), "Image copied")}>
                <Icon name="copy" /> Copy
              </button>
              <button className="tool-btn px2" onClick={() => setRenaming(current.name.replace(/\.png$/i, ""))}>
                <Icon name="edit" /> Rename
              </button>
              <button className="tool-btn px2" onClick={() => api.openScreenshot(instance.name, current.name)} aria-label="Open in the default viewer">
                <Icon name="external" />
              </button>
              <button className="tool-btn px2" onClick={() => api.revealPath(current.path)} aria-label="Show in folder">
                <Icon name="folder" />
              </button>
              <button className="tool-btn danger px2" onClick={() => remove(current)} aria-label="Delete">
                <Icon name="trash" />
              </button>
              <button className="icon-btn" onClick={() => setOpen(null)} aria-label="Close">
                <Icon name="close" size={12} />
              </button>
            </figcaption>
          </figure>
          <button className="viewer-nav next" aria-label="Next" disabled={open === shots!.length - 1} onClick={() => setOpen((i) => (i === null ? i : i + 1))}>
            <Icon name="chevronRight" size={24} />
          </button>
          {toast && <span className="toast pix px2">{toast}</span>}
        </div>
      )}
    </div>
  );
}
