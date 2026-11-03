import { type CSSProperties, type ReactNode, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import type { Instance } from "./api";
import { type Background, mediaUrl } from "./theme";

type ButtonProps = {
  children: ReactNode;
  onClick?: () => void;
  variant?: "play" | "accent" | "warn" | "secondary" | "danger";
  size?: "lg" | "md" | "sm";
  disabled?: boolean;
  type?: "button" | "submit";
  label?: string;
  idle?: boolean;
  className?: string;
};

/** The chunky pixel button: a face that lifts on hover and sinks onto its edge on press. */
export function B3D({
  children,
  onClick,
  variant = "secondary",
  size = "md",
  disabled,
  type = "button",
  label,
  idle,
  className = "",
}: ButtonProps) {
  const corner = size === "sm" ? "px2" : "px4";
  return (
    <button
      type={type}
      className={`b3d ${size} ${variant} ${idle ? "idle" : ""} ${className}`}
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      title={label}
    >
      <span className={`edge ${corner}`} />
      <span className={`face ${corner}`}>{children}</span>
    </button>
  );
}

const SEGMENTS = 32;

/** Pixel on/off switch. */
export function Switch({ on, onChange, label }: { on: boolean; onChange: (on: boolean) => void; label: string }) {
  return (
    <button role="switch" aria-checked={on} aria-label={label} title={label} className={`switch-px ${on ? "on" : ""}`} onClick={() => onChange(!on)}>
      <span className="knob" />
    </button>
  );
}

export function XpBar({ done, total }: { done: number; total: number }) {
  const filled = Math.min(SEGMENTS, Math.round((done / Math.max(total, 1)) * SEGMENTS));
  return (
    <div className="xp" aria-hidden="true">
      {Array.from({ length: SEGMENTS }, (_, i) => (
        <span key={i} className={i < filled ? "on" : i === filled ? "on blink" : ""} />
      ))}
    </div>
  );
}

export function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  const dialog = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const first = dialog.current?.querySelector<HTMLElement>("input, select, button");
    first?.focus();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Rendered into <body>: panels clip their content with clip-path, which would cut a dialog in half.
  return createPortal(
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal px4 drop" role="dialog" aria-modal="true" aria-label={title} ref={dialog}>
        <div className="modal-head">
          <h2 className="pix">{title}</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Close">
            <Icon name="close" />
          </button>
        </div>
        {children}
      </div>
    </div>,
    document.body,
  );
}

