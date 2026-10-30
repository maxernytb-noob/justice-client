import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, clips as clipsApi, social, type ChatMessage, type Clip, type Presence, type SocialProfile, type SocialUser } from "../api";
import { Icon } from "../icons";
import { errText, fmtAgo, fmtSize, useStore } from "../store";
import { Empty, Modal, SkeletonList } from "../components/common";
import { tr } from "../i18n";
import Select from "../components/Select";
import { useJoinFriend } from "../components/FriendsPlaying";
import { ACHIEVEMENTS, BADGES, BANNERS, CAPES, earned, head, type Stats } from "../achievements";
import { McIcon } from "../components/McIcons";
import { PartyPanel, useParty } from "../components/Party";

type SocialEvent =
  | { type: "message"; message: ChatMessage }
  | { type: "presence"; uuid: string; online: boolean; presence: Presence | null }
  | { type: "friend-request" | "friend-accepted" | "friend-removed" | "connected" | "offline" | "signed-out" | "signed-in" | "typing"; [k: string]: any };

function presenceText(u: Pick<SocialUser, "online" | "presence" | "lastSeen">) {
  if (!u.online) return tr(`Offline · ${fmtAgo(u.lastSeen)}`, `Offline · ${fmtAgo(u.lastSeen)}`);
  const p = u.presence;
  if (p?.state === "playing") {
    const where = p.server ? tr(` na ${p.server}`, ` on ${p.server}`) : tr(" (singleplayer)", " (singleplayer)");
    return `${tr("Hraje", "Playing")} ${p.instance ?? p.mc ?? "Minecraft"}${where}`;
  }
  return tr("Online v launcheru", "Online in the launcher");
}

function Head({ uuid, size = 36, online }: { uuid: string; size?: number; online?: boolean }) {
  return (
    <span className="fhead" style={{ width: size, height: size }}>
      <img src={head(uuid, size * 2)} alt="" />
      {online !== undefined && <span className={`dot ${online ? "on" : ""}`} />}
    </span>
  );
}

// ---------------- page ----------------

