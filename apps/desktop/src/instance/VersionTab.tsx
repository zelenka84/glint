import { useEffect, useMemo, useState } from "react";
import { api, type Engine, type Instance, type LoaderKind, type LoaderVersion, type VersionList } from "../api";
import { EnginePicker, HORIZON_VERSION, HorizonNote, LOADERS, LoaderPicker } from "../InstanceModals";
import { B3D, Icon, type IconName } from "../ui";

const KIND_BY_LABEL: Record<string, LoaderKind> = { Fabric: "fabric", Quilt: "quilt", Forge: "forge", NeoForge: "neoforge" };

function Component({ icon, name, version, note }: { icon: IconName; name: string; version: string; note?: string }) {
  return (
    <div className="component-row px2">
      <span className="component-icon px2">
        <Icon name={icon} />
      </span>
      <span className="grow">
        <b>{name}</b>
        {note && <span className="muted small"> · {note}</span>}
      </span>
      <span className="mono">{version}</span>
    </div>
  );
}

export function VersionTab({
  instance,
  versions,
  experimental,
  onChanged,
}: {
  instance: Instance;
  versions: VersionList | null;
  experimental: boolean;
  onChanged: () => void;
}) {
  const currentKind = instance.loader ? KIND_BY_LABEL[instance.loader] : "vanilla";
  const [engine, setEngine] = useState<Engine>(instance.engine);
  const horizon = engine === "horizon";
  const [gameVersion, setGameVersion] = useState(instance.gameVersion);
  const [loader, setLoader] = useState<LoaderKind | "vanilla">(currentKind);
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersion[] | null>(null);
  const [loaderVersion, setLoaderVersion] = useState(instance.loaderVersion ?? "");
  const [snapshots, setSnapshots] = useState(false);
  const [modCount, setModCount] = useState(0);
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api.installedContent(instance.name, "mod").then((m) => setModCount(m.length), () => {});
  }, [instance.name]);

  useEffect(() => {
    setLoaderVersions(null);
    if (horizon || loader === "vanilla") return;
    let cancelled = false;
    api.loaderVersions(loader, gameVersion).then(
      (list) => {
        if (cancelled) return;
        setLoaderVersions(list);
        const keep = loader === currentKind && gameVersion === instance.gameVersion && list.some((v) => v.id === instance.loaderVersion);
        setLoaderVersion(keep ? instance.loaderVersion! : (list.find((v) => v.recommended) ?? list[0])?.id ?? "");
      },
      (e) => {
        if (cancelled) return;
        setLoaderVersions([]);
        setStatus({ ok: false, text: String(e) });
      },
    );
    return () => {
      cancelled = true;
    };
  }, [horizon, loader, gameVersion, currentKind, instance.gameVersion, instance.loaderVersion]);

  const options = useMemo(() => (versions?.versions ?? []).filter((v) => snapshots || v.release || v.id === instance.gameVersion), [versions, snapshots, instance.gameVersion]);
  const release = versions?.versions.find((v) => v.id === instance.gameVersion)?.releaseTime;
  const changed =
    engine !== instance.engine ||
    (!horizon &&
      (gameVersion !== instance.gameVersion || loader !== currentKind || (loader !== "vanilla" && loaderVersion !== (instance.loaderVersion ?? ""))));
  const ready = horizon || loader === "vanilla" || !!loaderVersion;
  const flavour = LOADERS.find((l) => l.id === loader)!.label;

  async function apply() {
    setSaving(true);
    try {
      await api.changeVersion(
        instance.name,
        horizon ? HORIZON_VERSION : gameVersion,
        horizon || loader === "vanilla" ? null : { kind: loader, version: loaderVersion },
        engine,
      );
      setStatus({ ok: true, text: "Saved. The next launch downloads what is needed." });
      onChanged();
    } catch (e) {
      setStatus({ ok: false, text: String(e) });
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="tab-pane">
      <div className="pane-head">
        <h2 className="pix">Version</h2>
        <div className="grow" />
        <button className="tool-btn px2" onClick={() => api.openInstanceFolder(instance.name)}>
          <Icon name="folder" /> Game folder
        </button>
        <button className="tool-btn px2" onClick={() => api.openLibrariesFolder()}>
          <Icon name="archive" /> Libraries
        </button>
      </div>

      <section className="set-card px4">
        <header>
          <h3 className="pix">Components</h3>
          <p className="muted small">What this instance runs, in load order.</p>
        </header>
        <div className="component-list">
          <Component icon="cube" name="Minecraft" version={instance.gameVersion} note={release ? `released ${new Date(release).toLocaleDateString()}` : undefined} />
          {instance.engine === "horizon" ? (
            <Component icon="terminal" name="Horizon" version="Nightly" note="experimental Rust engine, updates itself on launch" />
          ) : instance.loader ? (
            <Component icon="mods" name={instance.loader} version={instance.loaderVersion ?? ""} note="mod loader" />
          ) : (
            <div className="component-row muted px2">
              <span className="component-icon px2"><Icon name="mods" /></span>
              <span className="grow">No mod loader · vanilla</span>
            </div>
          )}
          {instance.engine !== "horizon" && (
            <Component icon="terminal" name="Java" version="Auto" note="picked for this Minecraft version, change it in Settings" />
          )}
        </div>
      </section>

      <section className="set-card px4">
        <header>
          <h3 className="pix">Change version</h3>
          <p className="muted small">Worlds stay as they are. Opening a world in an older version than it was saved in can break it; back it up first.</p>
        </header>
        {(experimental || instance.engine === "horizon") && <EnginePicker value={engine} onChange={setEngine} />}
        {horizon ? (
          <HorizonNote />
        ) : (
          <>
        <div className="row">
          <label className="field grow">
            <span>Minecraft version</span>
            <select className="select px2" value={gameVersion} onChange={(e) => setGameVersion(e.target.value)}>
              {options.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.id}
                  {v.id === versions?.latestRelease ? " (latest)" : ""}
                  {v.id === instance.gameVersion ? " (current)" : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="check">
            <input type="checkbox" checked={snapshots} onChange={(e) => setSnapshots(e.target.checked)} />
            Snapshots
          </label>
        </div>
        <LoaderPicker value={loader} onChange={setLoader} />
        {loader !== "vanilla" && (
          <label className="field">
            <span>{flavour} version</span>
            <select className="select px2" value={loaderVersion} disabled={!loaderVersions} onChange={(e) => setLoaderVersion(e.target.value)}>
              {!loaderVersions && <option>Loading…</option>}
              {loaderVersions?.length === 0 && <option value="">No {flavour} for {gameVersion}</option>}
              {loaderVersions?.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.id}
                  {v.recommended ? " (recommended)" : ""}
                  {v.id === instance.loaderVersion && loader === currentKind ? " (current)" : ""}
                </option>
              ))}
            </select>
          </label>
        )}
          </>
        )}
        {changed && modCount > 0 && (
          <p className="hint px2">
            <Icon name="warn" /> {modCount} mod{modCount > 1 ? "s" : ""} stay installed.
            {horizon
              ? " Horizon does not run Java mods."
              : loader === "vanilla"
              ? " Vanilla ignores them."
              : " Some may not work with the new version; use Check updates in Mods afterwards."}
          </p>
        )}
        {status && <p className={`small ${status.ok ? "ok-text" : "error"}`}>{status.text}</p>}
        <div className="row">
          <B3D variant="accent" size="sm" disabled={!changed || !ready || saving || instance.running} onClick={apply}>
            {saving ? <span className="spinner small" /> : <Icon name="check" size={12} />}
            <span className="pix">Apply</span>
          </B3D>
          {changed && (
            <button
              className="tool-btn px2"
              onClick={() => {
                setEngine(instance.engine);
                setGameVersion(instance.gameVersion);
                setLoader(currentKind);
              }}
            >
              Reset
            </button>
          )}
        </div>
      </section>
    </div>
  );
}