const ICONS = {
  home: "M7 2h2v2H7zM5 4h6v2H5zM3 6h10v2H3zM4 8h3v6H4zM9 8h3v6H9zM7 8h2v2H7z",
  library: "M2 2h5v5H2zM9 2h5v5H9zM2 9h5v5H2zM9 9h5v5H9z",
  mods: "M2 2h12v2H2zM2 12h12v2H2zM2 4h2v8H2zM12 4h2v8h-2zM7 7h2v2H7z",
  friends: "M3 3h4v4H3zM2 8h6v5H2zM10 4h3v3h-3zM9 8h5v4H9z",
  studio: "M5 4h2v2H5zM3 7h2v2H3zM5 10h2v2H5zM9 4h2v2H9zM11 7h2v2h-2zM9 10h2v2H9z",
  settings: "M6 1h4v2H6zM6 13h4v2H6zM1 6h2v4H1zM13 6h2v4h-2zM3 3h10v3H3zM3 10h10v3H3zM3 6h3v4H3zM10 6h3v4h-3z",
  folder: "M1 3h6v2H1zM1 5h14v9H1z",
  edit: "M10 2h2v2h-2zM12 4h2v2h-2zM8 4h2v2H8zM6 6h4v2H6zM4 8h4v2H4zM2 10h4v4H2z",
  trash: "M5 1h6v2H5zM2 3h12v2H2zM3 6h10v9H3z",
  plus: "M7 3h2v10H7zM3 7h10v2H3z",
  close: "M3 3h2v2H3zM5 5h2v2H5zM7 7h2v2H7zM9 9h2v2H9zM11 11h2v2h-2zM11 3h2v2h-2zM9 5h2v2H9zM5 9h2v2H5zM3 11h2v2H3z",
  minimize: "M4 8h8v1H4z",
  maximize: "M4 4h8v1H4zM4 11h8v1H4zM4 5h1v6H4zM11 5h1v6h-1z",
  play: "M2 0h2v16H2zM4 2h2v12H4zM6 4h2v8H6zM8 6h2v4H8z",
  check: "M2 8h2v2H2zM4 10h2v2H4zM6 8h2v2H6zM8 6h2v2H8zM10 4h2v2h-2zM12 2h2v2h-2z",
  user: "M5 2h6v6H5zM3 10h10v4H3z",
  link: "M1 5h7v1H1zM1 10h7v1H1zM1 5h1v6H1zM8 5h7v1H8zM8 10h7v1H8zM14 5h1v6h-1zM5 7h6v2H5z",
  search: "M5 2h5v2H5zM3 4h2v5H3zM10 4h2v5h-2zM5 9h5v2H5zM11 11h2v2h-2zM13 13h2v2h-2z",
  stop: "M3 3h10v10H3z",
  terminal: "M1 2h14v2H1zM1 12h14v2H1zM1 4h2v8H1zM13 4h2v8h-2zM4 6h2v1H4zM6 7h2v1H6zM4 8h2v1H4zM9 9h3v1H9z",
  cube: "M4 1h8v2H4zM2 3h12v2H2zM2 5h5v9H2zM9 5h5v9H9z",
  brush: "M11 1h4v4h-4zM9 5h3v2H9zM7 7h3v2H7zM2 9h5v6H2z",
  sun: "M7 1h2v2H7zM7 13h2v2H7zM1 7h2v2H1zM13 7h2v2h-2zM3 3h2v2H3zM11 3h2v2h-2zM3 11h2v2H3zM11 11h2v2h-2zM5 5h6v6H5z",
  notes: "M3 1h8v2H3zM11 3h2v2h-2zM3 3h2v12H3zM11 5h2v10h-2zM5 13h6v2H5zM6 6h4v1H6zM6 9h4v1H6z",
  globe: "M5 1h6v2H5zM3 3h2v2H3zM11 3h2v2h-2zM1 5h2v6H1zM13 5h2v6h-2zM3 11h2v2H3zM11 11h2v2h-2zM5 13h6v2H5zM7 3h2v10H7zM3 7h10v2H3z",
  server: "M2 1h12v6H2zM2 9h12v6H2z",
  image: "M1 2h14v2H1zM1 12h14v2H1zM1 4h2v8H1zM13 4h2v8h-2zM4 9h2v3H4zM6 7h2v5H6zM8 9h4v3H8zM10 5h2v2h-2z",
  file: "M3 1h7v2H3zM10 3h3v2h-3zM3 3h2v12H3zM11 5h2v10h-2zM5 13h6v2H5zM6 7h4v1H6zM6 10h4v1H6z",
  back: "M2 7h12v2H2zM4 5h2v2H4zM6 3h2v2H6zM4 9h2v2H4zM6 11h2v2H6z",
  copy: "M5 1h9v2H5zM12 3h2v8h-2zM2 5h8v10H2z",
  refresh: "M4 2h7v2H4zM11 4h2v2h-2zM13 2h2v5h-5V5h3zM2 4h2v7H2zM4 11h2v2H4zM6 13h6v-2h2v4H6z",
  download: "M7 1h2v8H7zM3 7h2v2H3zM11 7h2v2h-2zM5 9h2v2H5zM9 9h2v2H9zM7 11h2v1H7zM1 13h14v2H1z",
  upload: "M7 4h2v8H7zM5 4h2V2h2v2h2v2H5zM3 6h2v2H3zM11 6h2v2h-2zM1 13h14v2H1z",
  archive: "M1 2h14v4H1zM2 7h12v8H2zM6 9h4v2H6z",
  arrowUp: "M7 3h2v11H7zM5 5h2v2H5zM9 5h2v2H9zM3 7h2v2H3zM11 7h2v2h-2z",
  arrowDown: "M7 2h2v11H7zM5 9h2v2H5zM9 9h2v2H9zM3 7h2v2H3zM11 7h2v2h-2z",
  chevronLeft: "M9 2h2v2H9zM7 4h2v2H7zM5 6h2v4H5zM7 10h2v2H7zM9 12h2v2H9z",
  chevronRight: "M5 2h2v2H5zM7 4h2v2H7zM9 6h2v4H9zM7 10h2v2H7zM5 12h2v2H5z",
  wrap: "M1 3h14v2H1zM1 7h11v2H1zM12 9h2v2h-2zM8 11h4v2H8zM1 11h5v2H1z",
  follow: "M7 1h2v10H7zM3 7h2v2H3zM11 7h2v2h-2zM5 9h2v2H5zM9 9h2v2H9zM2 13h12v2H2z",
  external: "M9 1h6v6h-2V4h-1V3h-3zM8 6h2v2H8zM6 8h2v2H6zM1 3h6v2H3v8h8V9h2v6H1z",
  seed: "M7 1h2v3H7zM5 4h6v2H5zM3 6h10v6H3zM5 12h6v2H5z",
  pin: "M6 1h4v2H6zM4 3h8v6H4zM7 9h2v6H7z",
  warn: "M7 1h2v2H7zM6 3h4v2H6zM5 5h6v2H5zM4 7h8v2H4zM3 9h10v2H3zM2 11h12v3H2z",
  heart: "M2 2h4v2H2zM10 2h4v2h-4zM1 4h14v4H1zM2 8h12v2H2zM4 10h8v2H4zM6 12h4v2H6z",
  code: "M4 4h2v2H4zM2 6h2v4H2zM4 10h2v2H4zM10 4h2v2h-2zM12 6h2v4h-2zM10 10h2v2h-2zM8 3h2v3H8zM7 6h2v4H7zM6 10h2v3H6z",
  bug: "M6 1h4v2H6zM4 3h8v2H4zM3 5h10v8H3zM5 13h6v2H5zM1 6h2v2H1zM13 6h2v2h-2zM1 10h2v2H1zM13 10h2v2h-2z",
  book: "M1 2h6v12H1zM9 2h6v12H9zM7 3h2v11H7z",
  chat: "M1 2h14v9H1zM3 11h4v2H3zM3 13h2v2H3z",
  clock: "M5 1h6v2H5zM3 3h2v2H3zM11 3h2v2h-2zM1 5h2v6H1zM13 5h2v6h-2zM3 11h2v2H3zM11 11h2v2h-2zM5 13h6v2H5zM7 4h2v5H7zM9 7h3v2H9z",
} as const;

