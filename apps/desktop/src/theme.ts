import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api } from "./api";
import type { SceneName } from "./scenes";

/** Colour themes from the design canvas. Each also picks the home screen scene. */
export const themes = {
  mine: {
    name: "Mine",
    swatch: "#1a1b20",
    vars: {
      bg: "#0b0c0e", panel: "#141518", card: "#1c1d22", raise: "#24252b", line: "#2c2e34", edge: "#050506",
      text: "#eeedea", text2: "#c2c0ba", muted: "#a3a19b", link: "#8ceef1",
      accent: "#5ce1e6", "accent-ink": "#8ceef1", "on-accent": "#04211f", "accent-edge": "#22858a",
      "accent-light": "#bff6f8", "accent-dark": "#22858a",
      ok: "#7ee08c", warn: "#ffb547", "warn-edge": "#a8691a", "on-warn": "#2b1a00", danger: "#ff8a7a",
      play: "#6fdc7f", "play-edge": "#2e8a3e", "on-play": "#08200d",
      plaque: "rgba(11,12,14,0.86)", chip: "#24252b", "chip-fg": "#dad8d2", overlay: "rgba(0,0,0,0.6)",
    },
  },
  sunset: {
    name: "Sunset",
    swatch: "#d8673a",
    vars: {
      bg: "#120c0b", panel: "#1b1311", card: "#251a16", raise: "#2e211c", line: "#3d2c25", edge: "#060303",
      text: "#f6eee7", text2: "#d3c3b8", muted: "#b9a69a", link: "#ffd27a",
      accent: "#ffc24b", "accent-ink": "#ffd27a", "on-accent": "#2b1a00", "accent-edge": "#a8691a",
      "accent-light": "#ffe3a3", "accent-dark": "#a8691a",
      ok: "#9be38a", warn: "#ff9a5c", "warn-edge": "#a3482a", "on-warn": "#2b1000", danger: "#ff8a7a",
      play: "#6fdc7f", "play-edge": "#2e8a3e", "on-play": "#08200d",
      plaque: "rgba(18,12,11,0.86)", chip: "#2f221d", "chip-fg": "#ebddd2", overlay: "rgba(0,0,0,0.6)",
    },
  },
  day: {
    name: "Day",
    swatch: "#86c5f2",
    vars: {
      bg: "#e7ecef", panel: "#ffffff", card: "#f2f5f8", raise: "#dfe7ee", line: "#cbd5de", edge: "#a9b8c6",
      text: "#14202b", text2: "#3a4856", muted: "#566573", link: "#1f5fb8",
      accent: "#2a6fd0", "accent-ink": "#1f5fb8", "on-accent": "#ffffff", "accent-edge": "#174a8c",
      "accent-light": "#8fb8f0", "accent-dark": "#174a8c",
      ok: "#1f7a36", warn: "#f0a63a", "warn-edge": "#a9580a", "on-warn": "#2b1a00", danger: "#b3261e",
      play: "#5ccb6c", "play-edge": "#2c7a3a", "on-play": "#062010",
      plaque: "rgba(255,255,255,0.9)", chip: "#e3e9ee", "chip-fg": "#24313d", overlay: "rgba(20,32,43,0.45)",
    },
  },
} as const;

export type ThemeName = keyof typeof themes;

export const fonts = [
  { id: "pixelify", name: "Pixelify Sans", family: "'Pixelify Sans'" },
  { id: "silkscreen", name: "Silkscreen", family: "'Silkscreen'" },
  { id: "tiny5", name: "Tiny5", family: "'Tiny5'" },
  { id: "handjet", name: "Handjet", family: "'Handjet'" },
  { id: "press", name: "Press Start 2P", family: "'Press Start 2P'" },
  { id: "tektur", name: "Tektur", family: "'Tektur'" },
  { id: "unbounded", name: "Unbounded", family: "'Unbounded'" },
] as const;

export type FontId = (typeof fonts)[number]["id"];

/** Ready-made accent colours, named after things from the game. */
export const accentPresets = [
  { name: "Diamond", color: "#5ce1e6" },
  { name: "Emerald", color: "#4fd27a" },
  { name: "Gold", color: "#ffc24b" },
  { name: "Copper", color: "#e8875a" },
  { name: "Redstone", color: "#ff5c5c" },
  { name: "Cherry", color: "#ff8fc7" },
  { name: "Amethyst", color: "#b48cff" },
  { name: "Lapis", color: "#4a7dff" },
  { name: "Slime", color: "#a8e04a" },
  { name: "Quartz", color: "#e9e4dc" },
] as const;

export type Corners = "pixel" | "round" | "square";
export type Motion = "system" | "full" | "off";

export type Background = {
  /** Reference into Glint's media folder, e.g. `backgrounds/17.png`; null shows the theme's pixel scene. */
  image: string | null;
  blur: number;
  dim: number;
  /** Focal point in percent, used when the image is cropped to fit. */
  x: number;
  y: number;
  /** Behind the whole window instead of only the Home hero. */
  everywhere: boolean;
};

