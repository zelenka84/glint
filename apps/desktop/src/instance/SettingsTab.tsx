import { type ReactNode, useEffect, useMemo, useState } from "react";
import { api, type Instance, type InstanceSettings, type JavaInstall, type World } from "../api";
import { CoverPicker } from "../InstanceModals";
import { B3D, Icon, Switch, coverFor, isCustomCover } from "../ui";

const JVM_PRESETS: { label: string; hint: string; args: string }[] = [
  {
    label: "Aikar's G1GC",
    hint: "Tuned garbage collector flags, good for big modpacks",
    args: "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:InitiatingHeapOccupancyPercent=15",
  },
  { label: "Generational ZGC", hint: "Very short pauses on Java 21+, uses a bit more memory", args: "-XX:+UseZGC -XX:+ZGenerational" },
  { label: "Clear", hint: "Back to the game's defaults", args: "" },
];

const SIZES: [number, number][] = [
  [854, 480],
  [1280, 720],
  [1600, 900],
  [1920, 1080],
];

function Section({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <section className="set-card px4">
      <header>
        <h3 className="pix">{title}</h3>
        {hint && <p className="muted small">{hint}</p>}
      </header>
      {children}
    </section>
  );
}

function gb(mb: number) {
  return mb % 1024 === 0 ? `${mb / 1024} GB` : `${(mb / 1024).toFixed(1)} GB`;
}

