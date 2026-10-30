import { useEffect, useState } from "react";
import { api, type Server, type ServerStatus } from "../api";
import { Icon } from "../icons";
import { errText, LOADER_LABEL, useStore } from "../store";
import { Empty, Modal } from "../components/common";
import { tr } from "../i18n";
import Select from "../components/Select";

const MC_COLORS: Record<string, string> = {
  "0": "#000", "1": "#00a", "2": "#0a0", "3": "#0aa", "4": "#a00", "5": "#a0a", "6": "#fa0", "7": "#aaa",
  "8": "#555", "9": "#55f", a: "#5f5", b: "#5ff", c: "#f55", d: "#f5f", e: "#ff5", f: "#fff",
};

export function Motd({ text }: { text: string }) {
  const parts: { color: string; bold: boolean; text: string }[] = [];
  let color = "#aaa";
  let bold = false;
  const segs = text.split("§");
  parts.push({ color, bold, text: segs[0] });
  for (const s of segs.slice(1)) {
    const c = s[0]?.toLowerCase();
    if (c && MC_COLORS[c]) {
      color = MC_COLORS[c];
      bold = false;
    } else if (c === "l") bold = true;
    else if (c === "r") {
      color = "#aaa";
      bold = false;
    }
    parts.push({ color, bold, text: s.slice(1) });
  }
  return (
    <div className="motd">
      {parts.map((p, i) => (
        <span key={i} style={{ color: p.color, fontWeight: p.bold ? 700 : 400 }}>{p.text}</span>
      ))}
    </div>
  );
}

function EditServer({ server, onClose, onSaved }: { server: Partial<Server>; onClose: () => void; onSaved: () => void }) {
  const { instances, toast } = useStore();
  const [s, setS] = useState<Partial<Server>>({ pinned: true, ...server, instance: server.instance || instances[0]?.id || "" });
  const saveIt = async () => {
    try {
      await api.saveServer(s);
      onSaved();
      onClose();
    } catch (e) {
      toast(errText(e), "error");
    }
  };
  return (
    <Modal
      title={server.id ? tr("Upravit server", "Edit server") : tr("Přidat server", "Add server")}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>{tr("Zrušit", "Cancel")}</button>
          <button className="btn primary" onClick={saveIt} disabled={!s.address?.trim()}>{tr("Uložit", "Save")}</button>
        </>
      }
    >
      <div className="col">
        <div className="field">
          <label>{tr("Název", "Name")}</label>
          <input className="input" placeholder={tr("Můj server", "My server")} value={s.name ?? ""} onChange={(e) => setS({ ...s, name: e.target.value })} />
        </div>
        <div className="field">
          <label>{tr("Adresa", "Address")}</label>
          <input className="input mono" placeholder="play.server.cz" value={s.address ?? ""} onChange={(e) => setS({ ...s, address: e.target.value })} />
        </div>
        <div className="field">
          <label>{tr("Spouštět s instancí", "Launch with instance")}</label>
          <Select className="select" value={s.instance} onChange={(e) => setS({ ...s, instance: e.target.value })}>
            {instances.map((i) => (
              <option key={i.id} value={i.id}>
                {i.name} — {LOADER_LABEL[i.loader]} {i.mcVersion}
              </option>
            ))}
          </Select>
        </div>
        <label className="row" style={{ cursor: "pointer" }}>
          <input type="checkbox" checked={!!s.pinned} onChange={(e) => setS({ ...s, pinned: e.target.checked })} /> {tr("Připnout na hlavní stránku", "Pin to the home page")}
        </label>
      </div>
    </Modal>
  );
}

