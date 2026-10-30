import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { join } from "@tauri-apps/api/path";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, media } from "../api";
import { Icon } from "../icons";
import { errText, fmtSize, useStore } from "../store";
import { openFolder, Switch, TaskProgress } from "../components/common";
import { ACCENT_PRESETS, BG_STYLES, normalizeHex, THEME_PRESETS } from "../theme";
import { SecretsCard } from "../components/Secrets";
import { tr } from "../i18n";
import Select from "../components/Select";
import { customQuickServers, QUICK_SERVERS } from "../components/QuickJoin";
import Customize from "../components/Customize";
import { snlConnect, snlDisconnect, useSonolith } from "../components/sonolithStore";
import { MAX_CUSTOM_QUICK, uiPrefs } from "../ui";

function Secret({ value, onSave, placeholder }: { value: string; onSave: (v: string) => void; placeholder?: string }) {
  const [v, setV] = useState(value);
  const [show, setShow] = useState(false);
  useEffect(() => setV(value), [value]);
  return (
    <div className="row" style={{ width: 380 }}>
      <input className="input mono" style={{ fontSize: 12 }} type={show ? "text" : "password"} value={v} placeholder={placeholder} onChange={(e) => setV(e.target.value)} />
      <button className="btn icon" onClick={() => setShow(!show)} aria-label={show ? tr("Skrýt", "Hide") : tr("Zobrazit", "Show")}>
        <Icon name={show ? "eyeoff" : "eye"} size={16} />
      </button>
      {v !== value && (
        <button className="btn primary" onClick={() => onSave(v.trim())}>
          {tr("Uložit", "Save")}
        </button>
      )}
    </div>
  );
}

