import { useEffect, useRef, useState } from "react";
import { Icon } from "../icons";
import { tr } from "../i18n";
import { useStore } from "../store";
import { itemSub, itemTitle, snlCall, useSonolith, type SnlItem } from "./sonolithStore";

type Tab = "queue" | "search" | "playlists" | "lyrics";

interface QueueEntry {
  index: number;
  entryId: string;
  item: SnlItem;
}
interface LyricLine {
  startMs: number | null;
  text: string;
}

const TABS: { key: Tab; icon: string; label: [string, string]; scope: string }[] = [
  { key: "queue", icon: "list", label: ["Fronta", "Queue"], scope: "queue:read" },
  { key: "search", icon: "search", label: ["Hledat", "Search"], scope: "search" },
  { key: "playlists", icon: "layers", label: ["Playlisty", "Playlists"], scope: "library:read" },
  { key: "lyrics", icon: "music", label: ["Text", "Lyrics"], scope: "lyrics:read" },
];

function Row({ item, active, onPlay, onQueue }: { item: SnlItem; active?: boolean; onPlay: () => void; onQueue?: () => void }) {
  return (
    <div className={`snl-row ${active ? "now" : ""}`} onClick={onPlay}>
      <div className="snl-thumb" style={item.thumbnailUrl ? { backgroundImage: `url("${item.thumbnailUrl}")` } : undefined}>
        {!item.thumbnailUrl && <Icon name="music" size={14} />}
      </div>
      <div className="grow ellipsis">
        <div className="n ellipsis">{itemTitle(item)}</div>
        <div className="k ellipsis">{itemSub(item) || (item.trackCount ? tr(`${item.trackCount} skladeb`, `${item.trackCount} tracks`) : "")}</div>
      </div>
      {onQueue && (
        <button
          className="snl-add"
          title={tr("Přidat do fronty", "Add to queue")}
          onClick={(e) => {
            e.stopPropagation();
            onQueue();
          }}
        >
          <Icon name="plus" size={13} />
        </button>
      )}
    </div>
  );
}

