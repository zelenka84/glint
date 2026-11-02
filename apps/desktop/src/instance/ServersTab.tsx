import { type FormEvent, useCallback, useEffect, useState } from "react";
import { api, type Instance, type PlayTarget, type Server, type ServerResources, type ServerStatus } from "../api";
import { B3D, Icon } from "../ui";

type Ping = { state: "loading" } | { state: "ok"; status: ServerStatus } | { state: "down"; error: string };

/** Five pixel bars, like the game's server list. */
function Signal({ ping }: { ping: Ping | undefined }) {
  let bars = 0;
  let tone = "down";
  if (ping?.state === "ok") {
    const ms = ping.status.latencyMs;
    bars = ms === 0 ? 3 : ms < 80 ? 5 : ms < 150 ? 4 : ms < 300 ? 3 : ms < 600 ? 2 : 1;
    tone = bars >= 4 ? "good" : bars >= 3 ? "fair" : "poor";
  }
  const title =
    ping?.state === "ok" ? (ping.status.latencyMs ? `${ping.status.latencyMs} ms` : "Online") : ping?.state === "down" ? ping.error : "Pinging…";
  return (
    <span className={`signal ${ping?.state === "loading" ? "loading" : tone}`} title={title} aria-label={title}>
      {[1, 2, 3, 4, 5].map((i) => (
        <span key={i} className={i <= bars ? "on" : ""} style={{ height: 3 + i * 3 }} />
      ))}
    </span>
  );
}

function ServerIcon({ ping, server }: { ping: Ping | undefined; server: Server }) {
  const src = ping?.state === "ok" && ping.status.favicon ? ping.status.favicon : server.icon ? `data:image/png;base64,${server.icon}` : null;
  return src ? (
    <img className="world-icon px2" style={{ width: 64, height: 64 }} src={src} alt="" />
  ) : (
    <span className="world-icon placeholder px2" style={{ width: 64, height: 64 }} aria-hidden="true">
      <Icon name="server" size={28} />
    </span>
  );
}

function Motd({ ping }: { ping: Ping | undefined }) {
  if (!ping || ping.state === "loading") return <span className="motd muted">Pinging…</span>;
  if (ping.state === "down") return <span className="motd down">Can't reach the server</span>;
  return (
    <span className="motd">
      {ping.status.motd.map((span, i) => (
        <span key={i} style={{ color: span.color ?? undefined, fontWeight: span.bold ? 700 : undefined, fontStyle: span.italic ? "italic" : undefined }}>
          {span.text}
        </span>
      ))}
    </span>
  );
}

const RESOURCES: { id: ServerResources; label: string }[] = [
  { id: "prompt", label: "Ask before downloading" },
  { id: "enabled", label: "Always download" },
  { id: "disabled", label: "Never download" },
];

function ServerForm({ initial, onSave, onCancel }: { initial: Server; onSave: (s: Server) => void; onCancel: () => void }) {
  const [draft, setDraft] = useState(initial);
  function submit(e: FormEvent) {
    e.preventDefault();
    if (draft.address.trim()) onSave({ ...draft, name: draft.name.trim() || "Minecraft Server" });
  }
  return (
    <form className="server-form px4" onSubmit={submit}>
      <label className="field grow">
        <span>Name</span>
        <input className="input px2" autoFocus value={draft.name} placeholder="Minecraft Server" onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
      </label>
      <label className="field grow">
        <span>Address</span>
        <input className="input px2 mono" value={draft.address} placeholder="play.example.net" onChange={(e) => setDraft({ ...draft, address: e.target.value })} />
      </label>
      <label className="field">
        <span>Server resource packs</span>
        <select className="select px2" value={draft.resources} onChange={(e) => setDraft({ ...draft, resources: e.target.value as ServerResources })}>
          {RESOURCES.map((r) => (
            <option key={r.id} value={r.id}>
              {r.label}
            </option>
          ))}
        </select>
      </label>
      <div className="row">
        <button type="button" className="tool-btn px2" onClick={onCancel}>Cancel</button>
        <button type="submit" className="tool-btn accent px2" disabled={!draft.address.trim()}>
          <Icon name="check" /> Save
        </button>
      </div>
    </form>
  );
}

