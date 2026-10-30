import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { clips as clipsApi, type Clip, type ClipsDevices, type ClipsSettings, type ClipsStatus, type Monitor } from "../api";
import { Icon } from "../icons";
import { errText, fmtAgo, fmtSize, useStore } from "../store";
import { Empty, Modal, SkeletonList, Switch } from "../components/common";
import { locale, tr } from "../i18n";
import Select from "../components/Select";
import ClipsFeed, { PublishClip } from "../components/ClipsFeed";

const IS_LINUX = navigator.userAgent.includes("Linux");
/** Rough HEVC bitrate per quality preset (Mbit/s), for the RAM estimate. */
const MBPS: Record<ClipsSettings["quality"], number> = { small: 7, balanced: 12, high: 22 };

function fmtTime(s: number) {
  if (!isFinite(s) || s < 0) s = 0;
  const m = Math.floor(s / 60);
  return `${m}:${(s % 60).toFixed(1).padStart(4, "0")}`;
}

function fmtLen(s: number) {
  const r = Math.round(s);
  return r >= 60 ? `${Math.floor(r / 60)}:${String(r % 60).padStart(2, "0")}` : `0:${String(r).padStart(2, "0")}`;
}

// ---------------- status ----------------

function StatusPill({ status, hotkey }: { status: ClipsStatus | null; hotkey: string }) {
  if (!status) return null;
  const map = {
    recording: { cls: "rec", text: tr("Nahrává", "Recording"), sub: tr(`Poslední ${Math.round(status.bufferSecs)} s v paměti · ${hotkey} uloží klip`, `Last ${Math.round(status.bufferSecs)} s in memory · ${hotkey} saves a clip`) },
    starting: { cls: "start", text: tr("Spouští se…", "Starting…"), sub: tr("Připravuji nahrávání", "Getting the recorder ready") },
    paused: { cls: "idle", text: tr("Čeká na hru", "Waiting for a game"), sub: tr("Zapne se samo, když spustíš Minecraft", "Starts by itself when you launch Minecraft") },
    error: { cls: "err", text: tr("Chyba", "Error"), sub: status.error ?? "" },
  } as const;
  const m = map[status.state];
  return (
    <div className={`clips-status ${m.cls}`}>
      <span className="dot" />
      <div className="grow" style={{ minWidth: 0 }}>
        <b>{m.text}</b>
        <span className="ellipsis">{m.sub}</span>
      </div>
    </div>
  );
}

// ---------------- page ----------------

