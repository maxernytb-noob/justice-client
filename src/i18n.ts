// Every UI string is written in both languages right where it's used: tr("Hledat", "Search").
// Keeping the pair together means a new string can't silently miss its translation.
import type { ReactNode } from "react";

let lang: "cs" | "en" = "cs";
export function setLang(l: "cs" | "en") {
  lang = l;
}
export function isEn() {
  return lang === "en";
}

export function tr<T extends ReactNode>(cs: T, en: T): T {
  return lang === "en" ? en : cs;
}

/** Locale for dates / numbers. */
export function locale() {
  return lang === "en" ? "en-GB" : "cs-CZ";
}

// Kept for older call sites: maps the handful of navigation labels.
const EN: Record<string, string> = {
  "Domů": "Home",
  "Instance": "Instances",
  "Mody": "Mods",
  "Natáčení": "Recording",
  "Klipy": "Clips",
  "Servery": "Servers",
  "Přátelé": "Friends",
  "Nastavení": "Settings",
};
export function t(s: string): string {
  return lang === "en" ? EN[s] ?? s : s;
}