export function ServersTab({ instance, canPlay, onPlay }: { instance: Instance; canPlay: boolean; onPlay: (target: PlayTarget) => void }) {
  const [servers, setServers] = useState<Server[] | null>(null);
  const [pings, setPings] = useState<Record<string, Ping>>({});
  const [editing, setEditing] = useState<number | "new" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const pingAll = useCallback((list: Server[]) => {
    for (const s of list) {
      setPings((p) => ({ ...p, [s.address]: { state: "loading" } }));
      api.pingServer(s.address).then(
        (status) => setPings((p) => ({ ...p, [s.address]: { state: "ok", status } })),
        (e) => setPings((p) => ({ ...p, [s.address]: { state: "down", error: String(e) } })),
      );
    }
  }, []);

  const refresh = useCallback(() => {
    api.listServers(instance.name).then(
      (list) => {
        setServers(list);
        pingAll(list);
      },
      (e) => setError(String(e)),
    );
  }, [instance.name, pingAll]);
  useEffect(refresh, [refresh]);

  async function save(next: Server[]) {
    try {
      await api.saveServers(instance.name, next);
      setServers(next);
      setEditing(null);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  function move(i: number, by: number) {
    if (!servers) return;
    const next = [...servers];
    const [item] = next.splice(i, 1);
    next.splice(i + by, 0, item);
    save(next);
  }

  const locked = instance.running;

  return (
    <div className="tab-pane">
      <div className="pane-head">
        <h2 className="pix">Servers</h2>
        {servers && <span className="count px2">{servers.length}</span>}
        <div className="grow" />
        <button className="tool-btn px2" onClick={refresh}>
          <Icon name="refresh" /> Refresh
        </button>
        <B3D variant="accent" size="sm" disabled={locked} onClick={() => setEditing("new")}>
          <Icon name="plus" size={12} />
          <span className="pix">Add server</span>
        </B3D>
      </div>
      {locked && <p className="hint px2"><Icon name="warn" /> The game keeps the server list open while it runs; close it to edit the list.</p>}
      {error && (
        <div className="notice px2" role="alert">
          <span className="grow">{error}</span>
          <button className="icon-btn" aria-label="Dismiss" onClick={() => setError(null)}>
            <Icon name="close" size={12} />
          </button>
        </div>
      )}

      {editing === "new" && (
        <ServerForm
          initial={{ name: "", address: "", icon: null, resources: "prompt" }}
          onCancel={() => setEditing(null)}
          onSave={(s) => {
            save([...(servers ?? []), s]);
            pingAll([s]);
          }}
        />
      )}

      {servers === null ? (
        <div className="skeleton-list">{[0, 1].map((i) => <div key={i} className="skeleton tall px4" />)}</div>
      ) : servers.length === 0 && editing !== "new" ? (
        <div className="empty px4">
          <Icon name="server" size={28} />
          <p className="pix">No servers yet</p>
          <p className="muted">Add one here or in game; both edit the same list.</p>
        </div>
      ) : (
        <div className="world-list">
          {servers.map((s, i) =>
            editing === i ? (
              <ServerForm
                key={`${s.address}-${i}`}
                initial={s}
                onCancel={() => setEditing(null)}
                onSave={(updated) => {
                  save(servers.map((x, j) => (j === i ? updated : x)));
                  if (updated.address !== s.address) pingAll([updated]);
                }}
              />
            ) : (
              <article key={`${s.address}-${i}`} className="world-card server-card px4">
                <ServerIcon ping={pings[s.address]} server={s} />
                <div className="world-main">
                  <div className="world-title">
                    <b>{s.name}</b>
                    <span className="muted small mono">{s.address}</span>
                  </div>
                  <Motd ping={pings[s.address]} />
                  <div className="world-actions">
                    <button className="tool-btn px2" disabled={locked} onClick={() => setEditing(i)}>
                      <Icon name="edit" /> Edit
                    </button>
                    <button className="tool-btn px2" disabled={locked || i === 0} aria-label="Move up" onClick={() => move(i, -1)}>
                      <Icon name="arrowUp" />
                    </button>
                    <button className="tool-btn px2" disabled={locked || i === servers.length - 1} aria-label="Move down" onClick={() => move(i, 1)}>
                      <Icon name="arrowDown" />
                    </button>
                    <button className="tool-btn danger px2" disabled={locked} aria-label={`Remove ${s.name}`} onClick={() => save(servers.filter((_, j) => j !== i))}>
                      <Icon name="trash" />
                    </button>
                  </div>
                </div>
                <div className="server-side">
                  <div className="server-stats">
                    {(() => {
                      const ping = pings[s.address];
                      return ping?.state === "ok" ? (
                        <span className="players" title={ping.status.sample.join("\n") || undefined}>
                          <b>{ping.status.online}</b>
                          <span className="muted">/{ping.status.max}</span>
                        </span>
                      ) : null;
                    })()}
                    <Signal ping={pings[s.address]} />
                  </div>
                  {(() => {
                    const ping = pings[s.address];
                    return ping?.state === "ok" ? <span className="muted small">{ping.status.version}</span> : null;
                  })()}
                  <B3D variant="play" size="sm" disabled={!canPlay || instance.running} onClick={() => onPlay({ server: s.address })} label={`Join ${s.name}`}>
                    <Icon name="play" size={12} />
                    <span className="pix">Join</span>
                  </B3D>
                </div>
              </article>
            ),
          )}
        </div>
      )}
    </div>
  );
}
