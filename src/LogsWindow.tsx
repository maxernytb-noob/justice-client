import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, type Instance } from "./api";
import { Icon } from "./icons";
import { tr } from "./i18n";

// Standalone live-log window (its own OS window, like Lunar Client's). The launcher pops it on
// launch (Settings → Hra → log window toggle) or on demand. It streams the same `game-log` events
// the in-app Log tab uses, but outside the app, so it can sit on a second monitor while you play.

const LEVEL = (l: string) => (/error|exception|fatal|crash/i.test(l) ? "err" : /\bwarn/i.test(l) ? "warn" : "");

async function openFolder(path: string) {
  try {
    await openPath(path);
  } catch {
    await revealItemInDir(path);
  }
}

export default function LogsWindow() {
  const [names, setNames] = useState<Record<string, string>>({});
  const [order, setOrder] = useState<string[]>([]); // instances seen this session, newest launch last
  const [running, setRunning] = useState<Set<string>>(new Set());
  const [logs, setLogs] = useState<Record<string, string[]>>({});
  const [active, setActive] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [follow, setFollow] = useState(true);
  const box = useRef<HTMLDivElement>(null);
  const win = useMemo(() => getCurrentWindow(), []);

  const see = (id: string) => setOrder((o) => (o.includes(id) ? o : [...o, id]));
  const loadLogs = (id: string) =>
    api.logs(id).then((l) => setLogs((m) => ({ ...m, [id]: l }))).catch(() => {});

  useEffect(() => {
    api.listInstances().then((xs: Instance[]) => setNames(Object.fromEntries(xs.map((i) => [i.id, i.name])))).catch(() => {});
    api.running().then((ids) => {
      setRunning(new Set(ids));
      ids.forEach((id) => { see(id); loadLogs(id); });
      setActive((a) => a ?? ids[0] ?? null);
    }).catch(() => {});

    const subs = [
      listen<string>("game-start", ({ payload }) => {
        see(payload);
        setRunning((r) => new Set(r).add(payload));
        setLogs((m) => ({ ...m, [payload]: [] }));
        setActive(payload);
      }),
      listen<{ instance: string }>("game-exit", ({ payload }) => {
        setRunning((r) => { const n = new Set(r); n.delete(payload.instance); return n; });
      }),
      listen<{ instance: string; line: string }>("game-log", ({ payload }) => {
        see(payload.instance);
        setLogs((m) => {
          const prev = m[payload.instance] ?? [];
          const next = prev.length > 3000 ? [...prev.slice(-2500), payload.line] : [...prev, payload.line];
          return { ...m, [payload.instance]: next };
        });
        setActive((a) => a ?? payload.instance);
      }),
    ];
    return () => { subs.forEach((s) => s.then((f) => f())); };
  }, []);

  const lines = active ? logs[active] ?? [] : [];
  const shown = filter ? lines.filter((l) => l.toLowerCase().includes(filter.toLowerCase())) : lines;

  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [shown, follow, active]);

  const name = (id: string) => names[id] ?? id;
  const copyLog = () => navigator.clipboard.writeText(lines.join("\n")).catch(() => {});
  const openDir = async () => { if (active) openFolder(await api.instancePath(active, "logs")); };

  return (
    <div className="logswin">
      <header className="lw-bar" data-tauri-drag-region>
        <div className="lw-brand" data-tauri-drag-region>
          <span className="lw-dot" />
          <strong>Justice</strong> <span className="muted">· {tr("Logy", "Logs")}</span>
        </div>
        <div className="lw-grow" data-tauri-drag-region />
        <div className="lw-winbtns">
          <button onClick={() => win.minimize()} aria-label={tr("Minimalizovat", "Minimize")}><Icon name="minus" size={15} /></button>
          <button className="close" onClick={() => win.close()} aria-label={tr("Zavřít", "Close")}><Icon name="x" size={15} /></button>
        </div>
      </header>

      <div className="lw-tabs">
        {order.length === 0 && <span className="muted small">{tr("Log se objeví, až spustíš hru.", "Logs show up once a game starts.")}</span>}
        {order.map((id) => (
          <button key={id} className={`lw-tab ${id === active ? "on" : ""}`} onClick={() => setActive(id)}>
            <span className={`lw-run ${running.has(id) ? "live" : "off"}`} />
            {name(id)}
          </button>
        ))}
      </div>

      <div className="lw-tools">
        <div className="search grow">
          <Icon name="search" size={14} />
          <input className="input" placeholder={tr("Filtrovat…", "Filter…")} value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
        <label className="lw-follow" title={tr("Rolovat za novými řádky", "Scroll with new lines")}>
          <input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} /> {tr("Sledovat", "Follow")}
        </label>
        <button className="btn" onClick={copyLog} disabled={!lines.length} title={tr("Kopírovat", "Copy")}><Icon name="copy" size={15} /></button>
        <button className="btn" onClick={openDir} disabled={!active} title={tr("Složka s logy", "Logs folder")}><Icon name="folder" size={15} /></button>
      </div>

      <div className="logbox lw-box" ref={box}>
        {shown.length === 0 ? (
          <span className="muted">{active && running.has(active) ? tr("Čekám na výstup hry…", "Waiting for game output…") : tr("Zatím nic.", "Nothing yet.")}</span>
        ) : (
          shown.map((l, i) => <div key={i} className={LEVEL(l)}>{l}</div>)
        )}
      </div>
    </div>
  );
}
