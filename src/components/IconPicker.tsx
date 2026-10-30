import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import { Icon, INSTANCE_ICONS } from "../icons";
import { tr } from "../i18n";
import { errText, useStore } from "../store";
import { InstIcon, Modal, parseIcon } from "./common";
import { McIcon, MC_ICONS } from "./McIcons";

const COLORS = ["#e11d38", "#f97316", "#eab308", "#22c55e", "#14b8a6", "#3b82f6", "#6366f1", "#a855f7", "#ec4899", "#64748b", "#18181b"];

type Tab = "icons" | "color" | "picture";

/**
 * Instance icon chooser: line or pixel icons, a tile colour, or a picture (own file or the
 * instance's newest screenshot). `instanceId` enables pictures (they're copied into the instance).
 */
export default function IconPicker({ value, template, instanceId, onChange, onClose }: { value: string; template?: string; instanceId?: string; onChange: (icon: string) => void; onClose: () => void }) {
  const { toast } = useStore();
  const [tab, setTab] = useState<Tab>("icons");
  const [draft, setDraft] = useState(value);
  const [busy, setBusy] = useState(false);
  const { base, color } = parseIcon(draft);
  const withColor = (b: string, c: string | null) => (c && !b.startsWith("img:") ? `${b}@${c}` : b);

  const picture = async (path: string) => {
    if (!instanceId) return;
    setBusy(true);
    try {
      const n = await api.setInstanceIcon(instanceId, path);
      setDraft(n.icon);
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };
  const pickFile = async () => {
    const f = await open({ filters: [{ name: tr("Obrázek", "Picture"), extensions: ["png", "jpg", "jpeg", "webp", "gif"] }], title: tr("Ikona instance", "Instance icon") });
    if (typeof f === "string") picture(f);
  };
  const fromScreenshot = async () => {
    if (!instanceId) return;
    const p = await api.lastPlace(instanceId).catch(() => null);
    if (!p?.image) return toast(tr("Tahle instance zatím nemá žádný screenshot", "This instance has no screenshot yet"), "info");
    picture(p.image);
  };

  return (
    <Modal
      title={tr("Ikona instance", "Instance icon")}
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={onClose}>
            {tr("Zrušit", "Cancel")}
          </button>
          <button
            className="btn primary"
            disabled={busy}
            onClick={() => {
              onChange(draft);
              onClose();
            }}
          >
            {tr("Použít", "Apply")}
          </button>
        </>
      }
    >
      <div className="ip">
        <div className="ip-preview">
          <InstIcon inst={{ icon: draft, template: template ?? "" }} size={84} />
          <div className="seg">
            <button className={tab === "icons" ? "on" : ""} onClick={() => setTab("icons")}>
              {tr("Ikony", "Icons")}
            </button>
            <button className={tab === "color" ? "on" : ""} onClick={() => setTab("color")} disabled={base.startsWith("img:")}>
              {tr("Barva", "Colour")}
            </button>
            {instanceId && (
              <button className={tab === "picture" ? "on" : ""} onClick={() => setTab("picture")}>
                {tr("Obrázek", "Picture")}
              </button>
            )}
          </div>
        </div>

        {tab === "icons" && (
          <>
            <div className="ip-head">{tr("Minecraft", "Minecraft")}</div>
            <div className="ip-grid">
              {MC_ICONS.map((n) => (
                <button key={n} className={`ip-cell ${base === `mc:${n}` ? "on" : ""}`} onClick={() => setDraft(withColor(`mc:${n}`, color))} title={n}>
                  <McIcon name={n} size={26} />
                </button>
              ))}
            </div>
            <div className="ip-head">{tr("Jednoduché", "Simple")}</div>
            <div className="ip-grid">
              {INSTANCE_ICONS.map((n) => (
                <button key={n} className={`ip-cell ${base === n ? "on" : ""}`} onClick={() => setDraft(withColor(n, color))} title={n}>
                  <Icon name={n} size={20} />
                </button>
              ))}
            </div>
          </>
        )}

        {tab === "color" && (
          <div className="ip-colors">
            <button className={`ip-color none ${!color ? "on" : ""}`} onClick={() => setDraft(base)} title={tr("Bez barvy", "No colour")}>
              <Icon name="x" size={14} />
            </button>
            {COLORS.map((c) => (
              <button key={c} className={`ip-color ${color === c ? "on" : ""}`} style={{ background: c }} onClick={() => setDraft(withColor(base, c))} />
            ))}
            <label className="ip-color custom" title={tr("Vlastní barva", "Custom colour")}>
              <input type="color" value={color ?? "#e11d38"} onChange={(e) => setDraft(withColor(base, e.target.value))} />
              <Icon name="palette" size={14} />
            </label>
          </div>
        )}

        {tab === "picture" && instanceId && (
          <div className="ip-picture">
            <button className="ip-card" onClick={pickFile} disabled={busy}>
              <Icon name="upload" size={20} />
              <b>{tr("Vlastní obrázek", "Your own picture")}</b>
              <span>PNG, JPG, WEBP, GIF</span>
            </button>
            <button className="ip-card" onClick={fromScreenshot} disabled={busy}>
              <Icon name="camera" size={20} />
              <b>{tr("Poslední screenshot", "Latest screenshot")}</b>
              <span>{tr("Z téhle instance", "From this instance")}</span>
            </button>
          </div>
        )}
      </div>
    </Modal>
  );
}