export type IconName = keyof typeof ICONS;

export function Icon({ name, size = 16 }: { name: IconName; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" shapeRendering="crispEdges" aria-hidden="true">
      <path d={ICONS[name]} fill="currentColor" />
    </svg>
  );
}

/** Library cover presets: a tiny pixel landscape per instance. */
export const covers = {
  meadow: { sky: "#5b8fd6", far: "#8fb7ea", ground: "#5a3d26", grass: "#5fa34a", block: "#e8e2c8" },
  dusk: { sky: "#f0a35e", far: "#c46a4a", ground: "#3b2a20", grass: "#6a9a3e", block: "#5a7a2e" },
  forge: { sky: "#1f3b4d", far: "#2d5566", ground: "#3a2a1f", grass: "#4f7a3a", block: "#b87333" },
  cave: { sky: "#2b2f3a", far: "#454b5a", ground: "#1d1f26", grass: "#5ce1e6", block: "#f5c76a" },
  snow: { sky: "#a9c7e8", far: "#e8f1fb", ground: "#6b5a4a", grass: "#f4f8fc", block: "#3f8f37" },
  nether: { sky: "#3a0f12", far: "#6e1d1d", ground: "#4a1414", grass: "#c2402e", block: "#ffb547" },
} as const;

export type CoverName = keyof typeof covers;