export default function ClipsPage() {
  const { toast } = useStore();
  const [settings, setSettings] = useState<ClipsSettings | null>(null);
  const [status, setStatus] = useState<ClipsStatus | null>(null);
  const [list, setList] = useState<Clip[] | null>(null);
  const [game, setGame] = useState<string>("");
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<Clip | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [fresh, setFresh] = useState<string | null>(null);
  const [thumbs, setThumbs] = useState<Record<string, string>>({});
  const [view, setView] = useState<"mine" | "feed">("mine");
  const [publishing, setPublishing] = useState<Clip | null>(null);

  const reload = useCallback(() => clipsApi.list().then(setList).catch((e) => toast(errText(e), "error")), [toast]);
  const refreshStatus = useCallback(() => clipsApi.status().then(setStatus).catch(() => {}), []);

  useEffect(() => {
    clipsApi.settings().then(setSettings).catch(() => {});
    reload();
    refreshStatus();
    const t = setInterval(refreshStatus, 1500);
    const un = [
      listen<ClipsStatus>("clips-status", () => refreshStatus()),
      listen<Clip>("clips-saved", (e) => {
        setList((l) => [e.payload, ...(l ?? []).filter((c) => c.path !== e.payload.path)]);
        setFresh(e.payload.path);
        toast(tr(`Klip uložen · ${fmtSize(e.payload.size)}`, `Clip saved · ${fmtSize(e.payload.size)}`), "success");
      }),
      listen<{ path: string; thumb: string }>("clips-thumb", (e) => setThumbs((t) => ({ ...t, [e.payload.path]: e.payload.thumb }))),
      listen<{ kind: string; text: string }>("clips-toast", (e) => toast(e.payload.text, e.payload.kind === "error" ? "error" : "info")),
    ];
    return () => {
      clearInterval(t);
      un.forEach((p) => p.then((f) => f()));
    };
  }, [reload, refreshStatus, toast]);

  const games = useMemo(() => {
    const m = new Map<string, number>();
    (list ?? []).forEach((c) => m.set(c.game, (m.get(c.game) ?? 0) + 1));
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  }, [list]);

  const shown = (list ?? []).filter(
    (c) => (!game || c.game === game) && (!query || c.name.toLowerCase().includes(query.toLowerCase()) || c.game.toLowerCase().includes(query.toLowerCase())),
  );
  const totalSize = (list ?? []).reduce((a, c) => a + c.size, 0);

  const saveSettings = async (patch: Partial<ClipsSettings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    try {
      setSettings(await clipsApi.saveSettings(next));
      refreshStatus();
      if (patch.clipsDir) setTimeout(reload, 300);
    } catch (e) {
      toast(errText(e), "error");
    }
  };

  const remove = async (c: Clip) => {
    try {
      await clipsApi.remove(c.path);
      setList((l) => (l ?? []).filter((x) => x.path !== c.path));
      toast(tr("Klip přesunut do koše", "Clip moved to the recycle bin"), "info");
      return true;
    } catch (e) {
      toast(errText(e), "error");
      return false;
    }
  };

  const recording = status?.state === "recording";
  return (
    <div className="page clips-page">
      <div className="page-head">
        <h1>Clips</h1>
        <span className="sub">{tr("Ulož posledních pár sekund jedním tlačítkem — jako Medal", "Save the last few seconds with one key — like Medal")}</span>
        <div className="grow" />
        <div className="seg" style={{ margin: 0 }}>
          <button className={view === "mine" ? "on" : ""} onClick={() => setView("mine")}>{tr("Moje klipy", "My clips")}</button>
          <button className={view === "feed" ? "on" : ""} onClick={() => setView("feed")}>{tr("Feed", "Feed")}</button>
        </div>
        <button className="btn" onClick={() => clipsApi.openDir()}>
          <Icon name="folder" size={16} /> {tr("Složka", "Folder")}
        </button>
        <button className="btn" onClick={() => setShowSettings(true)}>
          <Icon name="settings" size={16} /> {tr("Nastavení", "Settings")}
        </button>
      </div>

      {view === "feed" && <ClipsFeed />}
      {view === "mine" && (<>
      <div className="clips-hero card">
        <StatusPill status={status} hotkey={settings?.hotkey ?? "F8"} />
        {settings && !settings.enabled && (
          <span className="badge amber">{tr("Automatické nahrávání je vypnuté", "Automatic recording is off")}</span>
        )}
        <button className="btn" onClick={() => clipsApi.setRecording(!(recording || status?.state === "starting")).then(refreshStatus)}>
          <Icon name={recording || status?.state === "starting" ? "stop" : "record"} size={14} />
          {recording || status?.state === "starting" ? tr("Zastavit", "Stop") : tr("Nahrávat teď", "Record now")}
        </button>
        <button className="btn primary" disabled={!recording} onClick={() => clipsApi.save()}>
          <Icon name="download" size={16} /> {tr("Uložit klip", "Save clip")} <kbd>{settings?.hotkey ?? "F8"}</kbd>
        </button>
      </div>

      {status?.recorderMissing && (
        <div className="callout warn">
          <Icon name="alert" size={18} />
          <span>
            {tr(
              "Na Linuxu Clips nahrávají přes gpu-screen-recorder. Nainstaluj ho: „yay -S gpu-screen-recorder“ (AUR) nebo z Flathubu.",
              "On Linux, Clips record with gpu-screen-recorder. Install it: “yay -S gpu-screen-recorder” (AUR) or from Flathub.",
            )}
          </span>
        </div>
      )}
      {status && !status.ffmpeg && (
        <div className="callout warn">
          <Icon name="alert" size={18} />
          <span>
            {IS_LINUX
              ? tr(
                  "Chybí ffmpeg (náhledy a střih klipů). Nainstaluj ho: „sudo pacman -S ffmpeg“.",
                  "ffmpeg is missing (clip thumbnails and trimming). Install it: “sudo pacman -S ffmpeg”.",
                )
              : tr(
                  "Chybí ffmpeg. Nainstaluj ho příkazem „winget install Gyan.FFmpeg“ v terminálu, nebo vyber ffmpeg.exe v nastavení Clips.",
                  "ffmpeg is missing. Install it with “winget install Gyan.FFmpeg” in a terminal, or pick ffmpeg.exe in the Clips settings.",
                )}
          </span>
        </div>
      )}
      {status?.hotkeyError && (
        <div className="callout warn">
          <Icon name="alert" size={18} />
          <span>{status.hotkeyError}</span>
        </div>
      )}

      <div className="clips-toolbar">
        <div className="chips">
          <button className={`chip ${!game ? "on" : ""}`} onClick={() => setGame("")}>
            {tr("Vše", "All")} <span className="muted">{list?.length ?? 0}</span>
          </button>
          {games.map(([g, n]) => (
            <button key={g} className={`chip ${game === g ? "on" : ""}`} onClick={() => setGame(g)}>
              {g} <span className="muted">{n}</span>
            </button>
          ))}
        </div>
        <div className="grow" />
        <span className="small muted">{list ? `${list.length} ${tr("klipů", "clips")} · ${fmtSize(totalSize)}` : ""}</span>
        <div className="search sm">
          <Icon name="search" size={15} />
          <input className="input" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={tr("Hledat klip…", "Search clips…")} />
        </div>
      </div>

      {!list ? (
        <SkeletonList />
      ) : !shown.length ? (
        <Empty
          icon="clips"
          title={list.length ? tr("Nic nenalezeno", "Nothing found") : tr("Zatím žádné klipy", "No clips yet")}
          text={
            list.length
              ? undefined
              : tr(
                  `Spusť Minecraft z launcheru a stiskni ${settings?.hotkey ?? "F8"}, když se stane něco dobrého. Uloží se posledních ${settings?.bufferSecs ?? 60} s.`,
                  `Launch Minecraft from the launcher and press ${settings?.hotkey ?? "F8"} when something good happens. The last ${settings?.bufferSecs ?? 60} s get saved.`,
                )
          }
        />
      ) : (
        <div className="clip-grid">
          {shown.map((c, i) => (
            <ClipCard key={c.path} clip={c} thumb={thumbs[c.path] ?? c.thumb} fresh={fresh === c.path} delay={Math.min(i, 12) * 25} onOpen={() => setEditing(c)} onDelete={() => remove(c)} onPublish={() => setPublishing(c)} />
          ))}
        </div>
      )}
      </>)}

      {editing && (
        <ClipEditor
          clip={editing}
          onClose={() => setEditing(null)}
          onChanged={(old, n, asNew) => {
            setList((l) => (asNew ? [n, ...(l ?? [])] : (l ?? []).map((x) => (x.path === old.path ? n : x))));
            setEditing(n);
          }}
          onDelete={async (c) => {
            if (await remove(c)) setEditing(null);
          }}
          onPublish={() => setPublishing(editing)}
        />
      )}
      {/* After the editor so it stacks on top of it when opened from the editor. */}
      {publishing && <PublishClip clip={publishing} onClose={() => setPublishing(null)} onPublished={() => { setView("feed"); setEditing(null); }} />}
      {showSettings && settings && (
        <ClipsSettingsModal settings={settings} status={status} save={saveSettings} onClose={() => setShowSettings(false)} />
      )}
    </div>
  );
}

