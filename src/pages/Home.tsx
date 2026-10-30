import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, social, type Instance, type LastPlace, type Presence, type ServerStatus, type SocialUser, type VersionArt } from "../api";
import { Icon, instanceIcon } from "../icons";
import { errText, fmtAgo, fmtDuration, LOADER_LABEL, useStore } from "../store";
import { customIconSrc, TaskProgress, parseIcon, InstGlyph } from "../components/common";
import CreateInstance from "../components/CreateInstance";
import { useJoinFriend } from "../components/FriendsPlaying";
import { PickInstance, QUICK_SERVERS, type QuickServer } from "../components/QuickJoin";
import { burstAt, speedrun } from "../components/Secrets";
import { head } from "../achievements";
import { uiPrefs, type HomeSection } from "../ui";
import { tr } from "../i18n";

// ---------- Last place per instance (cached for the session) ----------
const places = new Map<string, LastPlace | null>();
const placeSubs = new Set<() => void>();
function loadPlace(id: string) {
  if (places.has(id)) return;
  places.set(id, null);
  api
    .lastPlace(id)
    .then((p) => {
      places.set(id, p);
      placeSubs.forEach((f) => f());
    })
    .catch(() => {});
}
export function useLastPlace(id: string | undefined) {
  const [, force] = useState(0);
  useEffect(() => {
    if (!id) return;
    const f = () => force((x) => x + 1);
    placeSubs.add(f);
    loadPlace(id);
    return () => void placeSubs.delete(f);
  }, [id]);
  return id ? (places.get(id) ?? null) : null;
}
// A game that just closed may have new screenshots or a new place.
export function forgetPlace(id: string) {
  places.delete(id);
  loadPlace(id);
}
// ---------- Official art per Minecraft version (once per session) ----------
let artCache: Record<string, VersionArt> | null = null;
let artLoad: Promise<void> | null = null;
export function useVersionArt(version: string | undefined): VersionArt | null {
  const [, force] = useState(0);
  useEffect(() => {
    if (artCache) return;
    artLoad ??= api
      .versionArt()
      .then((m) => void (artCache = m))
      .catch(() => void (artCache = {}));
    artLoad.then(() => force((x) => x + 1));
  }, []);
  if (!version || !artCache) return null;
  // Snapshots and unknown versions: the newest release of the same line ("26.4-snapshot-5" -> none).
  return artCache[version] ?? null;
}

/** Light mode (Linux, ?lowfx=1): no effect that repaints on every mouse move or scroll step. */
const lowfx = () => document.documentElement.classList.contains("lowfx");

export const placeImg = (p: LastPlace | null) => (p?.image ? convertFileSrc(p.image) : null);

function PlaceBadge({ place, streamer }: { place: LastPlace | null; streamer?: boolean }) {
  if (!place) return <span className="place muted">{tr("Ještě nehráno", "Not played yet")}</span>;
  return (
    <span className="place">
      <Icon name={place.kind === "server" ? "server" : "globe"} size={11} />
      {place.kind === "server" && streamer ? "••••••" : place.name}
    </span>
  );
}

