import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { social, type Instance, type Presence, type SocialUser } from "../api";
import { Icon } from "../icons";
import { useStore } from "../store";
import { head } from "../achievements";
import { tr } from "../i18n";

/** The instance best suited to join a friend: same instance name + version, then same version + loader, then same version. */
export function instanceFor(p: Presence, instances: Instance[]): Instance | undefined {
  return (
    instances.find((i) => i.name === p.instance && i.mcVersion === p.mc) ??
    instances.find((i) => i.mcVersion === p.mc && i.loader === p.loader) ??
    instances.find((i) => i.mcVersion === p.mc)
  );
}

/** Joins the server a friend is playing on, or explains which version is missing. */
export function useJoinFriend() {
  const { instances, play, running, toast } = useStore();
  return (f: Pick<SocialUser, "name" | "presence">) => {
    const p = f.presence;
    if (!p?.server) return;
    const inst = instanceFor(p, instances);
    if (!inst) return toast(tr(`Nemáš instanci pro Minecraft ${p.mc ?? "?"} — vytvoř ji a zkus to znovu`, `You don't have an instance for Minecraft ${p.mc ?? "?"} — create one and try again`), "error");
    if (running.includes(inst.id)) return toast(tr(`${inst.name} už běží`, `${inst.name} is already running`), "info");
    toast(tr(`Připojuješ se k ${f.name} na ${p.server}`, `Joining ${f.name} on ${p.server}`), "info");
    play(inst.id, p.server);
  };
}

type PresenceEvent = { type: string; uuid?: string; online?: boolean; presence?: Presence | null };

/** Home-page strip: friends who are playing on a server right now, with a one-click Join. */
export default function FriendsPlaying() {
  const { go, settings } = useStore();
  const join = useJoinFriend();
  const [friends, setFriends] = useState<SocialUser[]>([]);

  useEffect(() => {
    let alive = true;
    const load = () =>
      social
        .status()
        .then((s) => (s.signedIn ? social.friends() : null))
        .then((r) => alive && r && setFriends(r.friends))
        .catch(() => {});
    load();
    const un = listen<PresenceEvent>("social-event", (e) => {
      const ev = e.payload;
      if (ev.type === "presence") setFriends((l) => l.map((f) => (f.uuid === ev.uuid ? { ...f, online: !!ev.online, presence: ev.presence ?? null } : f)));
      else if (ev.type === "friend-accepted" || ev.type === "friend-removed" || ev.type === "connected") load();
    });
    return () => {
      alive = false;
      un.then((f) => f());
    };
  }, []);

  const playing = friends.filter((f) => f.online && f.presence?.state === "playing" && f.presence.server);
  if (!playing.length) return null;
  return (
    <>
      <div className="section-title">
        <span className="grow">{tr("Kamarádi ve hře", "Friends in game")}</span>
        <button className="btn ghost sm" onClick={() => go({ page: "friends" })}>
          {tr("Přátelé", "Friends")} <Icon name="right" size={14} />
        </button>
      </div>
      <div className="inst-grid">
        {playing.map((f) => (
          <div key={f.uuid} className="inst-card" onClick={() => join(f)} title={tr("Připojit se", "Join")}>
            <div className="top">
              <span className="fhead" style={{ width: 42, height: 42 }}>
                <img src={head(f.uuid, 84)} alt="" />
                <span className="dot on" />
              </span>
              <div className="grow" style={{ minWidth: 0 }}>
                <div className="name ellipsis">{f.name}</div>
                <div className="ver ellipsis">
                  {f.presence?.instance ?? f.presence?.mc} · {settings?.streamerMode ? "••••••" : f.presence?.server}
                </div>
              </div>
            </div>
            <div className="row small" style={{ gap: 6, color: "var(--red)", fontWeight: 700 }}>
              <Icon name="play" size={12} /> {tr("Připojit se", "Join")}
            </div>
          </div>
        ))}
      </div>
    </>
  );
}
