import DOMPurify from "dompurify";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { type MouseEvent, useEffect, useState } from "react";
import {
  api,
  type ContentKind,
  type ContentUpdate,
  type Instance,
  type ModHit,
  type NewsItem,
  type PatchNote,
  type PlayTarget,
  type Screenshot,
  type Server,
  type ServerStatus,
  type World,
} from "../api";
import { ProjectIcon, compact } from "../content/ContentBrowser";
import { EmptyArt } from "../EmptyArt";
import type { InstanceTab } from "../instance/InstancePage";
import { WorldIcon } from "../instance/WorldsTab";
import { B3D, Icon, Modal } from "../ui";

/** Results kept between visits to Home, so it paints at once and refreshes quietly. */
const cache = new Map<string, unknown>();

function useCached<T>(key: string | null, load: () => Promise<T>): [T | undefined, () => void] {
  const [value, setValue] = useState<T | undefined>(() => (key ? (cache.get(key) as T | undefined) : undefined));
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (!key) {
      setValue(undefined);
      return;
    }
    setValue(cache.get(key) as T | undefined);
    let cancelled = false;
    load().then(
      (v) => {
        cache.set(key, v);
        if (!cancelled) setValue(v);
      },
      () => {},
    );
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, tick]);
  return [value, () => setTick((t) => t + 1)];
}

export function formatDuration(secs: number): string {
  if (secs < 60) return secs > 0 ? "under a minute" : "0 min";
  const minutes = Math.round(secs / 60);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest ? `${hours} h ${rest} min` : `${hours} h`;
}

function shortDate(iso: string): string {
  const t = Date.parse(iso);
  return t ? new Date(t).toLocaleDateString("en", { month: "short", day: "numeric" }) : "";
}

type Props = {
  instance: Instance;
  instances: Instance[];
  canPlay: boolean;
  onPlay: (name: string, target: PlayTarget) => void;
  onOpenTab: (name: string, tab: InstanceTab) => void;
  onDiscover: (kind: ContentKind, hit: ModHit) => void;
};

/** Everything under the hero on Home: where to go next, what's new and how much you've played. */
export function Dashboard(props: Props) {
  const { instance } = props;
  const java = instance.engine !== "horizon";
  return (
    <div className="dashboard">
      <div className="dash-main">
        <JumpBackIn {...props} />
        <Shots instance={instance} onOpen={() => props.onOpenTab(instance.name, "screenshots")} />
        {java && <Popular instance={instance} onDiscover={props.onDiscover} />}
      </div>
      <aside className="dash-side">
        <Stats instance={instance} instances={props.instances} />
        {java && <Updates instance={instance} onReview={(tab) => props.onOpenTab(instance.name, tab)} />}
        <News />
      </aside>
    </div>
  );
}

