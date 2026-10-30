import { useEffect, useState, type ReactNode } from "react";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, type Account, type Instance } from "../api";
import { Icon, instanceIcon } from "../icons";
import { errText, useStore } from "../store";
import { McIcon } from "./McIcons";
import { tr } from "../i18n";

/** Face (with the hat layer) cut out of a skin, as a data URL. */
function skinFace(data: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      const c = document.createElement("canvas");
      c.width = c.height = 8;
      const g = c.getContext("2d")!;
      g.drawImage(img, 8, 8, 8, 8, 0, 0, 8, 8);
      g.drawImage(img, 40, 8, 8, 8, 0, 0, 8, 8);
      resolve(c.toDataURL());
    };
    img.onerror = reject;
    img.src = data;
  });
}

export function Avatar({ account, size = 22 }: { account: Pick<Account, "uuid" | "username" | "kind"> & { skinUrl?: string }; size?: number }) {
  const [broken, setBroken] = useState(false);
  const [face, setFace] = useState<string | null>(null);
  // Offline accounts: the face of their Justice skin (by name it would be a premium player's head).
  const justiceSkin = account.kind !== "msa" ? account.skinUrl : undefined;
  useEffect(() => {
    setFace(null);
    if (!justiceSkin) return;
    api.fetchImage(justiceSkin).then(skinFace).then(setFace).catch(() => {});
  }, [justiceSkin]);
  const id = account.kind === "msa" ? account.uuid : account.username;
  if (face) return <img className="avatar" style={{ width: size, height: size, imageRendering: "pixelated" }} src={face} alt="" />;
  if (broken) return <span className="avatar" style={{ width: size, height: size }} />;
  return (
    <img
      className="avatar"
      style={{ width: size, height: size }}
      src={`https://mc-heads.net/avatar/${id}/${Math.max(32, size * 2)}`}
      onError={() => setBroken(true)}
      alt=""
    />
  );
}

/**
 * Instance icon string: "sword" (line icon), "mc:diamond" (pixel icon) or "img:<path>" (picture,
 * see set_instance_icon), optionally followed by "@#rrggbb", the tile's background colour.
 */
export function parseIcon(icon: string): { base: string; color: string | null } {
  const at = icon.lastIndexOf("@#");
  return at > 0 && !icon.startsWith("img:") ? { base: icon.slice(0, at), color: icon.slice(at + 1) } : { base: icon, color: null };
}

export function customIconSrc(icon: string): string | null {
  return icon.startsWith("img:") ? convertFileSrc(icon.slice(4)) : null;
}

/** Just the glyph (picture, pixel icon or line icon), no tile. */
export function InstGlyph({ icon, template, size = 20 }: { icon: string; template?: string; size?: number }) {
  const { base } = parseIcon(icon);
  const src = customIconSrc(base);
  if (src) return <img className="custom" src={src} alt="" />;
  if (base.startsWith("mc:")) return <McIcon name={base.slice(3)} size={Math.round(size * 1.15)} />;
  return <Icon name={instanceIcon(base, template)} size={size} />;
}

export function InstIcon({ inst, size = 42, iconSize }: { inst: Pick<Instance, "icon" | "template">; size?: number; iconSize?: number }) {
  const { base, color } = parseIcon(inst.icon);
  const src = customIconSrc(base);
  const style: React.CSSProperties = { width: size, height: size, padding: src ? 0 : undefined };
  if (color) {
    style.background = `linear-gradient(135deg, ${color}, color-mix(in srgb, ${color} 55%, #000))`;
    style.color = "#fff";
  }
  return (
    <div className={`ico ${color ? "tinted" : ""}`} style={style}>
      <InstGlyph icon={inst.icon} template={inst.template} size={iconSize ?? Math.round(size * 0.5)} />
    </div>
  );
}

export function Switch({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="switch">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span />
    </label>
  );
}

