import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { social, type Party, type SocialUser, type WarpTarget } from "../api";
import { Icon } from "../icons";
import { errText, useStore } from "../store";
import { head } from "../achievements";
import { tr } from "../i18n";
import { McIcon } from "./McIcons";
import { instanceFor } from "./FriendsPlaying";

type PartyEvent =
  | { type: "party"; party: Party | null }
  | { type: "party-invite"; party: Party; from: SocialUser }
  | { type: "party-warp"; from: SocialUser; target: WarpTarget }
  | { type: string; [k: string]: any };

/** Party state kept fresh from the realtime events. */
export function useParty() {
  const [party, setParty] = useState<Party | null>(null);
  const [invites, setInvites] = useState<Party[]>([]);
  const reload = useCallback(() => {
    social
      .party()
      .then((r) => {
        setParty(r.party);
        setInvites(r.invites);
      })
      .catch(() => {});
  }, []);
  useEffect(() => {
    reload();
    const un = listen<PartyEvent>("social-event", (e) => {
      const ev = e.payload;
      if (ev.type === "party") {
        setParty(ev.party ?? null);
        reload();
      } else if (ev.type === "party-invite" || ev.type === "signed-in" || ev.type === "connected") reload();
      else if (ev.type === "signed-out") {
        setParty(null);
        setInvites([]);
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, [reload]);
  return { party, invites, reload };
}

export function leaderName(p: Party) {
  return p.members.find((m) => m.uuid === p.leader)?.name ?? "?";
}

/** Joins a warp target: a server with the matching instance, or a shared world through the relay. */
export function useJoinTarget() {
  const { instances, play, running, toast } = useStore();
  return async (t: WarpTarget, from: string) => {
    if (running.length) {
      toast(tr("Hra už běží — ve hře otevři Přátele (U) a klikni na Připojit v party", "The game is already running — open Friends (U) in-game and click Join in the party box"), "info");
      return;
    }
    const inst = instanceFor({ state: "playing", instance: t.kind === "server" ? t.instance : undefined, mc: t.mc, loader: t.loader }, instances);
    if (!inst) {
      toast(tr(`Nemáš instanci pro Minecraft ${t.mc ?? "?"} — vytvoř ji a zkus to znovu`, `You don't have an instance for Minecraft ${t.mc ?? "?"} — create one and try again`), "error");
      return;
    }
    try {
      if (t.kind === "server") {
        toast(tr(`Připojuješ se k ${from} na ${t.address}`, `Joining ${from} on ${t.address}`), "info");
        await play(inst.id, t.address);
      } else {
        const port = await social.joinWorld(t.world);
        await play(inst.id, `127.0.0.1:${port}`);
      }
    } catch (e) {
      toast(errText(e), "error");
    }
  };
}

/** Sidebar box on the Friends page: invites, members and what the leader can do. */
export function PartyPanel({ me }: { me: string }) {
  const { toast } = useStore();
  const { party, invites, reload } = useParty();
  const [busy, setBusy] = useState(false);
  const act = async (f: () => Promise<unknown>, ok?: string) => {
    setBusy(true);
    try {
      await f();
      if (ok) toast(ok, "success");
      reload();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };
  if (!party && !invites.length) return null;
  const leader = party?.leader === me;
  return (
    <div className="party-box">
      {invites.map((p) => (
        <div key={p.id} className="party-invite">
          <Icon name="users" size={15} />
          <span className="grow ellipsis">{tr(`${leaderName(p)} tě zve do party`, `${leaderName(p)} invites you to a party`)}</span>
          <button className="btn sm icon primary" disabled={busy} onClick={() => act(() => social.partyAccept(p.id))} title={tr("Přijmout", "Accept")}>
            <Icon name="check" size={14} />
          </button>
          <button className="btn sm icon ghost" disabled={busy} onClick={() => act(() => social.partyDecline(p.id))} title={tr("Odmítnout", "Decline")}>
            <Icon name="x" size={14} />
          </button>
        </div>
      ))}
      {party && (
        <div className="party">
          <div className="row" style={{ gap: 6 }}>
            <b className="party-title">Party · {party.members.length}</b>
            <div className="grow" />
            <button className="btn sm ghost" disabled={busy} onClick={() => act(() => social.partyLeave())}>
              {tr("Opustit", "Leave")}
            </button>
          </div>
          {party.members.map((m) => (
            <div key={m.uuid} className="party-member">
              <span className="fhead" style={{ width: 24, height: 24 }}>
                <img src={head(m.uuid, 48)} alt="" />
                <span className={`dot ${m.online ? "on" : ""}`} />
              </span>
              <span className="grow ellipsis">
                {m.name} {m.uuid === party.leader && <span className="leader" title={tr("Vedoucí", "Leader")}><McIcon name="crown" size={14} /></span>}
                <span className="small muted" style={{ display: "block" }}>
                  {m.presence?.state === "playing" ? `${m.presence.instance ?? m.presence.mc ?? "Minecraft"}${m.presence.server ? ` · ${m.presence.server}` : ""}` : m.online ? tr("V launcheru", "In the launcher") : "Offline"}
                </span>
              </span>
              {leader && m.uuid !== me && (
                <button className="btn sm icon ghost" disabled={busy} onClick={() => act(() => social.partyKick(m.uuid))} title={tr("Vyhodit z party", "Kick from the party")}>
                  <Icon name="x" size={13} />
                </button>
              )}
            </div>
          ))}
          {party.invited.map((m) => (
            <div key={m.uuid} className="party-member muted">
              <span className="fhead" style={{ width: 24, height: 24, opacity: 0.5 }}>
                <img src={head(m.uuid, 48)} alt="" />
              </span>
              <span className="grow ellipsis small">{tr(`${m.name} · pozvaný`, `${m.name} · invited`)}</span>
              {leader && (
                <button className="btn sm icon ghost" disabled={busy} onClick={() => act(() => social.partyKick(m.uuid))} title={tr("Zrušit pozvánku", "Cancel invite")}>
                  <Icon name="x" size={13} />
                </button>
              )}
            </div>
          ))}
          {leader && (
            <button
              className="btn sm primary"
              disabled={busy}
              onClick={() => act(() => social.partyWarpHere(), tr("Parta dostala pozvánku k tobě", "Your party got the invite to join you"))}
              title={tr("Pošle ostatním tvůj server nebo sdílený svět", "Sends the others your server or shared world")}
            >
              <Icon name="play" size={13} /> {tr("Přivést partu ke mně", "Bring the party to me")}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

/** Party invites and warps pop up on any page of the launcher. */
export function PartyPrompts() {
  const [prompts, setPrompts] = useState<{ id: number; ev: PartyEvent }[]>([]);
  const join = useJoinTarget();
  const { toast } = useStore();
  useEffect(() => {
    let n = 0;
    const un = listen<PartyEvent>("social-event", (e) => {
      const ev = e.payload;
      if (ev.type === "party-invite" || ev.type === "party-warp") {
        const id = ++n;
        setPrompts((l) => [...l.slice(-2), { id, ev }]);
        setTimeout(() => setPrompts((l) => l.filter((p) => p.id !== id)), 60_000);
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);
  const close = (id: number) => setPrompts((l) => l.filter((p) => p.id !== id));
  if (!prompts.length) return null;
  return (
    <div className="party-prompts">
      {prompts.map(({ id, ev }) => {
        const e2 = ev as { type: string; from: SocialUser; target?: WarpTarget; party?: Party };
        const from = e2.from;
        const invite = ev.type === "party-invite";
        const t = e2.target;
        return (
          <div key={id} className="party-prompt">
            <img className="pp-head" src={head(from.uuid, 64)} alt="" />
            <div className="grow" style={{ minWidth: 0 }}>
              <b>{from.name}</b>
              <div className="small muted ellipsis">
                {invite
                  ? tr("tě zve do party", "invites you to a party")
                  : t?.kind === "world"
                    ? tr(`chce partu ve světě „${t.name}“`, `wants the party in the world “${t.name}”`)
                    : tr(`chce partu na ${t?.kind === "server" ? t.address : ""}`, `wants the party on ${t?.kind === "server" ? t.address : ""}`)}
              </div>
            </div>
            <button
              className="btn sm primary"
              onClick={async () => {
                close(id);
                if (invite) await social.partyAccept(e2.party!.id).catch((e) => toast(errText(e), "error"));
                else if (t) await join(t, from.name);
              }}
            >
              {invite ? tr("Přijmout", "Accept") : tr("Připojit", "Join")}
            </button>
            <button
              className="btn sm icon ghost"
              onClick={() => {
                close(id);
                if (invite) social.partyDecline(e2.party!.id).catch(() => {});
              }}
              aria-label={tr("Zavřít", "Close")}
            >
              <Icon name="x" size={14} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
