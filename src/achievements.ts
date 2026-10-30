// Achievements are computed in the launcher from local data and published with the profile.
import { tr } from "./i18n";

export interface Stats {
  playtimeSecs: number;
  instances: number;
  clips: number;
  friends: number;
  pvp?: { kills: number; deaths: number; bestCombo: number; hits: number };
}

/** Justice capes (textures in public/capes, same as in Justice Mod). */
export const CAPES: { id: string; name: [string, string] }[] = [
  { id: "justice", name: ["Justice", "Justice"] },
  { id: "midnight", name: ["Půlnoc", "Midnight"] },
  { id: "gold", name: ["Zlato", "Gold"] },
  { id: "frost", name: ["Mráz", "Frost"] },
  { id: "ember", name: ["Žhavé uhlíky", "Ember"] },
  { id: "toxic", name: ["Toxic", "Toxic"] },
  { id: "royal", name: ["Královský", "Royal"] },
  { id: "void", name: ["Void", "Void"] },
  { id: "crimson", name: ["Karmín", "Crimson"] },
  { id: "ocean", name: ["Oceán", "Ocean"] },
  { id: "forest", name: ["Les", "Forest"] },
  { id: "sunset", name: ["Západ slunce", "Sunset"] },
  { id: "aurora", name: ["Polární záře", "Aurora"] },
  { id: "galaxy", name: ["Galaxie", "Galaxy"] },
  { id: "lava", name: ["Láva", "Lava"] },
  { id: "glacier", name: ["Ledovec", "Glacier"] },
  { id: "rose", name: ["Růže", "Rose"] },
  { id: "storm", name: ["Bouře", "Storm"] },
  { id: "emerald", name: ["Smaragd", "Emerald"] },
  { id: "ruby", name: ["Rubín", "Ruby"] },
  { id: "sapphire", name: ["Safír", "Sapphire"] },
  { id: "magma", name: ["Magma", "Magma"] },
  { id: "neon", name: ["Neon", "Neon"] },
  { id: "candy", name: ["Bonbón", "Candy"] },
  { id: "shadow", name: ["Stín", "Shadow"] },
  { id: "desert", name: ["Poušť", "Desert"] },
  { id: "blossom", name: ["Květ", "Blossom"] },
  { id: "prism", name: ["Hranol", "Prism"] },
];

export const PETS: { id: string; name: [string, string] }[] = [
  { id: "cube", name: ["Kostka", "Cube"] },
  { id: "slime", name: ["Sliz", "Slime"] },
  { id: "ghost", name: ["Duch", "Ghost"] },
  { id: "bee", name: ["Včelka", "Bee"] },
  { id: "fox", name: ["Liška", "Fox"] },
  { id: "axolotl", name: ["Axolotl", "Axolotl"] },
  { id: "dragon", name: ["Dráček", "Dragon"] },
  { id: "cat", name: ["Kočka", "Cat"] },
  { id: "duck", name: ["Kachna", "Duck"] },
  { id: "bat", name: ["Netopýr", "Bat"] },
  { id: "star", name: ["Hvězda", "Star"] },
  { id: "heart", name: ["Srdce", "Heart"] },
];

export const ACHIEVEMENTS: { id: string; icon: string; mc: string; name: [string, string]; text: [string, string]; test: (s: Stats) => boolean }[] = [
  { id: "first", icon: "play", mc: "emerald", name: ["První kroky", "First steps"], text: ["Zahraj si poprvé přes Justice Client", "Play for the first time with Justice Client"], test: (s) => s.playtimeSecs > 0 },
  { id: "h10", icon: "clock", mc: "clock", name: ["Rozehřátý", "Warmed up"], text: ["10 hodin ve hře", "10 hours in-game"], test: (s) => s.playtimeSecs >= 10 * 3600 },
  { id: "h100", icon: "flame", mc: "flame", name: ["Nepřetržitě", "Non-stop"], text: ["100 hodin ve hře", "100 hours in-game"], test: (s) => s.playtimeSecs >= 100 * 3600 },
  { id: "h500", icon: "crown", mc: "crown", name: ["Legenda", "Legend"], text: ["500 hodin ve hře", "500 hours in-game"], test: (s) => s.playtimeSecs >= 500 * 3600 },
  { id: "inst5", icon: "layers", mc: "chest", name: ["Sběratel", "Collector"], text: ["5 instancí", "5 instances"], test: (s) => s.instances >= 5 },
  { id: "clip1", icon: "clips", mc: "clapper", name: ["Záznam!", "Rolling!"], text: ["Ulož první klip", "Save your first clip"], test: (s) => s.clips >= 1 },
  { id: "clip25", icon: "film", mc: "trophy", name: ["Režisér", "Director"], text: ["25 klipů", "25 clips"], test: (s) => s.clips >= 25 },
  { id: "friend1", icon: "users", mc: "steve", name: ["Parťák", "Buddy"], text: ["Prvního přítele", "Your first friend"], test: (s) => s.friends >= 1 },
  { id: "friend10", icon: "heart", mc: "heart", name: ["Společenský", "Social"], text: ["10 přátel", "10 friends"], test: (s) => s.friends >= 10 },
  { id: "kill1", icon: "sword", mc: "sword", name: ["První krev", "First blood"], text: ["Zabij prvního hráče (počítá Justice Mod)", "Kill your first player (counted by Justice Mod)"], test: (s) => (s.pvp?.kills ?? 0) >= 1 },
  { id: "kill100", icon: "skull", mc: "skull", name: ["Lovec", "Hunter"], text: ["100 zabitých hráčů", "100 player kills"], test: (s) => (s.pvp?.kills ?? 0) >= 100 },
  { id: "combo10", icon: "zap", mc: "bolt", name: ["Nedotknutelný", "Untouchable"], text: ["Combo 10 zásahů", "A 10-hit combo"], test: (s) => (s.pvp?.bestCombo ?? 0) >= 10 },
];

export function earned(s: Stats): string[] {
  return ACHIEVEMENTS.filter((a) => a.test(s)).map((a) => a.id);
}

export function achievementName(id: string) {
  const a = ACHIEVEMENTS.find((x) => x.id === id);
  return a ? tr(a.name[0], a.name[1]) : id;
}

export const BADGES: Record<string, { label: [string, string]; cls: string }> = {
  team: { label: ["Tým Justice", "Justice team"], cls: "red" },
  early: { label: ["Early", "Early"], cls: "amber" },
  offline: { label: ["Offline", "Offline"], cls: "" },
};

/** Profile banner presets (CSS backgrounds). */
export const BANNERS: Record<string, string> = {
  red: "linear-gradient(135deg, #e11d38, #5b0714)",
  night: "linear-gradient(135deg, #1e3a8a, #0b1024)",
  aurora: "linear-gradient(135deg, #10b981, #6366f1)",
  sunset: "linear-gradient(135deg, #f97316, #db2777)",
  gold: "linear-gradient(135deg, #eab308, #92400e)",
  mono: "linear-gradient(135deg, #3f3f46, #09090b)",
};

export function head(uuid: string, size = 64) {
  return `https://mc-heads.net/avatar/${uuid}/${size}`;
}
