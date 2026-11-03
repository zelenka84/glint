import { useCallback, useEffect, useMemo, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, type ContentKind, type ContentUpdate, type Instance, type InstalledMod } from "../api";
import { B3D, Icon, Switch, formatBytes } from "../ui";
import { ContentBrowser, KIND_LABEL, ProjectIcon } from "./ContentBrowser";
import { EmptyArt } from "../EmptyArt";

const DROP_EXT: Record<ContentKind, string> = { mod: ".jar", resourcepack: ".zip", shader: ".zip" };

/** Installed mods, resource packs or shaders of one instance, plus Modrinth browsing. */
export function ContentTab({ instance, kind }: { instance: Instance; kind: ContentKind }) {
  const label = KIND_LABEL[kind];
  const [items, setItems] = useState<InstalledMod[] | null>(null);
  const [mode, setMode] = useState<"installed" | "browse">("installed");
  const [filter, setFilter] = useState("");
  const [updates, setUpdates] = useState<ContentUpdate[] | null>(null);
  const [checking, setChecking] = useState(false);
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const [notice, setNotice] = useState<{ tone: "ok" | "bad"; text: string } | null>(null);
  const [dragging, setDragging] = useState(false);

  const refresh = useCallback(() => {
    api.installedContent(instance.name, kind).then(setItems, (e) => setNotice({ tone: "bad", text: String(e) }));
  }, [instance.name, kind]);

  useEffect(() => {
    setItems(null);
    setUpdates(null);
    setMode("installed");
    refresh();
  }, [refresh]);

  // Files dragged onto the window land in this folder.
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(async (event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") setDragging(true);
      else if (event.payload.type === "leave") setDragging(false);
      else if (event.payload.type === "drop") {
        setDragging(false);
        const paths = event.payload.paths.filter((p) => p.toLowerCase().endsWith(DROP_EXT[kind]));
        if (!paths.length) {
          setNotice({ tone: "bad", text: `Only ${DROP_EXT[kind]} files can be added here.` });
          return;
        }
        try {
          const added = await api.addContentFiles(instance.name, kind, paths);
          setNotice({ tone: "ok", text: `Added ${added.join(", ")}` });
          refresh();
        } catch (e) {
          setNotice({ tone: "bad", text: String(e) });
        }
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [instance.name, kind, refresh]);

  const installedIds = useMemo(() => new Set((items ?? []).flatMap((m) => (m.projectId ? [m.projectId] : []))), [items]);
  const updateFor = useMemo(() => new Map((updates ?? []).map((u) => [u.filename, u])), [updates]);
  const shown = useMemo(
    () => (items ?? []).filter((m) => m.title.toLowerCase().includes(filter.toLowerCase()) || m.filename.toLowerCase().includes(filter.toLowerCase())),
    [items, filter],
  );
  const enabledCount = (items ?? []).filter((m) => m.enabled).length;

  async function act(key: string, action: () => Promise<unknown>) {
    setBusy((b) => ({ ...b, [key]: true }));
    try {
      await action();
      refresh();
    } catch (e) {
      setNotice({ tone: "bad", text: String(e) });
    } finally {
      setBusy((b) => ({ ...b, [key]: false }));
    }
  }

  async function checkUpdates() {
    setChecking(true);
    try {
      const found = await api.contentUpdates(instance.name, kind);
      setUpdates(found);
      setNotice({ tone: "ok", text: found.length ? `${found.length} update${found.length > 1 ? "s" : ""} available` : "Everything is up to date" });
    } catch (e) {
      setNotice({ tone: "bad", text: String(e) });
    } finally {
      setChecking(false);
    }
  }

  async function update(u: ContentUpdate) {
    await act(u.filename, () => api.updateContent(instance.name, kind, u.projectId));
    setUpdates((list) => list?.filter((x) => x.projectId !== u.projectId) ?? null);
  }

  async function updateAll() {
    for (const u of updates ?? []) await update(u);
  }

  const needsLoader = kind === "mod" && !instance.loader;

  return (
    <div className={`tab-pane content-tab ${dragging ? "dragging" : ""}`}>
      <div className="pane-head">
        <h2 className="pix">{label.many}</h2>
        {items && <span className="count px2">{enabledCount}/{items.length} on</span>}
        <div className="grow" />
        {mode === "installed" ? (
          <>
            <button className="tool-btn px2" onClick={checkUpdates} disabled={checking || !items?.some((m) => m.managed)}>
              {checking ? <span className="spinner small" /> : <Icon name="refresh" />} Check updates
            </button>
            <button className="tool-btn px2" onClick={() => api.openContentFolder(instance.name, kind)}>
              <Icon name="folder" /> Folder
            </button>
            <B3D variant="accent" size="sm" disabled={needsLoader} onClick={() => setMode("browse")}>
              <Icon name="plus" size={12} />
              <span className="pix">Add {label.many.toLowerCase()}</span>
            </B3D>
          </>
        ) : (
          <button className="tool-btn px2" onClick={() => setMode("installed")}>
            <Icon name="back" /> Installed
          </button>
        )}
      </div>

      {kind === "shader" && !instance.loader && (
        <p className="hint px2">
          <Icon name="warn" /> Shaders need Iris, which needs a mod loader. Switch this instance to Fabric or NeoForge in
          the Version tab; Glint then installs Iris with the first shader.
        </p>
      )}
      {kind === "shader" && instance.loader && (
        <p className="hint px2">
          <Icon name="sun" /> Installing a shader also installs {instance.loader === "Forge" ? "Oculus" : "Iris"} if it is missing.
          Pick the shader in game under Options → Video → Shader Packs.
        </p>
      )}

      {notice && (
        <div className={`notice px2 ${notice.tone === "ok" ? "ok" : ""}`} role="status">
          <Icon name={notice.tone === "ok" ? "check" : "warn"} />
          <span className="grow">{notice.text}</span>
          <button className="icon-btn" aria-label="Dismiss" onClick={() => setNotice(null)}>
            <Icon name="close" size={12} />
          </button>
        </div>
      )}

      {mode === "browse" ? (
        <ContentBrowser
          instance={instance.name}
          kind={kind}
          installed={installedIds}
          onError={(text) => setNotice({ tone: "bad", text })}
          onInstalled={(titles, hit) => {
            setNotice({ tone: "ok", text: titles.length ? `Installed ${titles.join(" + ")}` : `${hit.title} is already up to date` });
            refresh();
          }}
        />
      ) : needsLoader ? (
        <div className="empty px4">
          <EmptyArt art="map" />
          <p className="pix">Vanilla runs no mods</p>
          <p className="muted">Pick Fabric, Quilt, Forge or NeoForge in the Version tab to add mods to this instance.</p>
        </div>
      ) : items === null ? (
        <div className="skeleton-list">{[0, 1, 2].map((i) => <div key={i} className="skeleton px4" />)}</div>
      ) : items.length === 0 ? (
        <div className="empty drop-hint px4">
          <EmptyArt art="chest" />
          <p className="pix">No {label.many.toLowerCase()} yet</p>
          <p className="muted">
            Find some on Modrinth, or drop {DROP_EXT[kind]} files onto this window.
          </p>
          <div className="empty-action">
            <B3D variant="accent" disabled={needsLoader} onClick={() => setMode("browse")}>
              <Icon name="search" />
              <span className="pix">Browse Modrinth</span>
            </B3D>
          </div>
        </div>
      ) : (
        <>
          <div className="log-toolbar">
            <label className="search slim px2 grow">
              <Icon name="search" />
              <input value={filter} placeholder={`Filter ${items.length} ${label.many.toLowerCase()}`} onChange={(e) => setFilter(e.target.value)} />
            </label>
            {updates && updates.length > 0 && (
              <button className="tool-btn accent px2" onClick={updateAll}>
                <Icon name="download" /> Update all ({updates.length})
              </button>
            )}
          </div>
          <div className="mod-list">
            {shown.map((item) => {
              const upd = updateFor.get(item.filename);
              return (
                <article key={item.filename} className={`mod-row px4 ${item.enabled ? "" : "disabled"}`}>
                  <ProjectIcon url={item.iconUrl} size={40} />
                  <div className="mod-text">
                    <div className="mod-title">
                      <b>{item.title}</b>
                      {item.version && <span className="muted small">{item.version}</span>}
                      {item.dependency && <span className="tag-muted px2">dependency</span>}
                      {!item.managed && <span className="tag-muted px2">local file</span>}
                    </div>
                    <p className="mod-desc mono small">
                      {item.filename}
                      {item.size > 0 && <span className="muted"> · {formatBytes(item.size)}</span>}
                    </p>
                  </div>
                  {upd && (
                    <button className="tool-btn accent px2" disabled={busy[item.filename]} onClick={() => update(upd)} title={`${upd.current} → ${upd.latest}`}>
                      {busy[item.filename] ? <span className="spinner small" /> : <Icon name="arrowUp" />} {upd.latest}
                    </button>
                  )}
                  <Switch
                    on={item.enabled}
                    label={`${item.enabled ? "Disable" : "Enable"} ${item.title}`}
                    onChange={(on) => act(item.filename, () => api.setContentEnabled(instance.name, kind, item.filename, on))}
                  />
                  <button
                    className="icon-btn danger"
                    aria-label={`Remove ${item.title}`}
                    disabled={busy[item.filename]}
                    onClick={() => act(item.filename, () => api.removeContent(instance.name, kind, item.filename))}
                  >
                    <Icon name="trash" />
                  </button>
                </article>
              );
            })}
            {shown.length === 0 && <p className="muted">Nothing matches “{filter}”.</p>}
          </div>
        </>
      )}

      {dragging && mode === "installed" && (
        <div className="drop-overlay px4">
          <Icon name="upload" size={32} />
          <span className="pix">Drop {DROP_EXT[kind]} files to add them</span>
        </div>
      )}
    </div>
  );
}
