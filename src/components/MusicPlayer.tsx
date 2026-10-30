import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { media, type MediaState } from "../api";
import { useStore } from "../store";
import { Icon } from "../icons";
import { tr } from "../i18n";
import { itemSub, itemTitle, snlCall, snlLive, useSonolith } from "./sonolithStore";
import SonolithPanel from "./SonolithPanel";
import { coverClicked, playNotes } from "./Secrets";

// ---------- Shared state (nav button + player) ----------
// The music comes from the system's media controls (Windows) or MPRIS (Linux): Sonolith, Spotify, a browser…
type Music = { state: MediaState | null; at: number; hidden: boolean; prefer: string | null };
const ls = (k: string) => {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
};
const lsSet = (k: string, v: string | null) => {
  try {
    v === null ? localStorage.removeItem(k) : localStorage.setItem(k, v);
  } catch {}
};
let music: Music = { state: null, at: 0, hidden: ls("jc.player.hidden") === "1", prefer: ls("jc.media.source") };
const subs = new Set<() => void>();
function set(p: Partial<Music>) {
  music = { ...music, ...p };
  subs.forEach((f) => f());
}
const useMusic = () => useSyncExternalStore((f) => (subs.add(f), () => subs.delete(f)), () => music);
function setHidden(h: boolean) {
  lsSet("jc.player.hidden", h ? "1" : "0");
  set({ hidden: h });
}
function setPrefer(id: string | null) {
  lsSet("jc.media.source", id);
  set({ prefer: id });
}

let polling = 0;
/** Same song, same state, position where we'd expect it: no need to re-render anything. */
function unchanged(a: MediaState | null, b: MediaState | null, at: number) {
  if (!a || !b) return a === b;
  if (a.id !== b.id || a.title !== b.title || a.artist !== b.artist || a.playing !== b.playing || a.thumbKey !== b.thumbKey || a.thumb !== b.thumb) return false;
  if (a.duration !== b.duration || a.canNext !== b.canNext || a.canPrev !== b.canPrev || a.canSeek !== b.canSeek) return false;
  if (a.sessions.length !== b.sessions.length || a.sessions.some((x, i) => x.id !== b.sessions[i].id || x.title !== b.sessions[i].title || x.playing !== b.sessions[i].playing)) return false;
  const expected = a.position + (a.playing ? (Date.now() - at) / 1000 : 0);
  return Math.abs(expected - b.position) < 2;
}

/** Polls the media controls (one poller for the whole app): every second while something plays,
 *  less often when nothing does or on light mode, not at all while the window is hidden. */
function usePoll() {
  useEffect(() => {
    if (polling++) return () => void polling--;
    let live = true;
    // The cover is only sent when the song changes; keep the last one here.
    let thumb: { key: string; url: string | null } = { key: "", url: null };
    const tick = () =>
      media
        // Only claim to have the cover when we really got one, or a late cover would never come.
        .state(music.prefer, thumb.url ? thumb.key : null)
        .then((s) => {
          if (!live) return;
          if (s) {
            if (s.thumbKey !== thumb.key || s.thumb) thumb = { key: s.thumbKey, url: s.thumb };
            s = { ...s, thumb: thumb.url };
          }
          if (!unchanged(music.state, s, music.at)) set({ state: s, at: Date.now() });
        })
        .catch(() => live && music.state && set({ state: null }));
    const lowfx = document.documentElement.classList.contains("lowfx");
    let t = 0;
    const loop = () => {
      if (!live) return;
      if (!document.hidden) tick();
      const playing = !!music.state?.playing;
      t = window.setTimeout(loop, playing ? (lowfx ? 2000 : 1000) : lowfx ? 5000 : 3000);
    };
    loop();
    // Coming back to the window: refresh right away.
    const onVis = () => !document.hidden && tick();
    document.addEventListener("visibilitychange", onVis);
    return () => {
      live = false;
      polling--;
      clearTimeout(t);
      document.removeEventListener("visibilitychange", onVis);
    };
  }, []);
}

const fmt = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

export function MusicNavButton() {
  usePoll();
  const m = useMusic();
  const playing = !!m.state?.playing;
  return (
    <button className={`navbtn music-nav ${playing ? "on" : ""}`} onClick={() => setHidden(!m.hidden)}>
      {playing ? (
        <span className="eq">
          <i />
          <i />
          <i />
          <i />
        </span>
      ) : (
        <Icon name="music" size={20} />
      )}
      <span className="label">{m.state?.title && playing ? m.state.title : tr("Hudba", "Music")}</span>
      <span className="tip">{m.state?.title ? `${m.state.title} · ${m.state.source}` : tr("Hudba", "Music")}</span>
    </button>
  );
}

