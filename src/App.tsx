import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useStore, type Page } from "./store";
import { Icon } from "./icons";
import { api, social } from "./api";
import { uiPrefs, type NavPage, type Shortcut } from "./ui";
import { t, tr } from "./i18n";
import { Avatar, CrashDialog, InstGlyph, Toasts, useAsync } from "./components/common";
import Home from "./pages/Home";
import Instances from "./pages/Instances";
import InstanceView from "./pages/InstanceView";
import Browse from "./pages/Browse";
import Recording from "./pages/Recording";
import ClipsPage from "./pages/Clips";
import Friends from "./pages/Friends";
import Servers from "./pages/Servers";
import SettingsPage from "./pages/Settings";
import Accounts from "./pages/Accounts";
import Onboarding from "./components/Onboarding";
import Splash from "./components/Splash";
import LaunchOverlay from "./components/LaunchOverlay";
import UpdatePill from "./components/UpdateDialog";
import DownloadsPill from "./components/DownloadsPill";
import { PartyPrompts } from "./components/Party";
import MusicPlayer, { MusicNavButton } from "./components/MusicPlayer";
import LogoButton, { useSecrets } from "./components/Secrets";
import CommandPalette, { openPalette } from "./components/CommandPalette";
import { sound } from "./sounds";
import NotificationBell from "./components/Notifications";

export const NAV: Record<NavPage, { icon: string; label: string }> = {
  home: { icon: "home", label: "Domů" },
  instances: { icon: "box", label: "Instance" },
  browse: { icon: "puzzle", label: "Mody" },
  recording: { icon: "video", label: "Natáčení" },
  servers: { icon: "server", label: "Servery" },
  friends: { icon: "users", label: "Přátelé" },
  clips: { icon: "clips", label: "Klipy" },
};

/** Sidebar sections, OneConfig-style. Pages keep the user's order and hiding from Customize. */
const SECTIONS: { label: [string, string] | null; pages: NavPage[]; music?: boolean }[] = [
  { label: null, pages: ["home"] },
  { label: ["Hra", "Play"], pages: ["instances", "browse", "servers"] },
  { label: ["Tvorba", "Create"], pages: ["recording", "clips"] },
  { label: ["Komunita", "Community"], pages: ["friends"], music: true },
];

/** Sidebar shortcut to an instance (opens it) or a saved server (joins it). */
function ShortcutButton({ sc }: { sc: Shortcut }) {
  const { instances, go, play } = useStore();
  const [servers] = useAsync(() => (sc.kind === "server" ? api.listServers() : Promise.resolve([])), [sc.kind]);
  if (sc.kind === "instance") {
    const inst = instances.find((i) => i.id === sc.id);
    if (!inst) return null;
    return (
      <button className="sc" onClick={() => go({ page: "instance", id: inst.id })} onDoubleClick={() => play(inst.id)}>
        <InstGlyph icon={inst.icon} template={inst.template} size={20} />
        <span className="label">{inst.name}</span>
        <span className="tip">{inst.name} · {tr("dvojklik = hrát", "double-click = play")}</span>
      </button>
    );
  }
  const srv = servers?.find((s) => s.id === sc.id);
  if (!srv) return null;
  return (
    <button className="sc" onClick={() => srv.instance && play(srv.instance, srv.address)} title={srv.name}>
      <Icon name="server" size={19} />
      <span className="label">{srv.name}</span>
      <span className="tip">{srv.name} · {tr("připojit", "join")}</span>
    </button>
  );
}

/** The search field in the title bar opens the command palette (also Ctrl+K). */
function TopSearch() {
  return (
    <button className="tb-search" onClick={() => openPalette()}>
      <Icon name="search" size={16} />
      <span>{tr("Hledat instance, mody, kamarády…", "Search instances, mods, friends…")}</span>
      <kbd>Ctrl K</kbd>
    </button>
  );
}