export function InstanceCard({ inst }: { inst: Instance }) {
  const { go, running, progress, play, settings } = useStore();
  const place = useLastPlace(inst.id);
  const art = useVersionArt(inst.mcVersion);
  // Small cards: the 540 px picture is plenty; the full-HD one is for the hero.
  const img = art?.image ?? placeImg(place);
  const isRunning = running.includes(inst.id);
  const icon = customIconSrc(parseIcon(inst.icon).base);
  const ref = useRef<HTMLDivElement>(null);
  // 3D tilt and a glare that follows the mouse.
  const tilt = (e: React.PointerEvent) => {
    const el = ref.current;
    if (!el || lowfx()) return;
    const r = el.getBoundingClientRect(), x = (e.clientX - r.left) / r.width, y = (e.clientY - r.top) / r.height;
    el.style.transform = `perspective(700px) rotateX(${(0.5 - y) * 8}deg) rotateY(${(x - 0.5) * 10}deg) translateY(-4px)`;
    el.style.setProperty("--mx", `${x * 100}%`);
    el.style.setProperty("--my", `${y * 100}%`);
  };
  return (
    <div
      ref={ref}
      className={`icard ${isRunning ? "running" : ""}`}
      onClick={() => go({ page: "instance", id: inst.id })}
      onPointerMove={tilt}
      onPointerLeave={() => ref.current && (ref.current.style.transform = "")}
    >
      <div className="glare" />
      <div className="art" style={img ? { backgroundImage: `url("${img}")` } : undefined}>
        {!img && (icon ? <img className="art-icon" src={icon} alt="" /> : <InstGlyph icon={inst.icon} template={inst.template} size={30} />)}
        <PlaceBadge place={place} streamer={settings?.streamerMode} />
      </div>
      <button
        className="qp"
        title={isRunning ? tr("Běží", "Running") : tr("Hrát", "Play")}
        disabled={!!progress[inst.id] || isRunning}
        onClick={(e) => {
          e.stopPropagation();
          play(inst.id);
        }}
      >
        <Icon name={isRunning ? "check" : "play"} size={16} />
      </button>
      <div className="info">
        <div className="name ellipsis">{inst.name}</div>
        {art?.name && <div className="drop ellipsis">{art.name}</div>}
        <div className="meta">
          {isRunning ? (
            <span className="badge red">
              <span className="dot red" /> {tr("Běží", "Running")}
            </span>
          ) : progress[inst.id] ? (
            <span className="grow">
              <TaskProgress task={inst.id} />
            </span>
          ) : (
            <>
              {LOADER_LABEL[inst.loader]} {inst.mcVersion} · {inst.lastPlayed ? fmtAgo(inst.lastPlayed) : tr("nehráno", "not played")}
              {inst.recordingMode && <span className="badge red">REC</span>}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// ---------- Friends (signed-in only) ----------
type PresenceEvent = { type: string; uuid?: string; online?: boolean; presence?: Presence | null };
function useFriends() {
  const [friends, setFriends] = useState<SocialUser[] | null>(null);
  useEffect(() => {
    let alive = true;
    const load = () =>
      social
        .status()
        .then((s) => (s.signedIn ? social.friends() : null))
        .then((r) => alive && setFriends(r ? r.friends : []))
        .catch(() => alive && setFriends([]));
    load();
    const un = listen<PresenceEvent>("social-event", (e) => {
      const ev = e.payload;
      if (ev.type === "presence") setFriends((l) => l && l.map((f) => (f.uuid === ev.uuid ? { ...f, online: !!ev.online, presence: ev.presence ?? null } : f)));
      else if (ev.type === "friend-accepted" || ev.type === "friend-removed" || ev.type === "connected") load();
    });
    return () => {
      alive = false;
      un.then((f) => f());
    };
  }, []);
  return friends;
}

function FriendsCard({ friends }: { friends: SocialUser[] | null }) {
  const { go, settings } = useStore();
  const join = useJoinFriend();
  const online = (friends ?? []).filter((f) => f.online).sort((a, b) => Number(!!b.presence?.server) - Number(!!a.presence?.server));
  return (
    <div className="card side-card">
      <div className="sec-h">
        <h2>{tr("Kamarádi", "Friends")}</h2>
        <button className="link" onClick={() => go({ page: "friends" })}>
          {online.length ? tr(`${online.length} online`, `${online.length} online`) : tr("Všichni", "All")} →
        </button>
      </div>
      {friends === null ? (
        <div className="muted small">…</div>
      ) : !online.length ? (
        <div className="muted small" style={{ padding: "6px 6px 2px" }}>
          {friends.length ? tr("Nikdo z kamarádů teď není online.", "None of your friends are online.") : tr("Přidej si kamarády a hrajte spolu.", "Add friends and play together.")}
        </div>
      ) : (
        online.slice(0, 5).map((f) => (
          <div key={f.uuid} className="friend" onClick={() => go({ page: "friends" })}>
            <span className="av on">
              <img src={head(f.uuid, 68)} alt="" />
            </span>
            <div className="grow ellipsis">
              <div className="n ellipsis">{f.name}</div>
              <div className="s ellipsis">
                {f.presence?.server ? (
                  <>
                    {tr("Hraje na", "Playing on")} <b>{settings?.streamerMode ? "••••••" : f.presence.server}</b>
                  </>
                ) : f.presence?.state === "playing" ? (
                  `${f.presence.instance ?? f.presence.mc} · ${tr("ve hře", "in game")}`
                ) : (
                  tr("V launcheru", "In the launcher")
                )}
              </div>
            </div>
            {f.presence?.server && (
              <button
                className="btn sm join"
                onClick={(e) => {
                  e.stopPropagation();
                  join(f);
                }}
              >
                {tr("Připojit", "Join")}
              </button>
            )}
          </div>
        ))
      )}
    </div>
  );
}

function ServerRow({ server }: { server: QuickServer }) {
  const { instances, play, settings } = useStore();
  const [st, setSt] = useState<ServerStatus | null | undefined>(undefined);
  const [picking, setPicking] = useState(false);
  useEffect(() => {
    let alive = true;
    (async () => {
      for (const a of [server.address, server.fallback].filter(Boolean) as string[]) {
        try {
          const s = await api.pingServer(a);
          if (alive) setSt(s);
          return;
        } catch {}
      }
      if (alive) setSt(null);
    })();
    return () => {
      alive = false;
    };
  }, [server.address]);
  const inst = instances.find((i) => i.id === settings?.quickServers?.[server.id]);
  return (
    <>
      <div className="srv" onClick={() => (inst ? play(inst.id, server.address) : setPicking(true))} title={tr("Připojit", "Join")}>
        {server.logo ? <img src={server.logo} alt="" /> : st?.icon ? <img src={st.icon} alt="" /> : <span className="srv-ic"><Icon name="server" size={16} /></span>}
        <div className="grow ellipsis">
          <div className="n">{server.name}</div>
          <div className="s ellipsis">{settings?.streamerMode ? "••••••" : server.address}</div>
        </div>
        <span className={`p ${st ? "" : "off"}`}>{st === undefined ? "…" : st ? `● ${st.pingMs} ms · ${st.online}/${st.max}` : "offline"}</span>
      </div>
      {picking && <PickInstance server={server} onClose={() => setPicking(false)} onPicked={(i) => (setPicking(false), play(i.id, server.address))} />}
    </>
  );
}

const NEWS: { tag: string; title: [string, string]; items: [string, string][] } = {
  tag: "0.7.6",
  title: ["Now Playing ve hře", "Now Playing in game"],
  items: [
    ["Písnička se ve hře zobrazí i když se obal nenačte", "The song shows in game even when the cover fails to load"],
    ["Rychlejší odpověď launcheru pro HUD", "Faster launcher answers for the HUD"],
  ],
};

/** Versions on the home page, newest first: each gets a ready "Justice" Fabric instance on demand. */
const HOME_VERSIONS = ["26.3", "26.2", "26.1.2", "1.21.11", "1.21.10"];
const JUSTICE_GROUP = "Justice";

/** The launcher-managed instance of a version (Fabric + performance mods + Justice Mod). */
function managedFor(instances: Instance[], version: string) {
  return instances.find((i) => i.group === JUSTICE_GROUP && i.mcVersion === version && i.loader === "fabric");
}

function VersionCard({ version, on, onPick }: { version: string; on: boolean; onPick: () => void }) {
  const art = useVersionArt(version);
  return (
    <button className={`vcard ${on ? "on" : ""}`} onClick={onPick}>
      <span className="vimg" style={art ? { backgroundImage: `url("${art.image}")` } : undefined} />
      <span className="vtext">
        <b>{version} Fabric</b>
        {art?.name && <small>{art.name}</small>}
      </span>
    </button>
  );
}

export default function Home() {
  const { instances, go, running, progress, play, settings, toast, reloadInstances, saveSettings } = useStore();
  const [creating, setCreating] = useState(false);
  const [preparing, setPreparing] = useState<string | null>(null);
  const friends = useFriends();
  const ui = (settings?.ui ?? {}) as { homeVersion?: string };
  const version = ui.homeVersion && HOME_VERSIONS.includes(ui.homeVersion) ? ui.homeVersion : HOME_VERSIONS[0];
  const pick = (v: string) => saveSettings({ ui: { ...(settings?.ui as object), homeVersion: v } });
  const art = useVersionArt(version);
  const inst = managedFor(instances, version);
  const place = useLastPlace(inst?.id);
  const img = art?.hd ?? art?.wide ?? art?.image ?? null;
  const isRunning = !!inst && running.includes(inst.id);
  const busy = preparing === version || (!!inst && !!progress[inst.id]);
  const playRef = useRef<HTMLButtonElement>(null);
  const artRef = useRef<HTMLDivElement>(null);
  // The art dims as the page scrolls down to the instances; a style write, no re-render.
  const onScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const art = artRef.current;
    if (!art) return;
    // Light mode dims once past a point instead of repainting the whole window on every scroll step.
    if (lowfx()) art.classList.toggle("dim", e.currentTarget.scrollTop > 160);
    else art.style.setProperty("--hs", Math.min(1, e.currentTarget.scrollTop / 500).toFixed(3));
  };
  const prevRunning = useRef(isRunning);
  useEffect(() => {
    // Burst when the game actually starts; refresh the place when it closes.
    if (inst && isRunning && !prevRunning.current && playRef.current) burstAt(playRef.current, 26);
    if (inst && !isRunning && prevRunning.current) forgetPlace(inst.id);
    prevRunning.current = isRunning;
  }, [isRunning]);
  const onlinePlaying = (friends ?? []).filter((f) => f.online && f.presence?.state === "playing");
  const own = instances.filter((i) => i.group !== JUSTICE_GROUP);
  const placeName = place && place.kind === "server" && settings?.streamerMode ? "••••••" : place?.name;

  /** Plays the version's Justice instance, creating it the first time. */
  const launch = async (server?: string) => {
    if (inst) return play(inst.id, server);
    setPreparing(version);
    try {
      toast(tr(`Připravuji Minecraft ${version} s Justice optimalizacemi…`, `Setting up Minecraft ${version} with Justice optimizations…`), "info");
      const made = await api.createInstance({ name: `Justice ${version}`, icon: "zap", group: JUSTICE_GROUP, mcVersion: version, loader: "fabric", loaderVersion: "" }, [], true);
      await reloadInstances();
      await play(made.id, server);
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setPreparing(null);
    }
  };

  const prefs = uiPrefs(settings?.ui);
  const showHome = (s: HomeSection) => !prefs.homeHidden.includes(s);
  return (
    <div className="page home3" onScroll={onScroll}>
      {/* The drop's art fills the whole window, under the sidebar and title bar too (like OneClient). */}
      {prefs.homeArt && createPortal(
        <div className="home-art" ref={artRef}>
          <div className="img" key={img ?? "none"} style={img ? { backgroundImage: `url("${img}")` } : undefined} />
        </div>,
        document.getElementById("root")!,
      )}
      <section className="vhero">
        {onlinePlaying.length > 0 && (
          <button className="friends-pill" onClick={() => go({ page: "friends" })}>
            <span className="stack">
              {onlinePlaying.slice(0, 3).map((f) => (
                <img key={f.uuid} src={head(f.uuid, 52)} alt="" />
              ))}
            </span>
            {onlinePlaying.length === 1 ? tr("1 kamarád hraje", "1 friend playing") : tr(`${onlinePlaying.length} kamarádi hrají`, `${onlinePlaying.length} friends playing`)}
          </button>
        )}
        <div className="body" key={version}>
          <h1>{version} Fabric</h1>
          <div className="drop-name">{art?.name || tr("Minecraft s Justice optimalizacemi", "Minecraft with Justice optimizations")}</div>
          {inst && progress[inst.id] && (
            <div style={{ maxWidth: 380 }}>
              <TaskProgress task={inst.id} />
            </div>
          )}
          <div className="actions">
            <div className={`play2 ${isRunning ? "running" : ""} ${busy ? "busy" : ""}`}>
              <button
                ref={playRef}
                className="go"
                disabled={busy}
                onClick={(e) => {
                  if (e.shiftKey) return speedrun(`${version} Fabric`);
                  if (isRunning && inst) return api.kill(inst.id);
                  launch();
                }}
              >
                {busy ? <span className="spinner" /> : <Icon name={isRunning ? "stop" : "play"} size={18} />}
                {isRunning ? tr("ZASTAVIT", "STOP") : busy ? (inst ? tr("SPOUŠTÍM", "STARTING") : tr("PŘIPRAVUJI", "SETTING UP")) : tr("HRÁT", "PLAY")}
              </button>
            </div>
            {place && !isRunning && (
              <button className="ghost2" disabled={busy} onClick={() => launch(place.kind === "server" ? place.name : `world:${place.name}`)}>
                <Icon name="right" size={17} /> <span className="ellipsis">{tr("Pokračovat", "Continue")}: {placeName}</span>
              </button>
            )}
            <button
              className="ghost2 icon"
              title={inst ? tr("Mody a nastavení instance", "Mods and instance settings") : tr("Instance vznikne při prvním spuštění", "The instance is created on first launch")}
              disabled={!inst}
              onClick={() => inst && go({ page: "instance", id: inst.id })}
            >
              <Icon name="settings" size={17} />
            </button>
          </div>
        </div>
        {showHome("versions") && <div className="vrow">
          {HOME_VERSIONS.map((v) => (
            <VersionCard key={v} version={v} on={v === version} onPick={() => pick(v)} />
          ))}
          <button className="vcard more" onClick={() => go({ page: "instances" })} title={tr("Všechny instance", "All instances")}>
            <Icon name="box" size={22} />
            <small>{tr("Další", "More")}</small>
          </button>
        </div>}
      </section>

      <div className={`home-grid ${showHome("instances") ? "" : "no-main"}`}>
        {showHome("instances") && <div>
          <div className="sec-h">
            <h2>{tr("Tvoje instance", "Your instances")}</h2>
            <button className="link" onClick={() => go({ page: "instances" })}>
              {tr("Zobrazit vše", "Show all")} →
            </button>
          </div>
          <div className="icards">
            {own.slice(0, 7).map((i) => (
              <InstanceCard key={i.id} inst={i} />
            ))}
            <div className="icard new" onClick={() => setCreating(true)}>
              <Icon name="plus" size={24} />
              <span>{tr("Nová instance", "New instance")}</span>
            </div>
          </div>
        </div>}
        <div className="side-col">
          {showHome("friends") && <FriendsCard friends={friends} />}
          {showHome("news") && <div className="card side-card news">
            <span className="tag">
              {tr("NOVINKA", "NEW")} · {NEWS.tag}
            </span>
            <h3>{tr(...NEWS.title)}</h3>
            <ul>
              {NEWS.items.map((it, i) => (
                <li key={i}>{tr(...it)}</li>
              ))}
            </ul>
          </div>}
          <div className="card side-card">
            <div className="sec-h">
              <h2>{tr("Servery", "Servers")}</h2>
              <button className="link" onClick={() => go({ page: "servers" })}>
                {tr("Všechny", "All")} →
              </button>
            </div>
            {QUICK_SERVERS.map((s) => (
              <ServerRow key={s.id} server={s} />
            ))}
          </div>
        </div>
      </div>
      {creating && <CreateInstance onClose={() => setCreating(false)} />}
    </div>
  );
}