export function Modal({ title, onClose, children, footer, wide }: { title: ReactNode; onClose: () => void; children: ReactNode; footer?: ReactNode; wide?: boolean }) {
  useEffect(() => {
    const k = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onClose]);
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`modal ${wide ? "wide" : ""}`}>
        <div className="modal-head">
          <h2>{title}</h2>
          <button className="btn ghost sm icon" onClick={onClose} aria-label={tr("Zavřít", "Close")}>
            <Icon name="x" size={16} />
          </button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>
  );
}

export function ProgressBar({ done, total }: { done: number; total: number }) {
  const indet = total <= 1 && done === 0;
  const pct = total > 0 ? Math.min(100, (done / total) * 100) : 0;
  return (
    <div className={`progress ${indet ? "indet" : ""}`}>
      <div style={{ width: `${pct}%` }} />
    </div>
  );
}

export function TaskProgress({ task }: { task: string }) {
  const { progress } = useStore();
  const p = progress[task];
  if (!p) return null;
  return (
    <div className="col" style={{ gap: 6 }}>
      <div className="row small muted">
        <span className="grow ellipsis">{p.label}…</span>
        {p.total > 1 && (
          <span>
            {p.total > 100000 ? `${(p.done / 1048576).toFixed(0)} / ${(p.total / 1048576).toFixed(0)} MB` : `${p.done} / ${p.total}`}
          </span>
        )}
      </div>
      <ProgressBar done={p.done} total={p.total} />
    </div>
  );
}

export function PlayButton({ inst, big, server }: { inst: Instance; big?: boolean; server?: string }) {
  const { running, progress, play, toast } = useStore();
  const isRunning = running.includes(inst.id);
  const busy = !!progress[inst.id];
  if (isRunning)
    return (
      <button
        className={`btn ${big ? "play" : "sm"}`}
        onClick={(e) => {
          e.stopPropagation();
          api.kill(inst.id).catch((err) => toast(errText(err), "error"));
        }}
      >
        <Icon name="stop" size={big ? 16 : 12} /> {tr("Zastavit", "Stop")}
      </button>
    );
  return (
    <button
      className={`btn primary ${big ? "play" : "sm"}`}
      disabled={busy}
      onClick={(e) => {
        e.stopPropagation();
        play(inst.id, server);
      }}
    >
      {busy ? <span className="spinner" /> : <Icon name="play" size={big ? 18 : 12} />}
      {tr("Hrát", "Play")}
    </button>
  );
}

export function Toasts() {
  const { toasts } = useStore();
  return (
    <div className="toasts">
      {toasts.map((x) => (
        <div key={x.id} className={`toast ${x.kind} ${x.icon ? "has-icon" : ""}`}>
          {x.icon && <McIcon name={x.icon} size={22} />}
          <span>{x.text}</span>
        </div>
      ))}
    </div>
  );
}

export async function openFolder(path: string) {
  try {
    await openPath(path);
  } catch {
    await revealItemInDir(path);
  }
}

