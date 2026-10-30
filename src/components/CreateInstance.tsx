import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type GameVersion, type Loader, type LoaderVersion } from "../api";
import { Icon, INSTANCE_ICONS } from "../icons";
import { errText, LOADER_LABEL, useStore } from "../store";
import { Modal, Switch, InstIcon } from "./common";
import { tr } from "../i18n";
import IconPicker from "./IconPicker";
import Select from "./Select";

const PACKS = [
  { id: "cinematic", icon: "film", title: "Cinematic", titleEn: "Cinematic", text: "Flashback, Iris, Distant Horizons, Freecam, voicechat", textEn: "Flashback, Iris, Distant Horizons, Freecam, voice chat" },
  { id: "smp", icon: "trees", title: "Survival SMP", titleEn: "Survival SMP", text: "Minimapa, Jade, AppleSkin, shadery, voicechat", textEn: "Minimap, Jade, AppleSkin, shaders, voice chat" },
  { id: "pvp", icon: "sword", title: "PvP", titleEn: "PvP", text: "Mody z UkuPvP: Armor HUD, TierTagger, minimapa, voicechat", textEn: "UkuPvP mods: Armor HUD, TierTagger, minimap, voice chat" },
  { id: "pvp18", icon: "gamepad", title: "1.8.9 PvP", titleEn: "1.8.9 PvP", text: "BedWars, SkyWars, Duels: 1.7 animace, Patcher, Keystrokes", textEn: "BedWars, SkyWars, Duels: 1.7 animations, Patcher, Keystrokes" },
];

