import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";
import { api, type MediaFile } from "../api";
import { Icon } from "../icons";
import { errText, fmtAgo, fmtDuration, fmtSize, LOADER_LABEL, useStore } from "../store";
import { Empty, openFolder, SkeletonList, Switch, useAsync } from "../components/common";
import { tr } from "../i18n";
import Select from "../components/Select";

function RecordingInstances() {
  const { instances, reloadInstances, toast, go } = useStore();
  const toggle = async (id: string, on: boolean) => {
    const inst = instances.find((i) => i.id === id)!;
    try {
      await api.updateInstance({ ...inst, recordingMode: on });
      reloadInstances();
    } catch (e) {
      toast(errText(e), "error");
    }
  };
  if (!instances.length) return <div className="muted">{tr("Nejdřív si vytvoř instanci.", "Create an instance first.")}</div>;
  return (
    <div className="list">
      {instances.map((i) => (
        <div key={i.id} className="list-row">
          <div className="grow" style={{ cursor: "pointer" }} onClick={() => go({ page: "instance", id: i.id })}>
            <div style={{ fontWeight: 600 }}>{i.name}</div>
            <div className="small muted">
              {LOADER_LABEL[i.loader]} {i.mcVersion}
            </div>
          </div>
          {i.recordingMode && <span className="badge red">● REC</span>}
          <Switch checked={i.recordingMode} onChange={(v) => toggle(i.id, v)} />
        </div>
      ))}
    </div>
  );
}

function MediaList({ kind }: { kind: "replay" | "video" | "screenshot" }) {
  const { toast, play, settings } = useStore();
  const [items, loading, reload] = useAsync(() => api.listMedia(kind), [kind]);
  const del = async (m: MediaFile) => {
    try {
      await api.deleteMedia(m.path);
      reload();
    } catch (e) {
      toast(errText(e), "error");
    }
  };
  if (loading && !items) return <SkeletonList />;
  if (!items?.length)
    return kind === "replay" ? (
      <Empty
        icon="film"
        title={tr("Zatím žádné replaye", "No replays yet")}
        text={tr(
          "Ve hře s Flashbackem začni nahrávat (klávesu najdeš v Options → Controls → Flashback). Replaye se tu pak objeví.",
          "Start recording in-game with Flashback (the key is in Options → Controls → Flashback). Replays will show up here.",
        )}
      />
    ) : kind === "video" ? (
      <Empty
        icon="video"
        title={tr("Žádná exportovaná videa", "No exported videos")}
        text={tr("Videa exportovaná z Flashbacku do složky instance se ukážou tady.", "Videos exported from Flashback into the instance folder show up here.")}
      />
    ) : (
      <Empty icon="camera" title={tr("Žádné screenshoty", "No screenshots")} text={tr("Screenshoty ve hře děláš klávesou F2.", "Take screenshots in-game with F2.")} />
    );

  if (kind === "screenshot")
    return (
      <div className="gallery">
        {items.map((m) => (
          <div key={m.path} className="shot" onClick={() => openPath(m.path)}>
            <img src={convertFileSrc(m.path)} loading="lazy" alt="" />
            <div className="cap row">
              <span className="grow ellipsis">{m.instanceName}</span>
              <span>{fmtAgo(m.modified)}</span>
            </div>
          </div>
        ))}
      </div>
    );

  return (
    <div className="list">
      {items.map((m) => (
        <div key={m.path} className="list-row">
          <div className="thumb">
            <Icon name={kind === "replay" ? "film" : "video"} size={16} />
          </div>
          <div className="grow">
            <div className="ellipsis" style={{ fontWeight: 600 }}>{m.name}</div>
            <div className="small muted ellipsis">
              {m.instanceName}
              {m.world && !settings?.streamerMode ? ` · ${m.world}` : ""}
              {m.durationSecs != null ? ` · ${fmtDuration(m.durationSecs)}` : ""} · {fmtSize(m.size)} · {fmtAgo(m.modified)}
            </div>
          </div>
          {kind === "replay" ? (
            <button className="btn sm" onClick={() => play(m.instance)} title={tr("Replaye otevřeš ve hře v menu Flashback", "Open replays in-game from the Flashback menu")}>
              <Icon name="play" size={12} /> {tr("Otevřít hru", "Open game")}
            </button>
          ) : (
            <button className="btn sm" onClick={() => openPath(m.path)}>
              <Icon name="play" size={12} /> {tr("Přehrát", "Play")}
            </button>
          )}
          <button className="btn sm ghost icon" onClick={() => openFolder(m.path.replace(/[\\/][^\\/]+$/, ""))}>
            <Icon name="folder" size={15} />
          </button>
          <button className="btn sm ghost icon danger" onClick={() => del(m)}>
            <Icon name="trash" size={15} />
          </button>
        </div>
      ))}
    </div>
  );
}

