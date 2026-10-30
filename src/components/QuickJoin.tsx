import { useEffect, useState } from "react";
import { api, type Instance, type ServerStatus } from "../api";
import { Icon } from "../icons";
import { LOADER_LABEL, useStore } from "../store";
import { InstIcon, Modal } from "./common";
import CreateInstance from "./CreateInstance";
import { tr } from "../i18n";
import { uiPrefs } from "../ui";
import justiceLogo from "../assets/justicesmp.png";
import ertenixLogo from "../assets/ertenix.png";

export interface QuickServer {
  id: string;
  name: string;
  address: string;
  /** Used when `address` doesn't answer (e.g. a DNS record still pointing at an old host). */
  fallback?: string;
  /** Missing for the user's own servers: they show the server's favicon and name instead. */
  logo?: string;
  /** The logo is a wide wordmark rather than a square icon. */
  wide?: boolean;
  /** Wordmark height in the title-bar button (px). */
  logoHeight?: number;
  /** Game version the server runs — instances on it are suggested first. */
  version?: string;
  accent: string;
}

export const QUICK_SERVERS: QuickServer[] = [
  { id: "justicesmp", name: "Justice SMP", address: "justicesmp.online", fallback: "178.95.164.26:30000", logo: justiceLogo, wide: true, logoHeight: 24, version: "1.21.11", accent: "#e11d38" },
  { id: "ertenix", name: "Ertenix", address: "ertenix.net", logo: ertenixLogo, wide: true, accent: "#38bdf8" },
];

/** Pings the server (falling back to its backup address) and reports which address answered. */
function useStatus(server: QuickServer) {
  const [st, setSt] = useState<ServerStatus | null | undefined>(undefined);
  const [addr, setAddr] = useState(server.address);
  const { address, fallback } = server;
  useEffect(() => {
    let alive = true;
    const ping = async () => {
      for (const a of [address, fallback].filter(Boolean) as string[]) {
        try {
          const s = await api.pingServer(a);
          if (alive) {
            setSt(s);
            setAddr(a);
          }
          return;
        } catch {
          // try the next address
        }
      }
      if (alive) setSt(null);
    };
    ping();
    const h = setInterval(ping, 60_000);
    return () => {
      alive = false;
      clearInterval(h);
    };
  }, [address, fallback]);
  return [st, addr] as const;
}

/** Instance picker for a quick-join server; the choice is remembered in settings. */
export function PickInstance({ server, onClose, onPicked }: { server: QuickServer; onClose: () => void; onPicked: (inst: Instance) => void }) {
  const { instances, settings, saveSettings } = useStore();
  const [creating, setCreating] = useState(false);
  const sorted = [...instances].sort((a, b) => Number(b.mcVersion === server.version) - Number(a.mcVersion === server.version));
  const pick = async (inst: Instance) => {
    await saveSettings({ quickServers: { ...(settings?.quickServers ?? {}), [server.id]: inst.id } });
    onPicked(inst);
  };
  return (
    <Modal
      title={
        <span className="row">
          {server.logo && <img src={server.logo} alt="" className={server.wide ? "qj-wordmark" : "qj-icon"} style={{ height: server.wide ? (server.logoHeight ?? 13) * 1.5 : 28 }} />}
          {!server.wide && server.name}
        </span>
      }
      onClose={onClose}
    >
      <div className="col">
        <span className="muted">
          {tr(
            `Vyber instanci, se kterou se budeš připojovat na ${server.name}. Launcher si ji zapamatuje — změnit ji můžeš v Nastavení.`,
            `Pick the instance to join ${server.name} with. The launcher remembers it — you can change it in Settings.`,
          )}
        </span>
        {sorted.length === 0 ? (
          <div className="callout">
            <Icon name="info" size={18} />
            <span>{tr("Zatím nemáš žádnou instanci.", "You don't have any instances yet.")}</span>
          </div>
        ) : (
          <div className="list">
            {sorted.map((i) => (
              <button key={i.id} className="list-row qj-pick" onClick={() => pick(i)}>
                <InstIcon inst={i} size={36} />
                <div className="grow" style={{ textAlign: "left" }}>
                  <div style={{ fontWeight: 600 }}>{i.name}</div>
                  <div className="small muted">
                    {LOADER_LABEL[i.loader]} {i.mcVersion}
                  </div>
                </div>
                {server.version && i.mcVersion === server.version && <span className="badge green">{tr("Doporučeno", "Recommended")}</span>}
                <Icon name="right" size={16} />
              </button>
            ))}
          </div>
        )}
        <button className="btn" onClick={() => setCreating(true)}>
          <Icon name="plus" size={16} /> {tr("Vytvořit novou instanci", "Create a new instance")}
        </button>
      </div>
      {creating && <CreateInstance onClose={() => setCreating(false)} />}
    </Modal>
  );
}

