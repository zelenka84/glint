import { type FormEvent, useEffect, useMemo, useState } from "react";
import { api, type Instance, type LoaderKind, type LoaderVersion, type NewInstance, type VersionList } from "./api";
import { B3D, Cover, type CoverName, Modal, covers } from "./ui";

function CoverPicker({ value, onChange }: { value: CoverName; onChange: (c: CoverName) => void }) {
  return (
    <fieldset className="cover-picker">
      <legend>Cover</legend>
      {(Object.keys(covers) as CoverName[]).map((key) => (
        <button
          key={key}
          type="button"
          className={`cover-option px2 ${key === value ? "selected" : ""}`}
          aria-pressed={key === value}
          aria-label={`Cover ${key}`}
          onClick={() => onChange(key)}
        >
          <Cover cover={key} height={44} />
        </button>
      ))}
    </fieldset>
  );
}

function MemoryField({ value, onChange }: { value: number; onChange: (mb: number) => void }) {
  return (
    <label className="field">
      <span>
        Memory · <b>{(value / 1024).toFixed(0)} GB</b>
      </span>
      <input
        type="range"
        min={2048}
        max={16384}
        step={1024}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
      />
    </label>
  );
}

const LOADERS: { id: LoaderKind | "vanilla"; label: string; hint: string }[] = [
  { id: "vanilla", label: "Vanilla", hint: "The game as Mojang ships it" },
  { id: "fabric", label: "Fabric", hint: "Light and fast, most performance mods" },
  { id: "quilt", label: "Quilt", hint: "Fabric fork, runs most Fabric mods" },
  { id: "forge", label: "Forge", hint: "The classic loader for big content mods" },
  { id: "neoforge", label: "NeoForge", hint: "Modern Forge successor, 1.20.2+" },
];

function LoaderPicker({ value, onChange }: { value: LoaderKind | "vanilla"; onChange: (v: LoaderKind | "vanilla") => void }) {
  return (
    <fieldset className="loader-picker">
      <legend>Mod loader</legend>
      {LOADERS.map((l) => (
        <button
          key={l.id}
          type="button"
          className={`loader-option px2 ${value === l.id ? "selected" : ""}`}
          aria-pressed={value === l.id}
          title={l.hint}
          onClick={() => onChange(l.id)}
        >
          <span className="pix">{l.label}</span>
        </button>
      ))}
    </fieldset>
  );
}

export function NewInstanceModal({
  versions,
  onCreate,
  onClose,
}: {
  versions: VersionList | null;
  onCreate: (instance: NewInstance) => Promise<void>;
  onClose: () => void;
}) {
  const [name, setName] = useState("");
  const [showSnapshots, setShowSnapshots] = useState(false);
  const [version, setVersion] = useState(versions?.latestRelease ?? "");
  const [loader, setLoader] = useState<LoaderKind | "vanilla">("vanilla");
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersion[] | null>(null);
  const [loaderVersion, setLoaderVersion] = useState("");
  const [loaderError, setLoaderError] = useState<string | null>(null);
  const [memory, setMemory] = useState(4096);
  const [jvmArgs, setJvmArgs] = useState("");
  const [advanced, setAdvanced] = useState(false);
  const [cover, setCover] = useState<CoverName>("meadow");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const options = useMemo(
    () => (versions?.versions ?? []).filter((v) => showSnapshots || v.release),
    [versions, showSnapshots],
  );

  useEffect(() => {
    setLoaderVersions(null);
    setLoaderVersion("");
    setLoaderError(null);
    if (loader === "vanilla" || !version) return;
    let cancelled = false;
    api.loaderVersions(loader, version).then(
      (list) => {
        if (cancelled) return;
        setLoaderVersions(list);
        setLoaderVersion((list.find((v) => v.recommended) ?? list[0]).id);
      },
      (e) => !cancelled && setLoaderError(String(e)),
    );
    return () => {
      cancelled = true;
    };
  }, [loader, version]);

  const loaderReady = loader === "vanilla" || !!loaderVersion;
  const flavour = LOADERS.find((l) => l.id === loader)!.label;

  async function submit(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    try {
      await onCreate({
        name,
        gameVersion: version,
        cover,
        maxMemoryMb: memory,
        loader: loader === "vanilla" ? null : { kind: loader, version: loaderVersion },
        extraJvmArgs: jvmArgs,
      });
    } catch (err) {
      setError(String(err));
      setSaving(false);
    }
  }

  return (
    <Modal title="New instance" onClose={onClose}>
      <form className="form" onSubmit={submit}>
        <label className="field">
          <span>Name</span>
          <input
            className="input px2"
            value={name}
            placeholder={version ? `${flavour} ${version}` : flavour}
            maxLength={48}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <div className="row">
          <label className="field grow">
            <span>Minecraft version</span>
            <select className="select px2" value={version} onChange={(e) => setVersion(e.target.value)}>
              {options.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.id}
                  {v.id === versions?.latestRelease ? " (latest)" : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="check">
            <input type="checkbox" checked={showSnapshots} onChange={(e) => setShowSnapshots(e.target.checked)} />
            Snapshots
          </label>
        </div>

        <LoaderPicker value={loader} onChange={setLoader} />
        {loader !== "vanilla" && (
          <label className="field">
            <span>{flavour} version</span>
            {loaderError ? (
              <p className="error">{loaderError}</p>
            ) : (
              <select
                className="select px2"
                value={loaderVersion}
                disabled={!loaderVersions}
                onChange={(e) => setLoaderVersion(e.target.value)}
              >
                {!loaderVersions && <option>Loading…</option>}
                {loaderVersions?.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.id}
                    {v.recommended ? " (recommended)" : ""}
                  </option>
                ))}
              </select>
            )}
          </label>
        )}

        <MemoryField value={memory} onChange={setMemory} />
        <CoverPicker value={cover} onChange={setCover} />

        <button type="button" className="disclosure" aria-expanded={advanced} onClick={() => setAdvanced((a) => !a)}>
          <span className="chevron">▸</span> Advanced
        </button>
        {advanced && (
          <label className="field">
            <span>Extra JVM arguments</span>
            <input
              className="input px2 mono"
              value={jvmArgs}
              placeholder="-XX:+UseZGC -XX:+ZGenerational"
              onChange={(e) => setJvmArgs(e.target.value)}
            />
            <span className="muted small">Separated by spaces. Memory is set above; leave this empty if unsure.</span>
          </label>
        )}

        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <B3D onClick={onClose}>Cancel</B3D>
          <B3D type="submit" variant="accent" disabled={!version || !loaderReady || saving}>
            <span className="pix">Create</span>
          </B3D>
        </div>
      </form>
    </Modal>
  );
}