/** Drag by the grip or cover; snaps to nearby edges and remembers where it was. The spot is kept
 *  relative to the nearest corner ("24 px from the right, 22 px from the bottom"), so a window that
 *  shrinks only pushes the player in for a while, and it goes back once the window grows again. */
function useDrag(ref: React.RefObject<HTMLDivElement | null>, mounted: boolean) {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const KEY = "jc.player.pos", M = 16, SNAP = 40;
    type Anchor = { ax: "l" | "r"; dx: number; ay: "t" | "b"; dy: number };
    const fromRect = (r: DOMRect): Anchor => {
      const right = r.left + r.width / 2 > innerWidth / 2, bottom = r.top + r.height / 2 > innerHeight / 2;
      return { ax: right ? "r" : "l", dx: right ? innerWidth - r.right : r.left, ay: bottom ? "b" : "t", dy: bottom ? innerHeight - r.bottom : r.top };
    };
    let anchor: Anchor | null = null;
    try {
      const p = JSON.parse(ls(KEY) ?? "null");
      if (p && "ax" in p) anchor = p;
      // Older versions saved the top-left corner as a fraction of the window.
      else if (p) anchor = fromRect(new DOMRect(p.x * innerWidth, p.y * innerHeight, el.offsetWidth, el.offsetHeight));
    } catch {}
    /** Puts the player at its anchor, pushed inside the window when the window is too small. */
    const apply = () => {
      if (!anchor) {
        Object.assign(el.style, { left: "", top: "", right: "", bottom: "" });
        return;
      }
      const dx = Math.max(M, Math.min(innerWidth - el.offsetWidth - M, anchor.dx));
      const dy = Math.max(M, Math.min(innerHeight - el.offsetHeight - M, anchor.dy));
      Object.assign(el.style, anchor.ax === "l" ? { left: `${dx}px`, right: "auto" } : { right: `${dx}px`, left: "auto" });
      Object.assign(el.style, anchor.ay === "t" ? { top: `${dy}px`, bottom: "auto" } : { bottom: `${dy}px`, top: "auto" });
    };
    apply();
    let start: { dx: number; dy: number; x0: number; y0: number; w: number; h: number } | null = null;
    let tx = 0, ty = 0, raf = 0;
    const down = (e: PointerEvent) => {
      if (e.button !== 0 || !(e.target as HTMLElement).closest(".mp-grip, .mp-cover")) return;
      const r = el.getBoundingClientRect();
      start = { dx: e.clientX - r.left, dy: e.clientY - r.top, x0: r.left, y0: r.top, w: r.width, h: r.height };
      el.classList.add("dragging");
      el.classList.remove("snap");
      e.preventDefault();
    };
    const move = (e: PointerEvent) => {
      if (!start) return;
      const x = Math.max(M, Math.min(innerWidth - start.w - M, e.clientX - start.dx));
      const y = Math.max(M, Math.min(innerHeight - start.h - M, e.clientY - start.dy));
      tx = Math.round(x - start.x0);
      ty = Math.round(y - start.y0);
      if (!raf) raf = requestAnimationFrame(() => ((raf = 0), (el.style.transform = `translate3d(${tx}px, ${ty}px, 0)`)));
    };
    const up = () => {
      if (!start) return;
      cancelAnimationFrame(raf);
      raf = 0;
      const moved = Math.abs(tx) + Math.abs(ty) > 3;
      const r = new DOMRect(start.x0 + tx, start.y0 + ty, start.w, start.h);
      el.style.transform = "";
      tx = ty = 0;
      start = null;
      el.classList.remove("dragging");
      if (!moved) return;
      const a = fromRect(r);
      // Close to an edge: stick to it.
      if (a.dx - M < SNAP) a.dx = M;
      if (a.dy - M < SNAP) a.dy = M;
      anchor = a;
      el.classList.add("snap");
      apply();
      lsSet(KEY, JSON.stringify(anchor));
    };
    // The window changed size, or the player did (song list opened): back to the anchor, kept inside.
    const resize = () => apply();
    const ro = new ResizeObserver(resize);
    ro.observe(el);
    const reset = (e: MouseEvent) => {
      if (!(e.target as HTMLElement).closest(".mp-grip")) return;
      lsSet(KEY, null);
      anchor = null;
      el.removeAttribute("style");
    };
    el.addEventListener("pointerdown", down);
    el.addEventListener("dblclick", reset);
    addEventListener("pointermove", move);
    addEventListener("pointerup", up);
    addEventListener("resize", resize);
    return () => {
      el.removeEventListener("pointerdown", down);
      el.removeEventListener("dblclick", reset);
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", up);
      removeEventListener("resize", resize);
      ro.disconnect();
    };
  }, [ref, mounted]);
}