function ClipCard({ clip, thumb, fresh, delay, onOpen, onDelete, onPublish }: { clip: Clip; thumb: string | null; fresh: boolean; delay: number; onOpen: () => void; onDelete: () => void; onPublish: () => void }) {
  return (
    <div className={`clip-card ${fresh ? "fresh" : ""}`} style={{ animationDelay: `${delay}ms` }} onClick={onOpen}>
      <div className="cthumb">
        {thumb ? <img src={convertFileSrc(thumb)} loading="lazy" alt="" /> : <div className="thumb-empty"><Icon name="clips" size={28} /></div>}
        <span className="len">{fmtLen(clip.duration)}</span>
        <span className="play-ico">
          <Icon name="play" size={22} />
        </span>
      </div>
      <div className="meta">
        <div className="name ellipsis" title={clip.name}>{clip.name}</div>
        <div className="small muted row">
          <span className="ellipsis grow">{clip.game}</span>
          <span>{fmtSize(clip.size)}</span>
          <span>·</span>
          <span>{fmtAgo(clip.modified / 1000)}</span>
        </div>
      </div>
      <div className="actions" onClick={(e) => e.stopPropagation()}>
        <button className="btn ghost sm icon" title={tr("Publikovat do feedu", "Publish to the feed")} onClick={onPublish}>
          <Icon name="share" size={15} />
        </button>
        <button className="btn ghost sm icon" title={tr("Ukázat ve složce", "Show in folder")} onClick={() => clipsApi.reveal(clip.path)}>
          <Icon name="folder" size={15} />
        </button>
        <button className="btn ghost sm icon danger" title={tr("Smazat", "Delete")} onClick={onDelete}>
          <Icon name="trash" size={15} />
        </button>
      </div>
    </div>
  );
}

// ---------------- player & trim ----------------

