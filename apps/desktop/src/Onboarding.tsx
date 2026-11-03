import { type FormEvent, useState } from "react";
import { ElySignInModal, PlayerHead, accountKindLabel } from "./Accounts";
import { api, type Accounts, type Instance, type LoaderKind, type VersionList } from "./api";
import { SparkleMark, Wordmark } from "./Logo";
import { scenes } from "./scenes";
import { ColourRow } from "./AppearancePanel";
import { type Appearance, fonts, sceneFor, themes, type ThemeName } from "./theme";
import { B3D, Cover, type CoverName, Icon } from "./ui";

const STORAGE_KEY = "glint.onboarded";

/** True once the welcome tour was finished or skipped on this machine. */
export function onboarded(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function markOnboarded() {
  try {
    localStorage.setItem(STORAGE_KEY, "1");
  } catch {
    // Without storage the tour shows again next launch, which is harmless.
  }
}

const STEPS = ["welcome", "look", "player", "instance", "done"] as const;
type Step = (typeof STEPS)[number];

type Starter = { id: string; loader: LoaderKind | null; title: string; hint: string; cover: CoverName };

const STARTERS: Starter[] = [
  { id: "vanilla", loader: null, title: "Vanilla", hint: "The game as Mojang ships it", cover: "meadow" },
  { id: "fabric", loader: "fabric", title: "Fabric", hint: "Ready for mods from Discover", cover: "cave" },
];

type Props = {
  appearance: Appearance;
  onAppearance: (a: Appearance) => void;
  accounts: Accounts | null;
  instances: Instance[];
  versions: VersionList | null;
  onAccounts: (a: Accounts) => void;
  onCreated: (name: string) => Promise<void>;
  onFinish: () => void;
};

export function Onboarding(props: Props) {
  const [step, setStep] = useState<Step>("welcome");
  const [back, setBack] = useState(false);
  const index = STEPS.indexOf(step);

  function go(to: Step) {
    setBack(STEPS.indexOf(to) < index);
    setStep(to);
  }
  const next = () => go(STEPS[Math.min(index + 1, STEPS.length - 1)]);
  const prev = () => go(STEPS[Math.max(index - 1, 0)]);

  const scene = sceneFor(props.appearance.theme);

  return (
    <div className="onboarding">
      <div key={scene} className="scene" dangerouslySetInnerHTML={{ __html: scenes[scene] }} />
      <div className="ob-card px4">
        {step !== "welcome" && (
          <div className="ob-progress" aria-label={`Step ${index} of ${STEPS.length - 1}`}>
            {STEPS.slice(1).map((s, i) => (
              <span key={s} className={`px2 ${i < index ? "on" : ""}`} />
            ))}
          </div>
        )}
        <div key={step} className={`ob-step ${back ? "back" : ""}`}>
          {step === "welcome" && <Welcome onStart={next} onSkip={props.onFinish} />}
          {step === "look" && <Look appearance={props.appearance} onAppearance={props.onAppearance} />}
          {step === "player" && <Player accounts={props.accounts} onAccounts={props.onAccounts} />}
          {step === "instance" && (
            <FirstInstance
              instances={props.instances}
              versions={props.versions}
              onCreated={async (name) => {
                await props.onCreated(name);
                next();
              }}
            />
          )}
          {step === "done" && <Done accounts={props.accounts} instances={props.instances} />}
        </div>
        {step !== "welcome" && (
          <div className="ob-actions">
            {step !== "done" ? (
              <B3D size="sm" onClick={prev}>
                Back
              </B3D>
            ) : (
              <span />
            )}
            {step === "done" ? (
              <B3D variant="play" onClick={props.onFinish}>
                <span className="pix">Let's play</span>
              </B3D>
            ) : (
              <B3D variant="accent" onClick={next}>
                <span className="pix">{nextLabel(step, props)}</span>
              </B3D>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function nextLabel(step: Step, props: Props): string {
  if (step === "player" && !props.accounts?.accounts.length) return "Skip for now";
  if (step === "instance" && !props.instances.length) return "Skip for now";
  return "Next";
}

function Welcome({ onStart, onSkip }: { onStart: () => void; onSkip: () => void }) {
  return (
    <div className="ob-hero">
      <div className="ob-mark">
        <SparkleMark size={72} />
      </div>
      <Wordmark height={34} />
      <p className="ob-lead">A fast, good-looking launcher for Minecraft: Java Edition.</p>
      <p className="muted small">Three quick steps: pick a look, add a player and set up your first game.</p>
      <div className="ob-hero-actions">
        <B3D variant="accent" size="lg" onClick={onStart}>
          <span className="pix">Get started</span>
        </B3D>
        <button className="link" onClick={onSkip}>
          Skip setup
        </button>
      </div>
    </div>
  );
}

function Look({ appearance, onAppearance }: { appearance: Appearance; onAppearance: (a: Appearance) => void }) {
  return (
    <>
      <h2 className="pix">Pick a look</h2>
      <p className="muted small">The backdrop follows the theme. Settings has more: your own colours, a background picture and the interface size.</p>
      <span className="label pix">THEME</span>
      <div className="theme-grid">
        {(Object.keys(themes) as ThemeName[]).map((key) => (
          <button
            key={key}
            className={`theme-option px2 ${appearance.theme === key && !appearance.tint ? "selected" : ""}`}
            aria-pressed={appearance.theme === key && !appearance.tint}
            onClick={() => onAppearance({ ...appearance, theme: key, accent: null, tint: null })}
          >
            <span className="swatch px2" style={{ background: themes[key].swatch }}>
              <span style={{ background: themes[key].vars.accent }} />
            </span>
            {themes[key].name}
          </button>
        ))}
      </div>
      <span className="label pix">ACCENT</span>
      <ColourRow label="Accent colour" value={appearance.accent} noneLabel="Theme" onChange={(accent) => onAppearance({ ...appearance, accent })} />
      <span className="label pix">DISPLAY FONT</span>
      <div className="ob-fonts">
        {fonts.map((f) => (
          <button
            key={f.id}
            className={`ob-font px2 ${appearance.font === f.id ? "selected" : ""}`}
            aria-pressed={appearance.font === f.id}
            title={f.name}
            onClick={() => onAppearance({ ...appearance, font: f.id })}
          >
            <span style={{ fontFamily: `${f.family}, monospace` }}>Play</span>
          </button>
        ))}
      </div>
    </>
  );
}

function Player({ accounts, onAccounts }: { accounts: Accounts | null; onAccounts: (a: Accounts) => void }) {
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [elyOpen, setElyOpen] = useState(false);

  async function add(e: FormEvent) {
    e.preventDefault();
    try {
      onAccounts(await api.addOfflineAccount(name));
      setName("");
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }

  const list = accounts?.accounts ?? [];
  return (
    <>
      <h2 className="pix">Who's playing?</h2>
      {accounts?.offlineAllowed ? (
        <p className="muted small">
          An offline name works in single player, on LAN and on servers with online-mode off. Ely.by adds a skin and
          a password-protected nickname.
        </p>
      ) : (
        <p className="muted small">
          Sign in once with a Microsoft account that owns Minecraft to use offline names and Ely.by accounts.
        </p>
      )}
      {list.length > 0 && (
        <ul className="account-list">
          {list.map((a) => (
            <li key={a.uuid} className={`account-row px2 ${a.uuid === accounts?.active ? "active" : ""}`}>
              <PlayerHead account={a} size={28} />
              <span className="grow">{a.name}</span>
              <span className="muted small">{accountKindLabel(a)}</span>
              {a.uuid === accounts?.active && <Icon name="check" size={12} />}
            </li>
          ))}
        </ul>
      )}
      {accounts?.offlineAllowed && (
        <>
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
          <div className="ob-or muted small">or</div>
          <div className="row">
            <B3D variant="secondary" onClick={() => setElyOpen(true)}>
              <span className="pix">Sign in with Ely.by</span>
            </B3D>
          </div>
        </>
      )}
      {error && <p className="error">{error}</p>}
      {elyOpen && (
        <ElySignInModal
          onClose={() => setElyOpen(false)}
          onSignIn={async (login, password, totp) => {
            onAccounts(await api.elySignIn(login, password, totp));
            setElyOpen(false);
          }}
        />
      )}
    </>
  );
}

function FirstInstance({
  instances,
  versions,
  onCreated,
}: {
  instances: Instance[];
  versions: VersionList | null;
  onCreated: (name: string) => Promise<void>;
}) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const latest = versions?.latestRelease;

  async function create(starter: Starter) {
    if (!latest) return;
    setBusy(starter.id);
    setError(null);
    try {
      let loader = null;
      if (starter.loader) {
        const list = await api.loaderVersions(starter.loader, latest);
        const pick = list.find((v) => v.recommended) ?? list[0];
        if (!pick) throw new Error(`${starter.title} is not out for ${latest} yet.`);
        loader = { kind: starter.loader, version: pick.id };
      }
      const name = await api.createInstance({
        name: `${starter.title} ${latest}`,
        gameVersion: latest,
        cover: starter.cover,
        maxMemoryMb: 4096,
        loader,
        extraJvmArgs: "",
      });
      await onCreated(name);
    } catch (e) {
      setError(String(e));
      setBusy(null);
    }
  }

  return (
    <>
      <h2 className="pix">Your first game</h2>
      {instances.length > 0 ? (
        <p className="muted small">
          You already have {instances.length === 1 ? "an instance" : `${instances.length} instances`}:{" "}
          {instances.map((i) => i.name).join(", ")}. More can be added from Library at any time.
        </p>
      ) : (
        <p className="muted small">
          An instance is a separate Minecraft with its own version, mods and worlds. Start with one of these; the
          game downloads on the first launch.
        </p>
      )}
      {instances.length === 0 && (
        <div className="ob-starters">
          {STARTERS.map((s) => (
            <button
              key={s.id}
              className={`ob-starter px2 ${busy === s.id ? "busy" : ""}`}
              disabled={!latest || !!busy}
              onClick={() => create(s)}
            >
              <Cover cover={s.cover} height={84} />
              <span className="ob-starter-text">
                <span className="pix">
                  {s.title} {latest ?? "…"}
                </span>
                <span className="muted small">{busy === s.id ? "Creating…" : s.hint}</span>
              </span>
            </button>
          ))}
        </div>
      )}
      {!latest && <p className="muted small">Loading Minecraft versions…</p>}
      {error && <p className="error">{error}</p>}
    </>
  );
}

function Done({ accounts, instances }: { accounts: Accounts | null; instances: Instance[] }) {
  const player = accounts?.accounts.find((a) => a.uuid === accounts.active) ?? null;
  return (
    <div className="ob-hero">
      <div className="ob-mark">
        <SparkleMark size={56} />
      </div>
      <h2 className="pix">All set{player ? `, ${player.name}` : ""}!</h2>
      <ul className="ob-summary">
        <li className={player ? "ok" : ""}>
          <Icon name={player ? "check" : "user"} size={12} />
          {player ? `Playing as ${player.name}` : "Add a player in Settings before you play"}
        </li>
        <li className={instances.length ? "ok" : ""}>
          <Icon name={instances.length ? "check" : "plus"} size={12} />
          {instances.length ? `${instances[0].name} is ready on Home` : "Create an instance from Library"}
        </li>
        <li className="ok">
          <Icon name="search" size={12} />
          Mods, resource packs and shaders live in Discover
        </li>
      </ul>
    </div>
  );
}
