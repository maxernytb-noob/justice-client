import { useEffect, useMemo, useState } from "react";
import { api, type OptimizePlan, type OptimizeReport } from "../api";
import { Icon } from "../icons";
import { tr } from "../i18n";
import { errText, useStore } from "../store";
import { sound } from "../sounds";
import { Modal, TaskProgress } from "./common";

const GROUPS: { key: OptimizePlan["changes"][number]["group"]; icon: string; title: [string, string] }[] = [
  { key: "mods", icon: "zap", title: ["Výkonové mody", "Performance mods"] },
  { key: "conflicts", icon: "alert", title: ["Konflikty", "Conflicts"] },
  { key: "video", icon: "monitor", title: ["Nastavení grafiky", "Video settings"] },
  { key: "sodium", icon: "cpu", title: ["Sodium", "Sodium"] },
  { key: "memory", icon: "box", title: ["Paměť", "Memory"] },
];

const TIER: Record<OptimizePlan["tier"], [string, string]> = {
  low: ["Slabší PC", "Low-end PC"],
  mid: ["Střední PC", "Mid-range PC"],
  high: ["Výkonné PC", "High-end PC"],
};

/** The Optimize button's dialog: analysis of the instance, ticked changes, apply, undo. */
export default function OptimizeModal({ instance, onClose, onDone }: { instance: string; onClose: () => void; onDone: () => void }) {
  const { toast } = useStore();
  const [plan, setPlan] = useState<OptimizePlan | null>(null);
  const [off, setOff] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<OptimizeReport | null>(null);

  const load = () => {
    setPlan(null);
    api
      .optimizePlan(instance)
      .then((p) => {
        setPlan(p);
        setOff(new Set());
      })
      .catch((e) => {
        toast(errText(e), "error");
        onClose();
      });
  };
  useEffect(load, [instance]);

  const chosen = useMemo(() => (plan ? plan.changes.filter((c) => !off.has(c.id)).map((c) => c.id) : []), [plan, off]);
  const toggle = (id: string) =>
    setOff((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  const apply = async () => {
    setBusy(true);
    try {
      const r = await api.optimizeApply(instance, chosen);
      setReport(r);
      sound("done");
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
      await api.optimizeUndo(instance);
      toast(tr("Změny z poslední optimalizace jsou vrácené", "The last optimization has been undone"), "success");
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
      <button className="btn ghost" onClick={undo} disabled={busy}>
        <Icon name="undo" size={15} /> {tr("Vrátit zpět", "Undo")}
      </button>
      <button className="btn primary" onClick={onClose}>
        {tr("Hotovo", "Done")}
      </button>
    </>
  ) : (
    <>
      {plan?.undoAvailable && (
        <button className="btn ghost" onClick={undo} disabled={busy} style={{ marginRight: "auto" }}>
          <Icon name="undo" size={15} /> {tr("Vrátit minulou optimalizaci", "Undo last optimization")}
        </button>
      )}
      <button className="btn ghost" onClick={onClose} disabled={busy}>
        {tr("Zrušit", "Cancel")}
      </button>
      <button className="btn primary" onClick={apply} disabled={busy || !plan || chosen.length === 0}>
        <Icon name="zap" size={15} /> {busy ? tr("Optimalizuju…", "Optimizing…") : tr(`Použít (${chosen.length})`, `Apply (${chosen.length})`)}
      </button>
    </>
  );

  return (
    <Modal title={tr("Optimalizace instance", "Optimize instance")} onClose={busy ? () => {} : onClose} footer={footer}>
      {!plan ? (
        <div className="opt-loading">
          <span className="spin" /> {tr("Zkoumám instanci a tvoje PC…", "Looking at the instance and your PC…")}
        </div>
      ) : report ? (
        <div className="opt-report">
          <div className="opt-done">
            <Icon name="check" size={22} />
            <div>
              <b>{tr("Hotovo", "Done")}</b>
              <span>{tr(`Použito změn: ${report.applied.length}`, `Changes applied: ${report.applied.length}`)}</span>
            </div>
          </div>
          {report.installed.length > 0 && (
            <p className="small">
              <b>{tr("Nainstalováno:", "Installed:")}</b> {report.installed.join(", ")}
            </p>
          )}
          {report.unavailable.length > 0 && (
            <p className="small muted">
              {tr("Pro tuhle verzi neexistuje:", "Not available for this version:")} {report.unavailable.join(", ")}
            </p>
          )}
          <p className="small muted">{tr("Před změnami se uložila záloha, takže všechno jde vrátit.", "A backup was saved first, so everything can be undone.")}</p>
        </div>
      ) : (
        <div className="opt">
          <div className="opt-hw">
            <Icon name="cpu" size={18} />
            <div>
              <b>{tr(...TIER[plan.tier])}</b>
              <span>{plan.hardware}</span>
            </div>
          </div>
          {busy && <TaskProgress task={instance} />}
          {plan.changes.length === 0 ? (
            <div className="opt-empty">
              <Icon name="check" size={20} />
              {tr("Tahle instance je už optimalizovaná. Není co zlepšit.", "This instance is already optimized. Nothing to improve.")}
            </div>
          ) : (
            GROUPS.filter((g) => plan.changes.some((c) => c.group === g.key)).map((g) => (
              <section key={g.key} className="opt-group">
                <h4>
                  <Icon name={g.icon} size={14} /> {tr(...g.title)}
                </h4>
                {plan.changes
                  .filter((c) => c.group === g.key)
                  .map((c) => (
                    <label key={c.id} className={`opt-row ${off.has(c.id) ? "off" : ""}`}>
                      <input type="checkbox" checked={!off.has(c.id)} onChange={() => toggle(c.id)} disabled={busy} />
                      <span className="opt-text">
                        <b>{c.title}</b>
                        {c.detail && <small>{c.detail}</small>}
                      </span>
                      {c.from && c.to && (
                        <span className="opt-diff">
                          <s>{c.from}</s>
                          <Icon name="arrow" size={12} />
                          <em>{c.to}</em>
                        </span>
                      )}
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