function Appearance() {
  const { settings: s, saveSettings } = useStore();
  const [hex, setHex] = useState(s?.accentColor ?? "");
  useEffect(() => setHex(s?.accentColor ?? ""), [s?.accentColor]);
  if (!s) return null;
  const valid = normalizeHex(hex);
  const current = normalizeHex(s.accentColor) ?? "#e11d38";
  return (
    <div className="card pad">
      <div className="setting" style={{ paddingTop: 0, alignItems: "flex-start" }}>
        <div className="txt">
          <b>{tr("Motiv", "Theme")}</b>
          <span>{tr("Barvy celého launcheru. Barvu a pozadí můžeš níž ještě doladit.", "Colours of the whole launcher. Fine-tune the colour and background below.")}</span>
          <div className="theme-swatches">
            {THEME_PRESETS.map((t) => {
              const bg = BG_STYLES[t.bg];
              const on = s.bgStyle === t.bg && current === t.accent;
              return (
                <button key={t.id} className={`theme-swatch ${on ? "on" : ""}`} onClick={() => saveSettings({ accentColor: t.accent, bgStyle: t.bg as typeof s.bgStyle })}>
                  <span className="pv" style={{ background: `radial-gradient(80px 50px at 80% 0%, ${t.accent}44, transparent), ${bg.bg}` }}>
                    <i style={{ background: t.accent }} />
                    <i className="l2" style={{ background: bg.panel3 }} />
                  </span>
                  {t.name}
                </button>
              );
            })}
          </div>
        </div>
      </div>
      <div className="setting" style={{ alignItems: "flex-start" }}>
        <div className="txt">
          <b>{tr("Barva launcheru", "Launcher colour")}</b>
          <span>{tr("Vyber předvolbu, nebo zadej vlastní hex kód (např. #3b82f6).", "Pick a preset or enter your own hex code (e.g. #3b82f6).")}</span>
          <div className="row wrap" style={{ gap: 8, marginTop: 12 }}>
            {ACCENT_PRESETS.map((p) => (
              <button
                key={p.hex}
                title={tr(p.name, p.nameEn)}
                aria-label={tr(p.name, p.nameEn)}
                onClick={() => saveSettings({ accentColor: p.hex })}
                style={{
                  width: 32, height: 32, borderRadius: 10, cursor: "pointer", background: p.hex,
                  border: current === p.hex ? "2px solid var(--text)" : "2px solid transparent",
                  boxShadow: current === p.hex ? `0 0 0 3px ${p.hex}55` : "none",
                }}
              />
            ))}
          </div>
          <div className="row" style={{ marginTop: 12, maxWidth: 360 }}>
            <input
              type="color"
              value={current}
              onChange={(e) => saveSettings({ accentColor: e.target.value })}
              style={{ width: 44, height: 36, padding: 0, border: "1px solid var(--line-2)", borderRadius: 9, background: "none", cursor: "pointer" }}
              aria-label={tr("Vybrat barvu", "Pick a colour")}
            />
            <input
              className="input mono"
              value={hex}
              maxLength={7}
              placeholder="#e11d38"
              onChange={(e) => {
                setHex(e.target.value);
                const n = normalizeHex(e.target.value);
                if (n) saveSettings({ accentColor: n });
              }}
              style={{ borderColor: hex && !valid ? "#ef4444" : undefined }}
            />
          </div>
          {hex && !valid && (
            <div className="small" style={{ color: "#ef4444", marginTop: 6 }}>
              {tr("Neplatný hex kód — použij formát #rrggbb.", "Invalid hex code — use the #rrggbb format.")}
            </div>
          )}
        </div>
      </div>
      <div className="setting" style={{ flexDirection: "column", alignItems: "stretch" }}>
        <div className="txt">
          <b>{tr("Pozadí", "Background")}</b>
          <span>{tr("OLED černá šetří oči v noci a vypadá nejčistěji na záznamu.", "OLED black is easy on the eyes at night and looks cleanest on recordings.")}</span>
        </div>
        <div className="chips" style={{ flexWrap: "wrap" }}>
          {Object.entries(BG_STYLES).map(([k, v]) => (
            <button key={k} className={`chip ${s.bgStyle === k ? "on" : ""}`} onClick={() => saveSettings({ bgStyle: k as typeof s.bgStyle })}>
              <span style={{ width: 12, height: 12, borderRadius: 4, background: v.panel2, border: "1px solid var(--line-2)" }} />
              {tr(v.name, v.nameEn)}
            </button>
          ))}
        </div>
      </div>
      <div className="setting">
        <div className="txt">
          <b>{tr("Náhled", "Preview")}</b>
        </div>
        <div className="row">
          <button className="btn primary">
            <Icon name="play" size={14} /> {tr("Hrát", "Play")}
          </button>
          <span className="chip on">{tr("Zapnuto", "On")}</span>
          <Switch checked onChange={() => {}} />
          <span className="badge red">● REC</span>
        </div>
      </div>
    </div>
  );
}

function DataDir() {
  const { settings: s, reloadInstances, reloadAccounts, toast, progress } = useStore();
  const [free, setFree] = useState<number | null>(null);
  const [moving, setMoving] = useState(false);
  useEffect(() => {
    if (s) api.freeSpace(s.dataDir).then(setFree);
  }, [s?.dataDir]);
  if (!s) return null;
  const move = async () => {
    const d = await open({ directory: true, defaultPath: s.dataDir });
    if (!d) return;
    const dest = await join(d as string, "JusticeClient");
    setMoving(true);
    try {
      await api.moveDataDir(dest);
      window.location.reload();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setMoving(false);
      reloadInstances();
      reloadAccounts();
    }
  };
  const low = free != null && free < 10 * 1024 ** 3;
  return (
    <div className="setting">
      <div className="txt">
        <b>{tr("Složka s daty", "Data folder")}</b>
        <span className="mono ellipsis" style={{ display: "block" }}>{s.dataDir}</span>
        {free != null && (
          <span style={{ color: low ? "#ef4444" : undefined }}>
            {tr("Volné místo na disku", "Free disk space")}: {fmtSize(free)}
            {low ? tr(" — málo místa, přesuň data na jiný disk", " — low on space, move your data to another drive") : ""}
          </span>
        )}
        {moving && progress["move-data"] && (
          <div style={{ marginTop: 8 }}>
            <TaskProgress task="move-data" />
          </div>
        )}
      </div>
      <button className="btn" onClick={() => openFolder(s.dataDir)} aria-label={tr("Otevřít složku", "Open folder")}>
        <Icon name="folder" size={16} />
      </button>
      <button
        className="btn"
        onClick={async () => openFolder(await api.logPath())}
        title={tr("Log launcheru (to samé, co vypisuje terminál)", "Launcher log (same as the terminal output)")}
      >
        {tr("Log", "Log")}
      </button>
      <button className="btn" onClick={move} disabled={moving}>
        {moving ? <span className="spinner" /> : <Icon name="download" size={16} />} {tr("Přesunout data", "Move data")}
      </button>
    </div>
  );
}

