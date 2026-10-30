import { useEffect, useRef, useState } from "react";
import { api, type Instance, type ModCard } from "../api";
import { Icon } from "../icons";
import { tr } from "../i18n";
import { errText, fmtNum, useStore } from "../store";
import { Modal } from "./common";
import Select from "./Select";

interface ChatMsg {
  role: "user" | "assistant";
  content: string;
  mods?: ModCard[];
  limited?: boolean;
}

const MC_VERSIONS = ["1.21.11", "1.21.10", "1.21.8", "1.21.4", "1.21.1", "1.20.6", "1.20.1", "1.19.4", "1.19.2", "1.18.2", "1.16.5", "1.12.2", "1.8.9"];
const SUGGESTIONS: [string, string][] = [
  ["Výkonový balíček pro slabší PC", "Performance pack for a weak PC"],
  ["Hezká grafika se shadery", "Pretty graphics with shaders"],
  ["Vychytávky a QoL mody", "Quality-of-life mods"],
  ["Mody na stavění", "Building mods"],
];

/** MaxPack: the modpack helper bot. Chat to it, it suggests real Modrinth mods you can build into a pack. */
export default function MaxPackChat({ instance, onClose, onBuilt }: { instance?: Instance; onClose: () => void; onBuilt?: (i: Instance) => void }) {
  const { toast } = useStore();
  const [mc, setMc] = useState(instance?.mcVersion ?? "1.21.11");
  const [loader, setLoader] = useState<string>(instance && instance.loader !== "vanilla" ? instance.loader : "fabric");
  const [msgs, setMsgs] = useState<ChatMsg[]>([
    {
      role: "assistant",
      content: tr(
        "Ahoj, já jsem MaxPack 👋 Řekni mi, jaký modpack chceš, a já ti poradím mody. Třeba „lehký výkonový balíček pro 1.21.11“ nebo „survival se shadery a minimapou“.",
        "Hey, I'm MaxPack 👋 Tell me what kind of modpack you want and I'll suggest mods. Try \"a light performance pack for 1.21.11\" or \"survival with shaders and a minimap\".",
      ),
    },
  ]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [building, setBuilding] = useState(false);
  const [picked, setPicked] = useState<Map<string, ModCard>>(new Map());
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    listRef.current?.scrollTo({ top: listRef.current.scrollHeight, behavior: "smooth" });
  }, [msgs, sending]);

  const send = async (text: string) => {
    const q = text.trim();
    if (!q || sending) return;
    setInput("");
    const history = [...msgs, { role: "user" as const, content: q }];
    setMsgs(history);
    setSending(true);
    try {
      const wire = history.filter((m) => m.role === "user" || m.role === "assistant").map((m) => ({ role: m.role, content: m.content }));
      const r = await api.modbotChat(mc, loader, wire);
      setMsgs((m) => [...m, { role: "assistant", content: r.reply, mods: r.mods, limited: r.limited }]);
      // Pre-tick every suggested mod.
      setPicked((p) => {
        const n = new Map(p);
        r.mods.forEach((mod) => n.set(mod.slug || mod.id, mod));
        return n;
      });
    } catch (e) {
      setMsgs((m) => [...m, { role: "assistant", content: tr(`Nepovedlo se se spojit: ${errText(e)}`, `Couldn't reach MaxPack: ${errText(e)}`) }]);
    } finally {
      setSending(false);
    }
  };

  const toggle = (mod: ModCard) =>
    setPicked((p) => {
      const n = new Map(p);
      const key = mod.slug || mod.id;
      if (n.has(key)) n.delete(key);
      else n.set(key, mod);
      return n;
    });

  const buildPack = async () => {
    if (picked.size === 0) return;
    setBuilding(true);
    const slugs = [...picked.values()].map((m) => m.slug || m.id);
    try {
      if (instance) {
        await api.modbotAdd(instance.id, slugs);
        toast(tr(`Přidáno ${slugs.length} modů do instance`, `Added ${slugs.length} mods to the instance`), "success");
        onClose();
      } else {
        const created = await api.modbotBuild(mc, loader, "", slugs);
        toast(tr(`Modpack sestaven: ${created.name}`, `Modpack built: ${created.name}`), "success");
        onBuilt?.(created);
        onClose();
      }
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBuilding(false);
    }
  };

  const footer = (
    <>
      <span className="muted small" style={{ marginRight: "auto" }}>
        {picked.size > 0 ? tr(`Vybráno ${picked.size} modů`, `${picked.size} mods selected`) : tr("Vyber mody z návrhů", "Pick mods from the suggestions")}
      </span>
      <button className="btn ghost" onClick={onClose} disabled={building}>{tr("Zavřít", "Close")}</button>
      <button className="btn primary" onClick={buildPack} disabled={building || picked.size === 0}>
        <Icon name="box" size={15} />{" "}
        {building
          ? tr("Sestavuju…", "Building…")
          : instance
            ? tr(`Přidat (${picked.size})`, `Add (${picked.size})`)
            : tr(`Sestavit modpack (${picked.size})`, `Build modpack (${picked.size})`)}
      </button>
    </>
  );

  return (
    <Modal
      title={
        <span className="row" style={{ gap: 8 }}>
          <span className="mp-badge"><Icon name="sparkles" size={15} /></span> MaxPack
        </span>
      }
      onClose={building ? () => {} : onClose}
      footer={footer}
    >
      <div className="mp-bar">
        <span className="muted small">{tr("Pro verzi", "For version")}</span>
        <Select className="select sm" value={mc} onChange={(e) => setMc(e.target.value)} disabled={!!instance}>
          {MC_VERSIONS.map((v) => <option key={v} value={v}>{v}</option>)}
        </Select>
        <Select className="select sm" value={loader} onChange={(e) => setLoader(e.target.value)} disabled={!!instance}>
          <option value="fabric">Fabric</option>
          <option value="quilt">Quilt</option>
          <option value="neoforge">NeoForge</option>
          <option value="forge">Forge</option>
        </Select>
      </div>

      <div className="mp-chat" ref={listRef}>
        {msgs.map((m, i) => (
          <div key={i} className={`mp-msg ${m.role}`}>
            {m.role === "assistant" && <span className="mp-ava"><Icon name="sparkles" size={13} /></span>}
            <div className="mp-bubble">
              <div className="mp-text">{m.content}</div>
              {m.limited && <div className="mp-note">{tr("Základní režim bez AI – pořád použitelné.", "Basic mode without AI – still usable.")}</div>}
              {m.mods && m.mods.length > 0 && (
                <div className="mp-cards">
                  {m.mods.map((mod) => {
                    const key = mod.slug || mod.id;
                    const on = picked.has(key);
                    return (
                      <button key={mod.id} className={`mp-card ${on ? "on" : ""}`} onClick={() => toggle(mod)}>
                        <div className="mp-ic" style={mod.iconUrl ? { backgroundImage: `url("${mod.iconUrl}")` } : undefined}>
                          {!mod.iconUrl && <Icon name="box" size={16} />}
                        </div>
                        <div className="mp-info">
                          <div className="mp-name">{mod.title} <span className="mp-dl">{fmtNum(mod.downloads)}</span></div>
                          <div className="mp-why ellipsis">{mod.reason || mod.description}</div>
                        </div>
                        <span className="mp-check"><Icon name={on ? "check" : "plus"} size={14} /></span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
        ))}
        {sending && (
          <div className="mp-msg assistant">
            <span className="mp-ava"><Icon name="sparkles" size={13} /></span>
            <div className="mp-bubble"><div className="mp-typing"><i /><i /><i /></div></div>
          </div>
        )}
      </div>

      {msgs.length <= 1 && (
        <div className="mp-sugg">
          {SUGGESTIONS.map((s) => (
            <button key={s[0]} className="mp-chip" onClick={() => send(tr(...s))}>{tr(...s)}</button>
          ))}
        </div>
      )}

      <form className="mp-composer" onSubmit={(e) => { e.preventDefault(); send(input); }}>
        <input
          className="input"
          placeholder={tr("Napiš, jaký modpack chceš…", "Describe the modpack you want…")}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          disabled={sending}
          autoFocus
        />
        <button type="submit" className="btn primary" disabled={sending || !input.trim()}><Icon name="right" size={16} /></button>
      </form>
    </Modal>
  );
}
