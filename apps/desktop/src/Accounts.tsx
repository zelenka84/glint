import { type FormEvent, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Account } from "./api";
import { B3D, Icon, Modal } from "./ui";

const skinUrl = (name: string) => `https://skinsystem.ely.by/skins/${encodeURIComponent(name)}.png`;

export const accountKindLabel = (account: Account) => (account.kind === "ely" ? "Ely.by" : "Offline");

/** The face (and hat layer) of an Ely.by skin, or a plain pixel person for offline names. */
export function PlayerHead({ account, size = 32 }: { account: Account | null; size?: number }) {
  const [loaded, setLoaded] = useState(false);
  const url = account?.kind === "ely" ? skinUrl(account.name) : null;

  useEffect(() => {
    setLoaded(false);
    if (!url) return;
    const img = new Image();
    img.onload = () => setLoaded(true);
    img.src = url;
  }, [url]);

  if (!url || !loaded) {
    return (
      <span className="avatar px2" style={{ width: size, height: size }}>
        <Icon name="user" />
      </span>
    );
  }
  return (
    <span
      className="avatar skin px2"
      role="img"
      aria-label={`${account!.name}'s skin`}
      style={{
        width: size,
        height: size,
        backgroundImage: `url("${url}"), url("${url}")`,
        backgroundSize: `${size * 8}px auto, ${size * 8}px auto`,
        backgroundPosition: `${-size * 5}px ${-size}px, ${-size}px ${-size}px`,
      }}
    />
  );
}

export function ElySignInModal({
  onSignIn,
  onClose,
}: {
  onSignIn: (login: string, password: string, totp?: string) => Promise<void>;
  onClose: () => void;
}) {
  const [login, setLogin] = useState("");
  const [password, setPassword] = useState("");
  const [totp, setTotp] = useState("");
  const [needsCode, setNeedsCode] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await onSignIn(login, password, needsCode ? totp : undefined);
    } catch (err) {
      const message = String(err);
      if (message.includes("TWO_FACTOR_REQUIRED")) {
        setNeedsCode(true);
        setError(needsCode ? "That code did not work. Try the next one from your app." : null);
      } else {
        setError(message);
      }
      setBusy(false);
    }
  }

  return (
    <Modal title="Sign in with Ely.by" onClose={onClose}>
      <form className="form" onSubmit={submit}>
        <p className="muted small">
          Ely.by gives your nickname a password and a skin that other players see on Ely.by servers. Glint sends your
          password only to Ely.by and keeps just a sign-in token in Windows Credential Manager.
        </p>
        <label className="field">
          <span>Nickname or email</span>
          <input className="input px2" value={login} autoComplete="username" onChange={(e) => setLogin(e.target.value)} />
        </label>
        <label className="field">
          <span>Password</span>
          <input
            className="input px2"
            type="password"
            value={password}
            autoComplete="current-password"
            onChange={(e) => setPassword(e.target.value)}
          />
        </label>
        {needsCode && (
          <label className="field">
            <span>Code from your authenticator app</span>
            <input
              className="input px2"
              value={totp}
              inputMode="numeric"
              autoComplete="one-time-code"
              maxLength={8}
              autoFocus
              onChange={(e) => setTotp(e.target.value.replace(/\D/g, ""))}
            />
          </label>
        )}
        {error && <p className="error">{error}</p>}
        <div className="modal-actions spread">
          <button type="button" className="link" onClick={() => openUrl("https://account.ely.by/register")}>
            Create an Ely.by account
          </button>
          <div className="row">
            <B3D onClick={onClose}>Cancel</B3D>
            <B3D type="submit" variant="accent" disabled={busy || !login || !password || (needsCode && !totp)}>
              <span className="pix">{busy ? "Signing in…" : "Sign in"}</span>
            </B3D>
          </div>
        </div>
      </form>
    </Modal>
  );
}