function HardwareCard() {
  const { settings: s, system, saveSettings } = useStore();
  const [hw, setHw] = useState<{ cpu: string; threads: number; gpus: string[]; totalRamMb: number } | null>(null);
  useEffect(() => {
    api.hardwareInfo().then(setHw).catch(() => {});
  }, []);
  if (!s) return null;
  const rec = system?.recommendedRamMb ?? 4096;
  return (
    <div className="card pad col" style={{ gap: 12 }}>
      <div className="row">
        <Icon name="cpu" size={18} style={{ color: "var(--red)" }} />
        <b className="grow">{tr("Tvůj počítač", "Your PC")}</b>
        {s.ramMb !== rec && (
          <button className="btn sm primary" onClick={() => saveSettings({ ramMb: rec, gc: "auto" })}>
            <Icon name="zap" size={14} /> {tr("Použít doporučené", "Use recommended")} ({(rec / 1024).toFixed(0)} GB RAM)
          </button>
        )}
      </div>
      <div className="stat-tiles">
        <div className="stat">
          <div className="l">{tr("Procesor", "CPU")}</div>
          <div className="small" style={{ fontWeight: 600, marginTop: 4 }}>{hw?.cpu || "…"}</div>
          <div className="small muted">{hw ? `${hw.threads} ${tr("vláken", "threads")}` : ""}</div>
        </div>
        <div className="stat">
          <div className="l">{tr("Grafika", "GPU")}</div>
          <div className="small" style={{ fontWeight: 600, marginTop: 4 }}>{hw?.gpus.join(", ") || "…"}</div>
        </div>
        <div className="stat">
          <div className="l">RAM</div>
          <div className="small" style={{ fontWeight: 600, marginTop: 4 }}>{((system?.totalRamMb ?? 0) / 1024).toFixed(0)} GB</div>
          <div className="small muted">
            {tr("hře doporučeno", "recommended for the game")} {(rec / 1024).toFixed(0)} GB
          </div>
        </div>
      </div>
      {hw && hw.threads <= 8 && (
        <div className="callout">
          <Icon name="info" size={18} />
          <span>
            {tr(
              `Máš ${hw.threads} vláken procesoru. Nejvíc FPS ti vezme Distant Horizons — na hraní zapni u instance Lehký režim a v OBS nahrávej přes NVENC (grafika), ne x264 (procesor).`,
              `Your CPU has ${hw.threads} threads. Distant Horizons costs the most FPS — turn on Light mode on the instance for normal play, and record in OBS with NVENC (GPU), not x264 (CPU).`,
            )}
          </span>
        </div>
      )}
    </div>
  );
}