/** Queue, search, playlists and lyrics from Sonolith, inside the music player. */
export default function SonolithPanel() {
  const snl = useSonolith();
  const { toast } = useStore();
  const scopes = snl.status?.grantedScopes ?? [];
  const tabs = TABS.filter((t) => scopes.includes(t.scope));
  const [tab, setTab] = useState<Tab>(tabs[0]?.key ?? "queue");
  const [queue, setQueue] = useState<QueueEntry[] | null>(null);
  const [q, setQ] = useState("");
  const [results, setResults] = useState<SnlItem[] | null>(null);
  const [playlists, setPlaylists] = useState<SnlItem[] | null>(null);
  const [lyrics, setLyrics] = useState<{ status: string; lines: LyricLine[] } | null>(null);
  const lyricsBox = useRef<HTMLDivElement>(null);
  const trackId = snl.player?.item?.id ?? null;
  const fail = (e: unknown) => toast(String(e), "error");

  useEffect(() => {
    if (tab !== "queue") return;
    snlCall<{ items: QueueEntry[] }>("queue.get", { fromIndex: Math.max(0, snl.player?.queueIndex ?? 0), limit: 40 })
      .then((r) => setQueue(r.items))
      .catch(() => setQueue([]));
  }, [tab, snl.queueRev, snl.player?.queueIndex]);

  useEffect(() => {
    if (tab !== "playlists" || playlists) return;
    snlCall<{ items: SnlItem[] }>("library.playlists", { limit: 50 })
      .then((r) => setPlaylists(r.items))
      .catch(() => setPlaylists([]));
  }, [tab]);

  useEffect(() => {
    if (tab !== "lyrics" || !trackId) return;
    setLyrics(null);
    snlCall<{ status: string; lyrics: { lines: LyricLine[] } | null }>("lyrics.get")
      .then((r) => setLyrics({ status: r.status, lines: r.lyrics?.lines ?? [] }))
      .catch(() => setLyrics({ status: "error", lines: [] }));
  }, [tab, trackId]);

  useEffect(() => {
    lyricsBox.current?.querySelector(".ly.now")?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [snl.lyricLine]);

  // Search as you type, once the typing pauses (Sonolith limits network calls to 2/s).
  useEffect(() => {
    if (tab !== "search") return;
    const s = q.trim();
    if (s.length < 2) return setResults(null);
    const t = setTimeout(() => {
      snlCall<{ items: SnlItem[] }>("search.query", { query: s, type: "songs", limit: 20 })
        .then((r) => setResults(r.items))
        .catch(fail);
    }, 450);
    return () => clearTimeout(t);
  }, [q, tab]);

  const play = (item: SnlItem) => snlCall("player.playItem", { item: { type: item.type, id: item.id } }).catch(fail);
  const addToQueue = (item: SnlItem) =>
    snlCall("queue.add", { items: [{ type: item.type, id: item.id }] })
      .then(() => toast(tr(`Přidáno do fronty: ${itemTitle(item)}`, `Added to queue: ${itemTitle(item)}`), "success"))
      .catch(fail);
  const canQueue = scopes.includes("queue:write");

  if (!tabs.length) {
    return <div className="snl-empty">{tr("Sonolith povolil jen přehrávání. Frontu, hledání a texty povolíš po odpojení a novém připojení.", "Sonolith only allowed playback. Disconnect and connect again to allow queue, search and lyrics.")}</div>;
  }

  return (
    <div className="snl">
      <div className="snl-tabs">
        {tabs.map((t) => (
          <button key={t.key} className={tab === t.key ? "on" : ""} onClick={() => setTab(t.key)}>
            <Icon name={t.icon} size={13} /> {tr(...t.label)}
          </button>
        ))}
      </div>
      <div className="snl-body">
        {tab === "queue" &&
          (queue === null ? (
            <div className="snl-empty"><span className="spin" /></div>
          ) : queue.length === 0 ? (
            <div className="snl-empty">{tr("Fronta je prázdná", "The queue is empty")}</div>
          ) : (
            queue.map((e) => <Row key={e.entryId} item={e.item} active={e.index === snl.player?.queueIndex} onPlay={() => {}} />)
          ))}
        {tab === "search" && (
          <>
            <input className="input snl-search" autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder={tr("Hledej písničku nebo interpreta…", "Search a song or artist…")} />
            {results?.map((it) => <Row key={it.id} item={it} onPlay={() => play(it)} onQueue={canQueue ? () => addToQueue(it) : undefined} />)}
            {results?.length === 0 && <div className="snl-empty">{tr("Nic nenalezeno", "Nothing found")}</div>}
          </>
        )}
        {tab === "playlists" &&
          (playlists === null ? (
            <div className="snl-empty"><span className="spin" /></div>
          ) : (
            playlists.map((p) => <Row key={p.id} item={p} onPlay={() => play(p)} />)
          ))}
        {tab === "lyrics" && (
          <div className="snl-lyrics" ref={lyricsBox}>
            {!lyrics ? (
              <div className="snl-empty"><span className="spin" /></div>
            ) : lyrics.status === "instrumental" ? (
              <div className="snl-empty">{tr("Instrumentálka, bez textu", "Instrumental, no lyrics")}</div>
            ) : lyrics.lines.length === 0 ? (
              <div className="snl-empty">{tr("Text pro tuhle písničku není", "No lyrics for this song")}</div>
            ) : (
              lyrics.lines.map((l, i) => (
                <p key={i} className={`ly ${snl.lyricTrack === trackId && snl.lyricLine === i ? "now" : ""} ${snl.lyricTrack === trackId && snl.lyricLine !== null && i < snl.lyricLine ? "past" : ""}`}>
                  {l.text || "· · ·"}
                </p>
              ))
            )}
          </div>
        )}
      </div>
    </div>
  );
}