function JumpBackIn({ instance, canPlay, onPlay, onOpenTab }: Props) {
  const [worlds] = useCached<World[]>(`${instance.name}:worlds`, () => api.listWorlds(instance.name));
  const [servers] = useCached<Server[]>(`${instance.name}:servers`, () => api.listServers(instance.name));
  const [pings, setPings] = useState<Record<string, ServerStatus | null>>({});
  const shownServers = (servers ?? []).slice(0, 3);

  useEffect(() => {
    let cancelled = false;
    for (const s of shownServers) {
      api.pingServer(s.address).then(
        (status) => !cancelled && setPings((p) => ({ ...p, [s.address]: status })),
        () => !cancelled && setPings((p) => ({ ...p, [s.address]: null })),
      );
    }
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [servers]);

  const shownWorlds = (worlds ?? []).slice(0, 3);
  const blocked = !canPlay || instance.running;
  const empty = worlds && servers && !shownWorlds.length && !shownServers.length;

  return (
    <section className="dash-card px4">
      <div className="dash-head">
        <h2 className="pix">Jump back in</h2>
        <button className="link" onClick={() => onOpenTab(instance.name, "worlds")}>
          All worlds
        </button>
      </div>
      {empty ? (
        <div className="dash-empty">
          <EmptyArt art="chest" size={72} />
          <span className="muted small">Your worlds and servers show up here after the first game.</span>
        </div>
      ) : (
        <div className="jump-grid">
          {shownWorlds.map((w) => (
            <button key={w.folder} className="jump px2" disabled={blocked} onClick={() => onPlay(instance.name, { world: w.folder })}>
              <WorldIcon path={w.icon} size={44} />
              <span className="jump-text">
                <b>{w.name}</b>
                <span className="muted small">
                  {w.hardcore ? "Hardcore" : w.gameMode} · {w.lastPlayed ? shortDate(new Date(w.lastPlayed).toISOString()) : "never"}
                </span>
              </span>
              <span className="jump-play">
                <Icon name="play" size={12} />
              </span>
            </button>
          ))}
          {shownServers.map((s) => {
            const status = pings[s.address];
            const icon = status?.favicon ?? (s.icon ? `data:image/png;base64,${s.icon}` : null);
            return (
              <button key={s.address} className="jump px2" disabled={blocked} onClick={() => onPlay(instance.name, { server: s.address })}>
                {icon ? (
                  <img className="world-icon px2" style={{ width: 44, height: 44 }} src={icon} alt="" />
                ) : (
                  <span className="world-icon placeholder px2" style={{ width: 44, height: 44 }}>
                    <Icon name="server" size={20} />
                  </span>
                )}
                <span className="jump-text">
                  <b>{s.name}</b>
                  <span className="muted small">
                    {status === undefined ? (
                      "Pinging…"
                    ) : status === null ? (
                      "Offline"
                    ) : (
                      <>
                        <span className="online-dot" /> {compact.format(status.online)} online · {status.latencyMs} ms
                      </>
                    )}
                  </span>
                </span>
                <span className="jump-play">
                  <Icon name="play" size={12} />
                </span>
              </button>
            );
          })}
        </div>
      )}
    </section>
  );
}

function Shots({ instance, onOpen }: { instance: Instance; onOpen: () => void }) {
  const [shots] = useCached<Screenshot[]>(`${instance.name}:shots`, () => api.listScreenshots(instance.name));
  if (!shots?.length) return null;
  return (
    <section className="dash-card px4">
      <div className="dash-head">
        <h2 className="pix">Screenshots</h2>
        <button className="link" onClick={onOpen}>
          All {shots.length}
        </button>
      </div>
      <div className="shot-strip">
        {shots.slice(0, 8).map((s) => (
          <button key={s.path} className="shot-thumb px2" onClick={onOpen} title={s.name}>
            <img src={convertFileSrc(s.path)} alt="" loading="lazy" />
          </button>
        ))}
      </div>
    </section>
  );
}

function Popular({ instance, onDiscover }: { instance: Instance; onDiscover: (kind: ContentKind, hit: ModHit) => void }) {
  const kind: ContentKind = instance.loader ? "mod" : "resourcepack";
  const [results] = useCached(`${instance.name}:popular:${kind}`, () => api.searchContent(instance.name, kind, ""));
  const hits = results?.hits.slice(0, 6) ?? [];
  if (!results) return null;
  return (
    <section className="dash-card px4">
      <div className="dash-head">
        <h2 className="pix">Popular {kind === "mod" ? "mods" : "resource packs"}</h2>
        <span className="muted small">
          for {instance.loader ?? "Vanilla"} {instance.gameVersion}
        </span>
      </div>
      <div className="popular-grid">
        {hits.map((hit) => (
          <button key={hit.projectId} className="popular px2" onClick={() => onDiscover(kind, hit)}>
            <ProjectIcon url={hit.iconUrl} size={40} />
            <span className="jump-text">
              <b>{hit.title}</b>
              <span className="muted small popular-desc">{hit.description}</span>
              <span className="muted small">
                <Icon name="download" size={10} /> {compact.format(hit.downloads)}
              </span>
            </span>
          </button>
        ))}
      </div>
    </section>
  );
}

function Stats({ instance, instances }: { instance: Instance; instances: Instance[] }) {
  const total = instances.reduce((sum, i) => sum + i.playTimeSecs, 0);
  const top = [...instances].filter((i) => i.playTimeSecs > 0).sort((a, b) => b.playTimeSecs - a.playTimeSecs).slice(0, 4);
  const max = top[0]?.playTimeSecs ?? 1;
  return (
    <section className="dash-card px4">
      <div className="dash-head">
        <h2 className="pix">Playtime</h2>
      </div>
      <div className="stat-row">
        <div className="stat">
          <span className="stat-value pix">{formatDuration(instance.playTimeSecs)}</span>
          <span className="muted small">in {instance.name}</span>
        </div>
        <div className="stat">
          <span className="stat-value pix">{instance.sessions}</span>
          <span className="muted small">{instance.sessions === 1 ? "session" : "sessions"}</span>
        </div>
      </div>
      {top.length > 0 ? (
        <>
          <span className="label pix">MOST PLAYED · {formatDuration(total)} TOTAL</span>
          <div className="bars">
            {top.map((i, n) => (
              <div key={i.name} className={`bar-row ${i.name === instance.name ? "current" : ""}`}>
                <span className="bar-name">{i.name}</span>
                <span className="bar px2">
                  <span style={{ width: `${Math.max(4, (i.playTimeSecs / max) * 100)}%`, animationDelay: `${n * 70}ms` }} />
                </span>
                <span className="muted small">{formatDuration(i.playTimeSecs)}</span>
              </div>
            ))}
          </div>
        </>
      ) : (
        <span className="muted small">Time in game is counted from the next launch.</span>
      )}
    </section>
  );
}

const UPDATE_KINDS: { kind: ContentKind; tab: InstanceTab; label: string; needsLoader: boolean }[] = [
  { kind: "mod", tab: "mods", label: "mods", needsLoader: true },
  { kind: "resourcepack", tab: "resourcepacks", label: "resource packs", needsLoader: false },
  { kind: "shader", tab: "shaders", label: "shaders", needsLoader: true },
];

function Updates({ instance, onReview }: { instance: Instance; onReview: (tab: InstanceTab) => void }) {
  const kinds = UPDATE_KINDS.filter((k) => !k.needsLoader || instance.loader);
  const [updates, reload] = useCached<{ kind: (typeof UPDATE_KINDS)[number]; items: ContentUpdate[] }[]>(
    `${instance.name}:updates`,
    () => Promise.all(kinds.map(async (k) => ({ kind: k, items: await api.contentUpdates(instance.name, k.kind) }))),
  );
  const [busy, setBusy] = useState<{ done: number; total: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const count = updates?.reduce((n, u) => n + u.items.length, 0) ?? 0;
  if (!updates || (count === 0 && !busy)) return null;

  async function updateAll() {
    const work = updates!.flatMap((u) => u.items.map((item) => ({ kind: u.kind.kind, item })));
    setBusy({ done: 0, total: work.length });
    setError(null);
    for (const [n, { kind, item }] of work.entries()) {
      try {
        await api.updateContent(instance.name, kind, item.projectId);
      } catch (e) {
        setError(String(e));
      }
      setBusy({ done: n + 1, total: work.length });
    }
    setBusy(null);
    reload();
  }

  return (
    <section className="dash-card updates px4">
      <div className="dash-head">
        <h2 className="pix">
          <Icon name="arrowUp" /> {count} {count === 1 ? "update" : "updates"}
        </h2>
      </div>
      <div className="update-kinds">
        {updates
          .filter((u) => u.items.length)
          .map((u) => (
            <button key={u.kind.kind} className="update-kind px2" onClick={() => onReview(u.kind.tab)}>
              <b>{u.items.length}</b> {u.kind.label}
              <span className="muted small grow">{u.items.slice(0, 2).map((i) => i.title).join(", ")}{u.items.length > 2 ? "…" : ""}</span>
              <Icon name="chevronRight" size={12} />
            </button>
          ))}
      </div>
      {error && <p className="error small">{error}</p>}
      <B3D variant="warn" disabled={!!busy} onClick={updateAll}>
        {busy ? <span className="spinner small" /> : <Icon name="arrowUp" size={14} />}
        <span className="pix">{busy ? `Updating ${busy.done}/${busy.total}` : "Update all"}</span>
      </B3D>
    </section>
  );
}

function News() {
  const [tab, setTab] = useState<"news" | "patch">("news");
  const [news] = useCached<NewsItem[]>("news", api.minecraftNews);
  const [notes] = useCached<PatchNote[]>("patchNotes", api.patchNotes);
  const [reading, setReading] = useState<PatchNote | null>(null);
  const list = tab === "news" ? news : notes;

  return (
    <section className="dash-card px4">
      <div className="dash-head">
        <h2 className="pix">What's new</h2>
        <div className="seg px2" role="tablist">
          <button role="tab" aria-selected={tab === "news"} className={tab === "news" ? "on" : ""} onClick={() => setTab("news")}>
            News
          </button>
          <button role="tab" aria-selected={tab === "patch"} className={tab === "patch" ? "on" : ""} onClick={() => setTab("patch")}>
            Patch notes
          </button>
        </div>
      </div>
      {!list ? (
        <div className="news-skeleton">
          {[0, 1, 2].map((i) => (
            <span key={i} className="px2" />
          ))}
        </div>
      ) : (
        <div className="news-list" key={tab}>
          {tab === "news"
            ? news!.slice(0, 5).map((n) => (
                <button key={n.id} className="news px2" onClick={() => n.link && openUrl(n.link)}>
                  {n.image && <img src={n.image} alt="" loading="lazy" />}
                  <span className="jump-text">
                    <b>{n.title}</b>
                    <span className="muted small">{n.text}</span>
                    <span className="news-date small">{shortDate(n.date)}</span>
                  </span>
                </button>
              ))
            : notes!.slice(0, 5).map((p) => (
                <button key={p.id} className="news px2" onClick={() => setReading(p)}>
                  {p.image && <img src={p.image} alt="" loading="lazy" />}
                  <span className="jump-text">
                    <b>{p.title}</b>
                    <span className="muted small">{p.shortText}</span>
                    <span className="news-date small">
                      <span className={`tag-muted px2 ${p.kind}`}>{p.kind}</span> {shortDate(p.date)}
                    </span>
                  </span>
                </button>
              ))}
        </div>
      )}
      {reading && <PatchReader note={reading} onClose={() => setReading(null)} />}
    </section>
  );
}

function PatchReader({ note, onClose }: { note: PatchNote; onClose: () => void }) {
  const [html, setHtml] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.patchNoteBody(note.contentPath).then((body) => setHtml(DOMPurify.sanitize(body)), (e) => setError(String(e)));
  }, [note.contentPath]);

  function openLinks(e: MouseEvent) {
    const link = (e.target as HTMLElement).closest("a");
    if (!link) return;
    e.preventDefault();
    const href = link.getAttribute("href") ?? "";
    if (/^https?:\/\//.test(href)) openUrl(href);
  }

  return (
    <Modal title={note.title} onClose={onClose} wide>
      {note.image && <img className="reader-cover px2" src={note.image} alt="" />}
      {error && <p className="error">{error}</p>}
      {html === null && !error ? (
        <div className="pp-loading">
          <span className="spinner" /> Loading patch notes…
        </div>
      ) : (
        <div className="markdown reader" onClick={openLinks} dangerouslySetInnerHTML={{ __html: html ?? "" }} />
      )}
    </Modal>
  );
}
