import { type FormEvent, useState } from "react";
import { ElySignInModal, PlayerHead, accountKindLabel } from "./Accounts";
import type { Accounts, AppInfo } from "./api";
import { type Appearance, fonts, themes, type ThemeName } from "./theme";
import { B3D, Icon } from "./ui";

type Props = {
  appearance: Appearance;
  onAppearance: (a: Appearance) => void;
  accounts: Accounts | null;
  info: AppInfo | null;
  onAddName: (name: string) => Promise<void>;
  onElySignIn: (login: string, password: string, totp?: string) => Promise<void>;
  onRemove: (uuid: string) => void;
  onActivate: (uuid: string) => void;
  onTour: () => void;
};

export function Settings(props: Props) {
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [elyOpen, setElyOpen] = useState(false);
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
          Offline names play in single player, on LAN and on servers with online-mode off. Ely.by accounts add a
          password-protected nickname and a skin, and work on servers that use Ely.by.
        </p>
        <ul className="account-list">
          {accounts?.accounts.map((a) => (
            <li key={a.uuid} className={`account-row px2 ${a.uuid === accounts.active ? "active" : ""}`}>
              <PlayerHead account={a} size={28} />
              <span className="grow">{a.name}</span>
              <span className="muted small">{accountKindLabel(a)}</span>
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
        {accounts?.offlineAllowed && (
          <div className="row">
            <B3D variant="accent" onClick={() => setElyOpen(true)}>
              <span className="pix">Sign in with Ely.by</span>
            </B3D>
          </div>
        )}
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
          <p className="muted small">
            Sign in once with a Microsoft account that owns Minecraft to use offline names and Ely.by accounts.
          </p>
        )}
        {error && <p className="error">{error}</p>}
        {elyOpen && (
          <ElySignInModal
            onClose={() => setElyOpen(false)}
            onSignIn={async (login, password, totp) => {
              await props.onElySignIn(login, password, totp);
              setElyOpen(false);
            }}
          />
        )}
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
            {props.info?.channel === "beta" ? " (beta)" : props.info?.devBuild ? " (development build)" : ""}
          </dd>
          <dt>Data folder</dt>
          <dd>{props.info?.dataDir ?? "…"}</dd>
        </dl>
        <div className="row">
          <button className="link" onClick={props.onTour}>
            Show the welcome tour again
          </button>
        </div>
      </section>
    </div>
  );
}