function QuickJoinSettings() {
  const { settings: s, saveSettings, instances } = useStore();
  const [newName, setNewName] = useState("");
  const [newAddr, setNewAddr] = useState("");
  if (!s) return null;
  const set = (id: string, inst: string) => {
    const next = { ...(s.quickServers ?? {}) };
    if (inst) next[id] = inst;
    else delete next[id];
    saveSettings({ quickServers: next });
  };
  const ui = uiPrefs(s.ui);
  const setShown = (id: string, on: boolean) =>
    saveSettings({ ui: { ...ui, quickHidden: on ? ui.quickHidden.filter((x) => x !== id) : [...ui.quickHidden, id] } });
  const custom = customQuickServers(s.ui);
  const addCustom = () => {
    const address = newAddr.trim();
    if (!address || ui.quickCustom.length >= MAX_CUSTOM_QUICK) return;
    const id = `custom-${Date.now().toString(36)}`;
    saveSettings({ ui: { ...ui, quickCustom: [...ui.quickCustom, { id, name: newName.trim(), address }] } });
    setNewName("");
    setNewAddr("");
  };
  const removeCustom = (id: string) => {
    const next = { ...(s.quickServers ?? {}) };
    delete next[id];
    saveSettings({ quickServers: next, ui: { ...ui, quickCustom: ui.quickCustom.filter((q) => q.id !== id) } });
  };
  return (
    <div className="card pad">
      {[...QUICK_SERVERS, ...custom].map((srv, i) => (
        <div key={srv.id} className="setting" style={i === 0 ? { paddingTop: 0 } : undefined}>
          <div className="txt row" style={{ gap: 12 }}>
            {srv.logo ? (
              <img src={srv.logo} alt="" className={srv.wide ? "qj-wordmark" : "qj-icon"} style={srv.wide ? { height: (srv.logoHeight ?? 13) * 1.3 } : { width: 32, height: 32 }} />
            ) : (
              <span className="qj-letter" style={{ width: 32, height: 32, fontSize: 15 }}>{srv.name.slice(0, 1).toUpperCase()}</span>
            )}
            <div>
              <b>{srv.name}</b>
              <span className="mono" style={{ display: "block" }}>{s.streamerMode ? "••••••" : srv.address}</span>
            </div>
          </div>
          <div className="row" style={{ gap: 12 }}>
            <Select className="select" style={{ width: 260 }} value={s.quickServers?.[srv.id] ?? ""} disabled={ui.quickHidden.includes(srv.id)} onChange={(e) => set(srv.id, e.target.value)}>
              <option value="">{tr("Zeptat se při kliknutí", "Ask when clicked")}</option>
              {instances.map((inst) => (
                <option key={inst.id} value={inst.id}>
                  {inst.name} — {inst.mcVersion}
                </option>
              ))}
            </Select>
            {srv.logo ? (
              <span style={{ display: "flex" }} title={tr("Zobrazit tlačítko v horní liště", "Show the button in the title bar")}>
                <Switch checked={!ui.quickHidden.includes(srv.id)} onChange={(on) => setShown(srv.id, on)} />
              </span>
            ) : (
              <button className="btn ghost sm icon" onClick={() => removeCustom(srv.id)} title={tr("Odebrat z horní lišty", "Remove from the title bar")}>
                <Icon name="trash" size={15} />
              </button>
            )}
          </div>
        </div>
      ))}
      {ui.quickCustom.length < MAX_CUSTOM_QUICK && (
        <div className="setting">
          <div className="txt">
            <b>{tr("Přidat vlastní server", "Add your own server")}</b>
            <span>{tr(`Až ${MAX_CUSTOM_QUICK} servery navíc v horní liště.`, `Up to ${MAX_CUSTOM_QUICK} extra servers in the title bar.`)}</span>
          </div>
          <div className="row" style={{ gap: 8 }}>
            <input className="input" style={{ width: 130 }} placeholder={tr("Název", "Name")} maxLength={24} value={newName} onChange={(e) => setNewName(e.target.value)} />
            <input
              className="input"
              style={{ width: 180 }}
              placeholder="mc.server.cz"
              value={newAddr}
              onChange={(e) => setNewAddr(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addCustom()}
            />
            <button className="btn" onClick={addCustom} disabled={!newAddr.trim()}>
              <Icon name="plus" size={15} /> {tr("Přidat", "Add")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/** The player reads Windows' media controls, so there is nothing to link; just a shortcut to Sonolith. */
const SCOPE_LABEL: Record<string, [string, string]> = {
  "player:read": ["Co hraje", "What's playing"],
  "player:control": ["Ovládání", "Control"],
  "queue:read": ["Fronta", "Queue"],
  "queue:write": ["Přidávání do fronty", "Adding to queue"],
  search: ["Hledání", "Search"],
  "library:read": ["Playlisty", "Playlists"],
  "lyrics:read": ["Texty písní", "Lyrics"],
};

function MusicSettings() {
  const { toast } = useStore();
  const snl = useSonolith();
  const st = snl.status;
  const [busy, setBusy] = useState(false);
  const connect = async () => {
    setBusy(true);
    try {
      await snlConnect();
      toast(tr("Sonolith je připojený", "Sonolith is connected"), "success", "note");
    } catch (e) {
      const m = String(e);
      toast(
        m.includes("denied")
          ? tr("Připojení jsi v Sonolithu odmítl. Zkusit znovu půjde za chvíli.", "You declined in Sonolith. You can try again in a moment.")
          : m.includes("blocked")
            ? tr("Justice Client je v Sonolithu zablokovaný. Odblokuješ ho v Sonolithu: Nastavení → Připojené aplikace.", "Justice Client is blocked in Sonolith. Unblock it in Sonolith: Settings → Connected apps.")
            : m.includes("expired")
              ? tr("Žádost vypršela. Zkus to znovu a potvrď ji v Sonolithu.", "The request expired. Try again and confirm it in Sonolith.")
              : errText(e),
        "error",
      );
    } finally {
      setBusy(false);
    }
  };
  const disconnect = async () => {
    await snlDisconnect().catch(() => {});
    toast(tr("Odpojeno od Sonolithu", "Disconnected from Sonolith"), "info");
  };
  const state = !st
    ? ""
    : st.pending
      ? tr("Potvrď připojení v Sonolithu…", "Confirm the connection in Sonolith…")
      : st.connected
        ? tr(`Připojeno · Sonolith ${st.appVersion}`, `Connected · Sonolith ${st.appVersion}`)
        : st.registered
          ? st.available
            ? tr("Připojuji…", "Connecting…")
            : tr("Sonolith neběží", "Sonolith isn't running")
          : st.available
            ? tr("Nepřipojeno", "Not connected")
            : tr("Sonolith neběží nebo má vypnuté API", "Sonolith isn't running or its API is off");
  return (
    <div className="col" style={{ gap: 14 }}>
      <div className="card pad snl-card">
        <div className="snl-hero">
          <div className="snl-logo">
            <Icon name="music" size={22} />
          </div>
          <div className="grow">
            <b>Sonolith</b>
            <span>{tr("Přehrávač hudby doporučený pro nejlepší zážitek s Justice Clientem", "The music player recommended for the best experience with Justice Client")}</span>
          </div>
          <span className={`snl-dot ${st?.connected ? "on" : st?.pending ? "wait" : ""}`}>{state}</span>
        </div>
        <p className="muted small" style={{ margin: "12px 0" }}>
          {tr(
            "Po připojení uvidíš v přehrávači launcheru frontu, hledání písniček, playlisty a text písně, který běží s hudbou. Bez připojení launcher ukazuje jen to, co hraje, přes systémové ovládání médií.",
            "Once connected, the launcher's player shows your queue, song search, playlists and lyrics that follow the music. Without it the launcher only shows what's playing, through the system media controls.",
          )}
        </p>
        {st?.connected && st.grantedScopes.length > 0 && (
          <div className="chips" style={{ marginBottom: 12 }}>
            {st.grantedScopes.map((sc) => (
              <span key={sc} className="chip on">
                <Icon name="check" size={12} /> {tr(...(SCOPE_LABEL[sc] ?? [sc, sc]))}
              </span>
            ))}
          </div>
        )}
        <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
          {st?.registered ? (
            <button className="btn" onClick={disconnect}>
              {tr("Odpojit od Sonolithu", "Disconnect from Sonolith")}
            </button>
          ) : (
            <button className="btn primary" onClick={connect} disabled={busy || st?.pending || !st?.available}>
              <Icon name="zap" size={14} /> {st?.pending ? tr("Čekám na potvrzení v Sonolithu…", "Waiting for Sonolith…") : tr("Připojit Sonolith", "Connect Sonolith")}
            </button>
          )}
          <button className="btn ghost" onClick={() => media.openMusicApp().catch((e) => toast(String(e), "error"))}>
            {tr("Otevřít Sonolith", "Open Sonolith")}
          </button>
          <a className="snl-credit" href="https://github.com/opecko/sonolith" target="_blank" rel="noreferrer">
            Uses Sonolith
          </a>
        </div>
      </div>
    </div>
  );
}

const SECTIONS: [string, [string, string], string][] = [
  ["appearance", ["Vzhled", "Appearance"], "image"],
  ["customize", ["Rozložení", "Layout"], "layers"],
  ["quick", ["Rychlé připojení", "Quick join"], "server"],
  ["general", ["Obecné", "General"], "settings"],
  ["performance", ["Výkon", "Performance"], "zap"],
  ["integrations", ["Integrace", "Integrations"], "share"],
  ["music", ["Hudba", "Music"], "music"],
  ["secrets", ["Tajemství", "Secrets"], "trophy"],
  ["about", ["O aplikaci", "About"], "info"],
];

/** Sticky section list; highlights the section in view and scrolls to it on click. */
function SettingsNav() {
  const [on, setOn] = useState("appearance");
  useEffect(() => {
    const els = SECTIONS.map(([id]) => document.getElementById(`set-${id}`)).filter(Boolean) as HTMLElement[];
    const io = new IntersectionObserver(
      (es) => {
        const v = es.filter((e) => e.isIntersecting).sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)[0];
        if (v) setOn(v.target.id.slice(4));
      },
      { rootMargin: "-10% 0px -70% 0px" },
    );
    els.forEach((e) => io.observe(e));
    return () => io.disconnect();
  }, []);
  return (
    <nav className="set-nav">
      {SECTIONS.map(([id, label, icon]) => (
        <button
          key={id}
          className={on === id ? "on" : ""}
          onClick={() => {
            setOn(id);
            document.getElementById(`set-${id}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
          }}
        >
          <Icon name={icon} size={16} /> {tr(...label)}
        </button>
      ))}
    </nav>
  );
}

export default function SettingsPage({ section }: { section?: string }) {
  // Sidebar "Appearance" (and other deep links) jump straight to their section.
  useEffect(() => {
    if (!section) return;
    const t = setTimeout(() => document.getElementById(`set-${section}`)?.scrollIntoView({ block: "start" }), 30);
    return () => clearTimeout(t);
  }, [section]);
  const { settings: s, saveSettings, system, checkUpdate } = useStore();
  if (!s) return null;
  const maxRam = Math.max(4096, Math.min(32768, (system?.totalRamMb ?? 16384) - 2048));

  return (
    <div className="page">
      <div className="page-head v2">
        <div>
          <h1>{tr("Nastavení", "Settings")}</h1>
          <div className="sub">{tr("Vzhled, hra, hudba a účty", "Look, game, music and accounts")}</div>
        </div>
      </div>
      <div className="settings-layout">
        <SettingsNav />
      <div className="col" style={{ maxWidth: 820, minWidth: 0 }}>
        <div className="section-title" id="set-appearance">{tr("Vzhled", "Appearance")}</div>
        <Appearance />

        <div className="section-title" id="set-customize">{tr("Rozložení", "Layout")}</div>
        <Customize />

        <div className="section-title" id="set-quick">{tr("Rychlé připojení", "Quick join")}</div>
        <QuickJoinSettings />

        <div className="section-title" id="set-general">{tr("Obecné", "General")}</div>
        <div className="card pad">
          <div className="setting" style={{ paddingTop: 0 }}>
            <div className="txt">
              <b>{tr("Jazyk", "Language")}</b>
              <span>{tr("Jazyk launcheru.", "The launcher's language.")}</span>
            </div>
            <div className="chips">
              <button className={`chip ${s.language === "cs" ? "on" : ""}`} onClick={() => saveSettings({ language: "cs" })}>Čeština</button>
              <button className={`chip ${s.language === "en" ? "on" : ""}`} onClick={() => saveSettings({ language: "en" })}>English</button>
            </div>
          </div>
          <div className="setting">
            <div className="txt">
              <b>{tr("Po spuštění hry", "When the game starts")}</b>
              <span>{tr("Schovaný launcher nebere výkon hře. Po zavření hry se sám ukáže.", "A hidden launcher doesn't take performance from the game. It comes back when the game closes.")}</span>
            </div>
            <Select className="select" style={{ width: 200 }} value={s.onLaunch} onChange={(e) => saveSettings({ onLaunch: e.target.value as typeof s.onLaunch })}>
              <option value="hide">{tr("Schovat launcher", "Hide the launcher")}</option>
              <option value="keep">{tr("Nechat otevřený", "Keep it open")}</option>
              <option value="close">{tr("Zavřít launcher", "Close the launcher")}</option>
            </Select>
          </div>
          <DataDir />
          <div className="setting">
            <div className="txt">
              <b>{tr("Otevřít okno s logy při spuštění hry", "Open the log window when a game starts")}</b>
              <span>{tr("Samostatné okno s živým logem hry (jako Lunar Client) – hodí se na druhý monitor nebo při ladění pádů.", "A separate window with the game's live log (like Lunar Client) – handy on a second monitor or when debugging crashes.")}</span>
            </div>
            <Switch checked={s.logWindowOnLaunch} onChange={(v) => saveSettings({ logWindowOnLaunch: v })} />
          </div>
          <div className="setting">
            <div className="txt">
              <b>{tr("Zálohovat světy po zavření hry", "Back up worlds when the game closes")}</b>
              <span>{tr("Po každém hraní se světy instance uloží do zálohy (zabírá místo).", "After every session the instance's worlds are backed up (uses disk space).")}</span>
            </div>
            <Switch checked={s.backupOnExit} onChange={(v) => saveSettings({ backupOnExit: v })} />
          </div>
        </div>

        <div className="section-title" id="set-performance">{tr("Výkon", "Performance")}</div>
        <HardwareCard />
        <div className="card pad">
          <div className="setting" style={{ paddingTop: 0 }}>
            <div className="txt">
              <b>Garbage collector</b>
              <span>
                {tr(
                  "ZGC skoro úplně odstraní záseky při úklidu paměti (doporučuje ho i Distant Horizons). Funguje na Minecraftu 1.20.5+.",
                  "ZGC almost completely removes stutters from memory cleanup (Distant Horizons recommends it too). Works on Minecraft 1.20.5+.",
                )}
              </span>
            </div>
            <Select className="select" style={{ width: 220 }} value={s.gc} onChange={(e) => saveSettings({ gc: e.target.value as typeof s.gc })}>
              <option value="auto">{tr("Automaticky (doporučeno)", "Automatic (recommended)")}</option>
              <option value="zgc">{tr("ZGC (plynulost)", "ZGC (smoothness)")}</option>
              <option value="g1">{tr("G1 (klasický)", "G1 (classic)")}</option>
            </Select>
          </div>
          <div className="setting">
            <div className="txt">
              <b>
                {tr("Výchozí paměť (RAM)", "Default memory (RAM)")}: {(s.ramMb / 1024).toFixed(1)} GB
              </b>
              <span>
                {tr(
                  `PC má ${((system?.totalRamMb ?? 0) / 1024).toFixed(0)} GB, doporučeno ${((system?.recommendedRamMb ?? 4096) / 1024).toFixed(1)} GB. Moc RAM = delší záseky při úklidu paměti.`,
                  `Your PC has ${((system?.totalRamMb ?? 0) / 1024).toFixed(0)} GB, recommended ${((system?.recommendedRamMb ?? 4096) / 1024).toFixed(1)} GB. Too much RAM = longer stutters during memory cleanup.`,
                )}
              </span>
              <input type="range" min={1024} max={maxRam} step={512} value={s.ramMb} onChange={(e) => saveSettings({ ramMb: Number(e.target.value) })} style={{ marginTop: 10 }} />
            </div>
          </div>
          <div className="setting">
            <div className="txt">
              <b>{tr("Vyšší priorita hry", "Higher game priority")}</b>
              <span>
                {tr(
                  "Systém dá Minecraftu přednost před ostatními programy (stabilnější FPS při nahrávání).",
                  "The system gives Minecraft priority over other programs (steadier FPS while recording).",
                )}
              </span>
            </div>
            <Switch checked={s.highPriority} onChange={(v) => saveSettings({ highPriority: v })} />
          </div>
          <div className="setting">
            <div className="txt">
              <b>{tr("Výchozí rozlišení okna", "Default window resolution")}</b>
            </div>
            <input className="input" style={{ width: 90 }} type="number" value={s.width} onChange={(e) => saveSettings({ width: Number(e.target.value) || 1280 })} />
            <span className="muted">×</span>
            <input className="input" style={{ width: 90 }} type="number" value={s.height} onChange={(e) => saveSettings({ height: Number(e.target.value) || 720 })} />
          </div>
          <div className="setting">
            <div className="txt">
              <b>{tr("Extra JVM argumenty (všechny instance)", "Extra JVM arguments (all instances)")}</b>
              <span>{tr("Launcher už používá optimalizované flagy. Měň jen když víš, co děláš.", "The launcher already uses optimized flags. Only change this if you know what you're doing.")}</span>
            </div>
            <input className="input mono" style={{ width: 300, fontSize: 12 }} value={s.jvmArgs} onChange={(e) => saveSettings({ jvmArgs: e.target.value })} />
          </div>
        </div>

        <div className="section-title" id="set-integrations">{tr("Integrace", "Integrations")}</div>
        <div className="card pad">
          <div className="setting" style={{ paddingTop: 0 }}>
            <div className="txt">
              <b>{tr("CurseForge API klíč", "CurseForge API key")}</b>
              <span>
                {tr("Zdarma na", "Free at")}{" "}
                <a href="#" onClick={(e) => (e.preventDefault(), openUrl("https://console.curseforge.com/"))}>
                  console.curseforge.com
                </a>{" "}
                →{" "}
                {tr(
                  "API keys. Justice Client má klíč zabudovaný — sem vlož jen vlastní, pokud ho chceš použít místo něj.",
                  "API keys. Justice Client has a key built in — only put your own here if you want to use it instead.",
                )}
              </span>
            </div>
            <Secret value={s.curseforgeKey} onSave={(v) => saveSettings({ curseforgeKey: v })} placeholder="$2a$10$..." />
          </div>
          <div className="setting">
            <div className="txt">
              <b>{tr("Discord status", "Discord status")}</b>
              <span>
                {tr(
                  "Ukáže na Discordu „Hraje Justice Client“, verzi a co zrovna hraješ.",
                  "Shows “Playing Justice Client” on Discord, with the version and what you're playing.",
                )}
              </span>
            </div>
            <Switch checked={s.discordRpc} onChange={(v) => saveSettings({ discordRpc: v })} />
          </div>
        </div>

        <div className="section-title" id="set-music">{tr("Hudba", "Music")}</div>
        <MusicSettings />

        <div className="section-title" id="set-secrets">{tr("Tajemství", "Secrets")}</div>
        <SecretsCard />

        <div className="section-title" id="set-about">{tr("O aplikaci", "About")}</div>
        <div className="card pad row">
          <div className="grow">
            <b>
              Justice <span style={{ color: "var(--red)" }}>Client</span> {system?.version}
            </b>
            <div className="small muted">
              {tr(
                "Launcher pro natáčení. Mody z Modrinthu a CurseForge, Java z Eclipse Temurin.",
                "A launcher for content creators. Mods from Modrinth and CurseForge, Java from Eclipse Temurin.",
              )}
            </div>
          </div>
          <button className="btn" onClick={() => checkUpdate(true)}>
            <Icon name="refresh" size={16} /> {tr("Zkontrolovat aktualizace", "Check for updates")}
          </button>
          <button className="btn" onClick={() => saveSettings({ onboarded: false })}>
            {tr("Průvodce nastavením", "Setup guide")}
          </button>
        </div>
      </div>
      </div>
    </div>
  );
}
