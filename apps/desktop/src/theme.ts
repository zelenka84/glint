import { useEffect, useState } from "react";
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

export type Appearance = { theme: ThemeName; font: FontId };

const STORAGE_KEY = "glint.appearance";
const DEFAULT: Appearance = { theme: "mine", font: "pixelify" };

function load(): Appearance {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT;
    const parsed = JSON.parse(raw) as Partial<Appearance>;
    const theme = parsed.theme && parsed.theme in themes ? parsed.theme : DEFAULT.theme;
    const font = fonts.some((f) => f.id === parsed.font) ? (parsed.font as FontId) : DEFAULT.font;
    return { theme, font };
  } catch {
    return DEFAULT;
  }
}

let themingTimer: ReturnType<typeof setTimeout> | undefined;

function apply({ theme, font }: Appearance) {
  const root = document.documentElement;
  // Cross-fade colours while the new theme lands (see .theming in App.css).
  root.classList.add("theming");
  clearTimeout(themingTimer);
  themingTimer = setTimeout(() => root.classList.remove("theming"), 450);
  for (const [key, value] of Object.entries(themes[theme].vars)) {
    root.style.setProperty(`--${key}`, value);
  }
  root.style.setProperty("--display-font", fonts.find((f) => f.id === font)!.family);
  root.style.colorScheme = theme === "day" ? "light" : "dark";
}

/** Current appearance, applied to the document and remembered between launches. */
export function useAppearance() {
  const [appearance, setAppearance] = useState<Appearance>(load);
  useEffect(() => {
    apply(appearance);
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(appearance));
    } catch {
      // Storage can be unavailable; the choice then lasts for this session only.
    }
  }, [appearance]);
  return [appearance, setAppearance] as const;
}

export const sceneFor = (theme: ThemeName): SceneName => theme;
