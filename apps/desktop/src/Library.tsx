import type { Instance } from "./api";
import type { Session } from "./session";
import { B3D, Cover, Icon, XpBar, coverFor, relativeTime } from "./ui";

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
};

export function Library(props: Props) {
  return (
    <div className="screen">
      <div className="screen-head">
        <h1 className="pix">Library</h1>
        <B3D variant="accent" onClick={props.onCreate}>
          <Icon name="plus" />
          <span className="pix">New instance</span>
        </B3D>
      </div>

      {props.instances.length === 0 ? (
        <div className="empty px4">
          <p className="pix">No instances yet</p>
          <p className="muted">Create one to pick a Minecraft version and start playing.</p>
        </div>
      ) : (
        <div className="card-grid">
          {props.instances.map((instance) => {
            const phase = props.sessions[instance.name]?.phase;
            const running = instance.running || phase?.kind === "running";
            return (
              <article key={instance.name} className="instance-card px4">
                <button className="cover-button" onClick={() => props.onOpen(instance.name)} aria-label={`Open ${instance.name}`}>
                  <Cover cover={coverFor(instance.cover, instance.name)} height={100} />
                  {running && <span className="badge pix px2">RUNNING</span>}
                </button>
                <div className="instance-body">
                  <span className="card-title">{instance.name}</span>
                  <span className="card-meta">
                    {instance.gameVersion} · {instance.loader ?? "Vanilla"} · {relativeTime(instance.lastPlayed)}
                  </span>
                  {phase?.kind === "busy" ? (
                    <XpBar done={phase.done} total={phase.total} />
                  ) : (
                    <div className="instance-actions">
                      <B3D
                        variant="play"
                        size="sm"
                        disabled={!props.canPlay || running}
                        onClick={() => props.onPlay(instance.name)}
                      >
                        <Icon name="play" size={12} />
                        <span className="pix">{running ? "Running" : "Play"}</span>
                      </B3D>
                      <B3D size="sm" label="Open folder" className="square" onClick={() => props.onOpenFolder(instance.name)}>
                        <Icon name="folder" />
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
          })}
        </div>
      )}
    </div>
  );
}