export function CrashDialog() {
  const { crash, clearCrash, instances, toast, go, play } = useStore();
  const [repairing, setRepairing] = useState(false);
  if (!crash?.crash) return null;
  const c = crash.crash;
  const inst = instances.find((i) => i.id === crash.instance);
  const canRepair = inst && (inst.loader === "fabric" || inst.loader === "quilt");
  const repair = async () => {
    if (!inst) return;
    setRepairing(true);
    try {
      const r = await api.repairCrash(inst.id);
      if (r.fixed.length) {
        toast(`${tr("Opraveno", "Fixed")}:\n${r.fixed.join("\n")}`, "success");
        clearCrash();
        play(inst.id);
      } else {
        toast(tr("Automatická oprava nic nenašla. Zkus vypnout podezřelý mod v záložce Obsah.", "The automatic fix found nothing. Try disabling the suspected mod in the Content tab."), "error");
      }
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setRepairing(false);
    }
  };
  const copyLog = async () => {
    const lines = await api.logs(crash.instance);
    await navigator.clipboard.writeText(lines.join("\n"));
    toast(tr("Log zkopírován", "Log copied"), "success");
  };
  return (
    <Modal
      title={
        <span className="row">
          <Icon name="alert" style={{ color: "var(--red)" }} /> {c.title}
        </span>
      }
      onClose={clearCrash}
      footer={
        <>
          <button className="btn ghost" onClick={copyLog}>
            <Icon name="copy" size={16} /> {tr("Kopírovat log", "Copy log")}
          </button>
          {c.crashFile && (
            <button className="btn ghost" onClick={() => openFolder(c.crashFile!)}>
              <Icon name="external" size={16} /> {tr("Záznam o pádu", "Crash report")}
            </button>
          )}
          <button
            className="btn"
            onClick={() => {
              clearCrash();
              go({ page: "instance", id: crash.instance, tab: "content" });
            }}
          >
            {tr("Upravit mody", "Edit mods")}
          </button>
          {canRepair ? (
            <button className="btn primary" onClick={repair} disabled={repairing}>
              {repairing ? <span className="spinner" /> : <Icon name="wrench" size={16} />} {tr("Automaticky opravit", "Fix automatically")}
            </button>
          ) : (
            inst && (
              <button
                className="btn primary"
                onClick={() => {
                  clearCrash();
                  play(inst.id);
                }}
              >
                <Icon name="refresh" size={16} /> {tr("Zkusit znovu", "Try again")}
              </button>
            )
          )}
        </>
      }
    >
      <div className="col">
        <p style={{ margin: 0, color: "var(--text-2)" }}>{c.detail}</p>
        {c.suspect && (
          <div className="callout red">
            <Icon name="puzzle" size={18} />
            <div>
              {tr("Podezřelý mod", "Suspected mod")}: <b style={{ color: "var(--text)" }}>{c.suspect}</b>
            </div>
          </div>
        )}
        {c.excerpt.length > 0 && (
          <div className="logbox" style={{ height: 180 }}>
            {c.excerpt.join("\n")}
          </div>
        )}
        <div className="small muted">
          {inst?.name} · {tr("kód ukončení", "exit code")} {crash.code ?? "?"} · {tr("běželo", "ran for")} {crash.seconds} s
        </div>
      </div>
    </Modal>
  );
}

export function useAsync<T>(fn: () => Promise<T>, deps: unknown[]): [T | undefined, boolean, () => void, string | null] {
  const [data, setData] = useState<T>();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [n, setN] = useState(0);
  useEffect(() => {
    let alive = true;
    setLoading(true);
    fn()
      .then((d) => alive && (setData(d), setError(null)))
      .catch((e) => alive && setError(errText(e)))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, n]);
  return [data, loading, () => setN((x) => x + 1), error];
}

export function Empty({ icon, title, text, children }: { icon: string; title: string; text?: string; children?: ReactNode }) {
  return (
    <div className="empty">
      <Icon name={icon} size={46} stroke={1.5} />
      <b>{title}</b>
      {text && <div>{text}</div>}
      {children && <div style={{ marginTop: 16 }}>{children}</div>}
    </div>
  );
}

/** Placeholder rows with a shimmer while a list loads. */
export function SkeletonList({ rows = 4 }: { rows?: number }) {
  return (
    <div className="list" aria-busy="true">
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="skel-row">
          <div className="skeleton" style={{ width: 36, height: 36, borderRadius: 9 }} />
          <div className="grow col" style={{ gap: 6 }}>
            <div className="skeleton" style={{ width: `${40 + ((i * 17) % 35)}%`, height: 12 }} />
            <div className="skeleton" style={{ width: `${25 + ((i * 11) % 30)}%`, height: 10 }} />
          </div>
        </div>
      ))}
    </div>
  );
}