export default function Friends() {
  const { toast, instances, activeAccount } = useStore();
  const [status, setStatus] = useState<{ signedIn: boolean; uuid: string | null } | null>(null);
  const [board, setBoard] = useState(false);
  const [signingIn, setSigningIn] = useState(false);
  const [signErr, setSignErr] = useState("");
  const [me, setMe] = useState<SocialUser | null>(null);
  const [data, setData] = useState<{ friends: SocialUser[]; incoming: SocialUser[]; outgoing: SocialUser[] } | null>(null);
  // Dev preview only: ?chat=<uuid> opens a conversation straight away (website screenshots).
  const [active, setActive] = useState<string | null>(() => (import.meta.env.DEV ? new URLSearchParams(location.search).get("chat") : null));
  const [addName, setAddName] = useState("");
  const [profileOf, setProfileOf] = useState<SocialUser | null>(null);
  const [editing, setEditing] = useState(false);
  const [connected, setConnected] = useState(true);

  const reload = useCallback(async () => {
    try {
      const [m, f] = await Promise.all([social.me(), social.friends()]);
      setMe(m);
      setData(f);
    } catch (e) {
      toast(errText(e), "error");
    }
  }, [toast]);

  useEffect(() => {
    social.status().then((s) => {
      setStatus(s);
      if (s.signedIn) reload();
    });
  }, [reload]);

  // Publish playtime, instances, clips, friends and the achievements they unlock.
  useEffect(() => {
    if (!me || !data) return;
    (async () => {
      const clipCount = await clipsApi.list().then((l) => l.length).catch(() => 0);
      const pvp = await social.pvpStats().catch(() => undefined);
      const stats: Stats = {
        playtimeSecs: instances.reduce((a, i) => a + i.playtimeSecs, 0),
        instances: instances.length,
        clips: clipCount,
        friends: data.friends.length,
        pvp,
      };
      const ach = earned(stats);
      const old = me.profile;
      if (JSON.stringify(old.stats) === JSON.stringify(stats) && JSON.stringify(old.achievements) === JSON.stringify(ach)) return;
      const n = await social.saveProfile({ ...old, stats, achievements: ach }).catch(() => null);
      if (n) setMe(n);
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [me?.uuid, data?.friends.length, instances.length]);

  useEffect(() => {
    const un = listen<SocialEvent>("social-event", (e) => {
      const ev = e.payload;
      if (ev.type === "presence") {
        setData((d) => d && { ...d, friends: d.friends.map((f) => (f.uuid === ev.uuid ? { ...f, online: ev.online, presence: ev.presence, lastSeen: ev.online ? f.lastSeen : Date.now() / 1000 } : f)) });
      } else if (ev.type === "friend-request") {
        toast(tr(`${ev.from?.name} ti poslal žádost o přátelství`, `${ev.from?.name} sent you a friend request`), "info");
        reload();
      } else if (ev.type === "friend-accepted" || ev.type === "friend-removed") {
        reload();
      } else if (ev.type === "message") {
        const m = ev.message;
        setData((d) => d && { ...d, friends: d.friends.map((f) => (f.uuid === m.from && m.from !== activeRef.current ? { ...f, unread: (f.unread ?? 0) + 1 } : f)) });
      } else if (ev.type === "connected") setConnected(true);
      else if (ev.type === "offline") setConnected(false);
      else if (ev.type === "signed-out") setStatus({ signedIn: false, uuid: null });
      else if (ev.type === "signed-in") social.status().then((s) => {
        setStatus(s);
        if (s.signedIn) reload();
      });
    });
    return () => {
      un.then((f) => f());
    };
  }, [reload, toast]);

  const activeRef = useRef<string | null>(null);
  activeRef.current = active;

  const signIn = async () => {
    setSigningIn(true);
    setSignErr("");
    try {
      await social.login();
      setStatus(await social.status());
      await reload();
    } catch (e) {
      setSignErr(errText(e));
    } finally {
      setSigningIn(false);
    }
  };

  const addFriend = async () => {
    const n = addName.trim();
    if (!n) return;
    try {
      const r = await social.request(n);
      toast(r.status === "friends" ? tr(`Teď jste s ${n} přátelé`, `You and ${n} are now friends`) : tr(`Žádost poslána hráči ${n}`, `Request sent to ${n}`), "success");
      setAddName("");
      reload();
    } catch (e) {
      toast(errText(e), "error");
    }
  };

  if (!status) return <div className="page"><SkeletonList /></div>;
  if (!status.signedIn && activeAccount?.kind === "offline")
    return (
      <div className="page">
        <div className="page-head">
          <h1>{tr("Přátelé", "Friends")}</h1>
        </div>
        <OfflineSignIn
          name={activeAccount.username}
          onDone={async () => {
            setStatus(await social.status());
            await reload();
          }}
        />
      </div>
    );
  if (!status.signedIn)
    return (
      <div className="page">
        <div className="page-head">
          <h1>{tr("Přátelé", "Friends")}</h1>
        </div>
        <div className="card pad col" style={{ maxWidth: 560, gap: 14 }}>
          <b style={{ fontSize: 16 }}>{tr("Přátelé a chat v Justice Clientu", "Friends and chat in Justice Client")}</b>
          <span className="muted">
            {tr(
              "Přidávej kamarády podle jejich Minecraft jména, piš si, posílej klipy a modpacky a zvi je do hry. Přihlášení ověří tvůj Minecraft účet stejně jako připojení na server — heslo ani token nikdo neuvidí.",
              "Add friends by their Minecraft name, chat, send clips and modpacks and invite them to play. Signing in verifies your Minecraft account the same way joining a server does — nobody sees your password or token.",
            )}
          </span>
          <button className="btn primary" style={{ alignSelf: "flex-start" }} onClick={signIn} disabled={signingIn}>
            {signingIn ? <span className="spinner" /> : <Icon name="users" size={16} />} {tr("Přihlásit se", "Sign in")}
          </button>
          {signErr && (
            <div className="callout red">
              <Icon name="alert" size={18} />
              <span>{signErr}</span>
            </div>
          )}
        </div>
      </div>
    );


  const friends = [...(data?.friends ?? [])].sort((a, b) => Number(b.online) - Number(a.online) || (b.unread ?? 0) - (a.unread ?? 0) || a.name.localeCompare(b.name));
  const activeFriend = friends.find((f) => f.uuid === active) ?? null;

  return (
    <div className="page friends-page">
      <div className="friends">
        <aside className="fl">
          {me && (
            <div className="me-card" style={{ background: BANNERS[me.profile.banner ?? ""] ?? BANNERS.red }}>
              <Head uuid={me.uuid} size={44} />
              <div className="grow" style={{ minWidth: 0 }}>
                <b>{me.name}</b>
                <span className="ellipsis">{me.profile.status || tr("Nastav si status…", "Set a status…")}</span>
              </div>
              <button className="btn ghost sm icon" onClick={() => setEditing(true)} title={tr("Upravit profil", "Edit profile")}>
                <Icon name="edit" size={15} />
              </button>
            </div>
          )}
          {!connected && <div className="small muted" style={{ padding: "0 4px" }}>{tr("Připojuji se znovu…", "Reconnecting…")}</div>}
          {me && <PartyPanel me={me.uuid} />}
          <div className="row" style={{ gap: 6 }}>
            <input className="input" placeholder={tr("Přidat přítele (MC jméno)", "Add friend (MC name)")} value={addName} maxLength={16} onChange={(e) => setAddName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && addFriend()} />
            <button className="btn icon" onClick={addFriend} disabled={!addName.trim()} aria-label={tr("Přidat", "Add")}>
              <Icon name="plus" size={16} />
            </button>
          </div>

          {!!data?.incoming.length && (
            <>
              <div className="fl-title">{tr("Žádosti", "Requests")}</div>
              {data.incoming.map((u) => (
                <div key={u.uuid} className="frow">
                  <Head uuid={u.uuid} />
                  <b className="grow ellipsis">{u.name}</b>
                  <button className="btn sm icon primary" onClick={() => social.respond(u.uuid, true).then(reload)} title={tr("Přijmout", "Accept")}>
                    <Icon name="check" size={14} />
                  </button>
                  <button className="btn sm icon ghost" onClick={() => social.respond(u.uuid, false).then(reload)} title={tr("Odmítnout", "Decline")}>
                    <Icon name="x" size={14} />
                  </button>
                </div>
              ))}
            </>
          )}
          {!!data?.outgoing.length && (
            <>
              <div className="fl-title">{tr("Odeslané žádosti", "Sent requests")}</div>
              {data.outgoing.map((u) => (
                <div key={u.uuid} className="frow muted">
                  <Head uuid={u.uuid} />
                  <span className="grow ellipsis">{u.name}</span>
                  <button className="btn sm ghost" onClick={() => social.remove(u.uuid).then(reload)}>
                    {tr("Zrušit", "Cancel")}
                  </button>
                </div>
              ))}
            </>
          )}

          <div className="fl-title row" style={{ gap: 6 }}>
            <span className="grow">
              {tr("Přátelé", "Friends")} · {friends.filter((f) => f.online).length}/{friends.length} online
            </span>
            <button className="btn sm ghost icon" onClick={() => setBoard(true)} title={tr("Žebříček přátel", "Friends leaderboard")}>
              <Icon name="trophy" size={14} />
            </button>
          </div>
          {!data ? (
            <SkeletonList rows={3} />
          ) : !friends.length ? (
            <div className="small muted" style={{ padding: "4px 6px" }}>{tr("Zatím nikdo. Přidej kamaráda podle jeho Minecraft jména.", "Nobody yet. Add a friend by their Minecraft name.")}</div>
          ) : (
            friends.map((f) => (
              <button
                key={f.uuid}
                className={`frow friend ${active === f.uuid ? "on" : ""}`}
                onClick={() => {
                  setActive(f.uuid);
                  setData((d) => d && { ...d, friends: d.friends.map((x) => (x.uuid === f.uuid ? { ...x, unread: 0 } : x)) });
                }}
              >
                <Head uuid={f.uuid} online={f.online} />
                <span className="grow" style={{ minWidth: 0, textAlign: "left" }}>
                  <b className="ellipsis" style={{ display: "block" }}>{f.name}</b>
                  <span className={`small ellipsis ${f.presence?.state === "playing" ? "playing" : "muted"}`} style={{ display: "block" }}>
                    {presenceText(f)}
                  </span>
                </span>
                {!!f.unread && <span className="badge red">{f.unread}</span>}
              </button>
            ))
          )}
        </aside>

        <section className="fc">
          {activeFriend && me ? (
            <Chat key={activeFriend.uuid} me={me} friend={activeFriend} onProfile={() => setProfileOf(activeFriend)} onRemoved={() => { setActive(null); reload(); }} />
          ) : (
            <FriendsOverview friends={friends} onOpen={setActive} />
          )}
        </section>
      </div>

      {profileOf && <ProfileModal user={profileOf} onClose={() => setProfileOf(null)} />}
      {board && me && <Leaderboard me={me} friends={data?.friends ?? []} onClose={() => setBoard(false)} />}
      {editing && me && (
        <EditProfile
          me={me}
          onClose={() => setEditing(false)}
          onSaved={(u) => {
            setMe(u);
            setEditing(false);
          }}
        />
      )}
    </div>
  );
}

/** Right side before a chat is opened: who is online and where they play, with one-click Join. */
function FriendsOverview({ friends, onOpen }: { friends: SocialUser[]; onOpen: (uuid: string) => void }) {
  const { settings } = useStore();
  const join = useJoinFriend();
  const online = friends.filter((f) => f.online);
  if (!online.length)
    return <Empty icon="users" title={tr("Nikdo není online", "Nobody is online")} text={tr("Vyber kamaráda vlevo a napiš mu, nebo mu pošli klip či modpack.", "Pick a friend on the left to chat, or send them a clip or a modpack.")} />;
  return (
    <div className="fo">
      <div className="fo-head">
        <h2>{tr("Kdo je online", "Who's online")}</h2>
        <span className="muted small">{tr("Klikni pro chat, Připojit tě hodí k nim do hry", "Click to chat, Join puts you in their game")}</span>
      </div>
      <div className="fo-grid">
        {online.map((f) => (
          <div key={f.uuid} className="fo-card" onClick={() => onOpen(f.uuid)}>
            <Head uuid={f.uuid} size={52} online />
            <div className="grow" style={{ minWidth: 0 }}>
              <b className="ellipsis" style={{ display: "block" }}>{f.name}</b>
              <span className={`small ellipsis ${f.presence?.state === "playing" ? "playing" : "muted"}`} style={{ display: "block" }}>
                {settings?.streamerMode && f.presence?.server ? tr("Hraje na serveru", "Playing on a server") : presenceText(f)}
              </span>
            </div>
            {f.presence?.server && (
              <button
                className="btn primary sm"
                onClick={(e) => {
                  e.stopPropagation();
                  join(f);
                }}
              >
                <Icon name="play" size={12} /> {tr("Připojit", "Join")}
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------------- chat ----------------

function Chat({ me, friend, onProfile, onRemoved }: { me: SocialUser; friend: SocialUser; onProfile: () => void; onRemoved: () => void }) {
  const { toast } = useStore();
  const [msgs, setMsgs] = useState<ChatMessage[] | null>(null);
  const [text, setText] = useState("");
  const [typing, setTyping] = useState(false);
  const [attach, setAttach] = useState<null | "clip" | "pack" | "invite">(null);
  const [more, setMore] = useState(true);
  const list = useRef<HTMLDivElement>(null);
  const lastTyping = useRef(0);

  const scrollDown = () => requestAnimationFrame(() => list.current && (list.current.scrollTop = list.current.scrollHeight));

  useEffect(() => {
    social.history(friend.uuid).then((h) => {
      setMsgs(h.messages);
      setMore(h.messages.length >= 60);
      scrollDown();
    });
    const un = listen<SocialEvent>("social-event", (e) => {
      const ev = e.payload;
      if (ev.type === "message") {
        const m = ev.message;
        if ((m.from === friend.uuid && m.to === me.uuid) || (m.from === me.uuid && m.to === friend.uuid)) {
          setMsgs((l) => (l && !l.some((x) => x.id === m.id) ? [...l, m] : l));
          setTyping(false);
          scrollDown();
          if (m.from === friend.uuid) social.history(friend.uuid).catch(() => {}); // marks as read
        }
      } else if (ev.type === "typing" && ev.from === friend.uuid) {
        setTyping(true);
        setTimeout(() => setTyping(false), 3500);
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, [friend.uuid, me.uuid]);

  const loadOlder = async () => {
    if (!msgs?.length) return;
    const h = await social.history(friend.uuid, msgs[0].id);
    setMore(h.messages.length >= 60);
    setMsgs((l) => [...h.messages, ...(l ?? [])]);
  };

  const joinFriend = useJoinFriend();
  const { party } = useParty();
  const inParty = !!party && [...party.members, ...party.invited].some((m) => m.uuid === friend.uuid);

  const send = async () => {
    const t = text.trim();
    if (!t) return;
    setText("");
    try {
      await social.send(friend.uuid, "text", t);
    } catch (e) {
      toast(errText(e), "error");
      setText(t);
    }
  };

  return (
    <div className="chat">
      <div className="chat-head" onClick={onProfile}>
        <Head uuid={friend.uuid} size={38} online={friend.online} />
        <div className="grow" style={{ minWidth: 0 }}>
          <b>{friend.name}</b>
          <span className={`small ellipsis ${friend.presence?.state === "playing" ? "playing" : "muted"}`} style={{ display: "block" }}>
            {presenceText(friend)}
          </span>
        </div>
        {friend.badges.map((b) => (
          <span key={b} className={`badge ${BADGES[b]?.cls ?? ""}`}>{BADGES[b] ? tr(BADGES[b].label[0], BADGES[b].label[1]) : b}</span>
        ))}
        {friend.online && friend.presence?.state === "playing" && friend.presence.server && (
          <button
            className="btn sm primary"
            title={friend.presence.server}
            onClick={(e) => {
              e.stopPropagation();
              joinFriend(friend);
            }}
          >
            <Icon name="play" size={13} /> {tr("Připojit se", "Join")}
          </button>
        )}
        {friend.online && !inParty && (!party || party.leader === me.uuid) && (
          <button
            className="btn sm"
            title={tr("Pozvat do party", "Invite to a party")}
            onClick={async (e) => {
              e.stopPropagation();
              try {
                await social.partyInvite(friend.uuid);
                toast(tr(`Pozvánka do party poslána hráči ${friend.name}`, `Party invite sent to ${friend.name}`), "success");
              } catch (err) {
                toast(errText(err), "error");
              }
            }}
          >
            <Icon name="plus" size={13} /> Party
          </button>
        )}
        <button
          className="btn ghost sm icon danger"
          title={tr("Odebrat z přátel", "Remove friend")}
          onClick={async (e) => {
            e.stopPropagation();
            if (!confirm(tr(`Odebrat ${friend.name} z přátel?`, `Remove ${friend.name} from friends?`))) return;
            await social.remove(friend.uuid);
            onRemoved();
          }}
        >
          <Icon name="logout" size={15} />
        </button>
      </div>

      <div className="chat-list" ref={list}>
        {more && msgs && msgs.length > 0 && (
          <button className="btn sm ghost" style={{ alignSelf: "center" }} onClick={loadOlder}>
            {tr("Starší zprávy", "Older messages")}
          </button>
        )}
        {!msgs ? (
          <SkeletonList rows={3} />
        ) : !msgs.length ? (
          <div className="muted small" style={{ textAlign: "center", marginTop: 40 }}>{tr(`Napiš ${friend.name} první zprávu`, `Say hi to ${friend.name}`)}</div>
        ) : (
          msgs.map((m, i) => {
            const mine = m.from === me.uuid;
            const showTime = i === 0 || m.created - msgs[i - 1].created > 600;
            return (
              <div key={m.id} className="msg-wrap">
                {showTime && <div className="msg-time">{new Date(m.created * 1000).toLocaleString()}</div>}
                <div className={`msg ${mine ? "mine" : ""} ${m.kind !== "text" ? "card-msg" : ""}`}>
                  <MessageBody m={m} mine={mine} />
                </div>
              </div>
            );
          })
        )}
        {typing && <div className="typing">{tr(`${friend.name} píše…`, `${friend.name} is typing…`)}</div>}
      </div>

      <div className="composer">
        <div className="attach">
          <button className="btn icon" onClick={() => setAttach("clip")} title={tr("Poslat klip", "Send a clip")}>
            <Icon name="clips" size={16} />
          </button>
          <button className="btn icon" onClick={() => setAttach("pack")} title={tr("Poslat modpack", "Send a modpack")}>
            <Icon name="layers" size={16} />
          </button>
          <button className="btn icon" onClick={() => setAttach("invite")} title={tr("Pozvat do hry", "Invite to play")}>
            <Icon name="gamepad" size={16} />
          </button>
        </div>
        <input
          className="input grow"
          value={text}
          maxLength={2000}
          placeholder={tr(`Zpráva pro ${friend.name}`, `Message ${friend.name}`)}
          onChange={(e) => {
            setText(e.target.value);
            if (Date.now() - lastTyping.current > 2500) {
              lastTyping.current = Date.now();
              social.typing(friend.uuid);
            }
          }}
          onKeyDown={(e) => e.key === "Enter" && send()}
        />
        <button className="btn primary icon" onClick={send} disabled={!text.trim()} aria-label={tr("Odeslat", "Send")}>
          <Icon name="right" size={16} />
        </button>
      </div>

      {attach === "clip" && <SendClip to={friend.uuid} onClose={() => setAttach(null)} />}
      {attach === "pack" && <SendPack to={friend.uuid} onClose={() => setAttach(null)} />}
      {attach === "invite" && <SendInvite to={friend.uuid} onClose={() => setAttach(null)} />}
    </div>
  );
}

const IS_LINUX = navigator.userAgent.includes("Linux");

/** No inline <video> on Linux: WebKitGTK often can't decode HEVC clips from Windows and the whole
 *  window crashed as soon as the chat loaded one. The launcher plays it in the system player instead. */
function LinuxClip({ m }: { m: ChatMessage }) {
  const { toast } = useStore();
  const [busy, setBusy] = useState(false);
  const open = async () => {
    setBusy(true);
    try {
      await social.openClip(m.meta.url);
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };
  const dur = m.meta.duration ? `${Math.floor(m.meta.duration / 60)}:${String(Math.floor(m.meta.duration % 60)).padStart(2, "0")} · ` : "";
  return (
    <div className="row" style={{ gap: 10, alignItems: "center" }}>
      <Icon name="clips" size={20} style={{ color: "var(--red)" }} />
      <div className="col" style={{ gap: 2, minWidth: 0 }}>
        <b className="ellipsis">{m.body}</b>
        <span className="small muted">{dur}{fmtSize(m.meta.size ?? 0)}</span>
      </div>
      <button className="btn primary" disabled={busy} onClick={open}>
        <Icon name="play" size={14} /> {busy ? tr("Stahuji…", "Downloading…") : tr("Přehrát", "Play")}
      </button>
    </div>
  );
}

function MessageBody({ m, mine }: { m: ChatMessage; mine: boolean }) {
  const { instances, play, reloadInstances, toast, go } = useStore();
  if (m.kind === "clip")
    return IS_LINUX ? (
      <LinuxClip m={m} />
    ) : (
      <div className="col" style={{ gap: 6 }}>
        <video src={m.meta.url} controls preload="metadata" className="msg-video" />
        <span className="small">
          <Icon name="clips" size={12} /> {m.body} · {fmtSize(m.meta.size ?? 0)}
        </span>
      </div>
    );
  if (m.kind === "pack")
    return (
      <div className="col" style={{ gap: 8 }}>
        <span className="row" style={{ gap: 8 }}>
          <Icon name="layers" size={18} />
          <span>
            <b>{m.meta.name ?? m.body}</b>
            <span className="small muted" style={{ display: "block" }}>
              {m.meta.loader} {m.meta.mc} · {tr("modpack", "modpack")}
            </span>
          </span>
        </span>
        {!mine && (
          <button
            className="btn sm primary"
            onClick={async () => {
              try {
                const inst = await api.importCode(m.meta.code);
                await reloadInstances();
                toast(tr(`Instance „${inst.name}“ nainstalována`, `Instance “${inst.name}” installed`), "success");
                go({ page: "instance", id: inst.id });
              } catch (e) {
                toast(errText(e), "error");
              }
            }}
          >
            <Icon name="download" size={14} /> {tr("Nainstalovat", "Install")}
          </button>
        )}
      </div>
    );
  if (m.kind === "world") {
    const join = async () => {
      const match =
        instances.find((i) => i.mcVersion === m.meta.mc && i.loader === (m.meta.loader || "fabric")) ?? instances.find((i) => i.mcVersion === m.meta.mc);
      if (!match) return toast(tr(`Nemáš instanci pro Minecraft ${m.meta.mc} — vytvoř ji a zkus to znovu`, `You don't have an instance for Minecraft ${m.meta.mc} — create one and try again`), "error");
      try {
        const port = await social.joinWorld(m.meta.world);
        await play(match.id, `127.0.0.1:${port}`);
      } catch (e) {
        toast(errText(e), "error");
      }
    };
    return (
      <div className="col" style={{ gap: 8 }}>
        <span className="row" style={{ gap: 8 }}>
          <Icon name="globe" size={18} />
          <span>
            <b>{m.body}</b>
            <span className="small muted" style={{ display: "block" }}>
              {tr("Singleplayer svět", "Singleplayer world")} · Minecraft {m.meta.mc}
            </span>
          </span>
        </span>
        {!mine && (
          <button className="btn sm primary" onClick={join}>
            <Icon name="play" size={13} /> {tr("Připojit do světa", "Join the world")}
          </button>
        )}
      </div>
    );
  }
  if (m.kind === "invite") {
    const join = async () => {
      const match =
        instances.find((i) => i.name === m.meta.instance && i.mcVersion === m.meta.mc) ??
        instances.find((i) => i.mcVersion === m.meta.mc && i.loader === m.meta.loader) ??
        instances.find((i) => i.mcVersion === m.meta.mc);
      try {
        if (match) return play(match.id, m.meta.server);
        if (m.meta.code) {
          const inst = await api.importCode(m.meta.code);
          await reloadInstances();
          return play(inst.id, m.meta.server);
        }
        toast(tr(`Nemáš instanci pro ${m.meta.mc} — vytvoř ji a zkus to znovu`, `You don't have an instance for ${m.meta.mc} — create one and try again`), "error");
      } catch (e) {
        toast(errText(e), "error");
      }
    };
    return (
      <div className="col" style={{ gap: 8 }}>
        <span className="row" style={{ gap: 8 }}>
          <Icon name="gamepad" size={18} />
          <span>
            <b>{tr("Pozvánka do hry", "Game invite")}</b>
            <span className="small muted" style={{ display: "block" }}>
              {m.meta.server} · {m.meta.instance ?? ""} {m.meta.mc}
            </span>
          </span>
        </span>
        {m.body && <span>{m.body}</span>}
        {!mine && (
          <button className="btn sm primary" onClick={join}>
            <Icon name="play" size={13} /> {tr("Připojit se", "Join")}
          </button>
        )}
      </div>
    );
  }
  return <span className="msg-text">{m.body}</span>;
}

// ---------------- attachments ----------------

function SendClip({ to, onClose }: { to: string; onClose: () => void }) {
  const { toast } = useStore();
  const [list, setList] = useState<Clip[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  useEffect(() => {
    clipsApi.list().then(setList).catch(() => setList([]));
  }, []);
  const send = async (c: Clip) => {
    setBusy(c.path);
    try {
      const up = await social.upload(c.path);
      await social.send(to, "clip", c.name, { url: up.url, size: up.size, duration: c.duration });
      onClose();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(null);
    }
  };
  return (
    <Modal title={tr("Poslat klip", "Send a clip")} onClose={() => !busy && onClose()}>
      <div className="small muted" style={{ marginBottom: 10 }}>{tr("Klip se nahraje na server Justice (max 300 MB) a vydrží 30 dní.", "The clip is uploaded to the Justice server (max 300 MB) and kept for 30 days.")}</div>
      {!list ? (
        <SkeletonList rows={3} />
      ) : !list.length ? (
        <div className="muted">{tr("Zatím nemáš žádné klipy.", "You don't have any clips yet.")}</div>
      ) : (
        <div className="list" style={{ maxHeight: 380, overflow: "auto" }}>
          {list.slice(0, 40).map((c) => (
            <div key={c.path} className="list-row">
              <div className="grow" style={{ minWidth: 0 }}>
                <b className="ellipsis" style={{ display: "block" }}>{c.name}</b>
                <span className="small muted">{c.game} · {Math.round(c.duration)} s · {fmtSize(c.size)}</span>
              </div>
              <button className="btn sm primary" disabled={!!busy || c.size > 300 * 1024 * 1024} onClick={() => send(c)}>
                {busy === c.path ? <span className="spinner" /> : <Icon name="upload" size={14} />} {tr("Poslat", "Send")}
              </button>
            </div>
          ))}
        </div>
      )}
    </Modal>
  );
}

function SendPack({ to, onClose }: { to: string; onClose: () => void }) {
  const { instances, toast } = useStore();
  const [busy, setBusy] = useState<string | null>(null);
  const send = async (id: string) => {
    setBusy(id);
    try {
      const inst = instances.find((i) => i.id === id)!;
      const code = await api.shareCode(id);
      await social.send(to, "pack", inst.name, { code, name: inst.name, mc: inst.mcVersion, loader: inst.loader });
      onClose();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(null);
    }
  };
  return (
    <Modal title={tr("Poslat modpack", "Send a modpack")} onClose={onClose}>
      <div className="small muted" style={{ marginBottom: 10 }}>{tr("Přítel si ho nainstaluje jedním klikem (mody, verze i nastavení).", "Your friend installs it with one click (mods, version and settings).")}</div>
      <div className="list" style={{ maxHeight: 380, overflow: "auto" }}>
        {instances.map((i) => (
          <div key={i.id} className="list-row">
            <div className="grow">
              <b>{i.name}</b>
              <span className="small muted" style={{ display: "block" }}>{i.loader} {i.mcVersion}</span>
            </div>
            <button className="btn sm primary" disabled={!!busy} onClick={() => send(i.id)}>
              {busy === i.id ? <span className="spinner" /> : <Icon name="share" size={14} />} {tr("Poslat", "Send")}
            </button>
          </div>
        ))}
      </div>
    </Modal>
  );
}

function SendInvite({ to, onClose }: { to: string; onClose: () => void }) {
  const { instances, toast } = useStore();
  const [inst, setInst] = useState(instances[0]?.id ?? "");
  const [server, setServer] = useState("justicesmp.online");
  const [note, setNote] = useState("");
  const [withPack, setWithPack] = useState(true);
  const send = async () => {
    const i = instances.find((x) => x.id === inst);
    if (!i || !server.trim()) return;
    try {
      const code = withPack ? await api.shareCode(i.id).catch(() => "") : "";
      await social.send(to, "invite", note.trim(), { server: server.trim(), instance: i.name, mc: i.mcVersion, loader: i.loader, code });
      onClose();
    } catch (e) {
      toast(errText(e), "error");
    }
  };
  return (
    <Modal
      title={tr("Pozvat do hry", "Invite to play")}
      onClose={onClose}
      footer={
        <button className="btn primary" onClick={send} disabled={!inst || !server.trim()}>
          <Icon name="gamepad" size={15} /> {tr("Poslat pozvánku", "Send invite")}
        </button>
      }
    >
      <div className="col" style={{ gap: 12 }}>
        <div className="field">
          <label>Server</label>
          <input className="input" value={server} onChange={(e) => setServer(e.target.value)} placeholder="justicesmp.online" />
        </div>
        <div className="field">
          <label>{tr("Instance", "Instance")}</label>
          <Select className="input" value={inst} onChange={(e) => setInst(e.target.value)}>
            {instances.map((i) => (
              <option key={i.id} value={i.id}>{i.name} · {i.loader} {i.mcVersion}</option>
            ))}
          </Select>
        </div>
        <label className="check">
          <input type="checkbox" checked={withPack} onChange={(e) => setWithPack(e.target.checked)} />{" "}
          {tr("Přiložit mody (když přítel instanci nemá, nainstaluje se sama)", "Attach the mods (if your friend lacks the instance, it installs itself)")}
        </label>
        <div className="field">
          <label>{tr("Zpráva (nepovinné)", "Message (optional)")}</label>
          <input className="input" value={note} maxLength={200} onChange={(e) => setNote(e.target.value)} placeholder={tr("Pojď na Justice SMP!", "Join me on Justice SMP!")} />
        </div>
      </div>
    </Modal>
  );
}

// ---------------- profiles ----------------

function ProfileView({ user }: { user: SocialUser }) {
  const s = user.profile.stats;
  const got = new Set(user.profile.achievements ?? []);
  return (
    <div className="profile">
      <div className="profile-banner" style={{ background: BANNERS[user.profile.banner ?? ""] ?? BANNERS.red }}>
        <Head uuid={user.uuid} size={64} />
      </div>
      <div className="profile-body">
        <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
          <h2 style={{ margin: 0 }}>{user.name}</h2>
          {user.badges.map((b) => (
            <span key={b} className={`badge ${BADGES[b]?.cls ?? ""}`}>{BADGES[b] ? tr(BADGES[b].label[0], BADGES[b].label[1]) : b}</span>
          ))}
        </div>
        {user.profile.status && <div className="muted">{user.profile.status}</div>}
        {user.profile.bio && <p style={{ whiteSpace: "pre-wrap", margin: "10px 0 0" }}>{user.profile.bio}</p>}
        {s && (
          <div className="stat-row">
            <div><b>{Math.round(s.playtimeSecs / 3600)} h</b><span>{tr("ve hře", "played")}</span></div>
            <div><b>{s.instances}</b><span>{tr("instancí", "instances")}</span></div>
            <div><b>{s.clips}</b><span>{tr("klipů", "clips")}</span></div>
            <div><b>{s.friends}</b><span>{tr("přátel", "friends")}</span></div>
          </div>
        )}
        {s?.pvp && (s.pvp.kills > 0 || s.pvp.deaths > 0) && (
          <div className="stat-row">
            <div><b>{s.pvp.kills}</b><span>{tr("killů", "kills")}</span></div>
            <div><b>{s.pvp.deaths}</b><span>{tr("smrtí", "deaths")}</span></div>
            <div><b>{kd(s.pvp)}</b><span>K/D</span></div>
            <div><b>{s.pvp.bestCombo}</b><span>{tr("nej combo", "best combo")}</span></div>
          </div>
        )}
        {user.profile.cape && (
          <div className="row small muted" style={{ gap: 8, marginTop: 10 }}>
            <CapePreview id={user.profile.cape} size={2} />
            {tr("Pláštěnka", "Cape")}: {capeName(user.profile.cape)}
          </div>
        )}
        <div className="section-title" style={{ marginTop: 16 }}>{tr("Achievementy", "Achievements")} · {got.size}/{ACHIEVEMENTS.length}</div>
        <div className="ach-grid">
          {ACHIEVEMENTS.map((a) => (
            <div key={a.id} className={`ach ${got.has(a.id) ? "got" : ""}`} title={tr(a.text[0], a.text[1])}>
              <McIcon name={a.mc} size={22} gray={!got.has(a.id)} />
              <span>{tr(a.name[0], a.name[1])}</span>
            </div>
          ))}
        </div>
        <div className="small muted" style={{ marginTop: 12 }}>{tr(`V Justice Clientu od ${new Date(user.created * 1000).toLocaleDateString()}`, `On Justice Client since ${new Date(user.created * 1000).toLocaleDateString()}`)}</div>
      </div>
    </div>
  );
}

function ProfileModal({ user, onClose }: { user: SocialUser; onClose: () => void }) {
  return (
    <Modal title={tr("Profil", "Profile")} onClose={onClose}>
      <ProfileView user={user} />
    </Modal>
  );
}

function EditProfile({ me, onClose, onSaved }: { me: SocialUser; onClose: () => void; onSaved: (u: SocialUser) => void }) {
  const { toast } = useStore();
  const [p, setP] = useState<SocialProfile>({ ...me.profile });
  const preview = useMemo(() => ({ ...me, profile: p }), [me, p]);
  const save = async () => {
    try {
      onSaved(await social.saveProfile(p));
    } catch (e) {
      toast(errText(e), "error");
    }
  };
  return (
    <Modal
      title={tr("Můj profil", "My profile")}
      onClose={onClose}
      wide
      footer={
        <button className="btn primary" onClick={save}>
          <Icon name="check" size={15} /> {tr("Uložit", "Save")}
        </button>
      }
    >
      <div className="grid2" style={{ alignItems: "start" }}>
        <div className="col" style={{ gap: 12 }}>
          <div className="field">
            <label>{tr("Status", "Status")}</label>
            <input className="input" value={p.status ?? ""} maxLength={80} onChange={(e) => setP({ ...p, status: e.target.value })} placeholder={tr("Točím nové video", "Filming a new video")} />
          </div>
          <div className="field">
            <label>Bio</label>
            <textarea className="input" rows={4} value={p.bio ?? ""} maxLength={300} onChange={(e) => setP({ ...p, bio: e.target.value })} placeholder={tr("Pár slov o sobě…", "A few words about you…")} />
          </div>
          <div className="field">
            <label>{tr("Justice pláštěnka", "Justice cape")}</label>
            <div className="cape-grid">
              <button className={`cape-pick ${!p.cape ? "on" : ""}`} onClick={() => setP({ ...p, cape: "" })} title={tr("Žádná", "None")}>
                <Icon name="x" size={16} />
              </button>
              {CAPES.map((c) => (
                <button key={c.id} className={`cape-pick ${p.cape === c.id ? "on" : ""}`} onClick={() => setP({ ...p, cape: c.id })} title={tr(c.name[0], c.name[1])}>
                  <CapePreview id={c.id} size={3} />
                </button>
              ))}
            </div>
            <span className="small muted">{tr("Uvidí ji ostatní hráči Justice Clientu ve hře (i na elytrách).", "Other Justice Client players see it in-game (on elytra too).")}</span>
          </div>
          <div className="field">
            <label>Banner</label>
            <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
              {Object.entries(BANNERS).map(([k, bg]) => (
                <button
                  key={k}
                  onClick={() => setP({ ...p, banner: k })}
                  style={{ width: 54, height: 30, borderRadius: 8, background: bg, cursor: "pointer", border: (p.banner ?? "red") === k ? "2px solid var(--text)" : "2px solid transparent" }}
                  aria-label={k}
                />
              ))}
            </div>
          </div>
        </div>
        <div className="card" style={{ overflow: "hidden" }}>
          <ProfileView user={preview} />
        </div>
      </div>
    </Modal>
  );
}

// ---------------- offline sign-in, capes, leaderboard ----------------

/** Offline accounts can't be verified by Mojang, so they sign in to Justice with a password. */
function OfflineSignIn({ name, onDone }: { name: string; onDone: () => void }) {
  const [mode, setMode] = useState<"login" | "register">("login");
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [checking, setChecking] = useState(true);

  // A saved session signs in by itself.
  useEffect(() => {
    social
      .login()
      .then(onDone)
      .catch(() => {})
      .finally(() => setChecking(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const register = mode === "register";
  const ok = pw.length >= 6 && (!register || pw === pw2);
  const submit = async () => {
    if (!ok) return;
    setBusy(true);
    setErr("");
    try {
      await social.loginPassword(pw, register);
      onDone();
    } catch (e) {
      setErr(errText(e));
    } finally {
      setBusy(false);
    }
  };
  if (checking) return <SkeletonList rows={2} />;
  return (
    <div className="card pad col" style={{ maxWidth: 560, gap: 14 }}>
      <b style={{ fontSize: 16 }}>{tr(`Přátelé a chat pro ${name}`, `Friends and chat for ${name}`)}</b>
      <span className="muted">
        {tr(
          "Offline účet neověří Mojang, proto si k němu vytvoř heslo. Jméno si nikdo jiný nevezme a přihlásíš se na jakémkoliv PC. Stačí jednou — launcher si přihlášení zapamatuje.",
          "Mojang can't verify an offline account, so give it a password. Nobody else can take the name and you can sign in on any PC. Only once — the launcher remembers it.",
        )}
      </span>
      <div className="chips">
        <button className={`chip ${!register ? "on" : ""}`} onClick={() => setMode("login")}>{tr("Už mám heslo", "I have a password")}</button>
        <button className={`chip ${register ? "on" : ""}`} onClick={() => setMode("register")}>{tr("Vytvořit účet", "Create account")}</button>
      </div>
      <input className="input" type="password" placeholder={tr("Heslo (aspoň 6 znaků)", "Password (at least 6 characters)")} value={pw} onChange={(e) => setPw(e.target.value)} onKeyDown={(e) => e.key === "Enter" && submit()} autoFocus />
      {register && (
        <input className="input" type="password" placeholder={tr("Heslo znovu", "Password again")} value={pw2} onChange={(e) => setPw2(e.target.value)} onKeyDown={(e) => e.key === "Enter" && submit()} />
      )}
      {register && pw2 && pw !== pw2 && <span className="small" style={{ color: "var(--red)" }}>{tr("Hesla se neshodují", "The passwords don't match")}</span>}
      <button className="btn primary" style={{ alignSelf: "flex-start" }} onClick={submit} disabled={!ok || busy}>
        {busy ? <span className="spinner" /> : <Icon name="users" size={16} />} {register ? tr("Vytvořit a přihlásit", "Create and sign in") : tr("Přihlásit se", "Sign in")}
      </button>
      {err && (
        <div className="callout red">
          <Icon name="alert" size={18} />
          <span>{err}</span>
        </div>
      )}
    </div>
  );
}

function capeName(id: string) {
  const c = CAPES.find((x) => x.id === id);
  return c ? tr(c.name[0], c.name[1]) : id;
}

/** The outer face of a cape texture (10x16 px at 1,1), scaled up with crisp pixels. */
function CapePreview({ id, size }: { id: string; size: number }) {
  return (
    <span
      className="cape-preview"
      style={{
        width: 10 * size,
        height: 16 * size,
        backgroundImage: `url(/capes/${id}.png)`,
        backgroundSize: `${64 * size}px ${32 * size}px`,
        backgroundPosition: `${-1 * size}px ${-1 * size}px`,
      }}
    />
  );
}

function kd(p: { kills: number; deaths: number }) {
  return p.deaths ? (p.kills / p.deaths).toFixed(2) : p.kills ? String(p.kills) : "0";
}

/** PvP from Justice Mod: you and your friends, sorted by kills. */
function Leaderboard({ me, friends, onClose }: { me: SocialUser; friends: SocialUser[]; onClose: () => void }) {
  const [sort, setSort] = useState<"kills" | "kd" | "combo" | "hours">("kills");
  const rows = [me, ...friends].map((u) => {
    const s = u.profile.stats;
    const p = s?.pvp ?? { kills: 0, deaths: 0, bestCombo: 0, hits: 0 };
    return { u, p, hours: Math.round((s?.playtimeSecs ?? 0) / 3600), kdv: p.deaths ? p.kills / p.deaths : p.kills };
  });
  const key = (r: (typeof rows)[number]) => (sort === "kills" ? r.p.kills : sort === "kd" ? r.kdv : sort === "combo" ? r.p.bestCombo : r.hours);
  rows.sort((a, b) => key(b) - key(a) || a.u.name.localeCompare(b.u.name));
  const head_ = (k: typeof sort, label: string) => (
    <button className={`lb-sort ${sort === k ? "on" : ""}`} onClick={() => setSort(k)}>
      {label}
    </button>
  );
  return (
    <Modal title={tr("Žebříček přátel", "Friends leaderboard")} onClose={onClose} wide>
      <div className="small muted" style={{ marginBottom: 10 }}>
        {tr("PvP počítá Justice Mod (killy hráčů, smrti, nejdelší combo). Aktualizuje se po minutě hraní.", "PvP is counted by Justice Mod (player kills, deaths, longest combo). Updates after a minute of play.")}
      </div>
      <div className="lb">
        <div className="lb-row lb-head">
          <span>#</span>
          <span>{tr("Hráč", "Player")}</span>
          {head_("kills", tr("Killy", "Kills"))}
          <span>{tr("Smrti", "Deaths")}</span>
          {head_("kd", "K/D")}
          {head_("combo", tr("Combo", "Combo"))}
          {head_("hours", tr("Hodiny", "Hours"))}
        </div>
        {rows.map((r, i) => (
          <div key={r.u.uuid} className={`lb-row ${r.u.uuid === me.uuid ? "me" : ""}`}>
            <span className={`lb-rank r${i + 1}`}>{i + 1}</span>
            <span className="row" style={{ gap: 8, minWidth: 0 }}>
              <Head uuid={r.u.uuid} size={24} />
              <b className="ellipsis">{r.u.name}</b>
            </span>
            <span>{r.p.kills}</span>
            <span>{r.p.deaths}</span>
            <span>{kd(r.p)}</span>
            <span>{r.p.bestCombo}</span>
            <span>{r.hours} h</span>
          </div>
        ))}
      </div>
    </Modal>
  );
}