function TitleBar() {
  const { settings, back, forward, canBack, canForward } = useStore();
  const win = getCurrentWindow();
  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="tb-arrows">
        <button onClick={back} disabled={!canBack} aria-label={tr("Zpět", "Back")}>
          <Icon name="left" size={18} />
        </button>
        <button onClick={forward} disabled={!canForward} aria-label={tr("Vpřed", "Forward")}>
          <Icon name="right" size={18} />
        </button>
      </div>
      <TopSearch />
      <div className="drag" data-tauri-drag-region />
      {settings?.streamerMode && (
        <span className="badge red" title={tr("Streamer mód je zapnutý", "Streamer mode is on")}>
          <Icon name="eyeoff" size={12} /> STREAMER
        </span>
      )}
      <DownloadsPill />
      <UpdatePill />
      <div className="winbtns">
        <button onClick={() => win.minimize()} aria-label={tr("Minimalizovat", "Minimize")}>
          <Icon name="minus" size={16} />
        </button>
        <button onClick={() => win.toggleMaximize()} aria-label={tr("Maximalizovat", "Maximize")}>
          <Icon name="square" size={13} />
        </button>
        <button className="close" onClick={() => win.close()} aria-label={tr("Zavřít", "Close")}>
          <Icon name="x" size={16} />
        </button>
      </div>
    </div>
  );
}