function ClipEditor({ clip, onClose, onChanged, onDelete, onPublish }: { clip: Clip; onClose: () => void; onChanged: (old: Clip, n: Clip, asNew: boolean) => void; onDelete: (c: Clip) => void; onPublish: () => void }) {
  const { toast } = useStore();
  const video = useRef<HTMLVideoElement>(null);
  const tl = useRef<HTMLDivElement>(null);
  const [dur, setDur] = useState(clip.duration || 1);
  const [time, setTime] = useState(0);
  const [sel, setSel] = useState<[number, number]>([0, clip.duration || 1]);
  const [playing, setPlaying] = useState(false);
  const [muted, setMuted] = useState(false);
  const [audio, setAudio] = useState("mix");
  const [precise, setPrecise] = useState(false);
  const [replace, setReplace] = useState(false);
  const [speed, setSpeed] = useState(1);
  const [text, setText] = useState("");
  const [busy, setBusy] = useState<number | null>(null);
  const [strip, setStrip] = useState<string | null>(null);
  const [name, setName] = useState(clip.name);
  const [codecErr, setCodecErr] = useState(false);
  const selRef = useRef(sel);
  selRef.current = sel;

  useEffect(() => {
    setName(clip.name);
    setStrip(null);
    clipsApi.strip(clip.path).then((p) => setStrip(convertFileSrc(p))).catch(() => {});
    const t = setTimeout(() => video.current?.play().catch(() => {}), 250);
    return () => clearTimeout(t);
  }, [clip.path, clip.name]);

  useEffect(() => {
    const un = listen<{ path: string; pct: number }>("clips-trim-progress", (e) => setBusy((b) => (b === null ? b : e.payload.pct)));
    return () => {
      un.then((f) => f());
    };
  }, []);

  // Loop inside the selection while playing.
  useEffect(() => {
    if (!playing) return;
    let raf = 0;
    const tick = () => {
      const v = video.current;
      if (v) {
        if (v.currentTime >= selRef.current[1] - 0.02) v.currentTime = selRef.current[0];
        setTime(v.currentTime);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [playing]);

  const play = (p: boolean) => {
    const v = video.current;
    if (!v) return;
    if (p) {
      if (v.currentTime < sel[0] || v.currentTime >= sel[1] - 0.05) v.currentTime = sel[0];
      v.play().catch(() => {});
    } else v.pause();
  };
  const seek = (t: number) => {
    if (video.current) video.current.currentTime = t;
    setTime(t);
  };

  const onPointerDown = (e: React.PointerEvent) => {
    const el = tl.current;
    if (!el) return;
    const handle = (e.target as HTMLElement).closest<HTMLElement>(".handle")?.dataset.h;
    const rect = el.getBoundingClientRect();
    const wasPlaying = playing;
    if (!handle) video.current?.pause();
    el.setPointerCapture(e.pointerId);
    const at = (x: number) => Math.max(0, Math.min(1, (x - rect.left) / rect.width)) * dur;
    const move = (x: number) => {
      const t = at(x);
      const [a, b] = selRef.current;
      if (handle === "l") {
        const s = Math.min(t, b - 0.5);
        setSel([s, b]);
        seek(s);
      } else if (handle === "r") {
        const s = Math.max(t, a + 0.5);
        setSel([a, s]);
        seek(s);
      } else seek(t);
    };
    move(e.clientX);
    el.onpointermove = (ev) => move(ev.clientX);
    el.onpointerup = () => {
      el.onpointermove = null;
      el.onpointerup = null;
      if (handle) seek(handle === "l" ? selRef.current[0] : Math.max(selRef.current[0], selRef.current[1] - 1.5));
      if (wasPlaying && !handle) play(true);
    };
  };

  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).tagName === "INPUT") return;
      const v = video.current;
      if (!v) return;
      const key = e.key.toLowerCase();
      if (key === "escape") {
        if (busy === null) onClose();
      } else if (key === " ") {
        e.preventDefault();
        play(v.paused);
      } else if (key === "i") setSel(([, b]) => [Math.min(v.currentTime, b - 0.5), b]);
      else if (key === "o") setSel(([a]) => [a, Math.max(v.currentTime, a + 0.5)]);
      else if (key === "m") setMuted((m) => !m);
      else if (key === "arrowleft") seek(Math.max(0, v.currentTime - (e.shiftKey ? 1 / 60 : 2)));
      else if (key === "arrowright") seek(Math.min(dur, v.currentTime + (e.shiftKey ? 1 / 60 : 2)));
    };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  });

  const rename = async () => {
    const v = name.trim();
    if (!v || v === clip.name) return setName(clip.name);
    try {
      const t = video.current?.currentTime ?? 0;
      const n = await clipsApi.rename(clip.path, v);
      onChanged(clip, n, false);
      setTimeout(() => seek(t), 100);
      toast(tr("Přejmenováno", "Renamed"), "success");
    } catch (e) {
      toast(errText(e), "error");
      setName(clip.name);
    }
  };

  const exportTrim = async () => {
    const full = sel[0] <= 0.05 && sel[1] >= dur - 0.05;
    const effect = speed !== 1 || text.trim() !== "";
    if (full && audio === "mix" && !precise && !effect && clip.audioTracks < 3) {
      toast(tr("Označ úsek na časové ose, nebo přidej efekt (rychlost / text)", "Select a range on the timeline, or add an effect (speed / text)"), "error");
      return;
    }
    setBusy(0);
    video.current?.pause();
    if (replace && video.current) {
      video.current.removeAttribute("src");
      video.current.load();
    }
    try {
      const n = await clipsApi.trim({ path: clip.path, start: sel[0], end: sel[1], audio, precise, replace, speed, text: text.trim() });
      toast(replace ? tr("Klip oříznut", "Clip trimmed") : tr(`Střih uložen · ${fmtSize(n.size)}`, `Trim saved · ${fmtSize(n.size)}`), "success");
      onChanged(clip, n, !replace);
    } catch (e) {
      toast(`${tr("Střih selhal", "Trim failed")}: ${errText(e)}`, "error");
      if (replace && video.current) video.current.src = convertFileSrc(clip.path);
    } finally {
      setBusy(null);
    }
  };

  const pct = (t: number) => `${(t / dur) * 100}%`;
  return (
    <div className="overlay clip-editor-bg" onMouseDown={(e) => e.target === e.currentTarget && busy === null && onClose()}>
      <div className="clip-editor">
        <div className="ce-head">
          <input className="ce-name" value={name} onChange={(e) => setName(e.target.value)} onBlur={rename} onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} />
          <span className="small muted">
            {clip.game} · {fmtSize(clip.size)} · {new Date(clip.modified).toLocaleString(locale())}
          </span>
          <div className="grow" />
          <button className="btn ghost sm icon" title={tr("Otevřít v přehrávači", "Open in video player")} onClick={() => openPath(clip.path).catch(() => {})}>
            <Icon name="play" size={16} />
          </button>
          <button className="btn ghost sm icon" title={tr("Ukázat ve složce", "Show in folder")} onClick={() => clipsApi.reveal(clip.path)}>
            <Icon name="folder" size={16} />
          </button>
          <button
            className="btn ghost sm icon danger"
            title={tr("Smazat", "Delete")}
            onClick={() => {
              if (video.current) {
                video.current.removeAttribute("src");
                video.current.load();
              }
              onDelete(clip);
            }}
          >
            <Icon name="trash" size={16} />
          </button>
          <button className="btn ghost sm icon" onClick={() => busy === null && onClose()} aria-label={tr("Zavřít", "Close")}>
            <Icon name="x" size={16} />
          </button>
        </div>

        <div className={`ce-player ${playing ? "playing" : ""}`}>
          <video
            ref={video}
            src={convertFileSrc(clip.path)}
            muted={muted}
            onClick={() => play(!playing)}
            onPlay={() => setPlaying(true)}
            onPause={() => setPlaying(false)}
            onLoadedMetadata={(e) => {
              const d = isFinite(e.currentTarget.duration) && e.currentTarget.duration > 0 ? e.currentTarget.duration : clip.duration || 1;
              setDur(d);
              setSel([0, d]);
            }}
            onSeeked={(e) => setTime(e.currentTarget.currentTime)}
            onError={() => setCodecErr(true)}
          />
          {!playing && (
            <button className="big-play" onClick={() => play(true)}>
              <Icon name="play" size={30} />
            </button>
          )}
          {codecErr && (
            <div className="codec-warn">
              {IS_LINUX
                ? tr(
                    "Launcher tohle video neumí přehrát (chybí GStreamer „libav“). Otevři ho tlačítkem ▶ nahoře v přehrávači — klip samotný je v pořádku.",
                    "The launcher can't play this video (GStreamer “libav” is missing). Open it with the ▶ button at the top in your video player — the clip itself is fine.",
                  )
                : tr(
                    "Video je v HEVC (H.265). Pro přehrání v launcheru nainstaluj „Rozšíření videa HEVC“ z Microsoft Storu — klip samotný je v pořádku.",
                    "The video is HEVC (H.265). Install “HEVC Video Extensions” from the Microsoft Store to play it here — the clip itself is fine.",
                  )}
            </div>
          )}
        </div>

        <div className="ce-controls">
          <button className="btn sm icon" onClick={() => play(!playing)}>
            <Icon name={playing ? "pause" : "play"} size={15} />
          </button>
          <button className={`btn sm icon ${muted ? "on" : ""}`} onClick={() => setMuted(!muted)} title="M">
            <Icon name={muted ? "mute" : "volume"} size={15} />
          </button>
          <span className="mono small">
            {fmtTime(time)} / {fmtTime(dur)}
          </span>
          <div className="grow" />
          <button className="btn sm" onClick={() => setSel(([, b]) => [Math.min(time, b - 0.5), b])} title="I">
            [ {tr("Začátek", "In")}
          </button>
          <button className="btn sm" onClick={() => setSel(([a]) => [a, Math.max(time, a + 0.5)])} title="O">
            {tr("Konec", "Out")} ]
          </button>
          <span className="badge">
            <Icon name="scissors" size={12} /> {(sel[1] - sel[0]).toFixed(1)} s
          </span>
        </div>

        <div className="ce-timeline" ref={tl} onPointerDown={onPointerDown}>
          <div className="strip" style={strip ? { backgroundImage: `url("${strip}")` } : undefined} />
          <div className="shade" style={{ left: 0, width: pct(sel[0]) }} />
          <div className="shade" style={{ left: pct(sel[1]), right: 0 }} />
          <div className="sel" style={{ left: pct(sel[0]), width: pct(sel[1] - sel[0]) }}>
            <span className="handle l" data-h="l" />
            <span className="handle r" data-h="r" />
          </div>
          <div className="ph" style={{ left: pct(time) }} />
        </div>

        <div className="ce-effects">
          <span className="small muted">{tr("Rychlost", "Speed")}</span>
          <div className="seg">
            {[0.5, 1, 1.5, 2].map((s) => (
              <button key={s} className={speed === s ? "on" : ""} onClick={() => setSpeed(s)}>{s}×</button>
            ))}
          </div>
          <div className="search sm grow">
            <Icon name="edit" size={14} />
            <input className="input" value={text} maxLength={60} onChange={(e) => setText(e.target.value)} placeholder={tr("Text přes klip (volitelné)", "Caption over the clip (optional)")} />
          </div>
          {(speed !== 1 || text.trim() !== "") && (
            <span className="small muted" title={tr("Efekty překódují klip", "Effects re-encode the clip")}><Icon name="info" size={12} /> {tr("překóduje", "re-encodes")}</span>
          )}
        </div>

        <div className="ce-export">
          {clip.audioTracks >= 2 && (
            <div className="seg">
              <button className={audio === "mix" ? "on" : ""} onClick={() => setAudio("mix")}>
                {tr("Hra + mikrofon", "Game + mic")}
              </button>
              <button className={audio === "game" ? "on" : ""} onClick={() => setAudio("game")}>
                {tr("Jen hra", "Game only")}
              </button>
              {clip.audioTracks >= 3 && (
                <button className={audio === "all" ? "on" : ""} onClick={() => setAudio("all")}>
                  {tr("Všechny stopy", "All tracks")}
                </button>
              )}
            </div>
          )}
          <label className="check" title={tr("Přesný střih na snímek (překóduje, pomalejší)", "Frame-exact cut (re-encodes, slower)")}>
            <input type="checkbox" checked={precise} onChange={(e) => setPrecise(e.target.checked)} /> {tr("Přesný střih", "Precise cut")}
          </label>
          <label className="check">
            <input type="checkbox" checked={replace} onChange={(e) => setReplace(e.target.checked)} /> {tr("Nahradit originál", "Replace original")}
          </label>
          <div className="grow" />
          <button className="btn" disabled={busy !== null} onClick={onPublish} title={tr("Nahrát klip do komunitního feedu", "Upload the clip to the community feed")}>
            <Icon name="share" size={15} /> {tr("Publikovat do feedu", "Publish to feed")}
          </button>
          <button className="btn primary" disabled={busy !== null} onClick={exportTrim}>
            {busy !== null ? <span className="spinner" /> : <Icon name="scissors" size={15} />}
            {busy !== null ? `${Math.round(busy)} %` : tr("Uložit střih", "Save trim")}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------- settings ----------------

