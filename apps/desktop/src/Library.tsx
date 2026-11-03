import { type FormEvent, useMemo, useState } from "react";
import { api, type Instance } from "./api";
import { EmptyState } from "./EmptyArt";
import { formatDuration } from "./home/Dashboard";
import type { Session } from "./session";
import { B3D, Cover, Icon, Modal, RustBadge, XpBar, coverFor, flavourOf, relativeTime } from "./ui";

type Props = {
  instances: Instance[];
  sessions: Record<string, Session>;
  canPlay: boolean;
  onCreate: () => void;
  onOpen: (name: string) => void;
  onPlay: (name: string) => void;
  onOpenFolder: (name: string) => void;
  onEdit: (instance: Instance) => void;
  onDelete: (instance: Instance) => void;
  onKill: (instance: Instance) => void;
  onChanged: () => void;
};

type Sort = "recent" | "name" | "playtime" | "version";

const SORTS: { id: Sort; label: string }[] = [
  { id: "recent", label: "Recently played" },
  { id: "name", label: "Name" },
  { id: "playtime", label: "Most played" },
  { id: "version", label: "Minecraft version" },
];

const SORT_KEY = "glint.library.sort";
const COLLAPSED_KEY = "glint.library.collapsed";
const UNGROUPED = "";

function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function store(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Only a convenience; the view still works without it.
  }
}

/** Newer versions first, comparing number by number ("1.21.10" after "1.21.4"). */
function compareVersions(a: string, b: string): number {
  return b.localeCompare(a, undefined, { numeric: true });
}

const sorters: Record<Sort, (a: Instance, b: Instance) => number> = {
  recent: (a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || a.name.localeCompare(b.name),
  name: (a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }),
  playtime: (a, b) => b.playTimeSecs - a.playTimeSecs || a.name.localeCompare(b.name),
  version: (a, b) => compareVersions(a.gameVersion, b.gameVersion) || a.name.localeCompare(b.name),
};

export function Library(props: Props) {
  const [query, setQuery] = useState("");
  const [sort, setSortState] = useState<Sort>(() => load(SORT_KEY, "recent"));
  const [collapsed, setCollapsed] = useState<string[]>(() => load(COLLAPSED_KEY, []));
  const [grouping, setGrouping] = useState<Instance | null>(null);

  const setSort = (s: Sort) => {
    setSortState(s);
    store(SORT_KEY, s);
  };
  const toggle = (group: string) => {
    const next = collapsed.includes(group) ? collapsed.filter((g) => g !== group) : [...collapsed, group];
    setCollapsed(next);
    store(COLLAPSED_KEY, next);
  };

  const groupNames = useMemo(
    () => [...new Set(props.instances.flatMap((i) => (i.group ? [i.group] : [])))].sort((a, b) => a.localeCompare(b)),
    [props.instances],
  );

  const sections = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matching = props.instances
      .filter(
        (i) =>
          !needle ||
          [i.name, i.gameVersion, i.loader ?? "vanilla", i.group ?? "", i.engine].some((field) => field.toLowerCase().includes(needle)),
      )
      .sort(sorters[sort]);
    const groups = groupNames.map((g) => ({ name: g, items: matching.filter((i) => i.group === g) }));
    const loose = matching.filter((i) => !i.group);
    return [...groups, { name: UNGROUPED, items: loose }].filter((s) => s.items.length);
  }, [props.instances, query, sort, groupNames]);

  const showHeaders = groupNames.length > 0;
  const total = props.instances.length;

  return (
    <div className="screen">
      <div className="screen-head">
        <h1 className="pix">Library</h1>
        <B3D variant="accent" onClick={props.onCreate}>
          <Icon name="plus" />
          <span className="pix">New instance</span>
        </B3D>
      </div>

      {total === 0 ? (
        <EmptyState art="chest" title="No instances yet">
          <p className="muted">Create one to pick a Minecraft version and start playing.</p>
          <div className="empty-action">
            <B3D variant="accent" onClick={props.onCreate}>
              <Icon name="plus" />
              <span className="pix">New instance</span>
            </B3D>
          </div>
        </EmptyState>
      ) : (
        <>
          <div className="library-bar">
            <label className="search slim px4 grow">
              <Icon name="search" />
              <input value={query} placeholder={`Search ${total} instances`} aria-label="Search instances" onChange={(e) => setQuery(e.target.value)} />
              {query && (
                <button className="icon-btn" aria-label="Clear search" onClick={() => setQuery("")}>
                  <Icon name="close" size={12} />
                </button>
              )}
            </label>
            <label className="field inline">
              <span>Sort</span>
              <select className="select px2" value={sort} onChange={(e) => setSort(e.target.value as Sort)}>
                {SORTS.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.label}
                  </option>
                ))}
              </select>
            </label>
          </div>

          {sections.length === 0 && (
            <EmptyState art="map" title="Nothing matches" compact>
              <p className="muted">No instance has “{query.trim()}” in its name, version or loader.</p>
            </EmptyState>
          )}

          {sections.map((section) => {
            const isCollapsed = showHeaders && collapsed.includes(section.name);
            return (
              <section key={section.name || "ungrouped"} className="library-group">
                {showHeaders && (
                  <button className="group-head" aria-expanded={!isCollapsed} onClick={() => toggle(section.name)}>
                    <span className="chevron">▸</span>
                    <h2 className="pix">{section.name || "Other"}</h2>
                    <span className="count px2">{section.items.length}</span>
                  </button>
                )}
                {!isCollapsed && (
                  <div className="card-grid">
                    {section.items.map((instance) => (
                      <InstanceCard key={instance.name} instance={instance} {...props} onGroup={() => setGrouping(instance)} />
                    ))}
                  </div>
                )}
              </section>
            );
          })}
        </>
      )}

      {grouping && (
        <GroupModal
          instance={grouping}
          groups={groupNames}
          onClose={() => setGrouping(null)}
          onSave={async (group) => {
            await api.setInstanceGroup(grouping.name, group);
            setGrouping(null);
            props.onChanged();
          }}
        />
      )}
    </div>
  );
}

