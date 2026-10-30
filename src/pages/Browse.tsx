import { useEffect, useMemo, useState } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, type ContentKind, type Hit, type InstallReport, type Instance, type ProjectDetails } from "../api";
import { Icon } from "../icons";
import { errText, fmtAgo, fmtNum, LOADER_LABEL, useStore } from "../store";
import { Empty, Modal, SkeletonList } from "../components/common";
import CreateInstance from "../components/CreateInstance";
import MaxPackChat from "../components/MaxPackChat";
import { tr } from "../i18n";
import Select from "../components/Select";

const FEATURED: { slug: string; title: string; text: [string, string]; icon: string }[] = [
  { slug: "flashback", title: "Flashback", text: ["Replay + cinematic kamera", "Replays + cinematic camera"], icon: "film" },
  { slug: "iris", title: "Iris Shaders", text: ["Shadery se Sodiem", "Shaders with Sodium"], icon: "sparkles" },
  { slug: "distanthorizons", title: "Distant Horizons", text: ["Obří dohled na záběry", "Huge view distance for shots"], icon: "globe" },
  { slug: "sodium", title: "Sodium", text: ["Nejvíc FPS", "The most FPS"], icon: "zap" },
  { slug: "freecam", title: "Freecam", text: ["Volná kamera", "Free camera"], icon: "camera" },
  { slug: "zoomify", title: "Zoomify", text: ["Plynulý zoom", "Smooth zoom"], icon: "search" },
];

const CATEGORIES: Record<string, [string, string, string][]> = {
  mod: [
    ["", "Vše", "All"], ["optimization", "Optimalizace", "Optimization"], ["utility", "Utility", "Utility"], ["decoration", "Dekorace", "Decoration"],
    ["adventure", "Dobrodružství", "Adventure"], ["technology", "Technika", "Technology"], ["library", "Knihovny", "Libraries"],
  ],
  shader: [
    ["", "Vše", "All"], ["realistic", "Realistické", "Realistic"], ["semi-realistic", "Polorealistické", "Semi-realistic"],
    ["vanilla-like", "Jako vanilla", "Vanilla-like"], ["cartoon", "Kreslené", "Cartoon"],
  ],
  resourcepack: [
    ["", "Vše", "All"], ["16x", "16x", "16x"], ["32x", "32x", "32x"], ["realistic", "Realistické", "Realistic"],
    ["simplistic", "Jednoduché", "Simplistic"], ["combat", "PvP", "PvP"],
  ],
  modpack: [
    ["", "Vše", "All"], ["optimization", "Optimalizace", "Optimization"], ["adventure", "Dobrodružství", "Adventure"], ["technology", "Technika", "Technology"],
    ["magic", "Magie", "Magic"], ["multiplayer", "Multiplayer", "Multiplayer"], ["lightweight", "Lehké", "Lightweight"], ["kitchen-sink", "Všechno možné", "Kitchen sink"],
  ],
};

const KIND_ICON: Record<ContentKind, string> = { mod: "puzzle", resourcepack: "image", shader: "sparkles", modpack: "box" };
const KIND_NAME: Record<ContentKind, [string, string]> = { mod: ["Mody", "Mods"], resourcepack: ["Resource packy", "Resource packs"], shader: ["Shadery", "Shaders"], modpack: ["Modpacky", "Modpacks"] };

function renderBody(d: ProjectDetails) {
  const html = d.hit.source === "modrinth" ? (marked.parse(d.body, { async: false }) as string) : d.body;
  return DOMPurify.sanitize(html, { FORBID_TAGS: ["iframe", "script", "style"] });
}

function ManualDownloads({ report, onClose }: { report: InstallReport; onClose: () => void }) {
  return (
    <Modal title={tr("Ruční stažení", "Manual download")} onClose={onClose} footer={<button className="btn" onClick={onClose}>{tr("Zavřít", "Close")}</button>}>
      <div className="col">
        <span className="muted">
          {tr(
            "Autoři těchto modů zakázali stahování mimo CurseForge. Stáhni je v prohlížeči a pak je přetáhni do instance.",
            "The authors of these mods disallow downloads outside CurseForge. Download them in your browser and drag them into the instance.",
          )}
        </span>
        <div className="list">
          {report.manual.map(([title, url]) => (
            <div key={url} className="list-row">
              <span className="grow">{title}</span>
              <button className="btn sm" onClick={() => openUrl(url)}>
                <Icon name="external" size={14} /> {tr("Otevřít", "Open")}
              </button>
            </div>
          ))}
        </div>
      </div>
    </Modal>
  );
}