export function EditInstanceModal({
  instance,
  onSave,
  onClose,
}: {
  instance: Instance;
  onSave: (changes: { name: string; maxMemoryMb: number; cover: string }) => Promise<void>;
  onClose: () => void;
}) {
  const [name, setName] = useState(instance.name);
  const [memory, setMemory] = useState(instance.maxMemoryMb);
  const [cover, setCover] = useState<CoverName>((instance.cover as CoverName) ?? "meadow");
  const [error, setError] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    try {
      await onSave({ name, maxMemoryMb: memory, cover });
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <Modal title="Edit instance" onClose={onClose}>
      <form className="form" onSubmit={submit}>
        <label className="field">
          <span>Name</span>
          <input className="input px2" value={name} maxLength={48} onChange={(e) => setName(e.target.value)} />
        </label>
        <p className="muted small">Minecraft {instance.gameVersion}</p>
        <MemoryField value={memory} onChange={setMemory} />
        <CoverPicker value={cover} onChange={setCover} />
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <B3D onClick={onClose}>Cancel</B3D>
          <B3D type="submit" variant="accent" disabled={!name.trim()}>
            <span className="pix">Save</span>
          </B3D>
        </div>
      </form>
    </Modal>
  );
}

export function KillInstanceModal({
  instance,
  onKill,
  onClose,
}: {
  instance: Instance;
  onKill: () => Promise<void>;
  onClose: () => void;
}) {
  const [error, setError] = useState<string | null>(null);
  return (
    <Modal title={`Kill ${instance.name}?`} onClose={onClose}>
      <div className="form">
        <p className="warning-box px2">
          The game closes immediately, without saving. Progress since the last autosave is lost, and a world
          that was being written at that moment can end up damaged.
        </p>
        <p className="muted small">Use this only when the game has frozen and will not close normally.</p>
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <B3D onClick={onClose}>Cancel</B3D>
          <B3D variant="danger" onClick={() => onKill().catch((e) => setError(String(e)))}>
            <span className="pix">Kill instance</span>
          </B3D>
        </div>
      </div>
    </Modal>
  );
}

export function DeleteInstanceModal({
  instance,
  onDelete,
  onClose,
}: {
  instance: Instance;
  onDelete: () => Promise<void>;
  onClose: () => void;
}) {
  const [error, setError] = useState<string | null>(null);
  return (
    <Modal title={`Delete ${instance.name}?`} onClose={onClose}>
      <div className="form">
        <p>Worlds, screenshots, settings and mods in this instance will be deleted. This cannot be undone.</p>
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <B3D onClick={onClose}>Cancel</B3D>
          <B3D variant="danger" onClick={() => onDelete().catch((e) => setError(String(e)))}>
            <span className="pix">Delete</span>
          </B3D>
        </div>
      </div>
    </Modal>
  );
}
