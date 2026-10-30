import { useEffect, useRef, useState } from "react";
import { Icon } from "../icons";
import { fmtDuration, fmtSize, useStore } from "../store";
import { tr } from "../i18n";
import { api, type Download } from "../api";
import { ProgressBar } from "./common";

const active = (d: Download) => d.status === "running" || d.status === "paused";

function Row({ d }: { d: Download }) {
  const pct = d.total > 0 ? Math.min(100, Math.round((d.done / d.total) * 100)) : 0;
  const indet = d.status === "running" && d.total === 0;
  return (
    <div className={`dl-row ${d.status}`}>
      <div className="dl-top">
        <span className="dl-name ellipsis">{d.name}</span>
        <div className="dl-ctl">
          {d.status === "running" && (
            <button title={tr("Pozastavit", "Pause")} onClick={() => api.downloadPause(d.id)}><Icon name="pause" size={13} /></button>
          )}
          {d.status === "paused" && (
            <button title={tr("Pokračovat", "Resume")} onClick={() => api.downloadResume(d.id)}><Icon name="play" size={13} /></button>
          )}
          {active(d) && (
            <button title={tr("Zrušit", "Cancel")} className="danger" onClick={() => api.downloadCancel(d.id)}><Icon name="x" size={13} /></button>
          )}
        </div>
      </div>
      <ProgressBar done={indet ? 0 : d.done} total={indet ? 1 : Math.max(d.total, 1)} />
      <div className="dl-meta">
        {d.status === "running" && (
          <>
            <span>{d.total > 0 ? `${fmtSize(d.done)} / ${fmtSize(d.total)}` : fmtSize(d.done)}</span>
            {d.speed > 0 && <span>{fmtSize(d.speed)}/s</span>}
            {d.etaSecs > 0 && <span>{tr("zbývá", "left")} {fmtDuration(d.etaSecs)}</span>}
          </>
        )}
        {d.status === "paused" && <span>{tr("Pozastaveno", "Paused")} · {pct}%</span>}
        {d.status === "done" && <span className="ok">{tr("Hotovo", "Done")}</span>}
        {d.status === "canceled" && <span className="muted">{tr("Zrušeno", "Canceled")}</span>}
        {d.status === "error" && <span className="err" title={d.error ?? ""}>{tr("Chyba", "Error")}</span>}
      </div>
    </div>
  );
}

/** Title-bar pill with the download queue (modpacks): progress, speed, ETA, pause/cancel. */
export default function DownloadsPill() {
  const { downloads } = useStore();
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => { if (box.current && !box.current.contains(e.target as Node)) setOpen(false); };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [open]);

  if (downloads.length === 0) return null;
  const running = downloads.filter((d) => d.status === "running");
  const anyActive = downloads.some(active);
  const totalSpeed = running.reduce((s, d) => s + d.speed, 0);
  // Aggregate percentage across downloads that know their size.
  const sized = downloads.filter((d) => active(d) && d.total > 0);
  const aggPct = sized.length ? Math.round((sized.reduce((s, d) => s + d.done, 0) / sized.reduce((s, d) => s + d.total, 0)) * 100) : null;

  return (
    <div className="dl-pill-wrap" ref={box}>
      <button className={`dl-pill ${anyActive ? "busy" : ""}`} onClick={() => setOpen((o) => !o)}>
        <Icon name="download" size={13} />
        {anyActive ? (
          <>
            {tr("Stahování", "Downloads")}
            {aggPct !== null && <span className="dl-pct">{aggPct}%</span>}
            {totalSpeed > 0 && <span className="dl-sp">{fmtSize(totalSpeed)}/s</span>}
          </>
        ) : (
          tr("Stahování", "Downloads")
        )}
        {downloads.length > 1 && <span className="dl-count">{downloads.length}</span>}
      </button>
      {open && (
        <div className="dl-pop">
          <div className="dl-pop-head">
            <b>{tr("Stahování", "Downloads")}</b>
            {downloads.some((d) => !active(d)) && (
              <button className="dl-clear" onClick={() => api.downloadClear()}>{tr("Uklidit hotové", "Clear finished")}</button>
            )}
          </div>
          <div className="dl-list">
            {downloads.map((d) => <Row key={d.id} d={d} />)}
          </div>
        </div>
      )}
    </div>
  );
}