function Details({ hit, kind, inst, installed, onInstall, onClose }: { hit: Hit; kind: ContentKind; inst: Instance; installed: boolean; onInstall: (version?: string) => Promise<void>; onClose: () => void }) {
  const [d, setD] = useState<ProjectDetails | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState("");
  useEffect(() => {
    api.projectDetails(hit.source, hit.id, kind, inst.id).then(setD).catch((e) => setErr(errText(e)));
  }, [hit, kind, inst.id]);
  const install = async (v?: string) => {
    setBusy(v ?? "latest");
    await onInstall(v);
    setBusy("");
  };
  return (
    <Modal
      wide
      title={
        <span className="row">
          <span className="thumb">{hit.iconUrl && <img src={hit.iconUrl} width={36} height={36} alt="" />}</span>
          <span>
            {hit.title}
            <div className="small muted" style={{ fontWeight: 400 }}>
              {tr("od", "by")} {hit.author} · {fmtNum(hit.downloads)} {tr("stažení", "downloads")}
            </div>
          </span>
        </span>
      }
      onClose={onClose}
      footer={
        <>
          <button className="btn ghost" onClick={() => openUrl(hit.url)}>
            <Icon name="external" size={16} /> {hit.source === "modrinth" ? "Modrinth" : "CurseForge"}
          </button>
          <div className="grow" />
          <button className="btn primary" onClick={() => install()} disabled={!!busy || (d !== null && d.versions.length === 0)}>
            {busy === "latest" ? <span className="spinner" /> : <Icon name={installed ? "refresh" : "download"} size={16} />}
            {kind === "modpack" ? tr("Vytvořit instanci", "Create instance") : installed ? tr("Přeinstalovat nejnovější", "Reinstall latest") : tr("Nainstalovat", "Install")}
          </button>
        </>
      }
    >
      {err ? (
        <div className="callout red">{err}</div>
      ) : !d ? (
        <div className="muted">{tr("Načítám…", "Loading…")}</div>
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 260px", gap: 20 }}>
          <div className="col" style={{ minWidth: 0 }}>
            {d.gallery.length > 0 && (
              <div className="row" style={{ overflowX: "auto", gap: 8, paddingBottom: 4 }}>
                {d.gallery.map((g) => (
                  <img key={g} src={g} style={{ height: 120, borderRadius: 8, border: "1px solid var(--line)" }} alt="" />
                ))}
              </div>
            )}
            <div className="md" dangerouslySetInnerHTML={{ __html: renderBody(d) || d.hit.description }} />
          </div>
          <div className="col" style={{ gap: 8 }}>
            <b className="small">{kind === "modpack" ? tr("Verze modpacku", "Modpack versions") : `${tr("Verze pro", "Versions for")} ${LOADER_LABEL[inst.loader]} ${inst.mcVersion}`}</b>
            {d.versions.length === 0 && <div className="callout red small">{tr("Pro tuhle verzi/loader neexistuje žádná verze.", "There is no version for this game version / loader.")}</div>}
            {d.versions.slice(0, 20).map((v) => (
              <div key={v.id} className="card" style={{ padding: "8px 10px" }}>
                <div className="row" style={{ gap: 6 }}>
                  <span className="ellipsis grow small" style={{ fontWeight: 600 }} title={v.name}>{v.versionNumber || v.name}</span>
                  {v.kind !== "release" && <span className={`badge ${v.kind === "beta" ? "amber" : "red"}`}>{v.kind}</span>}
                  <button className="btn sm icon" title={tr("Nainstalovat tuhle verzi", "Install this version")} onClick={() => install(v.id)} disabled={!!busy}>
                    {busy === v.id ? <span className="spinner" /> : <Icon name="download" size={13} />}
                  </button>
                </div>
                <div className="small muted">
                  {kind === "modpack" && v.gameVersions.length > 0 && `${[...v.loaders, v.gameVersions[v.gameVersions.length - 1]].join(" ")} · `}
                  {fmtAgo(Date.parse(v.date) / 1000)}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </Modal>
  );
}

export default function Browse({ instanceId, initialKind, initialQuery }: { instanceId?: string; initialKind?: string; initialQuery?: string }) {
  const { instances, settings, system, toast, go, reloadInstances } = useStore();
  const modable = instances;
  const [instId, setInstId] = useState(instanceId ?? modable.find((i) => i.loader !== "vanilla")?.id ?? modable[0]?.id ?? "");
  const inst = instances.find((i) => i.id === instId);
  const [source, setSource] = useState<"modrinth" | "curseforge">("modrinth");
  const [kind, setKind] = useState<ContentKind>((initialKind as ContentKind) || "mod");
  const [query, setQuery] = useState(initialQuery ?? "");
  const [debounced, setDebounced] = useState(initialQuery ?? "");
  const [sort, setSort] = useState("relevance");
  const [category, setCategory] = useState("");
  const [hits, setHits] = useState<Hit[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState("");
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [installedTitles, setInstalledTitles] = useState<string[]>([]);
  const [installing, setInstalling] = useState<Set<string>>(new Set());
  const [maxOpen, setMaxOpen] = useState(false);
  const [open, setOpen] = useState<Hit | null>(null);
  const [manual, setManual] = useState<InstallReport | null>(null);
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    const h = setTimeout(() => setDebounced(query), 300);
    return () => clearTimeout(h);
  }, [query]);
  useEffect(() => setCategory(""), [kind]);

  const refreshInstalled = () => {
    if (!instId) return;
    api.listContent(instId).then((l) => {
      setInstalled(new Set(l.map((x) => x.projectId).filter(Boolean)));
      setInstalledTitles(l.map((x) => x.title.toLowerCase()));
    });
  };
  useEffect(refreshInstalled, [instId]);

  const search = async (offset: number) => {
    if (!inst) return;
    setLoading(true);
    setErr("");
    try {
      // Modpacks bring their own version and loader, so they aren't filtered by the instance.
      const pack = kind === "modpack";
      const r = await api.search({ source, query: debounced, kind, gameVersion: pack ? "" : inst.mcVersion, loader: pack ? "" : inst.loader, sort, offset, category: source === "modrinth" ? category : "" });
      setHits((h) => (offset ? [...h, ...r.hits] : r.hits));
      setTotal(r.total);
    } catch (e) {
      setErr(errText(e));
      if (!offset) setHits([]);
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => {
    search(0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source, kind, debounced, sort, instId, category]);

  const installPack = async (h: { id: string; title: string; source: string }, version?: string) => {
    // Non-blocking: hand the download to the top-bar queue and let the user keep browsing.
    try {
      await api.downloadModpack(h.source, h.id, h.title, version);
      toast(tr(`Stahuju ${h.title} – průběh nahoře v liště`, `Downloading ${h.title} – progress is in the top bar`), "info");
      setOpen(null);
    } catch (e) {
      toast(`${h.title}: ${errText(e)}`, "error");
    }
  };

  const install = async (h: { id: string; slug?: string; title: string; source: string }, version?: string) => {
    if (kind === "modpack") return installPack(h, version);
    if (!inst) return;
    setInstalling((s) => new Set(s).add(h.id));
    try {
      const r = await api.installContent(inst.id, h.source, h.id, kind, version);
      if (r.manual.length) setManual(r);
      const extra = r.installed.filter((x) => x !== h.title);
      toast(
        r.installed.length
          ? `${tr("Nainstalováno", "Installed")}: ${h.title}${extra.length ? ` + ${tr("závislosti", "dependencies")} (${extra.join(", ")})` : ""}`
          : `${h.title}: ${tr("nic nenainstalováno", "nothing installed")}`,
        "success",
      );
      refreshInstalled();
    } catch (e) {
      toast(`${h.title}: ${errText(e)}`, "error");
    } finally {
      setInstalling((s) => {
        const n = new Set(s);
        n.delete(h.id);
        return n;
      });
    }
  };

  const noCfKey = source === "curseforge" && !settings?.curseforgeKey && !system?.hasCfKey;
  const showFeatured = kind === "mod" && !debounced && source === "modrinth" && inst && (inst.loader === "fabric" || inst.loader === "quilt");
  const featuredDone = useMemo(
    () => new Set(FEATURED.filter((f) => installedTitles.some((tt) => tt.startsWith(f.title.split(" ")[0].toLowerCase()))).map((f) => f.slug)),
    [installedTitles],
  );

  if (!instances.length)
    return (
      <div className="page">
        <Empty
          icon="puzzle"
          title={tr("Nejdřív si vytvoř instanci", "Create an instance first")}
          text={tr("Mody se instalují do konkrétní instance, aby seděla verze a loader.", "Mods are installed into a specific instance so the version and loader match.")}
        >
          <button className="btn primary" onClick={() => setCreating(true)}>
            <Icon name="plus" size={16} /> {tr("Nová instance", "New instance")}
          </button>
        </Empty>
        {creating && <CreateInstance onClose={() => { setCreating(false); reloadInstances(); }} />}
      </div>
    );

  return (
    <div className="page">
      <div className="page-head v2">
        {instanceId && (
          <button className="btn ghost icon" onClick={() => go({ page: "instance", id: instanceId, tab: "content" })}>
            <Icon name="left" size={18} />
          </button>
        )}
        <div>
          <h1>{tr("Mody a modpacky", "Mods and modpacks")}</h1>
          <div className="sub">
            {kind === "modpack"
              ? tr("Modpack si vytvoří vlastní instanci", "A modpack creates its own instance")
              : inst
                ? tr(`Pro ${LOADER_LABEL[inst.loader]} ${inst.mcVersion}`, `For ${LOADER_LABEL[inst.loader]} ${inst.mcVersion}`)
                : ""}
          </div>
        </div>
        <div className="grow" />
        {kind === "modpack" && (
          <button className="btn primary" onClick={() => setMaxOpen(true)} title={tr("Nech si poradit a sestavit modpack od MaxPacku", "Let MaxPack suggest and build a modpack")}>
            <Icon name="sparkles" size={16} /> {tr("Sestavit s MaxPackem", "Build with MaxPack")}
          </button>
        )}
        {kind !== "modpack" && (
          <Select className="inst-pick" style={{ width: 280 }} value={instId} onChange={(e) => setInstId(e.target.value)} title={tr("Instalovat do", "Install into")}>
            {instances.map((i) => (
              <option key={i.id} value={i.id}>
                {i.name} — {LOADER_LABEL[i.loader]} {i.mcVersion}
              </option>
            ))}
          </Select>
        )}
        <div className="seg" style={{ margin: 0 }}>
          <button className={source === "modrinth" ? "on" : ""} onClick={() => setSource("modrinth")}>
            Modrinth
          </button>
          <button className={source === "curseforge" ? "on" : ""} onClick={() => setSource("curseforge")}>
            CurseForge
          </button>
        </div>
      </div>

      <div className="kind-tabs">
        {(["mod", "modpack", "shader", "resourcepack"] as ContentKind[]).map((k) => (
          <button key={k} className={kind === k ? "on" : ""} onClick={() => setKind(k)}>
            <span className="ki">
              <Icon name={KIND_ICON[k]} size={18} />
            </span>
            {tr(KIND_NAME[k][0], KIND_NAME[k][1])}
          </button>
        ))}
      </div>

      <div className="row" style={{ marginBottom: 12, gap: 10 }}>
        <div className="search grow big-search">
          <Icon name="search" size={17} />
          <input className="input" placeholder={source === "modrinth" ? tr("Hledat na Modrinthu…", "Search Modrinth…") : tr("Hledat na CurseForge…", "Search CurseForge…")} value={query} onChange={(e) => setQuery(e.target.value)} autoFocus />
        </div>
        <Select className="select" style={{ width: 190, height: 44 }} value={sort} onChange={(e) => setSort(e.target.value)}>
          <option value="relevance">{tr("Relevance", "Relevance")}</option>
          <option value="downloads">{tr("Nejstahovanější", "Most downloaded")}</option>
          <option value="updated">{tr("Aktualizováno", "Updated")}</option>
          <option value="newest">{tr("Nejnovější", "Newest")}</option>
        </Select>
      </div>

      {source === "modrinth" && (
        <div className="chips" style={{ marginBottom: 18, flexWrap: "wrap" }}>
          {CATEGORIES[kind].map(([k, l, le]) => (
            <button key={k} className={`chip ${category === k ? "on" : ""}`} style={{ height: 30, fontSize: 12.5 }} onClick={() => setCategory(k)}>
              {tr(l, le)}
            </button>
          ))}
        </div>
      )}

      {inst?.loader === "vanilla" && kind === "mod" && (
        <div className="callout red" style={{ marginBottom: 14 }}>
          <Icon name="alert" size={18} />
          <span>
            {tr(
              `Instance ${inst.name} je Vanilla — mody v ní nepoběží. Vyber jinou instanci nebo změň loader v nastavení instance.`,
              `Instance ${inst.name} is Vanilla — mods won't run in it. Pick another instance or change the loader in the instance settings.`,
            )}
          </span>
        </div>
      )}

      {showFeatured && (
        <>
          <div className="section-title">{tr("Doporučené pro natáčení", "Recommended for recording")}</div>
          <div className="featured">
            {FEATURED.map((f) => {
              const done = featuredDone.has(f.slug);
              return (
                <div key={f.slug} className="feat">
                  <span className="fi">
                    <Icon name={f.icon} size={20} />
                  </span>
                  <b>{f.title}</b>
                  <div className="small muted" style={{ margin: "2px 0 12px" }}>{tr(f.text[0], f.text[1])}</div>
                  <button className="btn sm" style={{ width: "100%" }} disabled={installing.has(f.slug) || done} onClick={() => install({ id: f.slug, title: f.title, source: "modrinth" })}>
                    {installing.has(f.slug) ? <span className="spinner" /> : <Icon name={done ? "check" : "download"} size={14} />}
                    {done ? tr("Nainstalováno", "Installed") : tr("Nainstalovat", "Install")}
                  </button>
                </div>
              );
            })}
          </div>
          <div className="section-title">{tr("Populární", "Popular")}</div>
        </>
      )}

      {noCfKey ? (
        <Empty
          icon="key"
          title={tr("CurseForge potřebuje API klíč", "CurseForge needs an API key")}
          text={tr("Klíč je zdarma. Vlož ho v Nastavení → Integrace.", "The key is free. Add it in Settings → Integrations.")}
        >
          <button className="btn primary" onClick={() => go({ page: "settings" })}>{tr("Otevřít nastavení", "Open settings")}</button>
        </Empty>
      ) : err ? (
        <div className="callout red">
          <Icon name="alert" size={18} />
          <span>{err}</span>
        </div>
      ) : hits.length === 0 && !loading ? (
        <Empty
          icon="search"
          title={tr("Žádné výsledky", "No results")}
          text={
            kind === "modpack"
              ? tr("Zkus jiné hledání.", "Try another search.")
              : tr(
                  `Pro ${inst ? `${LOADER_LABEL[inst.loader]} ${inst.mcVersion}` : "tuhle instanci"} se nic nenašlo.`,
                  `Nothing found for ${inst ? `${LOADER_LABEL[inst.loader]} ${inst.mcVersion}` : "this instance"}.`,
                )
          }
        />
      ) : (
        <div className="hit-grid">
          {hits.map((h) => {
            const done = kind !== "modpack" && installed.has(h.id);
            return (
              <div key={h.id} className="hit" onClick={() => setOpen(h)}>
                <div className="thumb lg">{h.iconUrl ? <img src={h.iconUrl} width={56} height={56} alt="" loading="lazy" /> : <Icon name={KIND_ICON[kind]} size={22} />}</div>
                <div className="grow">
                  <div className="row" style={{ gap: 8 }}>
                    <b className="ellipsis">{h.title}</b>
                    <span className="small muted ellipsis">{tr("od", "by")} {h.author}</span>
                  </div>
                  <div className="desc">{h.description}</div>
                  <div className="stats">
                    <span>
                      <Icon name="download" size={12} /> {fmtNum(h.downloads)}
                    </span>
                    <span>
                      <Icon name="clock" size={12} /> {fmtAgo(Date.parse(h.updated) / 1000)}
                    </span>
                    {h.categories.slice(0, 3).map((c) => (
                      <span key={c} className="badge">{c}</span>
                    ))}
                  </div>
                </div>
                <div style={{ alignSelf: "center" }}>
                  <button
                    className={`btn ${done ? "" : "primary"} sm`}
                    disabled={installing.has(h.id)}
                    onClick={(e) => {
                      e.stopPropagation();
                      install(h);
                    }}
                  >
                    {installing.has(h.id) ? <span className="spinner" /> : <Icon name={done ? "check" : "download"} size={14} />}
                    {done ? tr("Nainstalováno", "Installed") : kind === "modpack" ? tr("Vytvořit instanci", "Create instance") : tr("Nainstalovat", "Install")}
                  </button>
                </div>
              </div>
            );
          })}
          {hits.length < total && (
            <button className="btn load-more" onClick={() => search(hits.length)} disabled={loading}>
              {loading ? <span className="spinner" /> : null} {tr("Načíst další", "Load more")}
            </button>
          )}
          {loading && hits.length === 0 && <SkeletonList rows={6} />}
        </div>
      )}

      {open && inst && <Details hit={open} kind={kind} inst={inst} installed={installed.has(open.id)} onInstall={(v) => install(open, v)} onClose={() => setOpen(null)} />}
      {manual && <ManualDownloads report={manual} onClose={() => setManual(null)} />}
      {maxOpen && <MaxPackChat onClose={() => setMaxOpen(false)} onBuilt={(i) => go({ page: "instance", id: i.id })} />}
    </div>
  );
}
