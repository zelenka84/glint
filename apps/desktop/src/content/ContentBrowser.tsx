import { useEffect, useRef, useState } from "react";
import { api, type ContentKind, type ModHit } from "../api";
import { B3D, Icon } from "../ui";
import { ProjectPage } from "./ProjectPage";

export const KIND_LABEL: Record<ContentKind, { one: string; many: string }> = {
  mod: { one: "mod", many: "Mods" },
  resourcepack: { one: "resource pack", many: "Resource packs" },
  shader: { one: "shader", many: "Shaders" },
};

export const compact = new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 });
export const HIDDEN_TAGS = new Set(["fabric", "quilt", "forge", "neoforge", "liteloader", "rift", "modloader", "minecraft", "iris", "optifine", "canvas", "vanilla"]);

export function ProjectIcon({ url, size = 48 }: { url: string | null; size?: number }) {
  const [broken, setBroken] = useState(false);
  return url && !broken ? (
    <img className="mod-icon px2" style={{ width: size, height: size }} src={url} alt="" loading="lazy" onError={() => setBroken(true)} />
  ) : (
    <span className="mod-icon placeholder px2" style={{ width: size, height: size }} aria-hidden="true">
      <Icon name="mods" size={Math.round(size / 2.4)} />
    </span>
  );
}

/** Modrinth search for one instance and kind, with one-click install. */
export function ContentBrowser({ instance, kind, installed, onInstalled, onError }: {
  instance: string;
  kind: ContentKind;
  /** Project ids already in the instance. */
  installed: Set<string>;
  onInstalled: (titles: string[], hit: ModHit) => void;
  onError: (message: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ModHit[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const [open, setOpen] = useState<ModHit | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const savedScroll = useRef(0);

  // The page scrolls inside <main>; remember where the list was so Back returns there.
  const scroller = () => root.current?.closest(".main") ?? null;
  function openProject(hit: ModHit) {
    savedScroll.current = scroller()?.scrollTop ?? 0;
    setOpen(hit);
    scroller()?.scrollTo({ top: 0 });
  }
  function closeProject() {
    setOpen(null);
    requestAnimationFrame(() => scroller()?.scrollTo({ top: savedScroll.current }));
  }

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    const timer = setTimeout(() => {
      api.searchContent(instance, kind, query).then(
        (results) => {
          if (cancelled) return;
          setHits(results.hits);
          setTotal(results.totalHits);
          setLoading(false);
        },
        (e) => {
          if (cancelled) return;
          onError(String(e));
          setLoading(false);
        },
      );
    }, 300);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance, kind, query]);

  async function loadMore() {
    setLoading(true);
    try {
      const more = await api.searchContent(instance, kind, query, hits.length);
      setHits((current) => [...current, ...more.hits]);
    } catch (e) {
      onError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function install(hit: ModHit) {
    setBusy((b) => ({ ...b, [hit.projectId]: true }));
    try {
      onInstalled(await api.installContent(instance, kind, hit.projectId), hit);
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy((b) => ({ ...b, [hit.projectId]: false }));
    }
  }

  return (
    <div className="browser" ref={root}>
      {open && (
        <ProjectPage
          instance={instance}
          kind={kind}
          hit={open}
          onBack={closeProject}
          onError={onError}
          onInstalled={(titles) => onInstalled(titles, open)}
        />
      )}
      <div className="browser-list" hidden={!!open}>
        <label className="search px4">
          <Icon name="search" />
          <input
            aria-label={`Search ${KIND_LABEL[kind].many.toLowerCase()}`}
            value={query}
            autoFocus
            placeholder={`Search ${KIND_LABEL[kind].many.toLowerCase()} on Modrinth`}
            onChange={(e) => setQuery(e.target.value)}
          />
          {loading && <span className="spinner" aria-label="Searching" />}
        </label>
        <div className="mod-list">
          {hits.map((hit) => {
            const isInstalled = installed.has(hit.projectId);
            const tags = hit.categories.filter((c) => !HIDDEN_TAGS.has(c)).slice(0, 3);
            return (
              <article
                key={hit.projectId}
                className="mod-row clickable px4"
                role="button"
                tabIndex={0}
                aria-label={`Open ${hit.title}`}
                onClick={() => openProject(hit)}
                onKeyDown={(e) => e.key === "Enter" && e.target === e.currentTarget && openProject(hit)}
              >
                <ProjectIcon url={hit.iconUrl} />
                <div className="mod-text">
                  <div className="mod-title">
                    <b>{hit.title}</b>
                    <span className="muted small">by {hit.author}</span>
                  </div>
                  <p className="mod-desc">{hit.description}</p>
                  <div className="mod-meta">
                    <span className="small muted">
                      <Icon name="download" size={10} /> {compact.format(hit.downloads)}
                    </span>
                    {tags.map((t) => (
                      <span key={t} className="tag-muted px2">{t}</span>
                    ))}
                  </div>
                </div>
                <span className="mod-action" onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
                  {isInstalled ? (
                    <span className="installed-chip px2">
                      <Icon name="check" size={12} /> Installed
                    </span>
                  ) : (
                    <B3D variant="accent" size="sm" disabled={busy[hit.projectId]} onClick={() => install(hit)}>
                      {busy[hit.projectId] ? <span className="spinner small" /> : <Icon name="download" size={12} />}
                      <span className="pix">{busy[hit.projectId] ? "Installing" : "Install"}</span>
                    </B3D>
                  )}
                </span>
              </article>
            );
          })}
          {!loading && hits.length === 0 && <p className="muted">Nothing found for this Minecraft version.</p>}
          {hits.length < total && (
            <div className="load-more">
              <B3D onClick={loadMore} disabled={loading}>
                Load more
              </B3D>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