function QuickButton({ server }: { server: QuickServer }) {
  const { settings, instances, play, running, progress, toast } = useStore();
  const [st, addr] = useStatus(server);
  const [picking, setPicking] = useState(false);
  const instId = settings?.quickServers?.[server.id];
  const inst = instances.find((i) => i.id === instId);
  const busy = inst ? !!progress[inst.id] || running.includes(inst.id) : false;

  const join = (i: Instance) => {
    if (running.includes(i.id)) return toast(tr(`${i.name} už běží`, `${i.name} is already running`), "info");
    play(i.id, addr);
  };
  const click = () => (inst ? join(inst) : setPicking(true));
  const players = st ? `${st.online}/${st.max}` : st === null ? "offline" : "…";
  const title = `${server.name}${settings?.streamerMode ? "" : ` · ${server.address}`} · ${players}${inst ? ` · ${inst.name}` : ""}`;

  return (
    <>
      <button
        className={`qj-btn ${busy ? "busy" : ""}`}
        style={{ ["--qj" as string]: server.accent }}
        onClick={click}
        onContextMenu={(e) => {
          e.preventDefault();
          setPicking(true);
        }}
        title={`${title}\n${tr("Pravým tlačítkem změníš instanci", "Right-click to change the instance")}`}
        aria-label={tr(`Hrát ${server.name}`, `Play ${server.name}`)}
      >
        <span className="qj-logo">
          {server.logo ? (
            <img src={server.logo} alt={server.name} className={server.wide ? "qj-wordmark" : "qj-icon"} style={server.logoHeight ? { height: server.logoHeight } : undefined} />
          ) : (
            <>
              {st?.icon ? <img src={st.icon} alt="" className="qj-icon" /> : <span className="qj-letter">{server.name.slice(0, 1).toUpperCase()}</span>}
              <span className="qj-name">{server.name}</span>
            </>
          )}
        </span>
        <span className="qj-status">
          <span className={`qj-dot ${st ? "on" : st === null ? "off" : ""}`} />
          {st ? st.online : st === null ? "off" : "…"}
        </span>
        <span className="qj-play">
          {busy ? <span className="spinner" style={{ width: 12, height: 12 }} /> : <Icon name="play" size={11} />}
          {tr("Hrát", "Play")}
        </span>
      </button>
      {picking && (
        <PickInstance
          server={server}
          onClose={() => setPicking(false)}
          onPicked={(i) => {
            setPicking(false);
            join(i);
          }}
        />
      )}
    </>
  );
}

/** The user's own title-bar servers as quick-join entries. */
export function customQuickServers(ui: unknown): QuickServer[] {
  return uiPrefs(ui).quickCustom.map((q) => ({ id: q.id, name: q.name || q.address, address: q.address, accent: "var(--red)" }));
}

/** Title-bar "Play" buttons: Justice SMP, Ertenix and the user's own servers. */
export default function QuickJoin() {
  const { settings } = useStore();
  const hidden = uiPrefs(settings?.ui).quickHidden;
  const shown = [...QUICK_SERVERS.filter((s) => !hidden.includes(s.id)), ...customQuickServers(settings?.ui)];
  if (!shown.length) return null;
  return (
    <div className="qj">
      {shown.map((s) => (
        <QuickButton key={s.id} server={s} />
      ))}
    </div>
  );
}
