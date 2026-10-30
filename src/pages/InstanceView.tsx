import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, type Backup, type GameVersion, type Installed, type Instance, type Loader, type LoaderVersion, type Update, type World } from "../api";
import { Icon, INSTANCE_ICONS, instanceIcon } from "../icons";
import { errText, fmtAgo, fmtDuration, fmtSize, LOADER_LABEL, useStore } from "../store";
import { Empty, InstIcon, Modal, openFolder, PlayButton, SkeletonList, Switch, TaskProgress, useAsync } from "../components/common";
import { tr } from "../i18n";
import Select from "../components/Select";
import OptimizeModal from "../components/OptimizeModal";
import RepairModal from "../components/RepairModal";
import IconPicker from "../components/IconPicker";
import { placeImg, useLastPlace } from "./Home";

const KIND_LABEL: Record<string, [string, string]> = {
  mod: ["Mody", "Mods"],
  resourcepack: ["Resource packy", "Resource packs"],
  shader: ["Shadery", "Shaders"],
};

function ContentTab({ inst }: { inst: Instance }) {
  const { go, toast, progress } = useStore();
  const [items, loading, reload] = useAsync(() => api.listContent(inst.id), [inst.id]);
  const [kind, setKind] = useState<string>("mod");
  const [q, setQ] = useState("");
  const [updates, setUpdates] = useState<Update[] | null>(null);
  const [checking, setChecking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const installing = !!progress[inst.id];

  useEffect(() => {
    if (!installing) reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [installing]);

  useEffect(() => {
    const un = getCurrentWebview().onDragDropEvent(async (e) => {
      if (e.payload.type === "over" || e.payload.type === "enter") setDragging(true);
      else if (e.payload.type === "leave") setDragging(false);
      else if (e.payload.type === "drop") {
        setDragging(false);
        try {
          const n = await api.addLocalFiles(inst.id, e.payload.paths);
          toast(`${tr("Přidáno souborů", "Files added")}: ${n}`, "success");
          reload();
        } catch (err) {
          toast(errText(err), "error");
        }
      }
    });
    return () => {
      un.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [inst.id]);

  const counts = useMemo(() => {
    const c: Record<string, number> = { mod: 0, resourcepack: 0, shader: 0 };
    items?.forEach((i) => c[i.kind]++);
    return c;
  }, [items]);
  const shown = (items ?? []).filter((i) => i.kind === kind && (!q || i.title.toLowerCase().includes(q.toLowerCase())));
  const updateFor = (m: Installed) => updates?.find((u) => u.file === m.file.replace(/\.disabled$/, "") && u.kind === m.kind);

  const check = async () => {
    setChecking(true);
    try {
      const u = await api.checkUpdates(inst.id);
      setUpdates(u);
      if (!u.length) toast(tr("Všechno je aktuální", "Everything is up to date"), "success");
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setChecking(false);
    }
  };

  const updateAll = async (list: Update[]) => {
    setBusy(true);
    try {
      const failed = await api.applyUpdates(inst.id, list);
      toast(
        failed.length
          ? `${tr("Některé se nepovedly", "Some failed")}:\n${failed.join("\n")}`
          : tr(`Aktualizováno: ${list.length} (záloha vytvořena)`, `Updated: ${list.length} (backup created)`),
        failed.length ? "error" : "success",
      );
      setUpdates((u) => u?.filter((x) => !list.includes(x)) ?? null);
      reload();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const [optimizing, setOptimizing] = useState(false);
  const optimize = () => setOptimizing(true);
  const [repairing, setRepairing] = useState(false);

  return (
    <div className="col" style={{ position: "relative" }}>
      {optimizing && <OptimizeModal instance={inst.id} onClose={() => setOptimizing(false)} onDone={reload} />}
      {repairing && <RepairModal instance={inst.id} onClose={() => setRepairing(false)} onDone={reload} />}
      {dragging && (
        <div className="overlay" style={{ position: "absolute", borderRadius: 12, border: "2px dashed var(--red)", background: "rgba(20,6,9,.85)" }}>
          <div className="col" style={{ alignItems: "center" }}>
            <Icon name="download" size={36} style={{ color: "var(--red)" }} />
            <b>{tr("Pusť soubory sem", "Drop files here")}</b>
            <span className="muted small">{tr(".jar → mody, .zip → resource packy / shadery", ".jar → mods, .zip → resource packs / shaders")}</span>
          </div>
        </div>
      )}
      <div className="row wrap">
        <div className="chips">
          {Object.entries(KIND_LABEL).map(([k, l]) => (
            <button key={k} className={`chip ${kind === k ? "on" : ""}`} onClick={() => setKind(k)}>
              {tr(l[0], l[1])} <span className="muted">{counts[k]}</span>
            </button>
          ))}
        </div>
        <div className="grow" />
        <div className="search" style={{ width: 200 }}>
          <Icon name="search" size={15} />
          <input className="input" placeholder={tr("Hledat", "Search")} value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        {inst.loader !== "vanilla" && (
          <button className="btn" onClick={optimize} disabled={busy || installing} title={tr("Výkonové mody, konflikty, grafika a paměť podle tvého PC", "Performance mods, conflicts, video settings and memory for your PC")}>
            <Icon name="zap" size={16} /> {tr("Optimalizovat", "Optimize")}
          </button>
        )}
        {inst.loader !== "vanilla" && (
          <button className="btn" onClick={() => setRepairing(true)} disabled={busy || installing} title={tr("Projde duplicity, závislosti, konflikty verzí a poslední pád – s náhledem a vrácením", "Checks duplicates, dependencies, version conflicts and the last crash – with preview and undo")}>
            <Icon name="wrench" size={16} /> {tr("Opravit", "Repair")}
          </button>
        )}
        <button className="btn" onClick={check} disabled={checking}>
          {checking ? <span className="spinner" /> : <Icon name="refresh" size={16} />} {tr("Zkontrolovat aktualizace", "Check for updates")}
        </button>
        <button className="btn primary" onClick={() => go({ page: "browse", id: inst.id, tab: kind })}>
          <Icon name="plus" size={16} /> {tr("Přidat obsah", "Add content")}
        </button>
      </div>

      <TaskProgress task={inst.id} />

      {inst.mcVersion.startsWith("1.8") && inst.loader === "forge" && items && !items.some((m) => /optifine/i.test(m.title + m.file)) && (
        <div className="callout" style={{ alignItems: "center" }}>
          <Icon name="zap" size={18} />
          <span className="grow">
            {tr("Pro nejvíc FPS na 1.8.9 přidej", "For the most FPS on 1.8.9 add")} <b style={{ color: "var(--text)" }}>OptiFine 1.8.9 HD U M5</b>.{" "}
            {tr(
              "Automaticky stáhnout nejde (autor to zakazuje) — stáhni ho a přetáhni .jar sem do okna.",
              "It can't be downloaded automatically (its author forbids it) — download it and drag the .jar into this window.",
            )}
          </span>
          <button className="btn sm primary" onClick={() => openUrl("https://optifine.net/downloads")}>
            <Icon name="external" size={14} /> OptiFine
          </button>
        </div>
      )}

      {updates && updates.length > 0 && (
        <div className="callout red" style={{ alignItems: "center" }}>
          <Icon name="download" size={18} />
          <span className="grow">
            {tr("Dostupné aktualizace", "Updates available")}: <b style={{ color: "var(--text)" }}>{updates.length}</b>.{" "}
            {tr("Před aktualizací se automaticky udělá záloha.", "A backup is made automatically before updating.")}
          </span>
          <button className="btn primary sm" onClick={() => updateAll(updates)} disabled={busy}>
            {busy ? <span className="spinner" /> : null} {tr("Aktualizovat vše", "Update all")}
          </button>
        </div>
      )}

      {inst.loader === "vanilla" && kind === "mod" ? (
        <Empty
          icon="puzzle"
          title={tr("Vanilla nepodporuje mody", "Vanilla doesn't support mods")}
          text={tr("V nastavení instance přepni loader na Fabric (doporučeno) nebo jiný.", "Switch the loader to Fabric (recommended) or another one in the instance settings.")}
        />
      ) : loading && !items ? (
        <SkeletonList />
      ) : shown.length === 0 ? (
        <Empty icon={kind === "shader" ? "sparkles" : kind === "resourcepack" ? "image" : "puzzle"} title={tr(`Žádné ${KIND_LABEL[kind][0].toLowerCase()}`, `No ${KIND_LABEL[kind][1].toLowerCase()}`)}
          text={tr("Přidej je z Modrinthu / CurseForge, nebo sem přetáhni soubory.", "Add them from Modrinth / CurseForge, or drag files in here.")}
        >
          <button className="btn primary" onClick={() => go({ page: "browse", id: inst.id, tab: kind })}>
            <Icon name="search" size={16} /> {tr("Procházet", "Browse")}
          </button>
        </Empty>
      ) : (
        <div className="list">
          {shown.map((m) => {
            const up = updateFor(m);
            return (
              <div key={m.file} className={`list-row ${m.enabled ? "" : "off"}`}>
                <div className="thumb">{m.iconUrl ? <img src={m.iconUrl} width={36} height={36} alt="" /> : <Icon name="puzzle" size={16} />}</div>
                <div className="grow">
                  <div className="row" style={{ gap: 8 }}>
                    <span className="ellipsis" style={{ fontWeight: 600 }}>{m.title}</span>
                    {m.source && <span className="badge">{m.source === "curseforge" ? "CurseForge" : "Modrinth"}</span>}
                    {up && <span className="badge amber">→ {up.newVersion}</span>}
                    {m.pinned && (
                      <button
                        className="badge"
                        style={{ border: 0, cursor: "pointer" }}
                        title={tr("Verzi zamkla oprava kompatibility — aktualizace ji přeskočí. Klikni pro odemčení.", "Locked by the compatibility fix — updates skip it. Click to unlock.")}
                        onClick={() => api.setPinned(inst.id, m.kind, m.file, false).then(reload)}
                      >
                        <Icon name="lock" size={11} /> {tr("zamčeno", "locked")}
                      </button>
                    )}
                  </div>
                  <div className="small muted ellipsis">
                    {m.version && `${m.version} · `}
                    {m.file} · {fmtSize(m.size)}
                  </div>
                </div>
                {up && (
                  <button className="btn sm" onClick={() => updateAll([up])} disabled={busy}>
                    <Icon name="download" size={14} />
                  </button>
                )}
                <Switch
                  checked={m.enabled}
                  onChange={(v) => api.toggleContent(inst.id, m.kind, m.file, v).then(reload).catch((e) => toast(errText(e), "error"))}
                />
                <button
                  className="btn sm ghost icon danger"
                  title={tr("Smazat", "Delete")}
                  onClick={() => api.removeContent(inst.id, m.kind, m.file).then(reload).catch((e) => toast(errText(e), "error"))}
                >
                  <Icon name="trash" size={15} />
                </button>
              </div>
            );
          })}
        </div>
      )}
      <div className="small muted">{tr("Tip: soubory .jar / .zip můžeš do tohohle okna rovnou přetáhnout myší.", "Tip: you can drag .jar / .zip files straight into this window.")}</div>
    </div>
  );
}

function ProfilesTab({ inst, onChange }: { inst: Instance; onChange: () => void }) {
  const { toast } = useStore();
  const [name, setName] = useState("");
  const names = Object.keys(inst.modProfiles ?? {});
  const run = (p: Promise<unknown>, msg: string) =>
    p.then(() => {
      toast(msg, "success");
      onChange();
    }).catch((e) => toast(errText(e), "error"));
  return (
    <div className="col">
      <div className="callout">
        <Icon name="layers" size={18} />
        <span>
          {tr(
            "Profil = uložená sada zapnutých modů. Například „Natáčení“ (Flashback, Iris, Distant Horizons) a „Hraní“ (jen FPS mody). Přepneš jedním klikem, nic se nemaže.",
            "A profile is a saved set of enabled mods. For example “Recording” (Flashback, Iris, Distant Horizons) and “Playing” (FPS mods only). Switch with one click, nothing gets deleted.",
          )}
        </span>
      </div>
      <div className="row">
        <input className="input" placeholder={tr("Název profilu (např. Natáčení)", "Profile name (e.g. Recording)")} value={name} onChange={(e) => setName(e.target.value)} />
        <button className="btn primary" disabled={!name.trim()} onClick={() => run(api.saveModProfile(inst.id, name.trim()), tr(`Profil ${name} uložen`, `Profile ${name} saved`)).then(() => setName(""))}>
          <Icon name="plus" size={16} /> {tr("Uložit aktuální mody jako profil", "Save current mods as a profile")}
        </button>
      </div>
      {names.length === 0 ? (
        <Empty icon="layers" title={tr("Žádné profily", "No profiles")} text={tr("Zapni mody, které chceš, a ulož je jako profil.", "Enable the mods you want and save them as a profile.")} />
      ) : (
        <div className="list">
          {names.map((n) => (
            <div key={n} className="list-row">
              <div className="thumb">
                <Icon name="layers" size={16} />
              </div>
              <div className="grow">
                <div style={{ fontWeight: 600 }}>{n}</div>
                <div className="small muted">{inst.modProfiles[n].length} {tr("zapnutých modů", "enabled mods")}</div>
              </div>
              {inst.modProfile === n ? (
                <span className="badge green">{tr("Aktivní", "Active")}</span>
              ) : (
                <button className="btn sm primary" onClick={() => run(api.applyModProfile(inst.id, n), tr(`Přepnuto na ${n}`, `Switched to ${n}`))}>
                  {tr("Přepnout", "Switch")}
                </button>
              )}
              <button className="btn sm" onClick={() => run(api.saveModProfile(inst.id, n), tr(`Profil ${n} přepsán`, `Profile ${n} overwritten`))} title={tr("Přepsat aktuálním stavem", "Overwrite with current state")}>
                <Icon name="refresh" size={14} />
              </button>
              <button className="btn sm ghost icon danger" onClick={() => run(api.deleteModProfile(inst.id, n), tr("Smazáno", "Deleted"))}>
                <Icon name="trash" size={15} />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function WorldsTab({ inst }: { inst: Instance }) {
  const { toast, play, running } = useStore();
  const [worlds, loading] = useAsync(() => api.listWorlds(inst.id), [inst.id]);
  const [busy, setBusy] = useState("");
  const backup = async (w: World) => {
    setBusy(w.name);
    try {
      await api.backupWorld(inst.id, w.name);
      toast(tr(`Záloha světa ${w.name} hotová`, `World ${w.name} backed up`), "success");
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy("");
    }
  };
  if (loading && !worlds) return <SkeletonList />;
  if (!worlds?.length) return <Empty icon="globe" title={tr("Žádné světy", "No worlds")} text={tr("Světy se objeví, až si nějaký ve hře vytvoříš.", "Worlds show up once you create one in the game.")} />;
  return (
    <div className="list">
      {worlds.map((w) => (
        <div key={w.name} className="list-row">
          <div className="thumb lg">{w.icon ? <img src={convertFileSrc(w.icon)} width={56} height={56} style={{ imageRendering: "pixelated" }} alt="" /> : <Icon name="globe" size={20} />}</div>
          <div className="grow">
            <div style={{ fontWeight: 600 }}>{w.name}</div>
            <div className="small muted">
              {fmtAgo(w.modified)} · {fmtSize(w.size)}
            </div>
          </div>
          <button className="btn sm primary" onClick={() => play(inst.id, `world:${w.name}`)} disabled={running.includes(inst.id)} title={tr("Spustí hru rovnou v tomhle světě", "Starts the game straight into this world")}>
            <Icon name="play" size={12} /> {tr("Hrát", "Play")}
          </button>
          <button className="btn sm" onClick={() => backup(w)} disabled={busy === w.name}>
            {busy === w.name ? <span className="spinner" /> : <Icon name="archive" size={14} />} {tr("Zálohovat", "Back up")}
          </button>
          <button className="btn sm ghost icon" onClick={() => openFolder(w.path)}>
            <Icon name="folder" size={15} />
          </button>
        </div>
      ))}
    </div>
  );
}

function BackupsTab({ inst }: { inst: Instance }) {
  const { toast } = useStore();
  const [list, , reload] = useAsync(() => api.listBackups(inst.id), [inst.id]);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<Backup | null>(null);
  const act = async (p: Promise<unknown>, msg: string) => {
    setBusy(true);
    try {
      await p;
      toast(msg, "success");
      reload();
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="col">
      <div className="row">
        <span className="muted grow">
          {tr(
            "Snapshot uloží mody, configy a nastavení instance. Před každou aktualizací modů se vytváří automaticky.",
            "A snapshot saves the instance's mods, configs and settings. One is made automatically before every mod update.",
          )}
        </span>
        <button className="btn" onClick={() => act(api.snapshot(inst.id), tr("Snapshot vytvořen", "Snapshot created"))} disabled={busy}>
          <Icon name="archive" size={16} /> {tr("Vytvořit snapshot", "Create snapshot")}
        </button>
      </div>
      {!list?.length ? (
        <Empty icon="archive" title={tr("Zatím žádné zálohy", "No backups yet")} />
      ) : (
        <div className="list">
          {list.map((b) => (
            <div key={b.file} className="list-row">
              <div className="thumb">
                <Icon name={b.kind === "world" ? "globe" : "layers"} size={16} />
              </div>
              <div className="grow">
                <div style={{ fontWeight: 600 }}>{b.kind === "world" ? `${tr("Svět", "World")}: ${b.label}` : `Snapshot: ${b.label}`}</div>
                <div className="small muted">
                  {b.created.replace("_", " ").replace(/-(\d\d)-(\d\d)$/, ":$1:$2")} · {fmtSize(b.size)}
                </div>
              </div>
              <button className="btn sm" onClick={() => setConfirm(b)} disabled={busy}>
                <Icon name="refresh" size={14} /> {tr("Obnovit", "Restore")}
              </button>
              <button className="btn sm ghost icon danger" onClick={() => act(api.deleteBackup(inst.id, b.file), tr("Záloha smazána", "Backup deleted"))}>
                <Icon name="trash" size={15} />
              </button>
            </div>
          ))}
        </div>
      )}
      {confirm && (
        <Modal
          title={tr("Obnovit zálohu?", "Restore backup?")}
          onClose={() => setConfirm(null)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setConfirm(null)}>{tr("Zrušit", "Cancel")}</button>
              <button
                className="btn primary"
                onClick={() => {
                  const b = confirm;
                  setConfirm(null);
                  act(api.restoreBackup(inst.id, b.file), tr("Obnoveno", "Restored"));
                }}
              >
                {tr("Obnovit", "Restore")}
              </button>
            </>
          }
        >
          {confirm.kind === "world"
            ? tr(
                `Svět ${confirm.label} se nahradí zálohou. Aktuální stav světa se nejdřív sám zazálohuje.`,
                `World ${confirm.label} will be replaced by the backup. Its current state is backed up first.`,
              )
            : tr(
                "Mody a configy se nahradí snapshotem. Aktuální stav se nejdřív sám zazálohuje.",
                "Mods and configs will be replaced by the snapshot. The current state is backed up first.",
              )}
        </Modal>
      )}
    </div>
  );
}

function LogTab({ inst }: { inst: Instance }) {
  const { running, toast } = useStore();
  const [lines, setLines] = useState<string[]>([]);
  const [filter, setFilter] = useState("");
  const [follow, setFollow] = useState(true);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.logs(inst.id).then(setLines);
    const un = listen<{ instance: string; line: string }>("game-log", ({ payload }) => {
      if (payload.instance === inst.id) setLines((l) => (l.length > 3000 ? [...l.slice(-2500), payload.line] : [...l, payload.line]));
    });
    return () => {
      un.then((f) => f());
    };
  }, [inst.id]);
  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [lines, follow]);

  const shown = filter ? lines.filter((l) => l.toLowerCase().includes(filter.toLowerCase())) : lines;
  return (
    <div className="col">
      <div className="row">
        <div className="search grow">
          <Icon name="search" size={15} />
          <input className="input" placeholder={tr("Filtrovat log…", "Filter log…")} value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
        <label className="row small muted" style={{ gap: 6 }}>
          <Switch checked={follow} onChange={setFollow} /> {tr("Sledovat", "Follow")}
        </label>
        <button className="btn" onClick={() => api.openLogsWindow()} title={tr("Živý log v samostatném okně", "Live log in a separate window")}>
          <Icon name="external" size={16} /> {tr("Otevřít v okně", "Pop out")}
        </button>
        <button className="btn" onClick={() => navigator.clipboard.writeText(lines.join("\n")).then(() => toast(tr("Zkopírováno", "Copied"), "success"))}>
          <Icon name="copy" size={16} /> {tr("Kopírovat log", "Copy log")}
        </button>
        <button className="btn" onClick={async () => openFolder(await api.instancePath(inst.id, "logs"))}>
          <Icon name="folder" size={16} />
        </button>
      </div>
      <div className="logbox" ref={box}>
        {shown.length === 0 ? (
          <span className="muted">{running.includes(inst.id) ? tr("Čekám na výstup hry…", "Waiting for game output…") : tr("Log se zobrazí po spuštění hry.", "The log shows up once the game starts.")}</span>
        ) : (
          shown.map((l, i) => (
            <div key={i} className={/error|exception|fatal/i.test(l) ? "err" : /warn/i.test(l) ? "warn" : ""}>
              {l}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function SettingsTab({ inst, onChange }: { inst: Instance; onChange: (i: Instance) => void }) {
  const { settings, system, toast, go, reloadInstances } = useStore();
  const [draft, setDraft] = useState(inst);
  const [versions, setVersions] = useState<GameVersion[]>([]);
  const [lvs, setLvs] = useState<LoaderVersion[]>([]);
  const [code, setCode] = useState("");
  const [confirmDel, setConfirmDel] = useState(false);
  useEffect(() => setDraft(inst), [inst]);
  useEffect(() => {
    api.gameVersions().then((v) => setVersions(v.filter((x) => x.kind === "release" || x.id === inst.mcVersion)));
  }, [inst.mcVersion]);
  useEffect(() => {
    if (draft.loader === "vanilla") return setLvs([]);
    api.loaderVersions(draft.loader, draft.mcVersion).then(setLvs).catch(() => setLvs([]));
  }, [draft.loader, draft.mcVersion]);

  const set = (p: Partial<Instance>) => setDraft((d) => ({ ...d, ...p }));
  const [picking, setPicking] = useState(false);
  const dirty = JSON.stringify(draft) !== JSON.stringify(inst);
  const saveIt = async () => {
    try {
      const saved = await api.updateInstance(draft);
      onChange(saved);
      reloadInstances();
      toast(tr("Uloženo", "Saved"), "success");
    } catch (e) {
      toast(errText(e), "error");
    }
  };
  const ram = draft.ramMb ?? settings?.ramMb ?? 4096;
  const maxRam = Math.max(4096, Math.min(32768, (system?.totalRamMb ?? 16384) - 2048));

  return (
    <div className="col" style={{ maxWidth: 760 }}>
      <div className="card pad">
        <div className="grid2">
          <div className="field">
            <label>{tr("Název", "Name")}</label>
            <input className="input" value={draft.name} onChange={(e) => set({ name: e.target.value })} />
          </div>
          <div className="field">
            <label>{tr("Složka (skupina)", "Folder (group)")}</label>
            <input className="input" placeholder={tr("např. Natáčení", "e.g. Recording")} value={draft.group} onChange={(e) => set({ group: e.target.value })} />
          </div>
        </div>
        <div className="field" style={{ marginTop: 14 }}>
          <label>{tr("Ikona", "Icon")}</label>
          <div className="row" style={{ gap: 14 }}>
            <button className="ip-trigger" onClick={() => setPicking(true)} title={tr("Změnit ikonu", "Change icon")}>
              <InstIcon inst={draft} size={56} />
              <span className="ip-edit">
                <Icon name="edit" size={13} />
              </span>
            </button>
            <div className="col" style={{ gap: 4 }}>
              <button className="btn sm" onClick={() => setPicking(true)}>
                <Icon name="palette" size={14} /> {tr("Změnit ikonu", "Change icon")}
              </button>
              <span className="muted small">{tr("Minecraft ikony, barva, vlastní obrázek nebo screenshot", "Minecraft icons, colour, your own picture or a screenshot")}</span>
            </div>
          </div>
          {picking && (
            <IconPicker
              value={draft.icon}
              template={draft.template}
              instanceId={inst.id}
              onClose={() => setPicking(false)}
              onChange={(icon) => {
                set({ icon });
                reloadInstances();
              }}
            />
          )}
        </div>
      </div>

      <div className="card pad">
        <div className="setting" style={{ paddingTop: 0 }}>
          <div className="txt">
            <b>{tr("Režim natáčení", "Recording mode")}</b>
            <span>
              {tr(
                "Vypne houpání kamery, změnu FOV při sprintu, zkreslení obrazu a otravné popupy. Po vypnutí se vrátí tvoje původní nastavení.",
                "Turns off view bobbing, FOV change when sprinting, screen distortion and annoying popups. Your original settings come back when you turn it off.",
              )}
            </span>
          </div>
          <Switch checked={draft.recordingMode} onChange={(v) => set({ recordingMode: v })} />
        </div>
        {["fabric", "quilt", "neoforge", "forge"].includes(draft.loader) && JUSTICE_MOD_VERSIONS.test(draft.mcVersion) && (
          <div className="setting">
            <div className="txt">
              <b>{tr("Justice Mod (menu ve hře)", "Justice Mod (in-game menu)")}</b>
              <span>
                {tr(
                  "Pravý Shift ve hře otevře menu jako v Lunaru: PvP HUD, zoom, freelook, nástroje na natáčení a pro streamery.",
                  "Right Shift in-game opens a Lunar-style menu: PvP HUD, zoom, freelook, recording and streamer tools.",
                )}
              </span>
            </div>
            <Switch checked={!draft.noJusticeMod} onChange={(v) => set({ noJusticeMod: !v })} />
          </div>
        )}
        {(draft.loader === "fabric" || draft.loader === "quilt") && (
          <div className="setting">
            <div className="txt">
              <b>{tr("Lehký režim (víc FPS)", "Light mode (more FPS)")}</b>
              <span>
                {tr(
                  "Při spuštění vypne nejnáročnější mody (Distant Horizons, dynamická světla, Continuity). Hodí se na normální hraní; na natáčení ho vypni a mody se vrátí.",
                  "Disables the heaviest mods at launch (Distant Horizons, dynamic lights, Continuity). Good for normal play; turn it off for recording and the mods come back.",
                )}
              </span>
            </div>
            <Switch checked={!!draft.lightMode} onChange={(v) => set({ lightMode: v })} />
          </div>
        )}
        <div className="setting">
          <div className="txt">
            <b>{tr("Paměť (RAM)", "Memory (RAM)")}: {(ram / 1024).toFixed(1)} GB</b>
            <span>
              {draft.ramMb ? tr("Vlastní hodnota pro tuhle instanci.", "Custom value for this instance.") : tr("Používá výchozí hodnotu z nastavení.", "Uses the default from Settings.")}{" "}
              {tr("Víc než 8 GB většinou nepomáhá.", "More than 8 GB rarely helps.")}
            </span>
            <input type="range" min={1024} max={maxRam} step={512} value={ram} onChange={(e) => set({ ramMb: Number(e.target.value) })} style={{ marginTop: 10 }} />
          </div>
          {draft.ramMb && (
            <button className="btn sm ghost" onClick={() => set({ ramMb: null })}>
              {tr("Výchozí", "Default")}
            </button>
          )}
        </div>
        <div className="setting">
          <div className="txt">
            <b>{tr("Rozlišení okna", "Window resolution")}</b>
            <span>{tr("Pro natáčení se hodí 1920×1080.", "1920×1080 is good for recording.")}</span>
          </div>
          <input className="input" style={{ width: 90 }} type="number" placeholder={String(settings?.width ?? 1280)} value={draft.width ?? ""} onChange={(e) => set({ width: e.target.value ? Number(e.target.value) : null })} />
          <span className="muted">×</span>
          <input className="input" style={{ width: 90 }} type="number" placeholder={String(settings?.height ?? 720)} value={draft.height ?? ""} onChange={(e) => set({ height: e.target.value ? Number(e.target.value) : null })} />
        </div>
      </div>

      <div className="card pad col">
        <b>{tr("Verze", "Version")}</b>
        <div className="grid2">
          <div className="field">
            <label>Minecraft</label>
            <Select className="select" value={draft.mcVersion} onChange={(e) => set({ mcVersion: e.target.value, loaderVersion: "" })}>
              {versions.map((v) => (
                <option key={v.id}>{v.id}</option>
              ))}
            </Select>
          </div>
          <div className="field">
            <label>Loader</label>
            <Select className="select" value={draft.loader} onChange={(e) => set({ loader: e.target.value as Loader, loaderVersion: "" })}>
              {Object.entries(LOADER_LABEL).map(([k, v]) => (
                <option key={k} value={k}>{v}</option>
              ))}
            </Select>
          </div>
        </div>
        {draft.loader !== "vanilla" && (
          <div className="field">
            <label>{tr("Verze loaderu", "Loader version")}</label>
            <Select className="select" value={draft.loaderVersion} onChange={(e) => set({ loaderVersion: e.target.value })}>
              <option value="">{tr("Doporučená (automaticky)", "Recommended (automatic)")}</option>
              {lvs.slice(0, 80).map((l) => (
                <option key={l.version}>{l.version}</option>
              ))}
            </Select>
          </div>
        )}
        {(draft.mcVersion !== inst.mcVersion || draft.loader !== inst.loader) && (
          <div className="callout red">
            <Icon name="alert" size={18} />
            <span>
              {tr(
                "Po změně verze nemusí mody fungovat. Pak je aktualizuj v záložce Obsah (Zkontrolovat aktualizace).",
                "Mods may stop working after a version change. Update them in the Content tab (Check for updates).",
              )}
            </span>
          </div>
        )}
      </div>

      <div className="card pad col">
        <b>Java</b>
        <div className="field">
          <label>{tr("Vlastní cesta k Javě (nech prázdné = launcher stáhne správnou Javu sám)", "Custom Java path (leave empty = the launcher downloads the right Java itself)")}</label>
          <input className="input mono" style={{ fontSize: 12 }} value={draft.javaPath} onChange={(e) => set({ javaPath: e.target.value })} />
        </div>
        <div className="field">
          <label>{tr("Extra JVM argumenty", "Extra JVM arguments")}</label>
          <input className="input mono" style={{ fontSize: 12 }} placeholder="-Dnecojineho=true" value={draft.jvmArgs} onChange={(e) => set({ jvmArgs: e.target.value })} />
        </div>
      </div>

      <div className="card pad col">
        <b>{tr("Sdílení", "Sharing")}</b>
        <span className="muted small">
          {tr(
            "Kód obsahuje verzi, loader a seznam modů z Modrinthu/CurseForge. Kamarád ho vloží v Nová instance → Importovat.",
            "The code contains the version, loader and list of Modrinth/CurseForge mods. A friend pastes it in New instance → Import.",
          )}
        </span>
        <div className="row">
          <button
            className="btn"
            onClick={async () => {
              try {
                const c = await api.shareCode(inst.id);
                setCode(c);
                await navigator.clipboard.writeText(c);
                toast(tr("Kód zkopírován do schránky", "Code copied to clipboard"), "success");
              } catch (e) {
                toast(errText(e), "error");
              }
            }}
          >
            <Icon name="share" size={16} /> {tr("Vytvořit sdílecí kód", "Create share code")}
          </button>
          <button
            className="btn"
            onClick={async () => {
              const dest = await save({ defaultPath: `${inst.name}.justice.zip`, filters: [{ name: "Justice pack", extensions: ["zip"] }] });
              if (!dest) return;
              try {
                await api.exportPack(inst.id, dest);
                toast(tr("Exportováno (včetně configů)", "Exported (including configs)"), "success");
              } catch (e) {
                toast(errText(e), "error");
              }
            }}
          >
            <Icon name="download" size={16} /> {tr("Exportovat se configy", "Export with configs")}
          </button>
        </div>
        {code && <textarea className="input" rows={3} readOnly value={code} onFocus={(e) => e.target.select()} />}
      </div>

      <div className="row">
        <button
          className="btn"
          onClick={async () => {
            const d = await api.duplicateInstance(inst.id);
            await reloadInstances();
            go({ page: "instance", id: d.id });
          }}
        >
          <Icon name="copy" size={16} /> {tr("Duplikovat", "Duplicate")}
        </button>
        <button className="btn danger" onClick={() => setConfirmDel(true)}>
          <Icon name="trash" size={16} /> {tr("Smazat instanci", "Delete instance")}
        </button>
        <div className="grow" />
        {dirty && (
          <>
            <button className="btn ghost" onClick={() => setDraft(inst)}>{tr("Zrušit", "Cancel")}</button>
            <button className="btn primary" onClick={saveIt}>
              <Icon name="check" size={16} /> {tr("Uložit", "Save")}
            </button>
          </>
        )}
      </div>

      {confirmDel && (
        <Modal
          title={tr(`Smazat ${inst.name}?`, `Delete ${inst.name}?`)}
          onClose={() => setConfirmDel(false)}
          footer={
            <>
              <button className="btn ghost" onClick={() => setConfirmDel(false)}>{tr("Zrušit", "Cancel")}</button>
              <button
                className="btn primary"
                onClick={async () => {
                  await api.deleteInstance(inst.id);
                  await reloadInstances();
                  go({ page: "instances" });
                }}
              >
                {tr("Smazat navždy", "Delete forever")}
              </button>
            </>
          }
        >
          {tr(
            "Smaže se instance včetně světů, modů a screenshotů. Zálohy ve složce backups zůstanou.",
            "The instance is deleted including its worlds, mods and screenshots. Backups in the backups folder stay.",
          )}
        </Modal>
      )}
    </div>
  );
}


/** Minecraft versions with a bundled Justice Mod build (keep in sync with src-tauri/src/justice_mod.rs). */
const JUSTICE_MOD_VERSIONS = /^(1\.21\.(9|10|11)|26\.[123](\.\d+)?)$/;
/** Header: the last place's screenshot behind the name, play and folder buttons. */
function InstanceBanner({ inst }: { inst: Instance }) {
  const { go, settings } = useStore();
  const place = useLastPlace(inst.id);
  const img = placeImg(place);
  return (
    <div className="inst-banner">
      <div className="img" style={img ? { backgroundImage: `url("${img}")` } : undefined} />
      <button className="btn ghost icon back" onClick={() => go({ page: "instances" })} aria-label={tr("Zpět", "Back")}>
        <Icon name="left" size={18} />
      </button>
      <div className="ib-body">
        <InstIcon inst={inst} size={64} />
        <div className="grow" style={{ minWidth: 0 }}>
          <h1 className="ellipsis">{inst.name}</h1>
          <div className="chips2">
            <span className="chip2">
              {LOADER_LABEL[inst.loader]} {inst.mcVersion}
            </span>
            {inst.playtimeSecs > 0 && (
              <span className="chip2">
                <Icon name="clock" size={12} /> {fmtDuration(inst.playtimeSecs)}
              </span>
            )}
            <span className="chip2">
              {tr("Naposledy", "Last played")}: {inst.lastPlayed ? fmtAgo(inst.lastPlayed) : tr("nikdy", "never")}
            </span>
            {place && (
              <span className="chip2">
                <Icon name={place.kind === "server" ? "server" : "globe"} size={12} /> {place.kind === "server" && settings?.streamerMode ? "••••••" : place.name}
              </span>
            )}
            {inst.recordingMode && <span className="chip2 acc">● {tr("Natáčení", "Recording")}</span>}
          </div>
        </div>
        <button className="ghost2" onClick={async () => openFolder(await api.instancePath(inst.id))}>
          <Icon name="folder" size={16} /> {tr("Složka", "Folder")}
        </button>
        <PlayButton inst={inst} big />
      </div>
    </div>
  );
}

export default function InstanceView({ id, tab: initialTab }: { id: string; tab?: string }) {
  const { go, toast, running } = useStore();
  const [inst, setInst] = useState<Instance | null>(null);
  const [tab, setTab] = useState(initialTab ?? "content");
  const load = () => api.getInstance(id).then(setInst).catch((e) => toast(errText(e), "error"));
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, running.length]);

  if (!inst) return <div className="page muted">{tr("Načítám…", "Loading…")}</div>;
  const TABS = [
    ["content", tr("Obsah", "Content")],
    ["profiles", tr("Profily modů", "Mod profiles")],
    ["worlds", tr("Světy", "Worlds")],
    ["backups", tr("Zálohy", "Backups")],
    ["log", "Log"],
    ["settings", tr("Nastavení", "Settings")],
  ];
  return (
    <div className="page">
      <InstanceBanner inst={inst} />
      <div className="tabs">
        {TABS.map(([k, l]) => (
          <button key={k} className={`tab ${tab === k ? "on" : ""}`} onClick={() => setTab(k)}>
            {l}
          </button>
        ))}
      </div>
      {tab === "content" && <ContentTab inst={inst} />}
      {tab === "profiles" && <ProfilesTab inst={inst} onChange={load} />}
      {tab === "worlds" && <WorldsTab inst={inst} />}
      {tab === "backups" && <BackupsTab inst={inst} />}
      {tab === "log" && <LogTab inst={inst} />}
      {tab === "settings" && <SettingsTab inst={inst} onChange={setInst} />}
    </div>
  );
}