/** A preset name, or a picture the user added: a reference into Glint's media folder. */
export const isCustomCover = (cover: string | null): cover is string => !!cover && cover.includes("/");

export function coverFor(name: string | null, fallbackKey: string): string {
  if (name && (name in covers || isCustomCover(name))) return name;
  const keys = Object.keys(covers) as CoverName[];
  let hash = 0;
  for (const ch of fallbackKey) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return keys[hash % keys.length];
}

export function Cover({ cover, height = 92 }: { cover: string; height?: number }) {
  if (!(cover in covers)) {
    return <img className="cover photo-cover" style={{ height }} src={mediaUrl(cover)} alt="" draggable={false} />;
  }
  const c = covers[cover as CoverName];
  return (
    <svg className="cover" style={{ height }} viewBox="0 0 48 24" preserveAspectRatio="xMidYMid slice" shapeRendering="crispEdges" aria-hidden="true">
      <rect width="48" height="24" fill={c.sky} />
      <rect x="8" y="4" width="2" height="2" fill="#fff" opacity="0.8" />
      <polygon fill={c.far} points="0,16 0,12 6,12 6,9 12,9 12,11 20,11 20,7 26,7 26,10 34,10 34,8 40,8 40,12 48,12 48,16" />
      <rect y="16" width="48" height="8" fill={c.ground} />
      <rect y="16" width="48" height="2" fill={c.grass} />
      <rect x="30" y="12" width="4" height="4" fill={c.block} />
      <rect x="30" y="12" width="4" height="1" fill="#fff" opacity="0.3" />
    </svg>
  );
}

/** The user's background picture with its blur, dim and focal point. */
export function Photo({ background, className = "" }: { background: Background & { image: string }; className?: string }) {
  const { image, blur, dim, x, y } = background;
  return (
    <div className={`photo ${className}`} style={{ "--dim": `${dim}%` } as CSSProperties} aria-hidden="true">
      <img
        src={mediaUrl(image)}
        alt=""
        draggable={false}
        style={{ objectPosition: `${x}% ${y}%`, filter: blur ? `blur(${blur}px)` : undefined, transform: blur ? `scale(${1 + blur * 0.006})` : undefined }}
      />
    </div>
  );
}

/** Opens the system file picker for one image and hands back the file. */
export function pickImage(): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/png,image/jpeg,image/gif,image/webp,image/bmp";
    input.onchange = () => resolve(input.files?.[0] ?? null);
    input.oncancel = () => resolve(null);
    input.click();
  });
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}

/** Marks a Horizon instance: on a cover's corner, or as a chip beside its version. */
export function RustBadge({ chip = false }: { chip?: boolean }) {
  return (
    <span className={`${chip ? "chip chip-engine" : "badge-engine"} pix px2`} title="Runs on Horizon, the experimental Rust engine">
      RUST
    </span>
  );
}

/** What an instance runs: "Horizon", "Fabric 0.16.9" or "Vanilla". */
export function flavourOf(instance: Instance, withVersion = true): string {
  if (instance.engine === "horizon") return "Horizon";
  if (!instance.loader) return "Vanilla";
  return withVersion && instance.loaderVersion ? `${instance.loader} ${instance.loaderVersion}` : instance.loader;
}

export function relativeTime(unixSeconds: number | null): string {
  if (!unixSeconds) return "Never played";
  const minutes = Math.round((Date.now() / 1000 - unixSeconds) / 60);
  if (minutes < 1) return "Played just now";
  if (minutes < 60) return `Played ${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `Played ${hours} h ago`;
  const days = Math.round(hours / 24);
  return days === 1 ? "Played yesterday" : `Played ${days} days ago`;
}
