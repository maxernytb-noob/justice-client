// Launcher look & layout preferences (stored as `settings.ui`), applied as CSS variables/classes.
import { convertFileSrc } from "@tauri-apps/api/core";
import { setSoundPrefs } from "./sounds";
import type { Page } from "./store";

export type NavPage = "home" | "instances" | "browse" | "recording" | "servers" | "friends" | "clips";

export interface Shortcut {
  kind: "instance" | "server";
  id: string;
}

export interface CustomQuick {
  id: string;
  name: string;
  address: string;
}

/** At most this many own servers fit into the title bar. */
export const MAX_CUSTOM_QUICK = 3;

export interface UiPrefs {
  radius: "sharp" | "normal" | "round";
  /** UI zoom, 0.85–1.25. */
  scale: number;
  font: "inter" | "system" | "rounded" | "mono";
  /** Custom background picture (absolute path), empty = none. */
  bgImage: string;
  /** Darkening over the picture, 0–1. */
  bgDim: number;
  /** Blur of the picture in px. */
  bgBlur: number;
  /** Translucent panels over the background. */
  glass: boolean;
  /** Sidebar order and hidden entries. */
  navOrder: NavPage[];
  navHidden: NavPage[];
  shortcuts: Shortcut[];
  /** Title-bar quick-join buttons the user turned off (server ids). */
  quickHidden: string[];
  /** The user's own servers shown next to Justice SMP / Ertenix in the title bar. */
  quickCustom: CustomQuick[];
  /** Page shown when the launcher opens. */
  startPage: Page;
  animations: boolean;
  /** Wide sidebar with labels and sections, or the icon rail. */
  sidebar: "full" | "compact";
  /** Spacing in lists and cards. */
  density: "comfortable" | "compact";
  /** Short UI sounds (play, done, error, notifications). */
  sounds: boolean;
  /** 0–1. */
  soundVolume: number;
  /** Home sections the user turned off. */
  homeHidden: HomeSection[];
  /** The drop's art behind the whole window on Home. */
  homeArt: boolean;
}

export type HomeSection = "versions" | "instances" | "friends" | "news";

export const NAV_ALL: NavPage[] = ["home", "instances", "browse", "recording", "servers", "friends", "clips"];

export const DEFAULT_UI: UiPrefs = {
  radius: "normal",
  scale: 1,
  font: "inter",
  bgImage: "",
  bgDim: 0.55,
  bgBlur: 6,
  glass: true,
  navOrder: NAV_ALL,
  navHidden: [],
  shortcuts: [],
  quickHidden: [],
  quickCustom: [],
  startPage: "home",
  animations: true,
  sidebar: "full",
  density: "comfortable",
  sounds: true,
  soundVolume: 0.5,
  homeHidden: [],
  homeArt: true,
};

/** Settings may hold an older/partial object (or null): fill in defaults and add new sidebar pages. */
export function uiPrefs(raw: unknown): UiPrefs {
  const p = { ...DEFAULT_UI, ...(raw && typeof raw === "object" ? (raw as Partial<UiPrefs>) : {}) };
  const order = (p.navOrder ?? []).filter((x) => NAV_ALL.includes(x));
  p.navOrder = [...order, ...NAV_ALL.filter((x) => !order.includes(x))];
  p.navHidden = (p.navHidden ?? []).filter((x) => NAV_ALL.includes(x));
  p.shortcuts = Array.isArray(p.shortcuts) ? p.shortcuts : [];
  p.quickHidden = Array.isArray(p.quickHidden) ? p.quickHidden : [];
  p.quickCustom = (Array.isArray(p.quickCustom) ? p.quickCustom : [])
    .filter((q) => q && typeof q.address === "string" && q.address.trim())
    .slice(0, MAX_CUSTOM_QUICK);
  p.scale = Math.min(1.25, Math.max(0.85, Number(p.scale) || 1));
  p.soundVolume = Math.min(1, Math.max(0, Number(p.soundVolume ?? 0.5)));
  p.homeHidden = Array.isArray(p.homeHidden) ? p.homeHidden : [];
  return p;
}

const FONTS: Record<UiPrefs["font"], string> = {
  inter: `"Inter", "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif`,
  system: `system-ui, "Segoe UI", "Cantarell", "Noto Sans", sans-serif`,
  rounded: `"Nunito", "Varela Round", "Segoe UI", system-ui, sans-serif`,
  mono: `"JetBrains Mono", "Cascadia Mono", "Consolas", monospace`,
};

export function applyUi(p: UiPrefs) {
  const root = document.documentElement;
  const s = root.style;
  s.setProperty("--font", FONTS[p.font] ?? FONTS.inter);
  const [r, rl] = p.radius === "sharp" ? [3, 4] : p.radius === "round" ? [14, 20] : [10, 14];
  s.setProperty("--radius", `${r}px`);
  s.setProperty("--radius-lg", `${rl}px`);
  root.classList.toggle("ui-sharp", p.radius === "sharp");
  root.classList.toggle("ui-round", p.radius === "round");
  root.classList.toggle("ui-noanim", !p.animations);
  root.classList.toggle("ui-dense", p.density === "compact");
  setSoundPrefs(p.sounds, p.soundVolume);
  // Animations off also means light mode (main.tsx turns it on for Linux regardless).
  const forcedLow = (/Linux/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent)) || new URLSearchParams(location.search).has("lowfx");
  root.classList.toggle("lowfx", forcedLow || !p.animations);
  (s as CSSStyleDeclaration & { zoom: string }).zoom = String(p.scale);

  const hasBg = !!p.bgImage;
  root.classList.toggle("ui-bgimage", hasBg);
  root.classList.toggle("ui-glass", hasBg && p.glass);
  s.setProperty("--bg-image", hasBg ? `url("${convertFileSrc(p.bgImage)}")` : "none");
  s.setProperty("--bg-dim", String(p.bgDim));
  s.setProperty("--bg-blur", `${p.bgBlur}px`);
}
