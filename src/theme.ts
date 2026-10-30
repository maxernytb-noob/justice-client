// Derives the whole accent palette from one hex colour and applies it as CSS variables.

export const ACCENT_PRESETS: { name: string; nameEn: string; hex: string }[] = [
  { name: "Justice červená", nameEn: "Justice red", hex: "#e11d38" },
  { name: "Modrá", nameEn: "Blue", hex: "#3b82f6" },
  { name: "Růžová", nameEn: "Pink", hex: "#ec4899" },
  { name: "Fialová", nameEn: "Purple", hex: "#8b5cf6" },
  { name: "Zelená", nameEn: "Green", hex: "#22c55e" },
  { name: "Oranžová", nameEn: "Orange", hex: "#f97316" },
  { name: "Tyrkysová", nameEn: "Cyan", hex: "#06b6d4" },
  { name: "Zlatá", nameEn: "Gold", hex: "#eab308" },
  { name: "Bílá", nameEn: "White", hex: "#e5e5e5" },
];

export const BG_STYLES: Record<string, { name: string; nameEn: string; bg: string; bg2: string; panel: string; panel2: string; panel3: string; line: string; line2: string }> = {
  oled: { name: "OLED černá", nameEn: "OLED black", bg: "#000000", bg2: "#050505", panel: "#0b0b0c", panel2: "#111113", panel3: "#18181b", line: "#1a1a1e", line2: "#26262b" },
  dark: { name: "Tmavá", nameEn: "Dark", bg: "#08080a", bg2: "#0d0d10", panel: "#121215", panel2: "#17171b", panel3: "#1e1e23", line: "#222228", line2: "#2e2e36" },
  graphite: { name: "Grafit", nameEn: "Graphite", bg: "#111317", bg2: "#15181d", panel: "#1a1d23", panel2: "#20242b", panel3: "#282c34", line: "#2b2f37", line2: "#383d47" },
  // Backgrounds of the theme presets below.
  void: { name: "Justice", nameEn: "Justice", bg: "#050506", bg2: "#09090b", panel: "#0e0e10", panel2: "#141417", panel3: "#1b1b1f", line: "#1c1c21", line2: "#2a2a30" },
  midnight: { name: "Midnight", nameEn: "Midnight", bg: "#05070d", bg2: "#080b13", panel: "#0d111b", panel2: "#121826", panel3: "#192030", line: "#1a2131", line2: "#273046" },
  aurora: { name: "Aurora", nameEn: "Aurora", bg: "#04080a", bg2: "#070c0f", panel: "#0b1215", panel2: "#10191d", panel3: "#172226", line: "#172327", line2: "#233338" },
  sakura: { name: "Sakura", nameEn: "Sakura", bg: "#0a0508", bg2: "#0e080c", panel: "#140c11", panel2: "#1b1117", panel3: "#24171f", line: "#26171f", line2: "#36222d" },
  mono: { name: "Mono", nameEn: "Mono", bg: "#0b0b0c", bg2: "#0f0f10", panel: "#141415", panel2: "#1a1a1b", panel3: "#222224", line: "#232325", line2: "#313134" },
};

/** One-click looks: background + accent together. */
export const THEME_PRESETS: { id: string; name: string; bg: string; accent: string }[] = [
  { id: "justice", name: "Justice", bg: "void", accent: "#e11d38" },
  { id: "midnight", name: "Midnight", bg: "midnight", accent: "#3b82f6" },
  { id: "aurora", name: "Aurora", bg: "aurora", accent: "#14b8a6" },
  { id: "sakura", name: "Sakura", bg: "sakura", accent: "#ec4899" },
  { id: "mono", name: "Mono", bg: "mono", accent: "#e5e5e5" },
];

export function normalizeHex(input: string): string | null {
  let h = input.trim().replace(/^#/, "");
  if (/^[0-9a-f]{3}$/i.test(h)) h = h.split("").map((c) => c + c).join("");
  return /^[0-9a-f]{6}$/i.test(h) ? `#${h.toLowerCase()}` : null;
}

function rgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function toHex([r, g, b]: number[]): string {
  return "#" + [r, g, b].map((x) => Math.round(Math.max(0, Math.min(255, x))).toString(16).padStart(2, "0")).join("");
}

/** a*(1-t) + b*t */
function mix(a: string, b: string, t: number): string {
  const [x, y] = [rgb(a), rgb(b)];
  return toHex(x.map((v, i) => v * (1 - t) + y[i] * t));
}

function luminance(hex: string): number {
  const [r, g, b] = rgb(hex).map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function applyTheme(accentInput: string, bgStyle: string) {
  const accent = normalizeHex(accentInput) ?? "#e11d38";
  const bg = BG_STYLES[bgStyle] ?? BG_STYLES.dark;
  const [r, g, b] = rgb(accent);
  const s = document.documentElement.style;
  const light = luminance(accent) > 0.45;

  s.setProperty("--bg", bg.bg);
  s.setProperty("--bg-2", bg.bg2);
  s.setProperty("--panel", bg.panel);
  s.setProperty("--panel-2", bg.panel2);
  s.setProperty("--panel-3", bg.panel3);
  s.setProperty("--line", bg.line);
  s.setProperty("--line-2", bg.line2);

  s.setProperty("--red", accent);
  s.setProperty("--red-2", mix(accent, "#000000", 0.12));
  s.setProperty("--red-dim", mix(accent, bg.bg, 0.8));
  s.setProperty("--red-tint", mix(accent, bg.bg, 0.9));
  s.setProperty("--red-border", mix(accent, bg.bg, 0.72));
  s.setProperty("--red-soft", mix(accent, "#ffffff", 0.45));
  s.setProperty("--red-glow", `rgba(${r}, ${g}, ${b}, 0.16)`);
  s.setProperty("--red-hero", `rgba(${r}, ${g}, ${b}, 0.28)`);
  s.setProperty("--red-hero-2", `rgba(${r}, ${g}, ${b}, 0.06)`);
  s.setProperty("--on-red", light ? "#0b0b0b" : "#ffffff");
  // v2 look: brighter accent for text on dark, glows and the hero tint.
  s.setProperty("--accent-2", mix(accent, "#ffffff", 0.3));
  s.setProperty("--accent-glow", `rgba(${r}, ${g}, ${b}, 0.35)`);
  s.setProperty("--accent-soft", `rgba(${r}, ${g}, ${b}, 0.14)`);
  s.setProperty("--hero-tint", `rgba(${Math.round(r * 0.5)}, ${Math.round(g * 0.5)}, ${Math.round(b * 0.5)}, 0.55)`);
}
