import { type ReactNode } from "react";
import type { Accounts, Instance, PlayTarget, VersionList } from "../api";
import { PlayControl } from "../Home";
import type { Session } from "../session";
import { B3D, Cover, Icon, type IconName, coverFor, relativeTime } from "../ui";
import { ContentTab } from "../content/ContentTab";
import { LogsTab } from "./LogsTab";
import { LogTab } from "./LogTab";
import { NotesTab } from "./NotesTab";
import { VersionTab } from "./VersionTab";
import { ScreenshotsTab } from "./ScreenshotsTab";
import { ServersTab } from "./ServersTab";
import { SettingsTab } from "./SettingsTab";
import { WorldsTab } from "./WorldsTab";

export type InstanceTab =
  | "log"
  | "version"
  | "mods"
  | "resourcepacks"
  | "shaders"
  | "worlds"
  | "servers"
  | "screenshots"
  | "notes"
  | "settings"
  | "logs";

const TABS: { id: InstanceTab; label: string; icon: IconName; group?: string }[] = [
  { id: "log", label: "Game log", icon: "terminal" },
  { id: "version", label: "Version", icon: "cube" },
  { id: "mods", label: "Mods", icon: "mods", group: "Content" },
  { id: "resourcepacks", label: "Resource packs", icon: "brush" },
  { id: "shaders", label: "Shaders", icon: "sun" },
  { id: "worlds", label: "Worlds", icon: "globe", group: "Play" },
  { id: "servers", label: "Servers", icon: "server" },
  { id: "screenshots", label: "Screenshots", icon: "image" },
  { id: "notes", label: "Notes", icon: "notes", group: "Instance" },
  { id: "settings", label: "Settings", icon: "settings" },
  { id: "logs", label: "Other logs", icon: "file" },
];

export type InstancePageProps = {
  instance: Instance;
  tab: InstanceTab;
  session: Session | undefined;
  accounts: Accounts | null;
  onTab: (tab: InstanceTab) => void;
  onBack: () => void;
  onPlay: (name: string, target?: PlayTarget) => void;
  onKill: (instance: Instance) => void;
  onOpenFolder: (name: string) => void;
  onClearLog: (name: string) => void;
  onChanged: (newName?: string) => void;
  onDelete: (instance: Instance) => void;
  versions: VersionList | null;
};

export function InstancePage(props: InstancePageProps) {
  const { instance, tab } = props;
  const flavour = instance.loader ? `${instance.loader} ${instance.loaderVersion ?? ""}` : "Vanilla";

  let content: ReactNode;
  switch (tab) {
    case "log":
      content = <LogTab instance={instance} session={props.session} onClear={() => props.onClearLog(instance.name)} />;
      break;
    case "mods":
      content = <ContentTab instance={instance} kind="mod" />;
      break;
    case "resourcepacks":
      content = <ContentTab instance={instance} kind="resourcepack" />;
      break;
    case "shaders":
      content = <ContentTab instance={instance} kind="shader" />;
      break;
    case "worlds":
      content = (
        <WorldsTab instance={instance} canPlay={!!props.accounts?.active} onPlay={(target) => props.onPlay(instance.name, target)} />
      );
      break;
    case "servers":
      content = (
        <ServersTab instance={instance} canPlay={!!props.accounts?.active} onPlay={(target) => props.onPlay(instance.name, target)} />
      );
      break;
    case "screenshots":
      content = <ScreenshotsTab instance={instance} />;
      break;
    case "settings":
      content = <SettingsTab instance={instance} onChanged={props.onChanged} onDelete={() => props.onDelete(instance)} />;
      break;
    case "version":
      content = <VersionTab instance={instance} versions={props.versions} onChanged={() => props.onChanged()} />;
      break;
    case "notes":
      content = <NotesTab instance={instance} />;
      break;
    case "logs":
      content = <LogsTab instance={instance} />;
      break;
  }

  return (
    <div className="screen instance-page">
      <header className="ip-hero px4">
        <div className="ip-cover" aria-hidden="true">
          <Cover cover={coverFor(instance.cover, instance.name)} height={168} />
        </div>
        <div className="ip-head">
          <button className="ip-back px2" onClick={props.onBack}>
            <Icon name="back" />
            <span>Library</span>
          </button>
          <h1 className="pix">{instance.name}</h1>
          <div className="chips">
            <span className="chip px2">{instance.gameVersion}</span>
            <span className="chip px2">{flavour}</span>
            <span className="chip px2">{instance.maxMemoryMb / 1024} GB</span>
            <span className="chip px2">{relativeTime(instance.lastPlayed)}</span>
          </div>
        </div>
        <div className="ip-actions row">
          <PlayControl
            instance={instance}
            session={props.session}
            canPlay={!!props.accounts?.active}
            onPlay={() => props.onPlay(instance.name)}
            onKill={() => props.onKill(instance)}
          />
          <B3D size="lg" label="Open instance folder" className="square" onClick={() => props.onOpenFolder(instance.name)}>
            <Icon name="folder" size={20} />
          </B3D>
        </div>
      </header>

      <div className="ip-body">
        <nav className="ip-nav" aria-label="Instance sections">
          {TABS.map((t) => (
            <div key={t.id} className="ip-nav-entry">
              {t.group && <span className="label ip-group">{t.group.toUpperCase()}</span>}
              <button
                className={`ip-tab px2 ${tab === t.id ? "active" : ""}`}
                aria-current={tab === t.id ? "page" : undefined}
                onClick={() => props.onTab(t.id)}
              >
                <Icon name={t.icon} />
                <span>{t.label}</span>
              </button>
            </div>
          ))}
        </nav>
        <section className="ip-content" key={tab}>
          {content}
        </section>
      </div>
    </div>
  );
}
