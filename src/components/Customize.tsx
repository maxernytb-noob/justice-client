import { useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import { Icon, instanceIcon } from "../icons";
import { useStore, type Page } from "../store";
import { InstGlyph, Switch, useAsync } from "./common";
import { tr, t } from "../i18n";
import Select from "./Select";
import { NAV_ALL, uiPrefs, type HomeSection, type NavPage, type UiPrefs } from "../ui";
import { setSoundPrefs, sound } from "../sounds";

const NAV_LABEL: Record<NavPage, [string, string, string]> = {
  home: ["home", "Domů", "Home"],
  instances: ["box", "Instance", "Instances"],
  browse: ["puzzle", "Mody", "Mods"],
  recording: ["video", "Natáčení", "Recording"],
  servers: ["server", "Servery", "Servers"],
  friends: ["users", "Přátelé", "Friends"],
  clips: ["clips", "Klipy", "Clips"],
};

function Seg<T extends string>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <div className="seg">
      {options.map(([v, l]) => (
        <button key={v} className={v === value ? "on" : ""} onClick={() => onChange(v)}>
          {l}
        </button>
      ))}
    </div>
  );
}

/** Settings → Customize: launcher look, sidebar, shortcuts and the in-game menu switch. */
export default function Customize() {
  const { settings, saveSettings, instances } = useStore();
  const [servers] = useAsync(() => api.listServers(), []);
  const [addKind, setAddKind] = useState<"instance" | "server">("instance");
  const [draft, setDraft] = useState<Partial<UiPrefs>>({});
  if (!settings) return null;
  const p = uiPrefs(settings.ui);
  const v = { ...p, ...draft };
  const set = (patch: Partial<UiPrefs>) => {
    setDraft({});
    saveSettings({ ui: { ...p, ...patch } });
  };

  const pickBg = async () => {
    const f = await open({ filters: [{ name: tr("Obrázek", "Picture"), extensions: ["png", "jpg", "jpeg", "webp", "gif"] }], title: tr("Obrázek na pozadí", "Background picture") });
    if (typeof f === "string") set({ bgImage: f });
  };
  const move = (i: number, d: -1 | 1) => {
    const order = [...p.navOrder];
    const j = i + d;
    if (j < 0 || j >= order.length) return;
    [order[i], order[j]] = [order[j], order[i]];
    set({ navOrder: order });
  };
  const toggleHidden = (n: NavPage) => set({ navHidden: p.navHidden.includes(n) ? p.navHidden.filter((x) => x !== n) : [...p.navHidden, n] });
  const candidates =
    addKind === "instance"
      ? instances.filter((i) => !p.shortcuts.some((s) => s.kind === "instance" && s.id === i.id)).map((i) => [i.id, i.name] as const)
      : (servers ?? []).filter((s) => s.instance && !p.shortcuts.some((x) => x.kind === "server" && x.id === s.id)).map((s) => [s.id, s.name] as const);

  return (
    <div className="col" style={{ gap: 14 }}>
      <div className="card pad">
        <div className="setting" style={{ paddingTop: 0, alignItems: "flex-start" }}>
          <div className="bg-preview" style={{ backgroundImage: v.bgImage ? `url("${convertFileSrc(v.bgImage)}")` : undefined }} />
          <div className="txt">
            <b>{tr("Obrázek na pozadí", "Background picture")}</b>
            <span>{tr("Vlastní tapeta za celým launcherem, třeba screenshot z tvého světa.", "Your own wallpaper behind the whole launcher, e.g. a screenshot of your world.")}</span>
            <div className="row" style={{ gap: 8, marginTop: 10 }}>
              <button className="btn sm" onClick={pickBg}>
                <Icon name="image" size={14} /> {tr("Vybrat obrázek", "Choose picture")}
              </button>
              {v.bgImage && (
                <button className="btn sm ghost" onClick={() => set({ bgImage: "" })}>
                  {tr("Odebrat", "Remove")}
                </button>
              )}
            </div>
          </div>
        </div>
        {v.bgImage && (
          <>
            <div className="setting">
              <div className="txt">
                <b>{tr("Ztmavení", "Darkening")}</b>
              </div>
              <input type="range" min={0} max={0.9} step={0.05} value={v.bgDim} style={{ width: 220 }} onChange={(e) => setDraft({ ...draft, bgDim: Number(e.target.value) })} onPointerUp={() => set({ bgDim: v.bgDim })} />
            </div>
            <div className="setting">
              <div className="txt">
                <b>{tr("Rozmazání", "Blur")}</b>
              </div>
              <input type="range" min={0} max={30} step={1} value={v.bgBlur} style={{ width: 220 }} onChange={(e) => setDraft({ ...draft, bgBlur: Number(e.target.value) })} onPointerUp={() => set({ bgBlur: v.bgBlur })} />
            </div>
            <div className="setting">
              <div className="txt">
                <b>{tr("Průhledné panely", "Glass panels")}</b>
                <span>{tr("Karty a menu jsou poloprůhledné a pozadí přes ně prosvítá.", "Cards and menus are translucent and the background shows through.")}</span>
              </div>
              <Switch checked={v.glass} onChange={(x) => set({ glass: x })} />
            </div>
          </>
        )}
        <div className="setting">
          <div className="txt">
            <b>{tr("Zaoblení", "Corners")}</b>
          </div>
          <Seg value={v.radius} options={[["sharp", tr("Ostré", "Sharp")], ["normal", tr("Normální", "Normal")], ["round", tr("Kulaté", "Round")]]} onChange={(x) => set({ radius: x })} />
        </div>
        <div className="setting">
          <div className="txt">
            <b>{tr("Velikost rozhraní", "Interface size")}</b>
            <span>{Math.round(v.scale * 100)} %</span>
          </div>
          <input type="range" min={0.85} max={1.25} step={0.05} value={v.scale} style={{ width: 220 }} onChange={(e) => setDraft({ ...draft, scale: Number(e.target.value) })} onPointerUp={() => set({ scale: v.scale })} />
        </div>
        <div className="setting">
          <div className="txt">
            <b>{tr("Písmo", "Font")}</b>
          </div>
          <Seg value={v.font} options={[["inter", "Inter"], ["system", tr("Systémové", "System")], ["rounded", tr("Oblé", "Rounded")], ["mono", "Mono"]]} onChange={(x) => set({ font: x })} />
        </div>
        <div className="setting">
          <div className="txt">
            <b>{tr("Animace", "Animations")}</b>
            <span>{tr("Vypni na slabších PC nebo když tě ruší.", "Turn off on slower PCs or if they bother you.")}</span>
          </div>
          <Switch checked={v.animations} onChange={(x) => set({ animations: x })} />
        </div>
        <div className="setting">
          <div className="txt">
            <b>{tr("Postranní panel", "Sidebar")}</b>
            <span>{tr("Široký se sekcemi, nebo jen ikony. V úzkém okně se ikony zapnou samy.", "Wide with sections, or icons only. Narrow windows switch to icons by themselves.")}</span>
          </div>
          <Seg value={v.sidebar} options={[["full", tr("Široký", "Wide")], ["compact", tr("Jen ikony", "Icons")]]} onChange={(x) => set({ sidebar: x })} />
        </div>
        <div className="setting">
          <div className="txt">
            <b>{tr("Hustota", "Density")}</b>
            <span>{tr("Kompaktní vejde víc věcí na obrazovku.", "Compact fits more on screen.")}</span>
          </div>
          <Seg value={v.density} options={[["comfortable", tr("Pohodlná", "Comfortable")], ["compact", tr("Kompaktní", "Compact")]]} onChange={(x) => set({ density: x })} />
        </div>
        <div className="setting">
          <div className="txt">
            <b>{tr("Zvuky rozhraní", "Interface sounds")}</b>
            <span>{tr("Krátký zvuk při spuštění hry, dokončení, pádu hry, zprávě od kamaráda a achievementu.", "A short sound when the game starts, something finishes, the game crashes, a friend writes or you earn an achievement.")}</span>
          </div>
          <div className="row" style={{ gap: 10 }}>
            {v.sounds && (
              <>
                <input type="range" min={0} max={1} step={0.05} value={v.soundVolume} style={{ width: 120 }} onChange={(e) => setDraft({ ...draft, soundVolume: Number(e.target.value) })} onPointerUp={() => { set({ soundVolume: v.soundVolume }); setSoundPrefs(true, v.soundVolume); sound("done"); }} />
                <button className="btn sm ghost" onClick={() => sound("launch", true)}>
                  <Icon name="play" size={12} /> {tr("Vyzkoušet", "Try")}
                </button>
              </>
            )}
            <Switch checked={v.sounds} onChange={(x) => set({ sounds: x })} />
          </div>
        </div>
        <div className="setting">
          <div className="txt">
            <b>{tr("Domovská stránka", "Home")}</b>
            <span>{tr("Co na ní chceš vidět.", "What you want to see there.")}</span>
          </div>
          <div className="chips">
            {(
              [
                ["versions", tr("Verze Minecraftu", "Minecraft versions")],
                ["instances", tr("Moje instance", "My instances")],
                ["friends", tr("Kamarádi", "Friends")],
                ["news", tr("Novinky", "News")],
              ] as [HomeSection, string][]
            ).map(([k, l]) => (
              <button key={k} className={`chip ${!v.homeHidden.includes(k) ? "on" : ""}`} onClick={() => set({ homeHidden: v.homeHidden.includes(k) ? v.homeHidden.filter((x) => x !== k) : [...v.homeHidden, k] })}>
                {l}
              </button>
            ))}
          </div>
        </div>
        <div className="setting">
          <div className="txt">
            <b>{tr("Obrázek updatu na pozadí", "Update art in the background")}</b>
            <span>{tr("Na domovské stránce přes celé okno, jako v OneClientu.", "Across the whole window on Home, like in OneClient.")}</span>
          </div>
          <Switch checked={v.homeArt} onChange={(x) => set({ homeArt: x })} />
        </div>
        <div className="setting" style={{ paddingBottom: 0 }}>
          <div className="txt">
            <b>{tr("Úvodní stránka", "Start page")}</b>
          </div>
          <Select className="input" style={{ width: 200 }} value={v.startPage} onChange={(e) => set({ startPage: e.target.value as Page })}>
            {NAV_ALL.map((n) => (
              <option key={n} value={n}>
                {tr(NAV_LABEL[n][1], NAV_LABEL[n][2])}
              </option>
            ))}
          </Select>
        </div>
      </div>

      <div className="card pad">
        <div className="setting" style={{ paddingTop: 0, alignItems: "flex-start" }}>
          <div className="txt">
            <b>{tr("Levý panel", "Sidebar")}</b>
            <span>{tr("Přeskládej šipkami, okem schováš, co nepoužíváš.", "Reorder with the arrows, hide what you don't use with the eye.")}</span>
            <div className="nav-edit" style={{ marginTop: 10, maxWidth: 360 }}>
              {p.navOrder.map((n, i) => {
                const hidden = p.navHidden.includes(n);
                return (
                  <div key={n} className="row" style={{ gap: 8, opacity: hidden ? 0.5 : 1 }}>
                    <Icon name={NAV_LABEL[n][0]} size={17} />
                    <span className="grow">{tr(NAV_LABEL[n][1], NAV_LABEL[n][2])}</span>
                    <button className="btn ghost sm icon" disabled={i === 0} onClick={() => move(i, -1)} aria-label={tr("Nahoru", "Up")}>
                      <Icon name="down" size={14} style={{ transform: "rotate(180deg)" }} />
                    </button>
                    <button className="btn ghost sm icon" disabled={i === p.navOrder.length - 1} onClick={() => move(i, 1)} aria-label={tr("Dolů", "Down")}>
                      <Icon name="down" size={14} />
                    </button>
                    <button className="btn ghost sm icon" onClick={() => toggleHidden(n)} aria-label={hidden ? tr("Zobrazit", "Show") : tr("Skrýt", "Hide")}>
                      <Icon name={hidden ? "eyeoff" : "eye"} size={15} />
                    </button>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
        <div className="setting" style={{ alignItems: "flex-start", paddingBottom: 0 }}>
          <div className="txt">
            <b>{tr("Zkratky v panelu", "Sidebar shortcuts")}</b>
            <span>{tr("Instance (klik = otevřít, dvojklik = hrát) nebo server (klik = připojit).", "Instances (click = open, double-click = play) or servers (click = join).")}</span>
            <div className="nav-edit" style={{ marginTop: 10, maxWidth: 360 }}>
              {p.shortcuts.map((sc) => {
                const inst = sc.kind === "instance" ? instances.find((i) => i.id === sc.id) : undefined;
                const srv = sc.kind === "server" ? servers?.find((s) => s.id === sc.id) : undefined;
                return (
                  <div key={`${sc.kind}:${sc.id}`} className="row" style={{ gap: 8 }}>
                    {inst ? <InstGlyph icon={inst.icon} template={inst.template} size={16} /> : <Icon name="server" size={16} />}
                    <span className="grow ellipsis">{inst?.name ?? srv?.name ?? sc.id}</span>
                    <button className="btn ghost sm icon danger" onClick={() => set({ shortcuts: p.shortcuts.filter((x) => x !== sc) })} aria-label={tr("Odebrat", "Remove")}>
                      <Icon name="x" size={14} />
                    </button>
                  </div>
                );
              })}
              <div className="row" style={{ gap: 8, padding: 0, border: 0, background: "none" }}>
                <Seg value={addKind} options={[["instance", t("Instance")], ["server", tr("Server", "Server")]]} onChange={setAddKind} />
                <Select
                  className="input grow"
                  value=""
                  onChange={(e) => e.target.value && set({ shortcuts: [...p.shortcuts, { kind: addKind, id: e.target.value }] })}
                >
                  <option value="">{candidates.length ? tr("+ Přidat…", "+ Add…") : tr("Není co přidat", "Nothing to add")}</option>
                  {candidates.map(([id, name]) => (
                    <option key={id} value={id}>
                      {name}
                    </option>
                  ))}
                </Select>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="card pad">
        <div className="setting" style={{ padding: 0 }}>
          <div className="txt">
            <b>{tr("Justice Mod (menu ve hře)", "Justice Mod (in-game menu)")}</b>
            <span>
              {tr(
                "Pravý Shift ve hře: PvP HUD, zoom, freelook, natáčení. Když ho vypneš, launcher ho do žádné instance nedá. Témata menu a HUD layouty najdeš přímo ve hře.",
                "Right Shift in-game: PvP HUD, zoom, freelook, recording tools. Turn it off and the launcher won't add it to any instance. Menu themes and HUD layouts are in-game.",
              )}
            </span>
          </div>
          <Switch checked={settings.justiceMod} onChange={(x) => saveSettings({ justiceMod: x })} />
        </div>
      </div>
    </div>
  );
}
