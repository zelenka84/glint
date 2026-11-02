import { useCallback, useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, type Accounts, type AppInfo, type Instance, type VersionList } from "./api";
import { Home } from "./Home";
import { DeleteInstanceModal, EditInstanceModal, KillInstanceModal, NewInstanceModal } from "./InstanceModals";
import { Library } from "./Library";
import { SparkleMark, Wordmark } from "./Logo";
import { applyEvent, type Session } from "./session";
import { Settings } from "./Settings";
import { sceneFor, useAppearance } from "./theme";
import { Icon, type IconName } from "./ui";
import "./fonts";
import "./App.css";

type Screen = "home" | "library" | "settings";

type Dialog =
  | { kind: "new" }
  | { kind: "edit"; instance: Instance }
  | { kind: "delete"; instance: Instance }
  | { kind: "kill"; instance: Instance }
  | null;

const NAV: { id: Screen | "mods" | "friends" | "studio"; label: string; icon: IconName; soon?: boolean }[] = [
  { id: "home", label: "Home", icon: "home" },
  { id: "library", label: "Library", icon: "library" },
  { id: "mods", label: "Mods", icon: "mods", soon: true },
  { id: "friends", label: "Friends", icon: "friends", soon: true },
  { id: "studio", label: "Glint Studio", icon: "studio", soon: true },
];

function TitleBar() {
  const win = getCurrentWindow();
  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="brand" data-tauri-drag-region>
        <SparkleMark />
        <Wordmark />
      </div>
      <div className="window-controls">
        <button aria-label="Minimize" onClick={() => win.minimize()}>
          <Icon name="minimize" />
        </button>
        <button aria-label="Maximize" onClick={() => win.toggleMaximize()}>
          <Icon name="maximize" />
        </button>
        <button className="close" aria-label="Close" onClick={() => win.close()}>
          <Icon name="close" size={12} />
        </button>
      </div>
    </header>
  );
}