function keyName(e: KeyboardEvent): string | null {
  const c = e.code;
  if (["ControlLeft", "ControlRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight"].includes(c)) return null;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift"].filter(Boolean) as string[];
  let k = c;
  if (c.startsWith("Key")) k = c.slice(3);
  else if (c.startsWith("Digit")) k = c.slice(5);
  return [...mods, k].join("+");
}

function HotkeyButton({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [listening, setListening] = useState(false);
  useEffect(() => {
    if (!listening) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.code === "Escape") return setListening(false);
      const k = keyName(e);
      if (k) {
        setListening(false);
        onChange(k);
      }
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, [listening, onChange]);
  return (
    <button className={`btn hotkey-btn ${listening ? "listening" : ""}`} onClick={() => setListening(true)}>
      {listening ? tr("Stiskni klávesu…", "Press a key…") : <kbd>{value}</kbd>}
    </button>
  );
}

function Row({ title, text, children, sub, disabled }: { title: string; text?: React.ReactNode; children: React.ReactNode; sub?: boolean; disabled?: boolean }) {
  return (
    <div className={`setting ${sub ? "sub" : ""}`} style={disabled ? { opacity: 0.45, pointerEvents: "none" } : undefined}>
      <div className="txt">
        <b>{title}</b>
        {text && <span>{text}</span>}
      </div>
      {children}
    </div>
  );
}

function Seg<T extends string | number>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <div className="seg">
      {options.map(([v, label]) => (
        <button key={String(v)} className={v === value ? "on" : ""} onClick={() => onChange(v)}>
          {label}
        </button>
      ))}
    </div>
  );
}

