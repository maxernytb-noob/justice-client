import { useEffect, useMemo, useState } from "react";
import { api, type FixReport, type RepairAction, type RepairPlan } from "../api";
import { Icon } from "../icons";
import { tr } from "../i18n";
import { errText, useStore } from "../store";
import { sound } from "../sounds";
import { Modal, TaskProgress } from "./common";

const GROUPS: { key: RepairAction["group"]; icon: string; title: [string, string] }[] = [
  { key: "crash", icon: "alert", title: ["Poslední pád", "Last crash"] },
  { key: "duplicate", icon: "copy", title: ["Duplicitní mody", "Duplicate mods"] },
  { key: "missing", icon: "download", title: ["Chybějící závislosti", "Missing dependencies"] },
  { key: "conflict", icon: "wrench", title: ["Konflikty verzí", "Version conflicts"] },
];

/** The Compatibility button's dialog: one guided pass over mods + the last crash, with preview and undo. */
export default function RepairModal({ instance, onClose, onDone }: { instance: string; onClose: () => void; onDone: () => void }) {
  const { toast } = useStore();
  const [plan, setPlan] = useState<RepairPlan | null>(null);
  const [off, setOff] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<FixReport | null>(null);

  const load = () => {
    setPlan(null);
    api
      .repairPlan(instance)
      .then((p) => { setPlan(p); setOff(new Set(p.actions.filter((a) => !a.checked).map((a) => a.id))); })
      .catch((e) => { toast(errText(e), "error"); onClose(); });
  };
  useEffect(load, [instance]);

  const chosen = useMemo(() => (plan ? plan.actions.filter((a) => !off.has(a.id)).map((a) => a.id) : []), [plan, off]);
  const toggle = (id: string) =>
    setOff((s) => { const n = new Set(s); if (n.has(id)) n.delete(id); else n.add(id); return n; });

  const apply = async () => {
    setBusy(true);
    try {
      const r = await api.repairApply(instance, chosen);
      setReport(r);
      sound(r.remaining.length ? "error" : "done");
      onDone();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const undo = async () => {
    setBusy(true);
    try {
      await api.repairUndo(instance);
      toast(tr("Poslední oprava je vrácená", "The last repair has been undone"), "success");
      setReport(null);
      onDone();
      load();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const footer = report ? (
    <>
      <button className="btn ghost" onClick={undo} disabled={busy} style={{ marginRight: "auto" }}>
        <Icon name="undo" size={15} /> {tr("Vrátit zpět", "Undo")}
      </button>
      <button className="btn primary" onClick={onClose}>{tr("Hotovo", "Done")}</button>
    </>
  ) : (
    <>
      {plan?.undoAvailable && (
        <button className="btn ghost" onClick={undo} disabled={busy} style={{ marginRight: "auto" }}>
          <Icon name="undo" size={15} /> {tr("Vrátit minulou opravu", "Undo last repair")}
        </button>
      )}
      <button className="btn ghost" onClick={onClose} disabled={busy}>{tr("Zrušit", "Cancel")}</button>
      <button className="btn primary" onClick={apply} disabled={busy || !plan || chosen.length === 0}>
        <Icon name="wrench" size={15} /> {busy ? tr("Opravuju…", "Repairing…") : tr(`Opravit (${chosen.length})`, `Repair (${chosen.length})`)}
      </button>
    </>
  );

  return (
    <Modal title={tr("Oprava instance", "Repair instance")} onClose={busy ? () => {} : onClose} footer={footer}>
      {!plan ? (
        <div className="opt-loading">
          <span className="spin" /> {tr("Procházím mody a poslední log…", "Checking mods and the last log…")}
        </div>
      ) : report ? (
        <div className="opt-report">
          <div className="opt-done">
            <Icon name={report.remaining.length ? "alert" : "check"} size={22} />
            <div>
              <b>{report.remaining.length ? tr("Z větší části opraveno", "Mostly fixed") : tr("Opraveno", "Fixed")}</b>
              <span>{tr(`Provedeno kroků: ${report.fixed.length}`, `Steps done: ${report.fixed.length}`)}</span>
            </div>
          </div>
          {report.fixed.length > 0 && (
            <ul className="rep-list">
              {report.fixed.map((f, i) => (
                <li key={i}><Icon name="check" size={13} /> {f}</li>
              ))}
            </ul>
          )}
          {report.remaining.length > 0 && (
            <div className="rep-remain">
              <b>{tr("Zůstalo nevyřešené:", "Still unresolved:")}</b>
              {report.remaining.map((c, i) => <div key={i} className="small">{c.text}</div>)}
            </div>
          )}
          <p className="small muted">{tr("Před opravou se uložila záloha modů, takže všechno jde vrátit.", "A backup of the mods was saved first, so everything can be undone.")}</p>
        </div>
      ) : (
        <div className="opt">
          {!plan.supported && (
            <div className="opt-empty">
              <Icon name="info" size={18} />
              {tr("Automatická oprava umí jen Fabric a Quilt. U téhle instance ukážu jen rozbor pádu.", "Automatic repair only works for Fabric and Quilt. For this instance I'll only show the crash analysis.")}
            </div>
          )}
          {plan.crash && (
            <div className="rep-crash">
              <div className="rep-crash-head">
                <Icon name="alert" size={16} />
                <b>{plan.crash.title}</b>
              </div>
              <p className="small">{plan.crash.detail}</p>
              {plan.crash.suspect && (
                <p className="small rep-suspect">{tr("Podezřelý mod:", "Suspected mod:")} <b>{plan.crash.suspect}</b></p>
              )}
            </div>
          )}
          {busy && <TaskProgress task={instance} />}
          {plan.actions.length === 0 ? (
            <div className="opt-empty">
              <Icon name="check" size={20} />
              {tr("Nic k opravě – mody vypadají v pořádku.", "Nothing to fix – the mods look healthy.")}
            </div>
          ) : (
            GROUPS.filter((g) => plan.actions.some((a) => a.group === g.key)).map((g) => (
              <section key={g.key} className="opt-group">
                <h4><Icon name={g.icon} size={14} /> {tr(...g.title)}</h4>
                {plan.actions.filter((a) => a.group === g.key).map((a) => (
                  <label key={a.id} className={`opt-row ${off.has(a.id) ? "off" : ""}`}>
                    <input type="checkbox" checked={!off.has(a.id)} onChange={() => toggle(a.id)} disabled={busy} />
                    <span className={`rep-dot ${a.severity}`} />
                    <span className="opt-text">
                      <b>{a.title}</b>
                      {a.detail && <small>{a.detail}</small>}
                    </span>
                  </label>
                ))}
              </section>
            ))
          )}
        </div>
      )}
    </Modal>
  );
}