export default function CreateInstance({ onClose }: { onClose: () => void }) {
  const { reloadInstances, go, toast } = useStore();
  const [mode, setMode] = useState<"new" | "import">("new");
  const [versions, setVersions] = useState<GameVersion[]>([]);
  const [snapshots, setSnapshots] = useState(false);
  const [name, setName] = useState("");
  const [icon, setIcon] = useState("");
  const [picking, setPicking] = useState(false);
  const [version, setVersion] = useState("");
  const [loader, setLoader] = useState<Loader>("fabric");
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersion[]>([]);
  const [loaderVersion, setLoaderVersion] = useState("");
  const [loaderErr, setLoaderErr] = useState("");
  const [packs, setPacks] = useState<string[]>(["cinematic"]);
  const [optimize, setOptimize] = useState(true);
  const [busy, setBusy] = useState(false);
  const [code, setCode] = useState("");

  useEffect(() => {
    api.gameVersions().then((v) => {
      setVersions(v);
      setVersion(v.find((x) => x.kind === "release")?.id ?? "");
    }).catch((e) => toast(errText(e), "error"));
  }, [toast]);

  useEffect(() => {
    setLoaderVersions([]);
    setLoaderVersion("");
    setLoaderErr("");
    if (loader === "vanilla" || !version) return;
    api.loaderVersions(loader, version)
      .then((l) => {
        setLoaderVersions(l);
        if (l.length === 0) setLoaderErr(tr(`${LOADER_LABEL[loader]} pro ${version} zatím neexistuje`, `${LOADER_LABEL[loader]} doesn't exist for ${version} yet`));
      })
      .catch(() => setLoaderErr(tr("Nepodařilo se načíst verze loaderu", "Couldn't load loader versions")));
  }, [loader, version]);

  const shown = useMemo(() => versions.filter((v) => (snapshots ? true : v.kind === "release")), [versions, snapshots]);
  const needsModLoader = packs.length > 0 || optimize;
  const modLoaderOk = !needsModLoader || loader !== "vanilla";

  const create = async () => {
    setBusy(true);
    try {
      const first = PACKS.find((p) => p.id === packs[0]);
      const label = packs.length ? PACKS.filter((p) => packs.includes(p.id)).map((p) => tr(p.title, p.titleEn)).join(" + ") : optimize ? "Max FPS" : LOADER_LABEL[loader];
      const inst = await api.createInstance(
        {
          name: name.trim() || `${label} ${version}`,
          icon: icon || first?.icon || (optimize ? "zap" : "box"),
          mcVersion: version,
          loader,
          loaderVersion,
        },
        loader === "vanilla" ? [] : packs,
        loader !== "vanilla" && optimize,
      );
      await reloadInstances();
      toast(tr(`Instance ${inst.name} je připravená`, `Instance ${inst.name} is ready`), "success");
      onClose();
      go({ page: "instance", id: inst.id });
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const importFile = async () => {
    const path = await open({ filters: [{ name: "Modpack", extensions: ["mrpack", "zip", "dawnpack"] }] });
    if (!path) return;
    setBusy(true);
    try {
      const inst = await api.importPack(path as string);
      await reloadInstances();
      toast(`${tr("Importováno", "Imported")}: ${inst.name}`, "success");
      onClose();
      go({ page: "instance", id: inst.id });
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const importCode = async () => {
    setBusy(true);
    try {
      const inst = await api.importCode(code);
      await reloadInstances();
      toast(`${tr("Importováno", "Imported")}: ${inst.name}`, "success");
      onClose();
      go({ page: "instance", id: inst.id });
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      wide
      title={tr("Nová instance", "New instance")}
      onClose={() => !busy && onClose()}
      footer={
        mode === "new" ? (
          <>
            {busy && <span className="muted small grow">{tr("Stahuji mody — může to chvíli trvat…", "Downloading mods — this can take a moment…")}</span>}
            <button className="btn ghost" onClick={onClose} disabled={busy}>
              {tr("Zrušit", "Cancel")}
            </button>
            <button className="btn primary" onClick={create} disabled={busy || !version || !modLoaderOk || (loader !== "vanilla" && !!loaderErr)}>
              {busy ? <span className="spinner" /> : <Icon name="plus" size={16} />} {tr("Vytvořit", "Create")}
            </button>
          </>
        ) : undefined
      }
    >
      <div className="tabs">
        <button className={`tab ${mode === "new" ? "on" : ""}`} onClick={() => setMode("new")}>
          {tr("Vytvořit novou", "Create new")}
        </button>
        <button className={`tab ${mode === "import" ? "on" : ""}`} onClick={() => setMode("import")}>
          {tr("Importovat", "Import")}
        </button>
      </div>

      {mode === "import" ? (
        <div className="col">
          <div className="card pad col">
            <b>{tr("Ze souboru", "From a file")}</b>
            <span className="muted small">{tr("Modrinth .mrpack, CurseForge .zip, Dawn Client .dawnpack nebo export z Justice Clientu.", "Modrinth .mrpack, CurseForge .zip, Dawn Client .dawnpack or a Justice Client export.")}</span>
            <div>
              <button className="btn" onClick={importFile} disabled={busy}>
                {busy ? <span className="spinner" /> : <Icon name="folder" size={16} />} {tr("Vybrat soubor", "Choose file")}
              </button>
            </div>
          </div>
          <div className="card pad col">
            <b>{tr("Ze sdíleného kódu", "From a share code")}</b>
            <span className="muted small">{tr("Kamarád ti poslal kód začínající JUSTICE1: — vlož ho sem a dostaneš stejné mody.", "A friend sent you a code starting with JUSTICE1: — paste it here to get the same mods.")}</span>
            <textarea className="input" rows={3} placeholder="JUSTICE1:..." value={code} onChange={(e) => setCode(e.target.value)} />
            <div>
              <button className="btn primary" onClick={importCode} disabled={busy || !code.trim()}>
                {busy ? <span className="spinner" /> : <Icon name="download" size={16} />} {tr("Importovat", "Import")}
              </button>
            </div>
          </div>
        </div>
      ) : (
        <div className="col" style={{ gap: 18 }}>
          <div className="field">
            <label>{tr("Balíčky modů — můžeš jich vybrat víc najednou", "Mod packs — you can pick several")}</label>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 8 }}>
              {PACKS.map((tp) => {
                const on = packs.includes(tp.id);
                return (
                  <button
                    key={tp.id}
                    className="card"
                    onClick={() => {
                      if (tp.id === "pvp18") {
                        // 1.8.9 PvP is its own world: Forge 1.8.9, can't mix with the modern Fabric packs.
                        setPacks(on ? [] : ["pvp18"]);
                        if (!on) {
                          setVersion("1.8.9");
                          setLoader("forge");
                        }
                        return;
                      }
                      setPacks((p) => (on ? p.filter((x) => x !== tp.id) : [...p.filter((x) => x !== "pvp18"), tp.id]));
                      if (!on && (loader === "vanilla" || (packs.includes("pvp18") && loader === "forge"))) setLoader("fabric");
                      if (!on && packs.includes("pvp18")) setVersion(versions.find((x) => x.kind === "release")?.id ?? version);
                    }}
                    style={{
                      padding: 12, textAlign: "left", cursor: "pointer", position: "relative",
                      borderColor: on ? "var(--red-2)" : undefined,
                      background: on ? "var(--red-tint)" : undefined,
                    }}
                  >
                    <Icon name={tp.icon} size={20} style={{ color: on ? "var(--red)" : "var(--text-3)" }} />
                    {on && <Icon name="check" size={16} style={{ position: "absolute", top: 10, right: 10, color: "var(--red)" }} />}
                    <div style={{ fontWeight: 600, marginTop: 6 }}>{tr(tp.title, tp.titleEn)}</div>
                    <div className="small muted" style={{ lineHeight: 1.35 }}>{tr(tp.text, tp.textEn)}</div>
                  </button>
                );
              })}
            </div>
            <div className="card row" style={{ padding: "10px 14px", marginTop: 4 }}>
              <Icon name="zap" size={18} style={{ color: optimize ? "var(--red)" : "var(--text-3)" }} />
              <div className="grow">
                <b>{tr("Optimalizace (Max FPS)", "Optimization (Max FPS)")}</b>
                <div className="small muted">Sodium, Lithium, FerriteCore, ImmediatelyFast, EntityCulling, ModernFix, Dynamic FPS</div>
              </div>
              <Switch
                checked={optimize}
                onChange={(v) => {
                  setOptimize(v);
                  if (v && loader === "vanilla") setLoader("fabric");
                }}
              />
            </div>
          </div>

          <div className="grid2">
            <div className="field">
              <label>{tr("Název", "Name")}</label>
              <input className="input" placeholder={tr("Moje instance", "My instance")} value={name} onChange={(e) => setName(e.target.value)} />
            </div>
            <div className="field">
              <label>{tr("Ikona", "Icon")}</label>
              <div className="row" style={{ gap: 10 }}>
                <button className="ip-trigger" onClick={() => setPicking(true)} title={tr("Vybrat ikonu", "Choose icon")}>
                  <InstIcon inst={{ icon: icon || "mc:diamond", template: packs[0] }} size={38} />
                  <span className="ip-edit">
                    <Icon name="edit" size={11} />
                  </span>
                </button>
                <button className="btn sm ghost" onClick={() => setPicking(true)}>
                  {tr("Vybrat ikonu", "Choose icon")}
                </button>
              </div>
              {picking && <IconPicker value={icon || "mc:diamond"} template={packs[0]} onClose={() => setPicking(false)} onChange={setIcon} />}
            </div>
          </div>

          <div className="field">
            <label>Loader</label>
            <div className="chips">
              {(["vanilla", "fabric", "quilt", "forge", "neoforge"] as Loader[]).map((l) => (
                <button key={l} className={`chip ${loader === l ? "on" : ""}`} onClick={() => setLoader(l)} disabled={needsModLoader && l === "vanilla"}>
                  {LOADER_LABEL[l]}
                </button>
              ))}
            </div>
            {needsModLoader && loader !== "fabric" && loader !== "quilt" && !packs.includes("pvp18") && (
              <span className="hint">
                {tr(
                  `Flashback, Iris a většina cinematic modů jsou pro Fabric — na ${LOADER_LABEL[loader]} se nainstaluje jen to, co existuje.`,
                  `Flashback, Iris and most cinematic mods are Fabric-only — on ${LOADER_LABEL[loader]} only what exists will be installed.`,
                )}
              </span>
            )}
          </div>

          <div className="grid2">
            <div className="field">
              <label className="row">
                <span className="grow">{tr("Verze Minecraftu", "Minecraft version")}</span>
                <label className="row small" style={{ gap: 6, cursor: "pointer" }}>
                  <input type="checkbox" checked={snapshots} onChange={(e) => setSnapshots(e.target.checked)} /> {tr("snapshoty", "snapshots")}
                </label>
              </label>
              <Select className="select" value={version} onChange={(e) => setVersion(e.target.value)}>
                {shown.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.id}
                    {v.kind !== "release" ? ` (${v.kind})` : ""}
                  </option>
                ))}
              </Select>
            </div>
            <div className="field">
              <label>{tr("Verze loaderu", "Loader version")}</label>
              {loader === "vanilla" ? (
                <Select className="select" disabled>
                  <option>—</option>
                </Select>
              ) : (
                <Select className="select" value={loaderVersion} onChange={(e) => setLoaderVersion(e.target.value)} disabled={!loaderVersions.length}>
                  <option value="">{tr("Doporučená (automaticky)", "Recommended (automatic)")}</option>
                  {loaderVersions.slice(0, 80).map((l) => (
                    <option key={l.version} value={l.version}>
                      {l.version}
                      {l.stable ? "" : " (beta)"}
                    </option>
                  ))}
                </Select>
              )}
              {loaderErr && <span className="hint" style={{ color: "var(--red-soft)" }}>{loaderErr}</span>}
            </div>
          </div>
        </div>
      )}
    </Modal>
  );
}