function ObsCard() {
  const { settings, saveSettings, toast } = useStore();
  const [scenes, setScenes] = useState<string[]>([]);
  const [testing, setTesting] = useState(false);
  const [url, setUrl] = useState(settings?.obsUrl ?? "");
  const [pw, setPw] = useState(settings?.obsPassword ?? "");
  useEffect(() => {
    setUrl(settings?.obsUrl ?? "");
    setPw(settings?.obsPassword ?? "");
  }, [settings?.obsUrl, settings?.obsPassword]);
  if (!settings) return null;
  const test = async () => {
    setTesting(true);
    try {
      await saveSettings({ obsUrl: url, obsPassword: pw });
      const s = await api.obsTest();
      setScenes(s);
      toast(tr(`OBS připojeno (${s.length} scén)`, `OBS connected (${s.length} scenes)`), "success");
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setTesting(false);
    }
  };
  return (
    <div className="card pad">
      <div className="setting" style={{ paddingTop: 0 }}>
        <div className="txt">
          <b>{tr("Propojení s OBS", "OBS integration")}</b>
          <span>
            {tr(
              "V OBS: Nástroje → Nastavení WebSocket serveru → Povolit. Zkopíruj heslo sem.",
              "In OBS: Tools → WebSocket Server Settings → Enable. Copy the password here.",
            )}
          </span>
        </div>
        <Switch checked={settings.obsEnabled} onChange={(v) => saveSettings({ obsEnabled: v })} />
      </div>
      {settings.obsEnabled && (
        <>
          <div className="grid2" style={{ margin: "12px 0" }}>
            <div className="field">
              <label>{tr("Adresa", "Address")}</label>
              <input className="input mono" value={url} onChange={(e) => setUrl(e.target.value)} />
            </div>
            <div className="field">
              <label>{tr("Heslo", "Password")}</label>
              <input className="input" type="password" value={pw} onChange={(e) => setPw(e.target.value)} />
            </div>
          </div>
          <button className="btn" onClick={test} disabled={testing}>
            {testing ? <span className="spinner" /> : <Icon name="refresh" size={16} />} {tr("Otestovat spojení", "Test connection")}
          </button>
          <div className="setting">
            <div className="txt">
              <b>{tr("Automaticky nahrávat", "Record automatically")}</b>
              <span>{tr("Při spuštění hry začne OBS nahrávat, po zavření hry přestane.", "OBS starts recording when the game starts and stops when it closes.")}</span>
            </div>
            <Switch checked={settings.obsAutoRecord} onChange={(v) => saveSettings({ obsAutoRecord: v })} />
          </div>
          <div className="setting">
            <div className="txt">
              <b>{tr("Přepnout na scénu", "Switch to scene")}</b>
              <span>{tr("Při spuštění hry se OBS přepne na tuhle scénu.", "OBS switches to this scene when the game starts.")}</span>
            </div>
            <Select className="select" style={{ width: 220 }} value={settings.obsScene} onChange={(e) => saveSettings({ obsScene: e.target.value })}>
              <option value="">{tr("— nepřepínat —", "— don't switch —")}</option>
              {[...new Set([settings.obsScene, ...scenes].filter(Boolean))].map((s) => (
                <option key={s}>{s}</option>
              ))}
            </Select>
          </div>
        </>
      )}
    </div>
  );
}

export default function Recording() {
  const { settings, saveSettings } = useStore();
  const [tab, setTab] = useState<"replay" | "video" | "screenshot">("replay");
  return (
    <div className="page">
      <div className="page-head">
        <h1>{tr("Natáčení", "Recording")}</h1>
        <span className="sub">{tr("Flashback, režim natáčení, OBS", "Flashback, recording mode, OBS")}</span>
      </div>

      <div className="grid2" style={{ alignItems: "start" }}>
        <div className="col">
          <div className="section-title">
            <Icon name="record" size={12} style={{ color: "var(--red)" }} /> {tr("Režim natáčení", "Recording mode")}
          </div>
          <div className="callout">
            <Icon name="info" size={18} />
            <span>
              {tr(
                "Vypne houpání kamery, změnu FOV při běhu a zkreslení obrazu. Po vypnutí se vrátí tvoje původní nastavení hry.",
                "Turns off view bobbing, FOV change when sprinting and screen distortion. Your original game settings come back when you turn it off.",
              )}
            </span>
          </div>
          <RecordingInstances />
        </div>
        <div className="col">
          <div className="section-title">{tr("Integrace", "Integrations")}</div>
          <ObsCard />
          <div className="card pad">
            <div className="setting" style={{ padding: 0 }}>
              <div className="txt">
                <b>{tr("Streamer mód", "Streamer mode")}</b>
                <span>
                  {tr(
                    "Skryje adresy serverů, UUID a názvy světů v launcheru, když ho ukazuješ na streamu.",
                    "Hides server addresses, UUIDs and world names in the launcher while you show it on stream.",
                  )}
                </span>
              </div>
              <Switch checked={!!settings?.streamerMode} onChange={(v) => saveSettings({ streamerMode: v })} />
            </div>
          </div>
        </div>
      </div>

      <div className="tabs" style={{ marginTop: 28 }}>
        <button className={`tab ${tab === "replay" ? "on" : ""}`} onClick={() => setTab("replay")}>{tr("Replaye", "Replays")} (Flashback)</button>
        <button className={`tab ${tab === "video" ? "on" : ""}`} onClick={() => setTab("video")}>{tr("Videa", "Videos")}</button>
        <button className={`tab ${tab === "screenshot" ? "on" : ""}`} onClick={() => setTab("screenshot")}>{tr("Screenshoty", "Screenshots")}</button>
      </div>
      <MediaList key={tab} kind={tab} />
    </div>
  );
}
