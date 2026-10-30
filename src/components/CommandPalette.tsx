import { useEffect, useMemo, useRef, useState } from "react";
import { social, type SocialUser } from "../api";
import { Icon } from "../icons";
import { t, tr } from "../i18n";
import { useStore, type Nav } from "../store";
import { InstGlyph } from "./common";
import { sound } from "../sounds";
import { secretSearch } from "./Secrets";

let setOpenGlobal: ((v: boolean) => void) | null = null;
/** Opens the palette from anywhere (sidebar Search, title bar). */
export function openPalette() {
  setOpenGlobal?.(true);
}

interface Item {
  key: string;
  group: string;
  icon: string;
  glyph?: React.ReactNode;
  title: string;
  hint?: string;
  run: (shift: boolean) => void;
}

const PAGES: { nav: Nav; icon: string; label: [string, string]; words: string }[] = [
  { nav: { page: "home" }, icon: "home", label: ["Domů", "Home"], words: "home domu start" },
  { nav: { page: "instances" }, icon: "box", label: ["Instance", "Instances"], words: "instances profiles" },
  { nav: { page: "browse" }, icon: "puzzle", label: ["Mody a modpacky", "Mods and modpacks"], words: "mods modpacks shaders resource packs modrinth curseforge" },
  { nav: { page: "servers" }, icon: "server", label: ["Servery", "Servers"], words: "servers multiplayer" },
  { nav: { page: "recording" }, icon: "video", label: ["Natáčení", "Recording"], words: "recording obs flashback natáčení" },
  { nav: { page: "clips" }, icon: "clips", label: ["Klipy", "Clips"], words: "clips klipy medal" },
  { nav: { page: "friends" }, icon: "users", label: ["Přátelé", "Friends"], words: "friends přátelé kamarádi chat party" },
  { nav: { page: "accounts" }, icon: "user", label: ["Účty a skiny", "Accounts and skins"], words: "accounts účty skins skiny capes pláště" },
  { nav: { page: "settings", tab: "appearance" }, icon: "palette", label: ["Vzhled", "Appearance"], words: "appearance theme vzhled motiv barva background pozadí" },
  { nav: { page: "settings", tab: "performance" }, icon: "zap", label: ["Výkon a Java", "Performance and Java"], words: "performance výkon java ram memory paměť gc" },
  { nav: { page: "settings", tab: "music" }, icon: "music", label: ["Hudba a Sonolith", "Music and Sonolith"], words: "music hudba sonolith spotify" },
  { nav: { page: "settings" }, icon: "settings", label: ["Nastavení", "Settings"], words: "settings nastavení" },
];

const norm = (s: string) => s.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "");