export default function Servers() {
  const { instances, play, settings, toast } = useStore();
  const [servers, setServers] = useState<Server[]>([]);
  const [status, setStatus] = useState<Record<string, ServerStatus | null>>({});
  const [edit, setEdit] = useState<Partial<Server> | null>(null);

  const load = () =>
    api.listServers().then((list) => {
      setServers(list);
      list.forEach((s) =>
        api.pingServer(s.address)
          .then((st) => setStatus((p) => ({ ...p, [s.id]: st })))
          .catch(() => setStatus((p) => ({ ...p, [s.id]: null }))),
      );
    });
  useEffect(() => {
    load();
  }, []);

  const join = (s: Server) => {
    const inst = instances.find((i) => i.id === s.instance) ?? instances[0];
    if (!inst) return toast(tr("Nejdřív si vytvoř instanci", "Create an instance first"), "error");
    play(inst.id, s.address);
  };

  return (
    <div className="page">
      <div className="page-head v2">
        <div>
          <h1>{tr("Servery", "Servers")}</h1>
          <div className="sub">{tr("Připojení jedním klikem s vybranou instancí", "One-click join with the chosen instance")}</div>
        </div>
        <div className="grow" />
        <button className="btn icon" onClick={load} title={tr("Obnovit", "Refresh")}>
          <Icon name="refresh" size={16} />
        </button>
        <button className="btn primary" onClick={() => setEdit({})}>
          <Icon name="plus" size={16} /> {tr("Přidat server", "Add server")}
        </button>
      </div>
      {servers.length === 0 ? (
        <Empty
          icon="server"
          title={tr("Žádné servery", "No servers")}
          text={tr(
            "Přidej si oblíbené servery — uvidíš počet hráčů a připojíš se jedním klikem.",
            "Add your favourite servers — see the player count and join with one click.",
          )}
        >
          <button className="btn primary" onClick={() => setEdit({})}>
            <Icon name="plus" size={16} /> {tr("Přidat server", "Add server")}
          </button>
        </Empty>
      ) : (
        <div className="srv-grid">
          {servers.map((s) => {
            const st = status[s.id];
            const inst = instances.find((i) => i.id === s.instance);
            const fill = st && st.max ? Math.min(100, (st.online / st.max) * 100) : 0;
            return (
              <div key={s.id} className={`srv-card ${st === null ? "off" : ""}`}>
                {st?.icon && <div className="srv-bg" style={{ backgroundImage: `url("${st.icon}")` }} />}
                <div className="srv-top">
                  <div className="srv-icon">{st?.icon ? <img src={st.icon} alt="" /> : <Icon name="server" size={24} />}</div>
                  <div className="grow" style={{ minWidth: 0 }}>
                    <div className="row" style={{ gap: 8 }}>
                      <b className="ellipsis">{s.name || s.address}</b>
                      {s.pinned && <Icon name="pin" size={13} style={{ color: "var(--red)", flex: "none" }} />}
                    </div>
                    <div className="small muted ellipsis">{settings?.streamerMode ? "••••••" : s.address}</div>
                  </div>
                  <span className={`srv-ping ${st ? "on" : ""}`}>{st ? `${st.pingMs} ms` : st === null ? "offline" : "…"}</span>
                </div>
                <div className="srv-motd">
                  {st ? <Motd text={st.motd} /> : <span className="small muted">{st === null ? tr("Server je offline nebo neodpovídá", "The server is offline or not responding") : tr("Pinguji…", "Pinging…")}</span>}
                </div>
                <div className="srv-players">
                  <div className="bar">
                    <i style={{ width: `${fill}%` }} />
                  </div>
                  <span>
                    {st ? `${st.online.toLocaleString()} / ${st.max.toLocaleString()}` : "—"} {tr("hráčů", "players")}
                    {st?.version ? ` · ${st.version}` : ""}
                  </span>
                </div>
                <div className="srv-foot">
                  <span className="chip2" title={tr("Instance pro připojení", "Instance used to join")}>
                    <Icon name="box" size={12} /> {inst ? inst.name : tr("První instance", "First instance")}
                  </span>
                  <div className="grow" />
                  <button className="btn sm ghost icon" onClick={() => setEdit(s)} title={tr("Upravit", "Edit")}>
                    <Icon name="settings" size={15} />
                  </button>
                  <button className="btn sm ghost icon danger" onClick={() => api.removeServer(s.id).then(load)} title={tr("Odebrat", "Remove")}>
                    <Icon name="trash" size={15} />
                  </button>
                  <button className="btn primary sm" onClick={() => join(s)}>
                    <Icon name="play" size={12} /> {tr("Připojit", "Join")}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}
      {edit && <EditServer server={edit} onClose={() => setEdit(null)} onSaved={load} />}
    </div>
  );
}
