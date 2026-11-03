import { useCallback, useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PlayerHead, accountKindLabel } from "./Accounts";
import { api, type Accounts, type AppInfo, type ContentKind, type Instance, type ModHit, type PlayTarget, type VersionList } from "./api";
import { Home } from "./Home";
import { DeleteInstanceModal, EditInstanceModal, KillInstanceModal, NewInstanceModal } from "./InstanceModals";
import { InstancePage, type InstanceTab } from "./instance/InstancePage";
import { Library } from "./Library";
import { Onboarding, markOnboarded, onboarded } from "./Onboarding";
import { Discover } from "./content/Discover";
import { SparkleMark, Wordmark } from "./Logo";
import { applyEvent, type Session } from "./session";
import { Settings } from "./Settings";
import { sceneFor, useAppearance } from "./theme";
import { Icon, type IconName, Photo, isCustomCover } from "./ui";
import "./fonts";
import "./App.css";

type Screen = "home" | "library" | "mods" | "settings" | "instance";

type Dialog =
  | { kind: "new" }
  | { kind: "edit"; instance: Instance }
  | { kind: "delete"; instance: Instance }
  | { kind: "kill"; instance: Instance }
  | null;

const NAV: { id: Screen | "mods" | "friends" | "studio"; label: string; icon: IconName; soon?: boolean }[] = [
  { id: "home", label: "Home", icon: "home" },
  { id: "library", label: "Library", icon: "library" },
  { id: "mods", label: "Discover", icon: "search" },
  { id: "friends", label: "Friends", icon: "friends", soon: true },
  { id: "studio", label: "Glint Studio", icon: "studio", soon: true },
];