/** A saved colour scheme: a base theme with its own accent, surface tint and font. */
export type CustomTheme = { id: string; name: string; theme: ThemeName; accent: string | null; tint: string | null; font: FontId };

export type Appearance = {
  /** Base palette and Home scene. */
  theme: ThemeName;
  accent: string | null;
  tint: string | null;
  font: FontId;
  background: Background;
  /** Interface scale in percent. */
  scale: number;
  corners: Corners;
  motion: Motion;
  sidebar: "full" | "compact";
  saved: CustomTheme[];
};

export const scales = [90, 100, 110, 125] as const;

export const DEFAULT_BACKGROUND: Background = { image: null, blur: 0, dim: 35, x: 50, y: 50, everywhere: false };

export const DEFAULT_APPEARANCE: Appearance = {
  theme: "mine",
  accent: null,
  tint: null,
  font: "unbounded",
  background: DEFAULT_BACKGROUND,
  scale: 100,
  corners: "pixel",
  motion: "system",
  sidebar: "full",
  saved: [],
};

// ---- colour maths ----

const HEX = /^#[0-9a-f]{6}$/i;

export const isHex = (value: unknown): value is string => typeof value === "string" && HEX.test(value);

type Rgb = [number, number, number];

function rgb(hex: string): Rgb {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function hex([r, g, b]: Rgb): string {
  return `#${[r, g, b].map((c) => Math.round(Math.min(255, Math.max(0, c))).toString(16).padStart(2, "0")).join("")}`;
}

/** Blends `a` towards `b`; t = 0 gives a, t = 1 gives b. */
export function mix(a: string, b: string, t: number): string {
  const [x, y] = [rgb(a), rgb(b)];
  return hex([0, 1, 2].map((i) => x[i] + (y[i] - x[i]) * t) as Rgb);
}

function luminance(color: string): number {
  const [r, g, b] = rgb(color).map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG contrast ratio, 1 to 21. */
export function contrast(a: string, b: string): number {
  const [la, lb] = [luminance(a), luminance(b)];
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

type Vars = Record<string, string>;

/** The accent and every colour that sits on or next to it, readable on this palette. */
function deriveAccent(vars: Vars, accent: string, light: boolean): Vars {
  // Text in the accent colour must read on every surface it lands on.
  const grounds = [vars.bg, vars.panel, vars.card];
  const readable = (c: string) => Math.min(...grounds.map((g) => contrast(c, g))) >= 4.5;
  const toward = light ? "#000000" : "#ffffff";
  let ink = mix(accent, toward, light ? 0.1 : 0.25);
  for (let t = light ? 0.1 : 0.25; t <= 1 && !readable(ink); t += 0.05) ink = mix(accent, toward, t);
  const deep = mix(accent, "#000000", 0.82);
  const edge = mix(accent, "#000000", 0.42);
  return {
    accent,
    "accent-ink": ink,
    link: ink,
    "on-accent": contrast(deep, accent) >= contrast("#ffffff", accent) ? deep : "#ffffff",
    "accent-edge": edge,
    "accent-dark": edge,
    "accent-light": mix(accent, "#ffffff", 0.55),
  };
}

const TINTED = { bg: 0.1, panel: 0.1, card: 0.12, raise: 0.14, line: 0.16, chip: 0.14, edge: 0.06 } as const;

/** Every CSS colour variable for this look. */
export function paletteFor({ theme, accent, tint }: Pick<Appearance, "theme" | "accent" | "tint">): Vars {
  const vars: Vars = { ...themes[theme].vars };
  const light = theme === "day";
  if (tint) {
    for (const [key, t] of Object.entries(TINTED)) vars[key] = mix(vars[key], tint, light ? t * 0.8 : t);
    const alpha = /,\s*([\d.]+)\)$/.exec(vars.plaque)?.[1] ?? "0.86";
    const [r, g, b] = rgb(vars.bg);
    vars.plaque = `rgba(${r},${g},${b},${alpha})`;
  }
  return accent ? { ...vars, ...deriveAccent(vars, accent, light) } : vars;
}

// ---- loading and saving ----

const CACHE_KEY = "glint.appearance";
const MEDIA_KEY = "glint.mediaDir";

const clamp = (value: unknown, min: number, max: number, fallback: number) =>
  typeof value === "number" && Number.isFinite(value) ? Math.min(max, Math.max(min, Math.round(value))) : fallback;

const oneOf = <T extends string>(value: unknown, options: readonly T[], fallback: T): T =>
  options.includes(value as T) ? (value as T) : fallback;

const themeNames = Object.keys(themes) as ThemeName[];
const fontIds = fonts.map((f) => f.id);

export function normalizeTheme(raw: unknown): Omit<CustomTheme, "id" | "name"> & { name?: string } {
  const t = (raw ?? {}) as Record<string, unknown>;
  return {
    name: typeof t.name === "string" && t.name.trim() ? t.name.trim().slice(0, 40) : undefined,
    theme: oneOf(t.theme ?? t.base, themeNames, "mine"),
    accent: isHex(t.accent) ? t.accent.toLowerCase() : null,
    tint: isHex(t.tint) ? t.tint.toLowerCase() : null,
    font: oneOf(t.font, fontIds, DEFAULT_APPEARANCE.font),
  };
}

/** Accepts anything stored by this or an older Glint and fills the gaps with defaults. */
export function normalize(raw: unknown): Appearance {
  const a = (raw ?? {}) as Record<string, unknown>;
  const bg = (a.background ?? {}) as Record<string, unknown>;
  const d = DEFAULT_BACKGROUND;
  const saved = Array.isArray(a.saved)
    ? a.saved.flatMap((s, i) => {
        const t = normalizeTheme(s);
        const id = typeof s?.id === "string" ? s.id : `t${i}`;
        return [{ ...t, id, name: t.name ?? "My theme" }];
      })
    : [];
  return {
    ...normalizeTheme(a),
    background: {
      image: typeof bg.image === "string" && bg.image ? bg.image : null,
      blur: clamp(bg.blur, 0, 24, d.blur),
      dim: clamp(bg.dim, 0, 90, d.dim),
      x: clamp(bg.x, 0, 100, d.x),
      y: clamp(bg.y, 0, 100, d.y),
      everywhere: bg.everywhere === true,
    },
    scale: (scales as readonly number[]).includes(a.scale as number) ? (a.scale as number) : 100,
    corners: oneOf(a.corners, ["pixel", "round", "square"] as const, "pixel"),
    motion: oneOf(a.motion, ["system", "full", "off"] as const, "system"),
    sidebar: oneOf(a.sidebar, ["full", "compact"] as const, "full"),
    saved,
  };
}

function readCache(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeCache(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Storage can be unavailable; the settings file still has everything.
  }
}

function loadCached(): Appearance {
  try {
    return normalize(JSON.parse(readCache(CACHE_KEY) ?? "null"));
  } catch {
    return DEFAULT_APPEARANCE;
  }
}

let mediaDir = readCache(MEDIA_KEY) ?? "";

/** URL for an image stored in Glint's media folder. */
export function mediaUrl(image: string): string {
  const sep = mediaDir.includes("\\") ? "\\" : "/";
  return convertFileSrc(`${mediaDir}${sep}${image.replace(/\//g, sep)}`);
}

let themingTimer: ReturnType<typeof setTimeout> | undefined;
let applied: Appearance | null = null;

function apply(a: Appearance) {
  const root = document.documentElement;
  const colours = !applied || applied.theme !== a.theme || applied.accent !== a.accent || applied.tint !== a.tint;
  if (colours && applied) {
    // Cross-fade colours while the new theme lands (see .theming in App.css).
    root.classList.add("theming");
    clearTimeout(themingTimer);
    themingTimer = setTimeout(() => root.classList.remove("theming"), 450);
  }
  if (colours) {
    for (const [key, value] of Object.entries(paletteFor(a))) root.style.setProperty(`--${key}`, value);
  }
  applied = a;
  root.style.setProperty("--display-font", fonts.find((f) => f.id === a.font)!.family);
  root.style.colorScheme = a.theme === "day" ? "light" : "dark";
  root.dataset.corners = a.corners;
  root.dataset.motion = a.motion;
  root.dataset.sidebar = a.sidebar;
  root.style.zoom = a.scale === 100 ? "" : String(a.scale / 100);
}

/**
 * Current appearance, applied to the document. It lives in `appearance.json` in the data folder so
 * every release channel shares it; localStorage keeps a copy for a flash-free start and is where
 * older versions kept it.
 */
export function useAppearance() {
  const [appearance, setAppearance] = useState<Appearance>(loadCached);
  const [, setMedia] = useState(mediaDir);
  const loaded = useRef(false);

  useEffect(() => {
    api.loadAppearance().then(
      ({ data, mediaDir: dir }) => {
        mediaDir = dir;
        writeCache(MEDIA_KEY, dir);
        setMedia(dir);
        loaded.current = true;
        if (data) setAppearance(normalize(data));
        else api.saveAppearance(loadCached()).catch(() => {});
      },
      () => {
        loaded.current = true;
      },
    );
  }, []);

  useEffect(() => {
    apply(appearance);
    writeCache(CACHE_KEY, JSON.stringify(appearance));
    if (!loaded.current) return;
    // Sliders fire on every step; write the file once they settle.
    const timer = setTimeout(() => api.saveAppearance(appearance).catch(() => {}), 400);
    return () => clearTimeout(timer);
  }, [appearance]);

  return [appearance, setAppearance] as const;
}

/** Whether the current colours match a saved theme (or a built-in one with no overrides). */
export const sameColours = (a: Pick<Appearance, "theme" | "accent" | "tint" | "font">, t: Omit<CustomTheme, "id" | "name">) =>
  a.theme === t.theme && a.accent === t.accent && a.tint === t.tint && a.font === t.font;

export const sceneFor = (theme: ThemeName): SceneName => theme;