export default function MusicPlayer() {
  const { toast } = useStore();
  usePoll();
  const m = useMusic();
  const ref = useRef<HTMLDivElement>(null);
  useDrag(ref, !m.hidden);
  const [panel, setPanel] = useState(false);
  const [popping, setPopping] = useState(false);
  const trackRef = useRef<HTMLDivElement>(null);
  const coverRef = useRef<HTMLDivElement>(null);
  const lastKey = useRef<string | null>(null);

  // Sonolith's own API, when connected, beats the system media controls: exact position, more info.
  const snl = useSonolith();
  const live = snlLive(snl) && !!snl.player?.item;
  const sp = snl.player;
  const fromSnl: MediaState | null =
    live && sp?.item
      ? {
          id: "sonolith",
          source: "Sonolith",
          title: itemTitle(sp.item),
          artist: itemSub(sp.item),
          album: sp.item.album?.title ?? "",
          thumb: sp.item.thumbnailUrl,
          thumbKey: sp.item.id,
          position: sp.positionMs / 1000,
          duration: (sp.durationMs ?? 0) / 1000,
          playing: sp.status === "playing" || sp.status === "buffering",
          canNext: sp.canGoNext,
          canPrev: sp.canGoPrevious,
          canSeek: true,
          sessions: [],
        }
      : null;
  const s = fromSnl ?? m.state;
  const at = fromSnl && sp ? sp.positionAt : m.at;
  const playing = !!s?.playing;
  const key = s ? `${s.id}|${s.title}|${s.artist}` : null;
  useEffect(() => {
    if (!key || key === lastKey.current) return;
    const first = lastKey.current === null;
    lastKey.current = key;
    if (first) return;
    const t = trackRef.current;
    if (t) {
      t.classList.remove("change");
      void t.offsetWidth;
      t.classList.add("change");
    }
    if (coverRef.current) playNotes(coverRef.current);
  }, [key]);
  // Keep the opened panel inside the window.
  useEffect(() => {
    const el = ref.current;
    if (!panel || !el?.style.top) return;
    const t = setTimeout(() => {
      const r = el.getBoundingClientRect(), over = r.bottom - (innerHeight - 16);
      if (over > 0) {
        el.classList.add("snap");
        el.style.top = `${Math.max(16, r.top - over)}px`;
      }
    }, 470);
    return () => clearTimeout(t);
  }, [panel]);

  if (m.hidden) return null;
  const cmd = (c: "playPause" | "next" | "previous" | "seek", v?: number) =>
    fromSnl
      ? snlCall(c === "playPause" ? "player.toggle" : c === "next" ? "player.next" : c === "previous" ? "player.previous" : "player.seek", c === "seek" ? { positionMs: Math.round((v ?? 0) * 1000) } : undefined).catch((e) => toast(String(e), "error"))
      : media
      .command(c, v, s?.id ?? m.prefer)
      .then(() => setTimeout(() => media.state(music.prefer).then((st) => st && set({ state: st, at: Date.now() })).catch(() => {}), 150))
      .catch(() => {});
  const openMusicApp = () => media.openMusicApp().catch((e) => toast(String(e), "error"));
  const togglePanel = () => {
    setPanel(!panel);
    setPopping(true);
    setTimeout(() => setPopping(false), 800);
  };

  return (
    <div ref={ref} className={`mplayer ${panel ? "open" : ""} ${popping ? "popping" : ""} ${playing ? "" : "paused"}`}>
      <div className="mp-grip" title={tr("Přetáhni přehrávač kamkoli · dvojklik = zpět do rohu", "Drag the player anywhere · double-click = back to the corner")} />
      {!s ? (
        <div className="mp-empty">
          <div className="mp-cover idle">
            <Icon name="music" size={20} />
          </div>
          <div className="grow">
            <b>{tr("Nic nehraje", "Nothing playing")}</b>
            <span>{tr("Pusť si hudbu v Sonolithu, Spotify nebo prohlížeči.", "Play music in Sonolith, Spotify or a browser.")}</span>
          </div>
          <button className="btn sm primary" onClick={openMusicApp}>
            Sonolith
          </button>
        </div>
      ) : (
        <>
          <div className="mp-row">
            <div className="mp-cover" ref={coverRef} onClick={coverClicked} style={s.thumb ? { backgroundImage: `url("${s.thumb}")` } : undefined} />
            <div className="mp-track" ref={trackRef}>
              <div className="t">{s.title || tr("Neznámá skladba", "Unknown track")}</div>
              <div className="a">{[s.artist, s.album].filter(Boolean).join(" · ") || s.source}</div>
            </div>
            <div className="mp-ctrl">
              <button onClick={() => cmd("previous")} disabled={!s.canPrev} aria-label={tr("Předchozí", "Previous")}>
                <Icon name="skipback" size={16} />
              </button>
              <button className="pp" onClick={() => cmd("playPause")} aria-label={playing ? tr("Pauza", "Pause") : tr("Přehrát", "Play")}>
                <Icon key={String(playing)} name={playing ? "pause" : "play"} size={16} />
              </button>
              <button onClick={() => cmd("next")} disabled={!s.canNext} aria-label={tr("Další", "Next")}>
                <Icon name="skipfwd" size={16} />
              </button>
            </div>
          </div>
          {s.duration > 0 && <MpBar s={s} at={at} onSeek={(v) => cmd("seek", v)} />}
          <div className="mp-tabs">
            <button className={panel ? "on" : ""} onClick={togglePanel}>
              <Icon name="volume" size={14} /> {s.source}
              {s.sessions.length > 1 && <span className="mp-count">{s.sessions.length}</span>}
            </button>
            <button className={fromSnl && panel ? "on" : ""} onClick={fromSnl ? togglePanel : openMusicApp}>
              <Icon name="list" size={14} /> {tr("Vybrat písničku", "Pick a song")}
            </button>
          </div>
          <div className="mp-lists">
            {fromSnl ? (
              <div className="inner">{panel && <SonolithPanel />}</div>
            ) : (
            <div className="inner">
              <div className="hdr">{tr("Co hraje", "What's playing")}</div>
              {s.sessions.map((x) => (
                <div
                  key={x.id}
                  className={`pl ${x.id === s.id ? "now" : ""}`}
                  onClick={() => {
                    setPrefer(x.id);
                    setPanel(false);
                  }}
                >
                  <div className="c src">{x.source.slice(0, 1)}</div>
                  <div className="grow ellipsis">
                    <div className="n ellipsis">{x.source}</div>
                    <div className="k ellipsis">
                      <Icon name={x.playing ? "play" : "pause"} size={10} />{" "}
                      {x.title || "—"}
                    </div>
                  </div>
                  <span className="go"><Icon name={x.id === s.id ? "music" : "play"} size={13} /></span>
                </div>
              ))}
              {m.prefer && (
                <div className="pl" onClick={() => setPrefer(null)}>
                  <div className="c src">A</div>
                  <div className="grow">
                    <div className="n">{tr("Automaticky", "Automatic")}</div>
                    <div className="k">{tr("Ukazuj to, co zrovna hraje", "Show whatever is playing")}</div>
                  </div>
                </div>
              )}
            </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}

/** Song progress. Ticks on its own so the rest of the player doesn't re-render every second. */
function MpBar({ s, at, onSeek }: { s: MediaState; at: number; onSeek: (v: number) => void }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!s.playing) return;
    // Light mode (Linux) repaints once a second; elsewhere the bar moves smoothly.
    const t = setInterval(() => !document.hidden && setNow(Date.now()), document.documentElement.classList.contains("lowfx") ? 1000 : 250);
    return () => clearInterval(t);
  }, [s.playing]);
  const pos = Math.min(s.duration || Infinity, s.position + (s.playing ? (Math.max(now, at) - at) / 1000 : 0));
  return (
    <div className="mp-bar">
      <span>{fmt(pos)}</span>
      <div
        className={`line ${s.canSeek ? "" : "fixed"}`}
        onClick={(e) => {
          if (!s.canSeek) return;
          const r = e.currentTarget.getBoundingClientRect();
          onSeek(Math.round(((e.clientX - r.left) / r.width) * s.duration));
        }}
      >
        <i style={{ width: `${(pos / s.duration) * 100}%` }} />
      </div>
      <span>{fmt(s.duration)}</span>
    </div>
  );
}