function TitleBar({ channel }: { channel: AppInfo["channel"] | undefined }) {
  const win = getCurrentWindow();
  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="brand" data-tauri-drag-region>
        <SparkleMark />
        <Wordmark />
        {channel && channel !== "public" && <span className={`channel-badge pix px2 ${channel}`}>{channel.toUpperCase()}</span>}
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
  const [page, setPage] = useState<{ name: string; tab: InstanceTab }>({ name: "", tab: "log" });
  const [tour, setTour] = useState(false);
  const [discoverOpen, setDiscoverOpen] = useState<{ kind: ContentKind; hit: ModHit } | null>(null);

  const refreshInstances = useCallback(() => api.instances().then(setInstances, (e) => setNotice(String(e))), []);

  useEffect(() => {
    api.appInfo().then(setInfo);
    api.versions().then(setVersions, (e) => setNotice(`Could not load Minecraft versions: ${e}`));
    Promise.all([api.accounts(), api.instances()]).then(
      ([accounts, instances]) => {
        setAccounts(accounts);
        setInstances(instances);
        // First launch: nothing set up yet. Anyone who already has players or games skips the tour.
        if (onboarded()) return;
        if (accounts.accounts.length === 0 && instances.length === 0) setTour(true);
        else markOnboarded();
      },
      (e) => setNotice(String(e)),
    );
  }, []);

  function finishTour() {
    markOnboarded();
    setTour(false);
    setScreen("home");
  }

  useEffect(() => {
    const unlisten = api.onPlayEvent(({ instance, ...event }) => {
      setSessions((all) => ({ ...all, [instance]: applyEvent(all[instance], event) }));
      if (["started", "exited", "failed", "killed"].includes(event.type)) {
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

  async function play(name: string, target?: PlayTarget) {
    setSelectedName(name);
    try {
      await api.play(name, target);
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
  const { background } = appearance;
  const backdrop = background.image && background.everywhere ? { ...background, image: background.image } : null;
  const pageInstance = instances.find((i) => i.name === page.name) ?? null;

  function openInstance(name: string, tab: InstanceTab = "log") {
    setSelectedName(name);
    setPage({ name, tab });
    setScreen("instance");
  }

  return (
    <div className={`app ${backdrop ? "has-backdrop" : ""}`}>
      {backdrop && <Photo key={backdrop.image} background={backdrop} className="app-backdrop" />}
      <TitleBar channel={info?.channel} />
      {tour ? (
        <Onboarding
          appearance={appearance}
          onAppearance={setAppearance}
          accounts={accounts}
          instances={instances}
          versions={versions}
          onAccounts={setAccounts}
          onCreated={async (name) => {
            await refreshInstances();
            setSelectedName(name);
          }}
          onFinish={finishTour}
        />
      ) : (
        <div className="body">
          <nav className="sidebar" aria-label="Sections">
            {NAV.map((item) => (
              <button
                key={item.id}
                className={`nav-item px2 ${screen === item.id || (screen === "instance" && item.id === "library") ? "active" : ""}`}
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
              <PlayerHead account={active} />
              <span className="account-text">
                <span className="account-name">{active?.name ?? "No player"}</span>
                <span className="muted small">{active ? accountKindLabel(active) : "Add a player"}</span>
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
                background={background}
                instances={instances}
                selected={selected}
                accounts={accounts}
                session={selected ? sessions[selected.name] : undefined}
                onSelect={setSelectedName}
                onPlay={play}
                onKill={(instance) => setDialog({ kind: "kill", instance })}
                onCreate={() => setDialog({ kind: "new" })}
                onOpenFolder={openFolder}
                onEdit={(instance) => openInstance(instance.name, "settings")}
                onManage={(instance) => openInstance(instance.name)}
                onLibrary={() => setScreen("library")}
                onAccount={async (uuid) => setAccounts(await api.setActiveAccount(uuid))}
                onPlayTarget={play}
                onOpenTab={openInstance}
                onDiscover={(kind, hit) => {
                  setDiscoverOpen({ kind, hit });
                  setScreen("mods");
                }}
              />
            )}
            {screen === "mods" && (
              <Discover
                instances={instances}
                initial={discoverOpen}
                onInitialUsed={() => setDiscoverOpen(null)}
                selected={selected?.name ?? null}
                onSelect={setSelectedName}
                onCreate={() => setDialog({ kind: "new" })}
              />
            )}
            {screen === "library" && (
              <Library
                instances={instances}
                sessions={sessions}
                canPlay={canPlay}
                onCreate={() => setDialog({ kind: "new" })}
                onOpen={(name) => openInstance(name)}
                onPlay={(name) => play(name)}
                onOpenFolder={openFolder}
                onEdit={(instance) => openInstance(instance.name, "settings")}
                onDelete={(instance) => setDialog({ kind: "delete", instance })}
                onKill={(instance) => setDialog({ kind: "kill", instance })}
                onChanged={refreshInstances}
              />
            )}
            {screen === "instance" && pageInstance && (
              <InstancePage
                instance={pageInstance}
                tab={page.tab}
                session={sessions[pageInstance.name]}
                accounts={accounts}
                onTab={(tab) => setPage((p) => ({ ...p, tab }))}
                onBack={() => setScreen("library")}
                onPlay={play}
                onKill={(instance) => setDialog({ kind: "kill", instance })}
                onOpenFolder={openFolder}
                onClearLog={(name) =>
                  setSessions((all) => (all[name] ? { ...all, [name]: { ...all[name], log: [] } } : all))
                }
                onChanged={async (newName) => {
                  await refreshInstances();
                  if (newName) {
                    setPage((p) => ({ ...p, name: newName }));
                    setSelectedName(newName);
                  }
                }}
                onDelete={(instance) => setDialog({ kind: "delete", instance })}
                versions={versions}
                experimental={!!info?.experimental}
              />
            )}
            {screen === "settings" && (
              <Settings
                appearance={appearance}
                onAppearance={setAppearance}
                accounts={accounts}
                info={info}
                onAddName={async (name) => setAccounts(await api.addOfflineAccount(name))}
                onElySignIn={async (login, password, totp) => setAccounts(await api.elySignIn(login, password, totp))}
                onRemove={async (uuid) => setAccounts(await api.removeAccount(uuid))}
                onActivate={async (uuid) => setAccounts(await api.setActiveAccount(uuid))}
                onTour={() => setTour(true)}
              />
            )}
          </main>
        </div>
      )}

      {dialog?.kind === "new" && (
        <NewInstanceModal
          versions={versions}
          experimental={!!info?.experimental}
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
            if (isCustomCover(dialog.instance.cover)) api.deleteMedia(dialog.instance.cover).catch(() => {});
            await refreshInstances();
            setDialog(null);
            if (screen === "instance" && page.name === dialog.instance.name) setScreen("library");
          }}
        />
      )}
    </div>
  );
}

export default App;
