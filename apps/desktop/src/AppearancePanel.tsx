import { type FormEvent, type PointerEvent, useEffect, useRef, useState } from "react";
import { api } from "./api";
import { scenes } from "./scenes";
import {
  type Appearance,
  type Background,
  type CustomTheme,
  DEFAULT_APPEARANCE,
  DEFAULT_BACKGROUND,
  accentPresets,
  fonts,
  isHex,
  mediaUrl,
  normalizeTheme,
  paletteFor,
  sameColours,
  scales,
  sceneFor,
  themes,
  type ThemeName,
} from "./theme";
import { B3D, Icon, Photo, Switch, pickImage } from "./ui";

type Change = (a: Appearance) => void;

function Segmented<T extends string | number>({ value, options, onChange, label }: {
  value: T;
  options: readonly { id: T; label: string }[];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div className="segmented px2" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button key={o.id} role="radio" aria-checked={o.id === value} className={`px2 ${o.id === value ? "on" : ""}`} onClick={() => onChange(o.id)}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

/** Built-in colour presets plus the system colour picker and a hex field. */
export function ColourRow({ value, onChange, noneLabel, label }: {
  value: string | null;
  onChange: (c: string | null) => void;
  noneLabel: string;
  label: string;
}) {
  const custom = value !== null && !accentPresets.some((p) => p.color === value);
  const [text, setText] = useState(value ?? "");
  useEffect(() => setText(value ?? ""), [value]);

  function commit() {
    const v = text.trim().replace(/^#?/, "#").toLowerCase();
    if (isHex(v)) onChange(v);
    else setText(value ?? "");
  }

  return (
    <div className="colour-row" role="radiogroup" aria-label={label}>
      <button role="radio" aria-checked={value === null} className={`colour-none px2 ${value === null ? "selected" : ""}`} onClick={() => onChange(null)}>
        {noneLabel}
      </button>
      {accentPresets.map((p) => (
        <button
          key={p.color}
          role="radio"
          aria-checked={value === p.color}
          aria-label={p.name}
          title={p.name}
          className={`colour-dot px2 ${value === p.color ? "selected" : ""}`}
          style={{ background: p.color }}
          onClick={() => onChange(p.color)}
        />
      ))}
      <label className={`colour-dot colour-custom px2 ${custom ? "selected" : ""}`} title="Pick any colour" style={custom ? { background: value! } : undefined}>
        <input type="color" value={value ?? "#5ce1e6"} aria-label="Pick any colour" onChange={(e) => onChange(e.target.value)} />
        {!custom && <Icon name="brush" size={12} />}
      </label>
      <input
        className="input hex px2 mono"
        value={text}
        placeholder="#rrggbb"
        maxLength={7}
        aria-label={`${label} hex code`}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && commit()}
      />
    </div>
  );
}

/** Click or drag on the picture to choose which part stays in view when it is cropped. */
function FocusPicker({ background, onChange }: { background: Background & { image: string }; onChange: (x: number, y: number) => void }) {
  const img = useRef<HTMLImageElement>(null);
  function move(e: PointerEvent) {
    if (e.type === "pointermove" && !e.buttons) return;
    const r = img.current!.getBoundingClientRect();
    const pct = (v: number, start: number, size: number) => Math.round(Math.min(100, Math.max(0, ((v - start) / size) * 100)));
    onChange(pct(e.clientX, r.left, r.width), pct(e.clientY, r.top, r.height));
  }
  return (
    <div
      className="focus-picker px2"
      title="Drag to choose the part of the picture that stays in view"
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        move(e);
      }}
      onPointerMove={move}
    >
      <img ref={img} src={mediaUrl(background.image)} alt="" draggable={false} />
      <span className="focus-dot" style={{ left: `${background.x}%`, top: `${background.y}%` }} />
    </div>
  );
}

function Slider({ label, value, min, max, unit, onChange }: {
  label: string;
  value: number;
  min: number;
  max: number;
  unit: string;
  onChange: (v: number) => void;
}) {
  return (
    <label className="field slider">
      <span>
        {label} · <b>{value}{unit}</b>
      </span>
      <input type="range" min={min} max={max} value={value} onChange={(e) => onChange(Number(e.target.value))} />
    </label>
  );
}

