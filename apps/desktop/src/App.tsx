import { type FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { api, type Accounts, type PlayEvent, type VersionList } from "./api";
import { SparkleMark, Wordmark } from "./Logo";
import "./App.css";

type LogEntry = { id: number; level: string; message: string };

type Phase =
  | { kind: "idle" }
  | { kind: "busy"; label: string; done: number; total: number }
  | { kind: "running" }
  | { kind: "failed"; message: string };

const MAX_LOG = 500;
const SEGMENTS = 32;

const mb = (bytes: number) => (bytes / 1e6).toFixed(0);

function phaseFor(event: PlayEvent, previous: Phase): Phase {
  switch (event.type) {
    case "resolving":
      return { kind: "busy", label: `Checking ${event.version}`, done: 0, total: 1 };
    case "downloading": {
      const what = event.what === "java" ? "Java" : "game files";
      const label = `Downloading ${what} · ${mb(event.doneBytes)}/${mb(event.totalBytes)} MB`;
      return { kind: "busy", label, done: event.doneBytes, total: Math.max(event.totalBytes, 1) };
    }
    case "preparing":
      return { kind: "busy", label: "Preparing", done: 1, total: 1 };
    case "started":
      return { kind: "running" };
    case "exited":
      return event.code === 0 || event.code === null
        ? { kind: "idle" }
        : { kind: "failed", message: `The game exited with code ${event.code}. Check the log below.` };
    case "failed":
      return { kind: "failed", message: event.message };
    case "log":
      return previous;
  }
}

function App() {
  const [versions, setVersions] = useState<VersionList | null>(null);
  const [showSnapshots, setShowSnapshots] = useState(false);
  const [version, setVersion] = useState<string>("");
  const [accounts, setAccounts] = useState<Accounts | null>(null);
  const [newName, setNewName] = useState("");
  const [adding, setAdding] = useState(false);
  const [nameError, setNameError] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const [log, setLog] = useState<LogEntry[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const logId = useRef(0);
  const logEnd = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.versions().then(
      (list) => {
        setVersions(list);
        setVersion((current) => current || list.latestRelease);
      },
      (e) => setLoadError(String(e)),
    );
    api.accounts().then(setAccounts, (e) => setLoadError(String(e)));
  }, []);

  useEffect(() => {
    const unlisten = api.onPlayEvent((event) => {
      setPhase((previous) => phaseFor(event, previous));
      if (event.type === "log") {
        const entry = { id: logId.current++, level: event.level, message: event.message };
        setLog((entries) => [...entries.slice(-(MAX_LOG - 1)), entry]);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    logEnd.current?.scrollIntoView({ block: "end" });
  }, [log]);

  const visibleVersions = useMemo(
    () => (versions?.versions ?? []).filter((v) => showSnapshots || v.release),
    [versions, showSnapshots],
  );
  const active = accounts?.accounts.find((a) => a.uuid === accounts.active) ?? null;
  const busy = phase.kind === "busy" || phase.kind === "running";

  async function addName(e: FormEvent) {
    e.preventDefault();
    try {
      setAccounts(await api.addOfflineAccount(newName));
      setNewName("");
      setNameError(null);
      setAdding(false);
    } catch (err) {
      setNameError(String(err));
    }
  }

  async function play() {
    setLog([]);
    try {
      await api.play(version);
      setPhase({ kind: "busy", label: "Starting", done: 0, total: 1 });
    } catch (err) {
      setPhase({ kind: "failed", message: String(err) });
    }
  }

  const filled = phase.kind === "busy" ? Math.round((phase.done / phase.total) * SEGMENTS) : 0;

  return (
    <div className="app">
      <header className="titlebar">
        <div className="brand">
          <SparkleMark />
          <Wordmark />
        </div>
        <div className="account">
          {accounts && accounts.accounts.length > 0 && (
            <label>
              <span className="visually-hidden">Player</span>
              <select
                className="select px2"
                value={accounts.active ?? ""}
                disabled={busy}
                onChange={async (e) => setAccounts(await api.setActiveAccount(e.target.value))}
              >
                {accounts.accounts.map((a) => (
                  <option key={a.uuid} value={a.uuid}>
                    {a.name} · offline
                  </option>
                ))}
              </select>
            </label>
          )}
          {active && (
            <>
              <button
                className="b3d sm icon"
                aria-label="Add another name"
                title="Add another name"
                disabled={busy}
                onClick={() => setAdding((a) => !a)}
              >
                <span className="edge px2" />
                <span className="face px2 secondary">+</span>
              </button>
              <button
                className="b3d sm icon"
                aria-label={`Remove ${active.name}`}
                title={`Remove ${active.name}`}
                disabled={busy}
                onClick={async () => setAccounts(await api.removeAccount(active.uuid))}
              >
                <span className="edge px2" />
                <span className="face px2 secondary">×</span>
              </button>
            </>
          )}
        </div>
      </header>

      <main className="content">
        <section className="hero px4">
          <div className="plaque px4">
            <span className="eyebrow pix">VANILLA</span>
            <h1 className="pix">{version ? `Minecraft ${version}` : "Minecraft"}</h1>

            <div className="row">
              <label className="field">
                <span>Version</span>
                <select
                  className="select px2"
                  value={version}
                  disabled={busy || !versions}
                  onChange={(e) => setVersion(e.target.value)}
                >
                  {visibleVersions.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.id}
                      {v.id === versions?.latestRelease ? " (latest)" : ""}
                    </option>
                  ))}
                </select>
              </label>
              <label className="check">
                <input
                  type="checkbox"
                  checked={showSnapshots}
                  disabled={busy}
                  onChange={(e) => setShowSnapshots(e.target.checked)}
                />
                Snapshots
              </label>
            </div>

            {accounts && (!active || adding) && (
              <form className="row" onSubmit={addName}>
                <label className="field grow">
                  <span>Offline name</span>
                  <input
                    className="input px2"
                    value={newName}
                    placeholder="3–16 letters, digits or _"
                    maxLength={16}
                    onChange={(e) => setNewName(e.target.value)}
                  />
                </label>
                <button className="b3d sm" type="submit" disabled={!newName}>
                  <span className="edge px2" />
                  <span className="face px2 secondary">Add</span>
                </button>
              </form>
            )}
            {nameError && <p className="error">{nameError}</p>}
            {!accounts?.offlineAllowed && !active && (
              <p className="muted">Sign in once with a Microsoft account that owns Minecraft to use offline names.</p>
            )}

            <div className="row play-row">
              {phase.kind === "busy" ? (
                <div className="progress px4" role="status" aria-live="polite">
                  <span className="pix progress-label">{phase.label}</span>
                  <div className="xp" aria-hidden="true">
                    {Array.from({ length: SEGMENTS }, (_, i) => (
                      <span key={i} className={i < filled ? "on" : i === filled ? "on blink" : ""} />
                    ))}
                  </div>
                </div>
              ) : (
                <button
                  className={`b3d play ${phase.kind === "running" ? "" : "idle"}`}
                  disabled={!active || !version || phase.kind === "running"}
                  onClick={play}
                >
                  <span className="edge px4" />
                  <span className="face px4 pix">
                    <svg width="16" height="16" viewBox="0 0 8 8" shapeRendering="crispEdges" aria-hidden="true">
                      <path d="M1 0h2v8H1zM3 1h2v6H3zM5 2h2v4H5zM7 3h1v2H7z" fill="currentColor" />
                    </svg>
                    {phase.kind === "running" ? "RUNNING" : "PLAY"}
                  </span>
                </button>
              )}
            </div>
            {phase.kind === "failed" && <p className="error">{phase.message}</p>}
            {loadError && <p className="error">{loadError}</p>}
          </div>
        </section>

        <section className="log px4" aria-label="Game log">
          <div className="log-head">
            <h2 className="pix">Game log</h2>
            <span className="muted">{log.length ? `${log.length} lines` : "Appears when the game starts"}</span>
          </div>
          <div className="log-body">
            {log.map((entry) => (
              <pre key={entry.id} className={`line ${entry.level.toLowerCase()}`}>
                {entry.message}
              </pre>
            ))}
            <div ref={logEnd} />
          </div>
        </section>
      </main>
    </div>
  );
}

export default App;