export function SettingsTab({ instance, onChanged, onDelete }: {
  instance: Instance;
  onChanged: (newName?: string) => void;
  onDelete: () => void;
}) {
  const [saved, setSaved] = useState<InstanceSettings | null>(null);
  const [draft, setDraft] = useState<InstanceSettings | null>(null);
  const [worlds, setWorlds] = useState<World[]>([]);
  const [javas, setJavas] = useState<JavaInstall[] | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [javaCheck, setJavaCheck] = useState<{ ok: boolean; text: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.instanceSettings(instance.name).then(
      (s) => {
        setSaved(s);
        setDraft(s);
      },
      (e) => setError(String(e)),
    );
    api.listWorlds(instance.name).then(setWorlds, () => {});
  }, [instance.name]);

  const dirty = useMemo(() => JSON.stringify(saved) !== JSON.stringify(draft), [saved, draft]);
  if (!draft) return <div className="skeleton-list">{[0, 1, 2].map((i) => <div key={i} className="skeleton tall px4" />)}</div>;

  const set = (patch: Partial<InstanceSettings>) => setDraft((d) => (d ? { ...d, ...patch } : d));
  const autoJoin = draft.autoJoinServer !== null ? "server" : draft.autoJoinWorld !== null ? "world" : "none";

  async function save() {
    if (!draft) return;
    setSaving(true);
    try {
      const name = await api.saveInstanceSettings(instance.name, draft);
      // A replaced picture is no longer used anywhere.
      if (isCustomCover(saved?.cover ?? null) && saved!.cover !== draft.cover) api.deleteMedia(saved!.cover!).catch(() => {});
      const fresh = { ...draft, name };
      setSaved(fresh);
      setDraft(fresh);
      setError(null);
      onChanged(name !== instance.name ? name : undefined);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function detect() {
    setDetecting(true);
    try {
      setJavas(await api.detectJava());
    } catch (e) {
      setError(String(e));
    } finally {
      setDetecting(false);
    }
  }

  async function checkJava(path: string) {
    try {
      setJavaCheck({ ok: true, text: `Java ${await api.checkJava(path)}` });
    } catch (e) {
      setJavaCheck({ ok: false, text: String(e) });
    }
  }

  return (
    <div className="tab-pane settings-tab">
      <div className="pane-head">
        <h2 className="pix">Settings</h2>
        <span className="muted small">Only for this instance</span>
      </div>
      {error && (
        <div className="notice px2" role="alert">
          <span className="grow">{error}</span>
          <button className="icon-btn" aria-label="Dismiss" onClick={() => setError(null)}>
            <Icon name="close" size={12} />
          </button>
        </div>
      )}

      <Section title="General">
        <label className="field">
          <span>Name</span>
          <input className="input px2" value={draft.name} maxLength={48} onChange={(e) => set({ name: e.target.value })} />
        </label>
        <CoverPicker value={coverFor(draft.cover, instance.name)} onChange={(c) => set({ cover: c })} />
      </Section>

      <Section title="Memory" hint="How much RAM the game may use. More than 8 GB rarely helps unless the modpack is huge.">
        <label className="field">
          <span>
            Maximum · <b>{gb(draft.maxMemoryMb)}</b>
          </span>
          <input type="range" min={1024} max={32768} step={512} value={draft.maxMemoryMb} onChange={(e) => set({ maxMemoryMb: Number(e.target.value), minMemoryMb: draft.minMemoryMb === null ? null : Math.min(draft.minMemoryMb, Number(e.target.value)) })} />
        </label>
        <div className="set-row">
          <Switch on={draft.minMemoryMb !== null} label="Reserve memory at start" onChange={(on) => set({ minMemoryMb: on ? Math.min(2048, draft.maxMemoryMb) : null })} />
          <span>Reserve memory at start (-Xms)</span>
        </div>
        {draft.minMemoryMb !== null && (
          <label className="field">
            <span>
              Minimum · <b>{gb(draft.minMemoryMb)}</b>
            </span>
            <input type="range" min={512} max={draft.maxMemoryMb} step={512} value={draft.minMemoryMb} onChange={(e) => set({ minMemoryMb: Number(e.target.value) })} />
          </label>
        )}
      </Section>

      <Section title="Java" hint="Glint downloads the right Java for each Minecraft version. Pick your own only if you know you need it.">
        <div className="segmented px2" role="radiogroup" aria-label="Java">
          <button role="radio" aria-checked={draft.javaPath === null} className={draft.javaPath === null ? "on" : ""} onClick={() => set({ javaPath: null })}>
            Automatic
          </button>
          <button role="radio" aria-checked={draft.javaPath !== null} className={draft.javaPath !== null ? "on" : ""} onClick={() => set({ javaPath: draft.javaPath ?? "" })}>
            Custom
          </button>
        </div>
        {draft.javaPath !== null && (
          <>
            <div className="set-row">
              <input className="input px2 mono grow" value={draft.javaPath} placeholder="C:\Program Files\Java\jdk-21\bin\javaw.exe" onChange={(e) => { set({ javaPath: e.target.value }); setJavaCheck(null); }} />
              <button className="tool-btn px2" onClick={() => checkJava(draft.javaPath ?? "")} disabled={!draft.javaPath}>
                <Icon name="check" /> Test
              </button>
              <button className="tool-btn px2" onClick={detect} disabled={detecting}>
                {detecting ? <span className="spinner small" /> : <Icon name="search" />} Find installed
              </button>
            </div>
            {javaCheck && <p className={`small ${javaCheck.ok ? "ok-text" : "error"}`}>{javaCheck.text}</p>}
            {javas && (
              <div className="java-list">
                {javas.length === 0 && <p className="muted small">No Java installs found.</p>}
                {javas.map((j) => (
                  <button key={j.path} className={`java-option px2 ${draft.javaPath === j.path ? "selected" : ""}`} onClick={() => { set({ javaPath: j.path }); setJavaCheck(j.version ? { ok: true, text: `Java ${j.version}` } : null); }}>
                    <b>{j.version ? `Java ${j.version}` : "Unknown version"}</b>
                    <span className="tag-muted px2">{j.source}</span>
                    <span className="muted small mono grow">{j.path}</span>
                  </button>
                ))}
              </div>
            )}
          </>
        )}
        <label className="field">
          <span>JVM arguments</span>
          <textarea className="input textarea px2 mono" rows={3} value={draft.jvmArgs} placeholder="-XX:+UseZGC" onChange={(e) => set({ jvmArgs: e.target.value })} />
        </label>
        <div className="chips">
          {JVM_PRESETS.map((p) => (
            <button key={p.label} className="chip-btn px2" title={p.hint} onClick={() => set({ jvmArgs: p.args })}>
              {p.label}
            </button>
          ))}
        </div>
      </Section>

      <Section title="Game window">
        <div className="set-row">
          <input className="input px2 num" type="number" min={320} placeholder="854" value={draft.width ?? ""} onChange={(e) => set({ width: e.target.value ? Number(e.target.value) : null })} aria-label="Width" />
          <span className="muted">×</span>
          <input className="input px2 num" type="number" min={240} placeholder="480" value={draft.height ?? ""} onChange={(e) => set({ height: e.target.value ? Number(e.target.value) : null })} aria-label="Height" />
          {SIZES.map(([w, h]) => (
            <button key={w} className={`chip-btn px2 ${draft.width === w && draft.height === h ? "on" : ""}`} onClick={() => set({ width: w, height: h })}>
              {w}×{h}
            </button>
          ))}
        </div>
        <div className="set-row">
          <Switch on={draft.fullscreen} label="Start in fullscreen" onChange={(fullscreen) => set({ fullscreen })} />
          <span>Start in fullscreen</span>
        </div>
      </Section>

      <Section title="Join on launch" hint="Skip the title screen. Opening a world needs Minecraft 1.20 or newer.">
        <div className="segmented px2" role="radiogroup" aria-label="Join on launch">
          {(["none", "server", "world"] as const).map((mode) => (
            <button
              key={mode}
              role="radio"
              aria-checked={autoJoin === mode}
              className={autoJoin === mode ? "on" : ""}
              onClick={() =>
                set({
                  autoJoinServer: mode === "server" ? draft.autoJoinServer ?? "" : null,
                  autoJoinWorld: mode === "world" ? draft.autoJoinWorld ?? worlds[0]?.folder ?? "" : null,
                })
              }
            >
              {mode === "none" ? "Title screen" : mode === "server" ? "Server" : "World"}
            </button>
          ))}
        </div>
        {autoJoin === "server" && (
          <input className="input px2 mono" value={draft.autoJoinServer ?? ""} placeholder="play.example.net" onChange={(e) => set({ autoJoinServer: e.target.value })} />
        )}
        {autoJoin === "world" &&
          (worlds.length ? (
            <select className="select px2" value={draft.autoJoinWorld ?? ""} onChange={(e) => set({ autoJoinWorld: e.target.value })}>
              {worlds.map((w) => (
                <option key={w.folder} value={w.folder}>
                  {w.name}
                </option>
              ))}
            </select>
          ) : (
            <p className="muted small">This instance has no worlds yet.</p>
          ))}
      </Section>

      <Section title="Launch commands" hint="Shell commands around the game. They get $INST_NAME, $INST_DIR, $INST_MC_DIR, $INST_JAVA and $INST_JAVA_ARGS.">
        <label className="field">
          <span>Before launch (a failure cancels the launch)</span>
          <input className="input px2 mono" value={draft.preLaunch ?? ""} placeholder="echo starting" onChange={(e) => set({ preLaunch: e.target.value })} />
        </label>
        <label className="field">
          <span>Wrapper (runs Java through this program)</span>
          <input className="input px2 mono" value={draft.wrapper ?? ""} placeholder="gamemoderun" onChange={(e) => set({ wrapper: e.target.value })} />
        </label>
        <label className="field">
          <span>After exit</span>
          <input className="input px2 mono" value={draft.postExit ?? ""} placeholder="echo bye" onChange={(e) => set({ postExit: e.target.value })} />
        </label>
      </Section>

      <Section title="Environment variables">
        {draft.env.map(([key, value], i) => (
          <div key={i} className="set-row env-row">
            <input className="input px2 mono" value={key} placeholder="NAME" onChange={(e) => set({ env: draft.env.map((kv, j) => (j === i ? [e.target.value, kv[1]] : kv)) })} />
            <span className="muted">=</span>
            <input className="input px2 mono grow" value={value} placeholder="value" onChange={(e) => set({ env: draft.env.map((kv, j) => (j === i ? [kv[0], e.target.value] : kv)) })} />
            <button className="icon-btn danger" aria-label="Remove variable" onClick={() => set({ env: draft.env.filter((_, j) => j !== i) })}>
              <Icon name="trash" />
            </button>
          </div>
        ))}
        <button className="tool-btn px2 self-start" onClick={() => set({ env: [...draft.env, ["", ""]] })}>
          <Icon name="plus" /> Add variable
        </button>
      </Section>

      <Section title="Danger zone">
        <div className="set-row">
          <span className="grow muted">Deletes the instance with its worlds, mods and screenshots.</span>
          <B3D variant="danger" size="sm" onClick={onDelete} disabled={instance.running}>
            <Icon name="trash" size={12} />
            <span className="pix">Delete instance</span>
          </B3D>
        </div>
      </Section>

      <div className={`save-bar px4 ${dirty ? "show" : ""}`} aria-hidden={!dirty}>
        <span className="grow">You have unsaved changes</span>
        <button className="tool-btn px2" onClick={() => setDraft(saved)} tabIndex={dirty ? 0 : -1}>
          Discard
        </button>
        <B3D variant="accent" size="sm" onClick={save} disabled={saving || !draft.name.trim() || instance.running}>
          {saving ? <span className="spinner small" /> : <Icon name="check" size={12} />}
          <span className="pix">Save</span>
        </B3D>
      </div>
    </div>
  );
}
