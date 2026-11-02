import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type Instance, type InstalledMod, type ModHit } from "./api";
import { B3D, Icon } from "./ui";

type Props = {
  instances: Instance[];
  selected: string | null;
  onSelect: (name: string) => void;
  onCreate: () => void;
};

const compact = new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 });
const LOADER_TAGS = new Set(["fabric", "quilt", "forge", "neoforge", "liteloader", "rift", "modloader"]);

function ModIcon({ url }: { url: string | null }) {
  const [broken, setBroken] = useState(false);
  return url && !broken ? (
    <img className="mod-icon px2" src={url} alt="" loading="lazy" onError={() => setBroken(true)} />
  ) : (
    <span className="mod-icon placeholder px2" aria-hidden="true">
      <Icon name="mods" size={20} />
    </span>
  );
}

export function Mods({ instances, selected, onSelect, onCreate }: Props) {
  const modded = useMemo(() => instances.filter((i) => i.loader), [instances]);
  const instance = modded.find((i) => i.name === selected) ?? modded[0] ?? null;

  const [tab, setTab] = useState<"browse" | "installed">("browse");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ModHit[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [installed, setInstalled] = useState<InstalledMod[]>([]);
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshInstalled = useCallback(() => {
    if (!instance) return;
    api.installedMods(instance.name).then(setInstalled, (e) => setError(String(e)));
  }, [instance]);

  useEffect(refreshInstalled, [refreshInstalled]);

  // Search as the player types, a moment after they stop.
  useEffect(() => {
    if (!instance) return;
    let cancelled = false;
    setLoading(true);
    const timer = setTimeout(() => {
      api.searchMods(instance.name, query).then(
        (results) => {
          if (cancelled) return;
          setHits(results.hits);
          setTotal(results.totalHits);
          setError(null);
          setLoading(false);
        },
        (e) => {
          if (cancelled) return;
          setError(String(e));
          setLoading(false);
        },
      );
    }, 300);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [instance, query]);

  async function loadMore() {
    if (!instance) return;
    setLoading(true);
    try {
      const more = await api.searchMods(instance.name, query, hits.length);
      setHits((current) => [...current, ...more.hits]);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function install(hit: ModHit) {
    if (!instance) return;
    setBusy((b) => ({ ...b, [hit.projectId]: true }));
    try {
      const titles = await api.installMod(instance.name, hit.projectId);
      setNotice(titles.length ? `Installed ${titles.join(" + ")} into ${instance.name}` : `${hit.title} is already up to date`);
      refreshInstalled();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy((b) => ({ ...b, [hit.projectId]: false }));
    }
  }

  async function act(action: () => Promise<void>) {
    try {
      await action();
      refreshInstalled();
    } catch (e) {
      setError(String(e));
    }
  }

  if (!instance) {
    return (
      <div className="screen">
        <div className="screen-head">
          <h1 className="pix">Mods</h1>
        </div>
        <div className="empty px4">
          <p className="pix">Mods need a modded instance</p>
          <p className="muted">Create an instance with Fabric, Quilt, Forge or NeoForge, then add mods from Modrinth here.</p>
          <div className="empty-action">
            <B3D variant="accent" onClick={onCreate}>
              <Icon name="plus" />
              <span className="pix">New instance</span>
            </B3D>
          </div>
        </div>
      </div>
    );
  }

  const installedIds = new Set(installed.map((m) => m.projectId).filter(Boolean));

  return (
    <div className="screen mods">
      <div className="screen-head">
        <h1 className="pix">Mods</h1>
        <label className="field inline">
          <span>Install into</span>
          <select className="select px2" value={instance.name} onChange={(e) => onSelect(e.target.value)}>
            {modded.map((i) => (
              <option key={i.name} value={i.name}>
                {i.name} · {i.loader} {i.gameVersion}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="tabs" role="tablist">
        <button role="tab" aria-selected={tab === "browse"} className={`tab px2 ${tab === "browse" ? "active" : ""}`} onClick={() => setTab("browse")}>
          <span className="pix">Browse Modrinth</span>
        </button>
        <button role="tab" aria-selected={tab === "installed"} className={`tab px2 ${tab === "installed" ? "active" : ""}`} onClick={() => setTab("installed")}>
          <span className="pix">Installed · {installed.length}</span>
        </button>
      </div>

      {notice && (
        <div className="notice ok px2" role="status">
          <Icon name="check" />
          <span className="grow">{notice}</span>
          <button className="icon-btn" aria-label="Dismiss" onClick={() => setNotice(null)}>
            <Icon name="close" size={12} />
          </button>
        </div>
      )}
      {error && (
        <div className="notice px2" role="alert">
          <span className="grow">{error}</span>
          <button className="icon-btn" aria-label="Dismiss" onClick={() => setError(null)}>
            <Icon name="close" size={12} />
          </button>
        </div>
      )}

      {tab === "browse" ? (
        <>
          <label className="search px4">
            <Icon name="search" />
            <input
              aria-label="Search mods"
              value={query}
              placeholder={`Search mods for ${instance.loader} ${instance.gameVersion}`}
              onChange={(e) => setQuery(e.target.value)}
            />
            {loading && <span className="muted small">Searching…</span>}
          </label>
          <div className="mod-list">
            {hits.map((hit) => {
              const isInstalled = installedIds.has(hit.projectId);
              const tags = hit.categories.filter((c) => !LOADER_TAGS.has(c)).slice(0, 3);
              return (
                <article key={hit.projectId} className="mod-row px4">
                  <ModIcon url={hit.iconUrl} />
                  <div className="mod-text">
                    <div className="mod-title">
                      <b>{hit.title}</b>
                      <span className="muted small">by {hit.author}</span>
                    </div>
                    <p className="mod-desc">{hit.description}</p>
                    <div className="mod-meta">
                      <span className="small muted">{compact.format(hit.downloads)} downloads</span>
                      {tags.map((t) => (
                        <span key={t} className="tag-muted px2">{t}</span>
                      ))}
                    </div>
                  </div>
                  {isInstalled ? (
                    <span className="installed-chip px2">
                      <Icon name="check" size={12} /> Installed
                    </span>
                  ) : (
                    <B3D variant="accent" size="sm" disabled={busy[hit.projectId]} onClick={() => install(hit)}>
                      <span className="pix">{busy[hit.projectId] ? "Installing…" : "Install"}</span>
                    </B3D>
                  )}
                </article>
              );
            })}
            {!loading && hits.length === 0 && <p className="muted">Nothing found for this version and loader.</p>}
            {hits.length < total && (
              <div className="load-more">
                <B3D onClick={loadMore} disabled={loading}>
                  Load more
                </B3D>
              </div>
            )}
          </div>
        </>
      ) : (
        <div className="mod-list">
          {installed.length === 0 && <p className="muted">No mods yet. Find some in Browse Modrinth.</p>}
          {installed.map((mod) => (
            <article key={mod.filename} className={`mod-row px4 ${mod.enabled ? "" : "disabled"}`}>
              <label className="switch" title={mod.enabled ? "Disable" : "Enable"}>
                <input
                  type="checkbox"
                  checked={mod.enabled}
                  aria-label={`${mod.enabled ? "Disable" : "Enable"} ${mod.title}`}
                  onChange={(e) => act(() => api.setModEnabled(instance.name, mod.filename, e.target.checked))}
                />
              </label>
              <div className="mod-text">
                <div className="mod-title">
                  <b>{mod.title}</b>
                  {mod.version && <span className="muted small">{mod.version}</span>}
                  {mod.dependency && <span className="tag-muted px2">dependency</span>}
                  {!mod.managed && <span className="tag-muted px2">local file</span>}
                </div>
                <p className="mod-desc mono small">{mod.filename}</p>
              </div>
              <button className="icon-btn" aria-label={`Remove ${mod.title}`} onClick={() => act(() => api.removeMod(instance.name, mod.filename))}>
                <Icon name="trash" />
              </button>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