/** A small stand-in for the launcher that follows every change at once. */
function Preview({ appearance }: { appearance: Appearance }) {
  const scene = sceneFor(appearance.theme);
  const { background } = appearance;
  return (
    <div className="look-preview px4" aria-hidden="true">
      {background.image && background.everywhere && <Photo background={{ ...background, image: background.image }} className="lp-backdrop" />}
      <div className="lp-side">
        {[0, 1, 2].map((i) => (
          <span key={i} className={`lp-nav px2 ${i === 0 ? "on" : ""}`} />
        ))}
      </div>
      <div className="lp-main">
        <div className="lp-hero px2">
          {background.image ? (
            <Photo key={background.image} background={{ ...background, image: background.image }} className="scene" />
          ) : (
            <div key={scene} className="scene" dangerouslySetInnerHTML={{ __html: scenes[scene] }} />
          )}
          <div className="lp-plaque px2">
            <span className="lp-eyebrow pix">READY TO PLAY</span>
            <span className="lp-title pix">My World</span>
            <span className="lp-row">
              <span className="lp-play px2 pix">PLAY</span>
              <span className="lp-chip px2">1.21.4</span>
            </span>
          </div>
        </div>
        <div className="lp-cards">
          <span className="px2" />
          <span className="px2 on" />
          <span className="px2" />
        </div>
      </div>
    </div>
  );
}

function ThemeTile({ name, colours, selected, onPick, onExport, onDelete }: {
  name: string;
  colours: Pick<Appearance, "theme" | "accent" | "tint">;
  selected: boolean;
  onPick: () => void;
  onExport?: () => void;
  onDelete?: () => void;
}) {
  const p = paletteFor(colours);
  return (
    <div className={`theme-tile ${selected ? "selected" : ""}`}>
      <button className={`theme-option px2 ${selected ? "selected" : ""}`} aria-pressed={selected} onClick={onPick}>
        <span className="swatch px2" style={{ background: colours.tint || colours.accent ? p.card : themes[colours.theme].swatch }}>
          <span style={{ background: p.accent }} />
        </span>
        <span className="grow ellipsis">{name}</span>
      </button>
      {(onExport || onDelete) && (
        <span className="tile-actions">
          {onExport && (
            <button className="icon-btn" aria-label={`Export ${name}`} title="Export as a file" onClick={onExport}>
              <Icon name="upload" size={12} />
            </button>
          )}
          {onDelete && (
            <button className="icon-btn" aria-label={`Delete ${name}`} title="Delete" onClick={onDelete}>
              <Icon name="trash" size={12} />
            </button>
          )}
        </span>
      )}
    </div>
  );
}

function pickText(): Promise<string | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json,application/json";
    input.onchange = () => {
      const file = input.files?.[0];
      file ? file.text().then(resolve, () => resolve(null)) : resolve(null);
    };
    input.oncancel = () => resolve(null);
    input.click();
  });
}

