import { useEffect, useRef, useState } from "react";
import type { Accounts, Instance } from "./api";
import { scenes, type SceneName } from "./scenes";
import type { Session } from "./session";
import { B3D, Cover, Icon, XpBar, coverFor, relativeTime } from "./ui";

type Props = {
  scene: SceneName;
  instances: Instance[];
  selected: Instance | null;
  accounts: Accounts | null;
  session: Session | undefined;
  onSelect: (name: string) => void;
  onPlay: (name: string) => void;
  onKill: (instance: Instance) => void;
  onCreate: () => void;
  onOpenFolder: (name: string) => void;
  onEdit: (instance: Instance) => void;
  onLibrary: () => void;
  onAccount: (uuid: string) => void;
};

function PlayControl({ instance, session, canPlay, onPlay, onKill }: {
  instance: Instance;
  session: Session | undefined;
  canPlay: boolean;
  onPlay: () => void;
  onKill: () => void;
}) {
  const phase = session?.phase ?? { kind: "idle" };
  if (phase.kind === "busy") {
    return (
      <div className="progress px4" role="status" aria-live="polite">
        <span className="pix progress-label">{phase.label}</span>
        <XpBar done={phase.done} total={phase.total} />
      </div>
    );
  }
  const running = phase.kind === "running" || instance.running;
  return (
    <>
      <B3D variant="play" size="lg" idle={!running} disabled={!canPlay || running} onClick={onPlay}>
        <Icon name="play" size={16} />
        <span className="pix">{running ? "RUNNING" : "PLAY"}</span>
      </B3D>
      {running && (
        <B3D variant="danger" size="lg" className="square" label="Kill instance" onClick={onKill}>
          <Icon name="stop" size={18} />
        </B3D>
      )}
    </>
  );
}

export function Home(props: Props) {
  const { selected, session, accounts } = props;
  const active = accounts?.accounts.find((a) => a.uuid === accounts.active) ?? null;
  const [logOpen, setLogOpen] = useState(false);
  const logEnd = useRef<HTMLDivElement>(null);
  const log = session?.log ?? [];

  useEffect(() => {
    if (session?.phase.kind === "failed") setLogOpen(true);
  }, [session?.phase.kind]);
  useEffect(() => {
    if (logOpen) logEnd.current?.scrollIntoView({ block: "end" });
  }, [log, logOpen]);

  return (
    <div className="screen home">
      <section className="hero px4">
        <div className="scene" dangerouslySetInnerHTML={{ __html: scenes[props.scene] }} />
        <div className="plaque px4">
          {selected ? (
            <>
              <span className="eyebrow pix">{selected.lastPlayed ? "CONTINUE PLAYING" : "READY TO PLAY"}</span>
              <h1 className="pix">{selected.name}</h1>
              <div className="chips">
                <span className="chip px2">{selected.gameVersion}</span>
                <span className="chip px2">{selected.loader ? `${selected.loader} ${selected.loaderVersion ?? ""}` : "Vanilla"}</span>
                <span className="chip px2">{selected.maxMemoryMb / 1024} GB</span>
                <span className="chip px2">{relativeTime(selected.lastPlayed)}</span>
              </div>
              <div className="row play-row">
                <PlayControl
                  instance={selected}
                  session={session}
                  canPlay={!!active}
                  onPlay={() => props.onPlay(selected.name)}
                  onKill={() => props.onKill(selected)}
                />
                <B3D size="lg" label="Open instance folder" className="square" onClick={() => props.onOpenFolder(selected.name)}>
                  <Icon name="folder" size={20} />
                </B3D>
                <B3D size="lg" label="Edit instance" className="square" onClick={() => props.onEdit(selected)}>
                  <Icon name="edit" size={20} />
                </B3D>
                {accounts && accounts.accounts.length > 0 && (
                  <label className="field">
                    <span>Play as</span>
                    <select
                      className="select px2"
                      value={accounts.active ?? ""}
                      onChange={(e) => props.onAccount(e.target.value)}
                    >
                      {accounts.accounts.map((a) => (
                        <option key={a.uuid} value={a.uuid}>
                          {a.name}
                          {a.kind === "ely" ? " · Ely.by" : ""}
                        </option>
                      ))}
                    </select>
                  </label>
                )}
              </div>
              {!active && <p className="muted small">Add a player name in Settings to play.</p>}
              {session?.phase.kind === "failed" && <p className="error">{session.phase.message}</p>}
            </>
          ) : (
            <>
              <span className="eyebrow pix">WELCOME TO GLINT</span>
              <h1 className="pix">Create your first instance</h1>
              <p className="muted">An instance is a separate copy of the game with its own version, worlds and settings.</p>
              <div className="row play-row">
                <B3D variant="accent" size="lg" onClick={props.onCreate}>
                  <Icon name="plus" size={16} />
                  <span className="pix">New instance</span>
                </B3D>
              </div>
            </>
          )}
        </div>
      </section>

      {props.instances.length > 0 && (
        <section className="recent">
          <div className="section-head">
            <h2 className="pix">Library</h2>
            <button className="link" onClick={props.onLibrary}>
              All instances
            </button>
          </div>
          <div className="card-grid four">
            {props.instances.slice(0, 4).map((instance) => (
              <button
                key={instance.name}
                className={`lift ${instance.name === selected?.name ? "current" : ""}`}
                onClick={() => props.onSelect(instance.name)}
              >
                <span className="edge px4" />
                <span className="face px4 card">
                  <Cover cover={coverFor(instance.cover, instance.name)} height={80} />
                  {instance.running && <span className="badge pix px2">RUNNING</span>}
                  <span className="card-title">{instance.name}</span>
                  <span className="card-meta">{instance.gameVersion} · {relativeTime(instance.lastPlayed)}</span>
                </span>
              </button>
            ))}
          </div>
        </section>
      )}

      {selected && (
        <section className={`log px4 ${logOpen ? "open" : ""}`}>
          <button className="log-head" onClick={() => setLogOpen((o) => !o)} aria-expanded={logOpen}>
            <h2 className="pix">Game log</h2>
            <span className="muted small">{log.length ? `${log.length} lines` : "Appears when the game starts"}</span>
            <span className="chevron">{logOpen ? "▾" : "▸"}</span>
          </button>
          {logOpen && (
            <div className="log-body">
              {log.map((entry) => (
                <pre key={entry.id} className={`line ${entry.level.toLowerCase()}`}>
                  {entry.message}
                </pre>
              ))}
              <div ref={logEnd} />
            </div>
          )}
        </section>
      )}
    </div>
  );
}