function ClipsSettingsModal({ settings: s, status, save, onClose }: { settings: ClipsSettings; status: ClipsStatus | null; save: (p: Partial<ClipsSettings>) => void; onClose: () => void }) {
  const [devices, setDevices] = useState<ClipsDevices | null>(null);
  const [monitors, setMonitors] = useState<Monitor[] | null>(null);
  const [log, setLog] = useState<string[] | null>(null);
  // Sliders save on release; this keeps the thumb responsive meanwhile.
  const [draft, setDraft] = useState<Partial<ClipsSettings>>({});
  const v = { ...s, ...draft };
  useEffect(() => {
    clipsApi.devices().then(setDevices).catch(() => {});
    clipsApi.monitors().then(setMonitors).catch(() => {});
  }, []);
  const slider = (key: "bufferSecs" | "postSecs" | "micVolume" | "audioOffsetMs", min: number, max: number, step: number, fmt: (n: number) => string) => (
    <div className="row" style={{ gap: 10, width: 260 }}>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={v[key]}
        onChange={(e) => setDraft((d) => ({ ...d, [key]: Number(e.target.value) }))}
        onPointerUp={() => draft[key] !== undefined && save({ [key]: draft[key] })}
        onKeyUp={() => draft[key] !== undefined && save({ [key]: draft[key] })}
      />
      <span className="mono small" style={{ minWidth: 64, textAlign: "right" }}>
        {fmt(v[key])}
      </span>
    </div>
  );
  const devOpts = (list: string[] | undefined, cur: string, def: string | null | undefined) => {
    const all = [...(list ?? [])];
    if (cur && !all.includes(cur)) all.push(cur);
    return [["", `${tr("Výchozí zařízení Windows", "Windows default device")}${def ? ` (${def})` : ""}`], ...all.map((d) => [d, d])] as [string, string][];
  };
  const ram = Math.round(((v.bufferSecs + v.postSecs + 3) * MBPS[v.quality]) / 8);

  return (
    <Modal title={tr("Nastavení Clips", "Clips settings")} onClose={onClose} wide>
      <div className="clips-settings">
        <div className="section-title">{tr("Nahrávání", "Recording")}</div>
        <Row title={tr("Nahrávat při hraní Minecraftu", "Record while playing Minecraft")} text={tr("Replay buffer se zapne se hrou z launcheru a vypne po jejím zavření.", "The replay buffer starts with a game from the launcher and stops when it closes.")}>
          <Switch checked={s.enabled} onChange={(x) => save({ enabled: x })} />
        </Row>
        <Row
          title={tr("Klávesa pro uložení klipu", "Save clip hotkey")}
          text={
            status?.hotkeyError ??
            (IS_LINUX
              ? tr("Globálně funguje na X11. Na Waylandu si ji nastav jako zkratku systému: justice-client --save-clip.", "Works globally on X11. On Wayland bind it as a system shortcut: justice-client --save-clip.")
              : tr("Funguje globálně, i ve hře.", "Works globally, even in-game."))
          }
        >
          <HotkeyButton value={s.hotkey} onChange={(k) => save({ hotkey: k })} />
        </Row>
        <Row title={tr("Délka klipu", "Clip length")} text={tr(`Kolik sekund zpátky se uloží · ≈ ${ram} MB RAM`, `How many seconds back get saved · ≈ ${ram} MB RAM`)}>
          {slider("bufferSecs", 15, 300, 5, (n) => (n >= 60 ? `${Math.floor(n / 60)}:${String(n % 60).padStart(2, "0")} min` : `${n} s`))}
        </Row>
        <Row
          title={tr("Kvalita", "Quality")}
          text={
            IS_LINUX
              ? tr("Kódování na grafice přes gpu-screen-recorder (NVIDIA, AMD i Intel).", "GPU encoding through gpu-screen-recorder (NVIDIA, AMD and Intel).")
              : tr("HEVC na grafice NVIDIA — o třetinu menší soubory než H.264.", "HEVC on the NVIDIA GPU — a third smaller than H.264.")
          }
        >
          <Seg value={s.quality} options={[["small", tr("Menší", "Smaller")], ["balanced", tr("Vyvážená", "Balanced")], ["high", tr("Nejvyšší", "Highest")]]} onChange={(q) => save({ quality: q })} />
        </Row>
        <Row title={tr("Snímky za sekundu", "Frame rate")}>
          <Seg value={s.fps} options={[[30, "30"], [60, "60"], [120, "120"]]} onChange={(f) => save({ fps: f })} />
        </Row>
        <Row title="Monitor">
          <Select className="input" value={s.monitor} onChange={(e) => save({ monitor: Number(e.target.value) })}>
            {(monitors ?? [{ index: s.monitor, name: "", width: 0, height: 0, primary: false }]).map((m) => (
              <option key={m.index} value={m.index}>
                Monitor {m.index + 1}
                {m.width ? ` · ${m.width}×${m.height}` : ""}
                {m.primary ? tr(" (hlavní)", " (primary)") : ""}
              </option>
            ))}
          </Select>
        </Row>
        <Row title={tr("Nahrávat kurzor myši", "Record mouse cursor")}>
          <Switch checked={s.drawMouse} onChange={(x) => save({ drawMouse: x })} />
        </Row>
        <Row title={tr("Dotočit po stisku", "Keep recording after the press")} text={tr("Kolik sekund se nahraje ještě po zmáčknutí klávesy.", "Seconds still recorded after you press the key.")}>
          {slider("postSecs", 0, 5, 0.5, (n) => `${n.toFixed(1)} s`)}
        </Row>

        <div className="section-title">{tr("Zvuk", "Audio")}</div>
        <Row title={tr("Zvuk počítače", "PC audio")} text={tr("Hra, Discord a vše, co slyšíš.", "The game, Discord and everything you hear.")}>
          <Switch checked={s.gameAudio} onChange={(x) => save({ gameAudio: x })} />
        </Row>
        <Row title={tr("Výstupní zařízení", "Output device")} sub disabled={!s.gameAudio}>
          <Select className="input" value={s.gameDevice} onChange={(e) => save({ gameDevice: e.target.value })}>
            {devOpts(devices?.outputs, s.gameDevice, devices?.defaultOutput).map(([k, l]) => (
              <option key={k} value={k}>{l}</option>
            ))}
          </Select>
        </Row>
        <Row title={tr("Mikrofon", "Microphone")} text={tr("Na vlastní stopě, ve střihu ho můžeš vynechat.", "On its own track, you can leave it out when trimming.")}>
          <Switch checked={s.mic} onChange={(x) => save({ mic: x })} />
        </Row>
        <Row title={tr("Mikrofon – zařízení", "Microphone device")} sub disabled={!s.mic}>
          <Select className="input" value={s.micDevice} onChange={(e) => save({ micDevice: e.target.value })}>
            {devOpts(devices?.inputs, s.micDevice, devices?.defaultInput).map(([k, l]) => (
              <option key={k} value={k}>{l}</option>
            ))}
          </Select>
        </Row>
        <Row title={tr("Hlasitost mikrofonu v mixu", "Microphone volume in the mix")} sub disabled={!s.mic}>
          {slider("micVolume", 0, 2, 0.05, (n) => `${Math.round(n * 100)} %`)}
        </Row>
        <Row title={tr("Posun zvuku", "Audio offset")} text={tr("Jen když zvuk nesedí s obrazem. Kladné = zvuk později.", "Only if audio is out of sync. Positive = audio later.")}>
          {slider("audioOffsetMs", -300, 300, 10, (n) => `${n > 0 ? "+" : ""}${n} ms`)}
        </Row>

        <div className="section-title">{tr("Klipy a notifikace", "Clips & notifications")}</div>
        <Row title={tr("Složka s klipy", "Clips folder")} text={s.clipsDir}>
          <button
            className="btn sm"
            onClick={async () => {
              const d = await openDialog({ directory: true, defaultPath: s.clipsDir, title: tr("Složka pro klipy", "Clips folder") });
              if (typeof d === "string") save({ clipsDir: d });
            }}
          >
            {tr("Změnit", "Change")}
          </button>
        </Row>
        <Row title={tr("Třídit do složek podle instance", "Sort into folders by instance")} text={tr("Např. Klipy\\PvP 1.21.11\\…", "E.g. Clips\\PvP 1.21.11\\…")}>
          <Switch checked={s.gameFolders} onChange={(x) => save({ gameFolders: x })} />
        </Row>
        <Row title={tr("Notifikace ve hře", "In-game notification")} text={tr("Malé okno v rohu obrazovky (borderless a většina fullscreen her).", "A small popup in the corner (borderless and most fullscreen games).")}>
          <div className="row" style={{ gap: 8 }}>
            <button className="btn sm" onClick={() => clipsApi.testNotification()}>
              {tr("Vyzkoušet", "Try it")}
            </button>
            <Switch checked={s.overlay} onChange={(x) => save({ overlay: x })} />
          </div>
        </Row>
        <Row title={tr("Zvuk po uložení", "Sound when saved")}>
          <Switch checked={s.sound} onChange={(x) => save({ sound: x })} />
        </Row>
        <Row title="ffmpeg" text={s.ffmpegPath || status?.ffmpeg || tr("nenalezeno", "not found")}>
          <button
            className="btn sm"
            onClick={async () => {
              const f = await openDialog({ filters: IS_LINUX ? undefined : [{ name: "ffmpeg", extensions: ["exe"] }], title: tr("Vyber ffmpeg", "Choose ffmpeg") });
              if (typeof f === "string") save({ ffmpegPath: f });
            }}
          >
            {tr("Vybrat", "Browse")}
          </button>
        </Row>
        <Row title={tr("Diagnostika", "Diagnostics")} text={tr("Log nahrávání (ffmpeg, audio zařízení).", "Recording log (ffmpeg, audio devices).")}>
          <button className="btn sm" onClick={async () => setLog(log ? null : await clipsApi.log())}>
            {log ? tr("Skrýt log", "Hide log") : tr("Zobrazit log", "Show log")}
          </button>
        </Row>
        {log && <pre className="logbox">{log.join("\n") || tr("(prázdný)", "(empty)")}</pre>}
      </div>
    </Modal>
  );
}
