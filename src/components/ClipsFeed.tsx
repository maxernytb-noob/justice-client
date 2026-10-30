import { useEffect, useRef, useState } from "react";
import { clips as clipsApi, type Clip, type FeedClip } from "../api";
import { Icon } from "../icons";
import { tr } from "../i18n";
import { errText, fmtAgo, useStore } from "../store";
import { Modal } from "./common";

const splitTags = (s: string) =>
  s.split(/[\s,]+/).map((t) => t.trim().replace(/^[#@]/, "")).filter(Boolean).slice(0, 10);

/** Publish a local clip to the Justice feed: title, description, tagged players, hashtags. */
export function PublishClip({ clip, onClose, onPublished }: { clip: Clip; onClose: () => void; onPublished: () => void }) {
  const { toast } = useStore();
  const [title, setTitle] = useState(clip.name);
  const [desc, setDesc] = useState("");
  const [hashtags, setHashtags] = useState("");
  const [players, setPlayers] = useState("");
  const [busy, setBusy] = useState(false);

  const publish = async () => {
    setBusy(true);
    try {
      await clipsApi.publish(clip.path, title.trim(), desc.trim(), splitTags(players), splitTags(hashtags), Math.round((clip.duration || 0) * 1000), Math.round(clip.modified / 1000));
      toast(tr("Klip je ve feedu 🎉", "Your clip is in the feed 🎉"), "success");
      onPublished();
      onClose();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title={tr("Publikovat do feedu", "Publish to the feed")}
      onClose={busy ? () => {} : onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose} disabled={busy}>{tr("Zrušit", "Cancel")}</button>
          <button className="btn primary" onClick={publish} disabled={busy || !title.trim()}>
            <Icon name="share" size={15} /> {busy ? tr("Převádím a nahrávám…", "Converting and uploading…") : tr("Publikovat", "Publish")}
          </button>
        </>
      }
    >
      <div className="col" style={{ gap: 10 }}>
        <label className="field"><span>{tr("Název", "Title")}</span>
          <input className="input" value={title} maxLength={120} onChange={(e) => setTitle(e.target.value)} />
        </label>
        <label className="field"><span>{tr("Popis", "Description")}</span>
          <textarea className="input" value={desc} maxLength={600} rows={2} onChange={(e) => setDesc(e.target.value)} placeholder={tr("Co se ve klipu děje…", "What happens in the clip…")} />
        </label>
        <label className="field"><span>{tr("Hashtagy", "Hashtags")}</span>
          <input className="input" value={hashtags} onChange={(e) => setHashtags(e.target.value)} placeholder="pvp, clutch, montage" />
        </label>
        <label className="field"><span>{tr("Označit hráče", "Tag players")}</span>
          <input className="input" value={players} onChange={(e) => setPlayers(e.target.value)} placeholder="Notch, jeb_" />
        </label>
        <span className="small muted">{tr("Klip se převede na 720p (max 60 s) a nahraje na Justice. Uvidí ho ostatní ve feedu.", "The clip is converted to 720p (max 60 s) and uploaded to Justice. Everyone sees it in the feed.")}</span>
      </div>
    </Modal>
  );
}

function FeedCard({ c, onPlay, onLike, onReport, onDelete }: { c: FeedClip; onPlay: () => void; onLike: () => void; onReport: () => void; onDelete: () => void }) {
  return (
    <div className="feed-card">
      <div className="feed-thumb" onClick={onPlay}>
        {c.thumb ? <img src={c.thumb} loading="lazy" alt="" /> : <div className="thumb-empty"><Icon name="clips" size={28} /></div>}
        <span className="play-ico"><Icon name="play" size={22} /></span>
        {c.durationMs > 0 && <span className="len">{Math.floor(c.durationMs / 1000)}s</span>}
      </div>
      <div className="feed-meta">
        <div className="feed-title ellipsis" title={c.title}>{c.title || tr("Bez názvu", "Untitled")}</div>
        <div className="small muted row" style={{ gap: 6 }}>
          <Icon name="user" size={12} /> <span className="ellipsis grow">{c.ownerName}</span>
          <span>{fmtAgo(c.created)}</span>
        </div>
        {c.description && <div className="feed-desc small">{c.description}</div>}
        {(c.hashtags.length > 0 || c.tags.length > 0) && (
          <div className="feed-tags">
            {c.hashtags.map((h) => <span key={h} className="feed-hash">#{h}</span>)}
            {c.tags.map((t) => <span key={t} className="feed-player">@{t}</span>)}
          </div>
        )}
        <div className="feed-actions">
          <button className={`feed-like ${c.liked ? "on" : ""}`} onClick={onLike}>
            <Icon name="heart" size={15} /> {c.likes}
          </button>
          <div className="grow" />
          {c.mine ? (
            <button className="btn ghost sm icon danger" title={tr("Smazat", "Delete")} onClick={onDelete}><Icon name="trash" size={14} /></button>
          ) : (
            <button className="btn ghost sm icon" title={tr("Nahlásit", "Report")} onClick={onReport}><Icon name="alert" size={14} /></button>
          )}
        </div>
      </div>
    </div>
  );
}

/** The community clips feed: browse, like, report, and delete your own. */
export default function ClipsFeed() {
  const { toast } = useStore();
  const [clips, setClips] = useState<FeedClip[] | null>(null);
  const [mine, setMine] = useState(false);
  const [q, setQ] = useState("");
  const [playing, setPlaying] = useState<FeedClip | null>(null);
  const vref = useRef<HTMLVideoElement>(null);

  const load = () => {
    setClips(null);
    const hashtag = q.startsWith("#") ? q.slice(1) : "";
    const player = q.startsWith("@") ? q.slice(1) : "";
    clipsApi.feed(0, hashtag, player, mine).then((r) => setClips(r.clips)).catch((e) => { toast(errText(e), "error"); setClips([]); });
  };
  useEffect(load, [mine]);

  const like = async (c: FeedClip) => {
    try {
      const r = await clipsApi.like(c.id);
      setClips((cs) => (cs ?? []).map((x) => (x.id === c.id ? { ...x, likes: r.likes, liked: r.liked } : x)));
    } catch (e) {
      toast(errText(e), "error");
    }
  };
  const report = async (c: FeedClip) => {
    const reason = prompt(tr("Proč klip nahlašuješ?", "Why are you reporting this clip?")) ?? "";
    try { await clipsApi.report(c.id, reason); toast(tr("Nahlášeno, díky", "Reported, thanks"), "success"); } catch (e) { toast(errText(e), "error"); }
  };
  const del = async (c: FeedClip) => {
    if (!confirm(tr("Smazat tenhle klip z feedu?", "Delete this clip from the feed?"))) return;
    try { await clipsApi.feedDelete(c.id); setClips((cs) => (cs ?? []).filter((x) => x.id !== c.id)); } catch (e) { toast(errText(e), "error"); }
  };

  return (
    <div className="col" style={{ gap: 14 }}>
      <div className="clips-toolbar">
        <div className="chips">
          <button className={`chip ${!mine ? "on" : ""}`} onClick={() => setMine(false)}>{tr("Vše", "Everyone")}</button>
          <button className={`chip ${mine ? "on" : ""}`} onClick={() => setMine(true)}>{tr("Moje", "Mine")}</button>
        </div>
        <div className="grow" />
        <div className="search sm">
          <Icon name="search" size={15} />
          <input className="input" value={q} onChange={(e) => setQ(e.target.value)} onKeyDown={(e) => e.key === "Enter" && load()} placeholder={tr("#hashtag nebo @hráč", "#hashtag or @player")} />
        </div>
        <button className="btn sm" onClick={load}>{tr("Hledat", "Search")}</button>
      </div>

      {!clips ? (
        <div className="muted">{tr("Načítám feed…", "Loading the feed…")}</div>
      ) : clips.length === 0 ? (
        <div className="feed-empty">
          <Icon name="clips" size={28} />
          {tr("Zatím tu nic není. Publikuj svůj klip ze záložky „Moje klipy“.", "Nothing here yet. Publish a clip from the \"My clips\" tab.")}
        </div>
      ) : (
        <div className="feed-grid">
          {clips.map((c) => (
            <FeedCard key={c.id} c={c} onPlay={() => setPlaying(c)} onLike={() => like(c)} onReport={() => report(c)} onDelete={() => del(c)} />
          ))}
        </div>
      )}

      {playing && (
        <Modal title={playing.title || tr("Klip", "Clip")} onClose={() => setPlaying(null)} wide>
          <video ref={vref} className="feed-video" src={playing.url} controls autoPlay />
          <div className="small muted" style={{ marginTop: 8 }}>
            {playing.ownerName} · {fmtAgo(playing.created)}
            {playing.tags.length > 0 && <> · {tr("s", "with")} {playing.tags.map((t) => `@${t}`).join(" ")}</>}
          </div>
        </Modal>
      )}
    </div>
  );
}
