import { type ReactNode, useEffect, useRef } from "react";
import { createPortal } from "react-dom";

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

export function coverFor(name: string | null, fallbackKey: string): CoverName {
  if (name && name in covers) return name as CoverName;
  const keys = Object.keys(covers) as CoverName[];
  let hash = 0;
  for (const ch of fallbackKey) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return keys[hash % keys.length];
}

export function Cover({ cover, height = 92 }: { cover: CoverName; height?: number }) {
  const c = covers[cover];
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