/** Counts chat messages that arrive while the Friends page isn't open, with a toast for each. */
function useSocialNotifications(onFriends: boolean) {
  const { toast } = useStore();
  const [unread, setUnread] = useState(0);
  const [me, setMe] = useState<string | null>(null);
  const onRef = useRef(onFriends);
  onRef.current = onFriends;
  useEffect(() => {
    if (onFriends) setUnread(0);
  }, [onFriends]);
  useEffect(() => {
    const t = setInterval(() => social.status().then((s) => setMe(s.uuid)).catch(() => {}), 5000);
    social.status().then((s) => setMe(s.uuid)).catch(() => {});
    return () => clearInterval(t);
  }, []);
  useEffect(() => {
    const un = listen<any>("social-event", (e) => {
      const ev = e.payload;
      if (ev.type === "message" && ev.message.from !== me && !onRef.current) {
        setUnread((n) => n + 1);
        sound("notify");
        const m = ev.message;
        const what = m.kind === "world" ? tr(`tě zve do světa „${m.body}“`, `invites you to the world “${m.body}”`) : m.kind === "clip" ? tr("poslal klip", "sent a clip") : m.kind === "pack" ? tr("poslal modpack", "sent a modpack") : m.kind === "invite" ? tr("tě zve do hry", "invites you to play") : `: ${String(m.body).slice(0, 80)}`;
        social.friends().then((f) => {
          const name = f.friends.find((x) => x.uuid === m.from)?.name ?? tr("Přítel", "Friend");
          toast(`${name} ${what}`, "info");
        }).catch(() => {});
      } else if (ev.type === "friend-request" && !onRef.current) {
        setUnread((n) => n + 1);
        sound("notify");
        toast(tr(`${ev.from?.name} ti poslal žádost o přátelství`, `${ev.from?.name} sent you a friend request`), "info");
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, [me, toast]);
  return unread;
}

/** True below 900 px: tiling window managers squeeze the launcher, the icon rail fits better there. */
function useNarrow() {
  const [n, setN] = useState(() => innerWidth < 900);
  useEffect(() => {
    const r = () => setN(innerWidth < 900);
    addEventListener("resize", r);
    return () => removeEventListener("resize", r);
  }, []);
  return n;
}

export default function App() {
  const { nav, go, settings, activeAccount } = useStore();
  useSecrets();
  const prefs = uiPrefs(settings?.ui);
  const narrow = useNarrow();
  const wide = prefs.sidebar === "full" && !narrow;
  const [, force] = useState(0);
  useEffect(() => force((x) => x + 1), [settings?.language]);

  const current = nav.page === "instance" ? "instances" : nav.page === "accounts" ? "" : nav.page;
  const unread = useSocialNotifications(current === "friends");
  let page;
  switch (nav.page) {
    case "home": page = <Home />; break;
    case "instances": page = <Instances />; break;
    case "instance": page = <InstanceView key={nav.id} id={nav.id!} tab={nav.tab} />; break;
    case "browse": page = <Browse key={`${nav.id ?? "browse"}|${nav.q ?? ""}`} instanceId={nav.id} initialKind={nav.tab} initialQuery={nav.q} />; break;
    case "recording": page = <Recording />; break;
    case "clips": page = <ClipsPage />; break;
    case "friends": page = <Friends />; break;
    case "servers": page = <Servers />; break;
    case "settings": page = <SettingsPage section={nav.tab} />; break;
    case "accounts": page = <Accounts />; break;
  }

  return (
    <div className="shell">
      <nav className={`sidebar ${wide ? "wide" : ""}`}>
        <div className="brand">
          <LogoButton />
          {wide && (
            <span className="wordmark">
              <b>JUSTICE</b>
              <span>CLIENT</span>
            </span>
          )}
        </div>
        <button className="navbtn" onClick={() => openPalette()}>
          <Icon name="search" size={20} />
          <span className="label">{tr("Hledat", "Search")}</span>
          {wide && <kbd className="nav-kbd">Ctrl K</kbd>}
          <span className="tip">{tr("Hledat (Ctrl K)", "Search (Ctrl K)")}</span>
        </button>
        {SECTIONS.map((sec, si) => {
          const pages = prefs.navOrder.filter((p) => sec.pages.includes(p) && !prefs.navHidden.includes(p));
          if (!pages.length && !sec.music) return null;
          return (
            <div className="nav-sec" key={si}>
              {sec.label && (wide ? <div className="nav-head">{tr(...sec.label)}</div> : <div className="nav-sep" />)}
              {pages.map((p) => (
                <button key={p} className={`navbtn ${current === p ? "active" : ""}`} onClick={() => go({ page: p })}>
                  <Icon name={NAV[p].icon} size={20} />
                  <span className="label">{t(NAV[p].label)}</span>
                  {p === "friends" && unread > 0 && current !== "friends" && <span className="nav-badge">{unread > 9 ? "9+" : unread}</span>}
                  <span className="tip">{t(NAV[p].label)}</span>
                </button>
              ))}
              {sec.music && <MusicNavButton />}
            </div>
          );
        })}
        <div className="nav-sec">
          {wide ? <div className="nav-head">{tr("Přizpůsobení", "Personalization")}</div> : <div className="nav-sep" />}
          <button className={`navbtn ${current === "settings" && nav.tab === "appearance" ? "active" : ""}`} onClick={() => go({ page: "settings", tab: "appearance" })}>
            <Icon name="palette" size={20} />
            <span className="label">{tr("Vzhled", "Appearance")}</span>
            <span className="tip">{tr("Vzhled", "Appearance")}</span>
          </button>
          <button className={`navbtn ${current === "settings" && nav.tab !== "appearance" ? "active" : ""}`} onClick={() => go({ page: "settings" })}>
            <Icon name="settings" size={20} />
            <span className="label">{t("Nastavení")}</span>
            <span className="tip">{t("Nastavení")}</span>
          </button>
        </div>
        {prefs.shortcuts.length > 0 && (
          <div className="nav-sec">
            {wide ? <div className="nav-head">{tr("Zkratky", "Shortcuts")}</div> : <div className="nav-sep" />}
            {prefs.shortcuts.map((sc) => (
              <ShortcutButton key={`${sc.kind}:${sc.id}`} sc={sc} />
            ))}
          </div>
        )}
        <div className="spacer" />
        <div className="me-card">
          <button className="me-btn" onClick={() => go({ page: "accounts" })} title={activeAccount?.username ?? tr("Přidat účet", "Add account")}>
            {activeAccount ? <Avatar account={activeAccount} /> : <Icon name="user" size={18} />}
            {wide && (
              <span className="me-text">
                <b>{activeAccount?.username ?? tr("Přidat účet", "Add account")}</b>
                <small>{activeAccount ? (activeAccount.kind === "msa" ? "Microsoft" : tr("Offline účet", "Offline account")) : tr("Nepřihlášen", "Signed out")}</small>
              </span>
            )}
          </button>
          <NotificationBell />
        </div>
      </nav>
      <main className="main">
        <TitleBar />
        <div className="page-wrap" key={`${nav.page}|${nav.id ?? ""}`}>
          {page}
        </div>
        <CommandPalette />
        <MusicPlayer />
        <PartyPrompts />
        <Toasts />
        <CrashDialog />
        <LaunchOverlay />
        {settings && !settings.onboarded && <Onboarding />}
      </main>
      <Splash />
    </div>
  );
}