export default function CommandPalette() {
  const { instances, go, play } = useStore();
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [friends, setFriends] = useState<SocialUser[]>([]);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setOpenGlobal = setOpen;
    const k = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
      }
    };
    addEventListener("keydown", k);
    return () => {
      setOpenGlobal = null;
      removeEventListener("keydown", k);
    };
  }, []);

  useEffect(() => {
    if (!open) return;
    sound("open");
    setQ("");
    setSel(0);
    requestAnimationFrame(() => input.current?.focus());
    social
      .friends()
      .then((f) => setFriends(f.friends))
      .catch(() => setFriends([]));
  }, [open]);

  const close = () => setOpen(false);
  const items = useMemo<Item[]>(() => {
    const s = norm(q.trim());
    const out: Item[] = [];
    const pages = PAGES.filter((p) => !s || norm(tr(...p.label)).includes(s) || norm(p.words).includes(s));
    const insts = instances.filter((i) => !s || norm(i.name).includes(s) || i.mcVersion.includes(s));
    for (const i of insts.slice(0, s ? 8 : 5)) {
      out.push({
        key: `i:${i.id}`,
        group: tr("Instance", "Instances"),
        icon: "box",
        glyph: <InstGlyph icon={i.icon} template={i.template} size={16} />,
        title: i.name,
        hint: `${i.loader === "vanilla" ? "Vanilla" : i.loader[0].toUpperCase() + i.loader.slice(1)} ${i.mcVersion}`,
        run: (shift) => (shift ? play(i.id) : go({ page: "instance", id: i.id })),
      });
    }
    for (const p of pages.slice(0, s ? 6 : 12)) {
      out.push({ key: `p:${p.label[1]}`, group: tr("Stránky", "Pages"), icon: p.icon, title: t(tr(...p.label)), run: () => go(p.nav) });
    }
    if (s) {
      for (const f of friends.filter((f) => norm(f.name).includes(s)).slice(0, 5)) {
        out.push({ key: `f:${f.uuid}`, group: tr("Přátelé", "Friends"), icon: "user", title: f.name, hint: f.online ? tr("online", "online") : tr("offline", "offline"), run: () => go({ page: "friends" }) });
      }
      out.push({ key: "mods", group: tr("Hledat", "Search"), icon: "puzzle", title: tr(`Hledat mody: „${q.trim()}“`, `Search mods: “${q.trim()}”`), run: () => go({ page: "browse", q: q.trim() }) });
      out.push({ key: "packs", group: tr("Hledat", "Search"), icon: "box", title: tr(`Hledat modpacky: „${q.trim()}“`, `Search modpacks: “${q.trim()}”`), run: () => go({ page: "browse", tab: "modpack", q: q.trim() }) });
    }
    return out;
  }, [q, instances, friends, go, play]);

  useEffect(() => setSel(0), [q]);
  useEffect(() => {
    list.current?.querySelector(".cp-item.on")?.scrollIntoView({ block: "nearest" });
  }, [sel]);

  if (!open) return null;
  const run = (it: Item | undefined, shift: boolean) => {
    if (!it) return;
    close();
    it.run(shift);
  };
  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") return close();
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSel((v) => Math.min(items.length - 1, v + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSel((v) => Math.max(0, v - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (secretSearch(norm(q.trim()))) return close();
      run(items[sel], e.shiftKey);
    }
  };

  let lastGroup = "";
  return (
    <div className="cp-overlay" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div className="cp" role="dialog" aria-label={tr("Hledat", "Search")}>
        <label className="cp-input">
          <Icon name="search" size={18} />
          <input ref={input} value={q} onChange={(e) => setQ(e.target.value)} onKeyDown={onKey} placeholder={tr("Hledej instance, stránky, kamarády nebo mody…", "Search instances, pages, friends or mods…")} />
          <kbd>Esc</kbd>
        </label>
        <div className="cp-list" ref={list}>
          {items.length === 0 && <div className="cp-empty">{tr("Nic nenalezeno", "Nothing found")}</div>}
          {items.map((it, i) => {
            const head = it.group !== lastGroup ? it.group : null;
            lastGroup = it.group;
            return (
              <div key={it.key}>
                {head && <div className="cp-group">{head}</div>}
                <button className={`cp-item ${i === sel ? "on" : ""}`} onMouseEnter={() => setSel(i)} onClick={(e) => run(it, e.shiftKey)}>
                  <span className="cp-ic">{it.glyph ?? <Icon name={it.icon} size={16} />}</span>
                  <span className="cp-title">{it.title}</span>
                  {it.hint && <span className="cp-hint">{it.hint}</span>}
                  {i === sel && <Icon name="right" size={14} />}
                </button>
              </div>
            );
          })}
        </div>
        <div className="cp-foot">
          <span><kbd>↑</kbd><kbd>↓</kbd> {tr("vybrat", "select")}</span>
          <span><kbd>Enter</kbd> {tr("otevřít", "open")}</span>
          <span><kbd>Shift</kbd>+<kbd>Enter</kbd> {tr("spustit instanci", "play instance")}</span>
        </div>
      </div>
    </div>
  );
}