export function AppearancePanel({ appearance, onAppearance }: { appearance: Appearance; onAppearance: Change }) {
  const a = appearance;
  const set = (patch: Partial<Appearance>) => onAppearance({ ...a, ...patch });
  const setBg = (patch: Partial<Background>) => set({ background: { ...a.background, ...patch } });
  const [naming, setNaming] = useState<string | null>(null);
  const [notice, setNotice] = useState<{ tone: "ok" | "bad"; text: string; path?: string } | null>(null);
  const [busy, setBusy] = useState(false);

  async function chooseImage() {
    const file = await pickImage();
    if (!file) return;
    setBusy(true);
    try {
      const image = await api.importImage("background", file);
      if (a.background.image) api.deleteMedia(a.background.image).catch(() => {});
      setBg({ image, x: 50, y: 50 });
      setNotice(null);
    } catch (e) {
      setNotice({ tone: "bad", text: String(e) });
    } finally {
      setBusy(false);
    }
  }

  function removeImage() {
    if (a.background.image) api.deleteMedia(a.background.image).catch(() => {});
    setBg({ image: null, everywhere: false });
  }

  function saveTheme(e: FormEvent) {
    e.preventDefault();
    const name = naming?.trim();
    if (!name) return;
    const theme: CustomTheme = { id: `t${Date.now().toString(36)}`, name, theme: a.theme, accent: a.accent, tint: a.tint, font: a.font };
    set({ saved: [...a.saved, theme] });
    setNaming(null);
  }

  async function exportTheme(t: CustomTheme) {
    const file = { glint: "theme", version: 1, name: t.name, theme: t.theme, accent: t.accent, tint: t.tint, font: t.font };
    const contents = JSON.stringify(file, null, 2);
    try {
      const path = await api.exportTheme(t.name, contents);
      setNotice({ tone: "ok", text: `Saved ${t.name} to ${path}`, path });
    } catch (e) {
      setNotice({ tone: "bad", text: String(e) });
    }
  }

  async function importTheme() {
    const text = await pickText();
    if (text === null) return;
    try {
      const raw = JSON.parse(text);
      if (raw?.glint !== "theme") throw new Error("not a Glint theme file");
      const t = normalizeTheme(raw);
      const theme: CustomTheme = { ...t, id: `t${Date.now().toString(36)}`, name: t.name ?? "Imported theme" };
      onAppearance({ ...a, theme: t.theme, accent: t.accent, tint: t.tint, font: t.font, saved: [...a.saved, theme] });
      setNotice({ tone: "ok", text: `Added ${theme.name}` });
    } catch (e) {
      setNotice({ tone: "bad", text: `Could not read that file: ${e instanceof Error ? e.message : e}` });
    }
  }

  function reset() {
    if (a.background.image) api.deleteMedia(a.background.image).catch(() => {});
    onAppearance({ ...DEFAULT_APPEARANCE, saved: a.saved });
    setNotice(null);
  }

  const isDefault = JSON.stringify({ ...a, saved: [] }) === JSON.stringify(DEFAULT_APPEARANCE);
  const bg = a.background;

  return (
    <section className="panel px4 look">
      <div className="look-head">
        <h2 className="pix grow">Appearance</h2>
        <button className="link" disabled={isDefault} onClick={reset}>
          Reset to default
        </button>
      </div>

      <div className="look-layout">
        <div className="look-controls">
          <div className="look-group">
            <div className="look-label">
              <span className="label pix">THEME</span>
              <button className="link small" onClick={importTheme}>
                Import…
              </button>
            </div>
            <div className="theme-grid wide">
              {(Object.keys(themes) as ThemeName[]).map((key) => (
                <ThemeTile
                  key={key}
                  name={themes[key].name}
                  colours={{ theme: key, accent: null, tint: null }}
                  selected={a.theme === key && !a.accent && !a.tint}
                  onPick={() => set({ theme: key, accent: null, tint: null })}
                />
              ))}
              {a.saved.map((t) => (
                <ThemeTile
                  key={t.id}
                  name={t.name}
                  colours={t}
                  selected={sameColours(a, t)}
                  onPick={() => set({ theme: t.theme, accent: t.accent, tint: t.tint, font: t.font })}
                  onExport={() => exportTheme(t)}
                  onDelete={() => set({ saved: a.saved.filter((s) => s.id !== t.id) })}
                />
              ))}
              {naming === null ? (
                <button className="theme-option save-theme px2" onClick={() => setNaming("")} title="Save the current colours and font as a theme">
                  <Icon name="plus" size={12} />
                  Save current
                </button>
              ) : (
                <form className="save-form" onSubmit={saveTheme}>
                  <input
                    className="input px2"
                    autoFocus
                    value={naming}
                    maxLength={40}
                    placeholder="Theme name"
                    onChange={(e) => setNaming(e.target.value)}
                    onKeyDown={(e) => e.key === "Escape" && setNaming(null)}
                  />
                  <button type="submit" className="icon-btn" aria-label="Save theme" disabled={!naming.trim()}>
                    <Icon name="check" size={14} />
                  </button>
                </form>
              )}
            </div>
          </div>

          <div className="look-group">
            <span className="label pix">ACCENT</span>
            <ColourRow label="Accent colour" value={a.accent} noneLabel="Theme" onChange={(accent) => set({ accent })} />
          </div>

          <div className="look-group">
            <span className="label pix">SURFACE TINT</span>
            <ColourRow label="Surface tint" value={a.tint} noneLabel="None" onChange={(tint) => set({ tint })} />
          </div>

          <div className="look-group">
            <span className="label pix">DISPLAY FONT</span>
            <div className="font-list">
              {fonts.map((f) => (
                <button
                  key={f.id}
                  className={`font-option px2 ${a.font === f.id ? "selected" : ""}`}
                  aria-pressed={a.font === f.id}
                  onClick={() => set({ font: f.id })}
                >
                  <span className="font-sample" style={{ fontFamily: `${f.family}, monospace` }}>
                    Play
                  </span>
                  <span className="grow">{f.name}</span>
                  {a.font === f.id && <Icon name="check" size={12} />}
                </button>
              ))}
            </div>
          </div>

          <div className="look-group">
            <span className="label pix">BACKGROUND</span>
            <p className="muted small">A picture of your own instead of the pixel scene on Home. Glint keeps a copy in its data folder.</p>
            <div className="row">
              <B3D size="sm" onClick={chooseImage} disabled={busy}>
                <Icon name="image" size={14} /> {bg.image ? "Change picture" : "Choose picture"}
              </B3D>
              {bg.image && (
                <B3D size="sm" onClick={removeImage}>
                  <Icon name="trash" size={14} /> Use the pixel scene
                </B3D>
              )}
            </div>
            {bg.image && (
              <div className="bg-editor">
                <FocusPicker background={{ ...bg, image: bg.image }} onChange={(x, y) => setBg({ x, y })} />
                <div className="bg-sliders">
                  <Slider label="Blur" value={bg.blur} min={0} max={24} unit="px" onChange={(blur) => setBg({ blur })} />
                  <Slider label="Dim" value={bg.dim} min={0} max={90} unit="%" onChange={(dim) => setBg({ dim })} />
                  <label className="check">
                    <Switch label="Behind the whole window" on={bg.everywhere} onChange={(everywhere) => setBg({ everywhere })} />
                    Behind the whole window
                  </label>
                  {(bg.blur !== DEFAULT_BACKGROUND.blur || bg.dim !== DEFAULT_BACKGROUND.dim || bg.x !== 50 || bg.y !== 50) && (
                    <button className="link small" onClick={() => setBg({ blur: DEFAULT_BACKGROUND.blur, dim: DEFAULT_BACKGROUND.dim, x: 50, y: 50 })}>
                      Reset picture settings
                    </button>
                  )}
                </div>
              </div>
            )}
          </div>

          <div className="look-group">
            <span className="label pix">INTERFACE</span>
            <div className="look-options">
              <span className="muted small">Size</span>
              <Segmented label="Interface size" value={a.scale} options={scales.map((s) => ({ id: s, label: `${s}%` }))} onChange={(scale) => set({ scale })} />
              <span className="muted small">Corners</span>
              <Segmented
                label="Corners"
                value={a.corners}
                options={[
                  { id: "pixel", label: "Pixel" },
                  { id: "round", label: "Rounded" },
                  { id: "square", label: "Square" },
                ]}
                onChange={(corners) => set({ corners })}
              />
              <span className="muted small">Sidebar</span>
              <Segmented
                label="Sidebar"
                value={a.sidebar}
                options={[
                  { id: "full", label: "Labels" },
                  { id: "compact", label: "Icons only" },
                ]}
                onChange={(sidebar) => set({ sidebar })}
              />
              <span className="muted small">Motion</span>
              <Segmented
                label="Motion"
                value={a.motion}
                options={[
                  { id: "system", label: "Like Windows" },
                  { id: "full", label: "On" },
                  { id: "off", label: "Off" },
                ]}
                onChange={(motion) => set({ motion })}
              />
            </div>
          </div>
          {notice && (
            <p className={notice.tone === "ok" ? "muted small" : "error"}>
              {notice.text}
              {notice.path && (
                <>
                  {" · "}
                  <button className="link small" onClick={() => api.revealPath(notice.path!)}>
                    Show in folder
                  </button>
                </>
              )}
            </p>
          )}
        </div>

        <div className="look-aside">
          <Preview appearance={a} />
          <p className="muted small">Changes apply right away and are shared by every Glint channel on this computer.</p>
        </div>
      </div>
    </section>
  );
}