function App() {
  const [appearance, setAppearance] = useAppearance();
  const [screen, setScreen] = useState<Screen>("home");
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [versions, setVersions] = useState<VersionList | null>(null);
  const [accounts, setAccounts] = useState<Accounts | null>(null);
  const [instances, setInstances] = useState<Instance[]>([]);
  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [sessions, setSessions] = useState<Record<string, Session>>({});
  const [dialog, setDialog] = useState<Dialog>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refreshInstances = useCallback(() => api.instances().then(setInstances, (e) => setNotice(String(e))), []);

  useEffect(() => {
    api.appInfo().then(setInfo);
    api.accounts().then(setAccounts, (e) => setNotice(String(e)));
    api.versions().then(setVersions, (e) => setNotice(`Could not load Minecraft versions: ${e}`));
    refreshInstances();
  }, [refreshInstances]);

  useEffect(() => {
    const unlisten = api.onPlayEvent(({ instance, ...event }) => {
      setSessions((all) => ({ ...all, [instance]: applyEvent(all[instance], event) }));
      if (event.type === "started" || event.type === "exited" || event.type === "failed") {
        refreshInstances();
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refreshInstances]);

  const selected = useMemo(
    () => instances.find((i) => i.name === selectedName) ?? instances[0] ?? null,
    [instances, selectedName],
  );
  const canPlay = !!accounts?.active;

  async function play(name: string) {
    setSelectedName(name);
    try {
      await api.play(name);
      setSessions((all) => ({ ...all, [name]: { phase: { kind: "busy", label: "Starting", done: 0, total: 1 }, log: [] } }));
    } catch (e) {
      setSessions((all) => ({ ...all, [name]: { phase: { kind: "failed", message: String(e) }, log: all[name]?.log ?? [] } }));
    }
  }

  async function openFolder(name: string) {
    try {
      await api.openInstanceFolder(name);
    } catch (e) {
      setNotice(String(e));
    }
  }

  const active = accounts?.accounts.find((a) => a.uuid === accounts.active) ?? null;

  return (
    <div className="app">
      <TitleBar />
      <div className="body">
        <nav className="sidebar" aria-label="Sections">
          {NAV.map((item) => (
            <button
              key={item.id}
              className={`nav-item px2 ${screen === item.id ? "active" : ""}`}
              disabled={item.soon}
              aria-current={screen === item.id ? "page" : undefined}
              onClick={() => !item.soon && setScreen(item.id as Screen)}
            >
              <Icon name={item.icon} />
              <span className="grow">{item.label}</span>
              {item.soon && <span className="soon pix">soon</span>}
            </button>
          ))}
          <div className="grow" />
          <button
            className={`nav-item px2 ${screen === "settings" ? "active" : ""}`}
            aria-current={screen === "settings" ? "page" : undefined}
            onClick={() => setScreen("settings")}
          >
            <Icon name="settings" />
            <span className="grow">Settings</span>
          </button>
          <button className="account-card px4" onClick={() => setScreen("settings")}>
            <span className="avatar px2">
              <Icon name="user" />
            </span>
            <span className="account-text">
              <span className="account-name">{active?.name ?? "No player"}</span>
              <span className="muted small">{active ? "Offline name" : "Add a name"}</span>
            </span>
          </button>
        </nav>

        <main className="main">
          {notice && (
            <div className="notice px2" role="alert">
              <span className="grow">{notice}</span>
              <button className="icon-btn" aria-label="Dismiss" onClick={() => setNotice(null)}>
                <Icon name="close" size={12} />
              </button>
            </div>
          )}
          {screen === "home" && (
            <Home
              scene={sceneFor(appearance.theme)}
              instances={instances}
              selected={selected}
              accounts={accounts}
              session={selected ? sessions[selected.name] : undefined}
              onSelect={setSelectedName}
              onPlay={play}
              onKill={(instance) => setDialog({ kind: "kill", instance })}
              onCreate={() => setDialog({ kind: "new" })}
              onOpenFolder={openFolder}
              onEdit={(instance) => setDialog({ kind: "edit", instance })}
              onLibrary={() => setScreen("library")}
              onAccount={async (uuid) => setAccounts(await api.setActiveAccount(uuid))}
            />
          )}
          {screen === "library" && (
            <Library
              instances={instances}
              sessions={sessions}
              canPlay={canPlay}
              onCreate={() => setDialog({ kind: "new" })}
              onOpen={(name) => {
                setSelectedName(name);
                setScreen("home");
              }}
              onPlay={play}
              onOpenFolder={openFolder}
              onEdit={(instance) => setDialog({ kind: "edit", instance })}
              onDelete={(instance) => setDialog({ kind: "delete", instance })}
              onKill={(instance) => setDialog({ kind: "kill", instance })}
            />
          )}
          {screen === "settings" && (
            <Settings
              appearance={appearance}
              onAppearance={setAppearance}
              accounts={accounts}
              info={info}
              onAddName={async (name) => setAccounts(await api.addOfflineAccount(name))}
              onRemove={async (uuid) => setAccounts(await api.removeAccount(uuid))}
              onActivate={async (uuid) => setAccounts(await api.setActiveAccount(uuid))}
            />
          )}
        </main>
      </div>

      {dialog?.kind === "new" && (
        <NewInstanceModal
          versions={versions}
          onClose={() => setDialog(null)}
          onCreate={async (instance) => {
            const name = await api.createInstance(instance);
            await refreshInstances();
            setSelectedName(name);
            setDialog(null);
            setScreen("home");
          }}
        />
      )}
      {dialog?.kind === "edit" && (
        <EditInstanceModal
          instance={dialog.instance}
          onClose={() => setDialog(null)}
          onSave={async (changes) => {
            const name = await api.updateInstance(dialog.instance.name, changes);
            await refreshInstances();
            if (selectedName === dialog.instance.name) setSelectedName(name);
            setDialog(null);
          }}
        />
      )}
      {dialog?.kind === "kill" && (
        <KillInstanceModal
          instance={dialog.instance}
          onClose={() => setDialog(null)}
          onKill={async () => {
            await api.killInstance(dialog.instance.name);
            setDialog(null);
          }}
        />
      )}
      {dialog?.kind === "delete" && (
        <DeleteInstanceModal
          instance={dialog.instance}
          onClose={() => setDialog(null)}
          onDelete={async () => {
            await api.deleteInstance(dialog.instance.name);
            await refreshInstances();
            setDialog(null);
          }}
        />
      )}
    </div>
  );
}

export default App;