function InstanceCard({ instance, onGroup, ...props }: Props & { instance: Instance; onGroup: () => void }) {
  const phase = props.sessions[instance.name]?.phase;
  const running = instance.running || phase?.kind === "running";
  return (
    <article className="instance-card px4">
      <button className="cover-button" onClick={() => props.onOpen(instance.name)} aria-label={`Open ${instance.name}`}>
        <Cover cover={coverFor(instance.cover, instance.name)} height={100} />
        {running && <span className="badge pix px2">RUNNING</span>}
        {instance.engine === "horizon" && <RustBadge />}
      </button>
      <div className="instance-body">
        <span className="card-title">{instance.name}</span>
        <span className="card-meta">
          {instance.gameVersion} · {flavourOf(instance, false)} · {relativeTime(instance.lastPlayed)}
          {instance.playTimeSecs > 0 && ` · ${formatDuration(instance.playTimeSecs)}`}
        </span>
        {phase?.kind === "busy" ? (
          <XpBar done={phase.done} total={phase.total} />
        ) : (
          <div className="instance-actions">
            <B3D variant="play" size="sm" disabled={!props.canPlay || running} onClick={() => props.onPlay(instance.name)}>
              <Icon name="play" size={12} />
              <span className="pix">{running ? "Running" : "Play"}</span>
            </B3D>
            <B3D size="sm" label="Open folder" className="square" onClick={() => props.onOpenFolder(instance.name)}>
              <Icon name="folder" />
            </B3D>
            <B3D size="sm" label={instance.group ? `Group: ${instance.group}` : "Add to a group"} className="square" onClick={onGroup}>
              <Icon name="library" />
            </B3D>
            {running ? (
              <B3D size="sm" variant="danger" label="Kill instance" className="square" onClick={() => props.onKill(instance)}>
                <Icon name="stop" />
              </B3D>
            ) : (
              <>
                <B3D size="sm" label="Edit" className="square" onClick={() => props.onEdit(instance)}>
                  <Icon name="edit" />
                </B3D>
                <B3D size="sm" label="Delete" className="square" onClick={() => props.onDelete(instance)}>
                  <Icon name="trash" />
                </B3D>
              </>
            )}
          </div>
        )}
      </div>
    </article>
  );
}

function GroupModal({ instance, groups, onSave, onClose }: {
  instance: Instance;
  groups: string[];
  onSave: (group: string | null) => Promise<void>;
  onClose: () => void;
}) {
  const [name, setName] = useState(instance.group ?? "");
  const [error, setError] = useState<string | null>(null);

  async function save(group: string | null) {
    try {
      await onSave(group);
    } catch (e) {
      setError(String(e));
    }
  }

  function submit(e: FormEvent) {
    e.preventDefault();
    save(name.trim() || null);
  }

  return (
    <Modal title={`Group for ${instance.name}`} onClose={onClose}>
      <form className="form" onSubmit={submit}>
        {groups.length > 0 && (
          <div className="group-chips">
            {groups.map((g) => (
              <button key={g} type="button" className={`group-chip px2 ${g === name ? "selected" : ""}`} onClick={() => setName(g)}>
                {g}
              </button>
            ))}
          </div>
        )}
        <label className="field">
          <span>{groups.length ? "Or a new group" : "Group name"}</span>
          <input className="input px2" value={name} maxLength={32} placeholder="Modded, Servers, With friends…" onChange={(e) => setName(e.target.value)} />
        </label>
        {error && <p className="error">{error}</p>}
        <div className="modal-actions spread">
          {instance.group ? (
            <button type="button" className="link" onClick={() => save(null)}>
              Remove from {instance.group}
            </button>
          ) : (
            <span />
          )}
          <B3D type="submit" variant="accent" disabled={!name.trim()}>
            <span className="pix">Save</span>
          </B3D>
        </div>
      </form>
    </Modal>
  );
}
