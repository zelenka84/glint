import { type FormEvent, useState } from "react";
import type { Accounts, AppInfo } from "./api";
import { type Appearance, fonts, themes, type ThemeName } from "./theme";
import { B3D, Icon } from "./ui";

type Props = {
  appearance: Appearance;
  onAppearance: (a: Appearance) => void;
  accounts: Accounts | null;
  info: AppInfo | null;
  onAddName: (name: string) => Promise<void>;
  onRemove: (uuid: string) => void;
  onActivate: (uuid: string) => void;
};

export function Settings(props: Props) {
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const { appearance, accounts } = props;

  async function add(e: FormEvent) {
    e.preventDefault();
    try {
      await props.onAddName(name);
      setName("");
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <div className="screen settings">
      <div className="screen-head">
        <h1 className="pix">Settings</h1>
      </div>

      <section className="panel px4">
        <h2 className="pix">Players</h2>
        <p className="muted small">
          Offline names play in single player, on LAN and on servers with online-mode off.
          {props.info?.devBuild ? " Development build: offline names need no Microsoft sign-in." : ""}
        </p>
        <ul className="account-list">
          {accounts?.accounts.map((a) => (
            <li key={a.uuid} className={`account-row px2 ${a.uuid === accounts.active ? "active" : ""}`}>
              <Icon name="user" />
              <span className="grow">{a.name}</span>
              <span className="muted small">Offline</span>
              {a.uuid === accounts.active ? (
                <span className="tag px2">Active</span>
              ) : (
                <button className="link" onClick={() => props.onActivate(a.uuid)}>
                  Use
                </button>
              )}
              <button className="icon-btn" aria-label={`Remove ${a.name}`} onClick={() => props.onRemove(a.uuid)}>
                <Icon name="trash" />
              </button>
            </li>
          ))}
        </ul>
        {accounts?.offlineAllowed ? (
          <form className="row" onSubmit={add}>
            <label className="field grow">
              <span>Offline name</span>
              <input
                className="input px2"
                value={name}
                maxLength={16}
                placeholder="3–16 letters, digits or _"
                onChange={(e) => setName(e.target.value)}
              />
            </label>
            <B3D type="submit" disabled={!name}>
              <Icon name="plus" /> Add
            </B3D>
          </form>
        ) : (
          <p className="muted small">Sign in once with a Microsoft account that owns Minecraft to use offline names.</p>
        )}
        {error && <p className="error">{error}</p>}
      </section>

      <section className="panel px4">
        <h2 className="pix">Appearance</h2>
        <span className="label pix">THEME</span>
        <div className="theme-grid">
          {(Object.keys(themes) as ThemeName[]).map((key) => (
            <button
              key={key}
              className={`theme-option px2 ${appearance.theme === key ? "selected" : ""}`}
              aria-pressed={appearance.theme === key}
              onClick={() => props.onAppearance({ ...appearance, theme: key })}
            >
              <span className="swatch px2" style={{ background: themes[key].swatch }}>
                <span style={{ background: themes[key].vars.accent }} />
              </span>
              {themes[key].name}
            </button>
          ))}
        </div>
        <span className="label pix">DISPLAY FONT</span>
        <div className="font-list">
          {fonts.map((f) => (
            <button
              key={f.id}
              className={`font-option px2 ${appearance.font === f.id ? "selected" : ""}`}
              aria-pressed={appearance.font === f.id}
              onClick={() => props.onAppearance({ ...appearance, font: f.id })}
            >
              <span className="font-sample" style={{ fontFamily: `${f.family}, monospace` }}>
                Play
              </span>
              <span className="grow">{f.name}</span>
              {appearance.font === f.id && <Icon name="check" size={12} />}
            </button>
          ))}
        </div>
      </section>

      <section className="panel px4">
        <h2 className="pix">About</h2>
        <dl className="about">
          <dt>Version</dt>
          <dd>
            {props.info?.version ?? "…"}
            {props.info?.devBuild ? " (development build)" : ""}
          </dd>
          <dt>Data folder</dt>
          <dd>{props.info?.dataDir ?? "…"}</dd>
        </dl>
      </section>
    </div>
  );
}
