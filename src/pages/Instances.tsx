import { useMemo, useState } from "react";
import { Icon } from "../icons";
import { LOADER_LABEL, useStore } from "../store";
import { Empty } from "../components/common";
import CreateInstance from "../components/CreateInstance";
import { InstanceCard } from "./Home";
import { tr } from "../i18n";

export default function Instances() {
  const { instances } = useStore();
  const [q, setQ] = useState("");
  const [loader, setLoader] = useState("");
  const [creating, setCreating] = useState(false);

  const filtered = useMemo(
    () =>
      instances.filter(
        (i) =>
          (!q || i.name.toLowerCase().includes(q.toLowerCase()) || i.mcVersion.includes(q)) && (!loader || i.loader === loader),
      ),
    [instances, q, loader],
  );
  const groups = useMemo(() => {
    const m = new Map<string, typeof filtered>();
    for (const i of filtered) {
      const g = i.group || "";
      m.set(g, [...(m.get(g) ?? []), i]);
    }
    return [...m.entries()].sort((a, b) => (a[0] === "" ? 1 : b[0] === "" ? -1 : a[0].localeCompare(b[0])));
  }, [filtered]);

  return (
    <div className="page">
      <div className="page-head v2">
        <div>
          <h1>{tr("Instance", "Instances")}</h1>
          <div className="sub">
            {instances.length} {tr("instancí · klikni pro detail, ▶ pro spuštění", "instances · click for details, ▶ to play")}
          </div>
        </div>
        <div className="grow" />
        <div className="search" style={{ width: 240 }}>
          <Icon name="search" size={15} />
          <input className="input" placeholder={tr("Hledat", "Search")} value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <button className="btn primary" onClick={() => setCreating(true)}>
          <Icon name="plus" size={16} /> {tr("Nová instance", "New instance")}
        </button>
      </div>
      <div className="seg" style={{ marginBottom: 18 }}>
        {["", "vanilla", "fabric", "quilt", "forge", "neoforge"]
          .filter((l) => !l || instances.some((i) => i.loader === l))
          .map((l) => (
            <button key={l} className={loader === l ? "on" : ""} onClick={() => setLoader(l)}>
              {l ? LOADER_LABEL[l] : tr("Vše", "All")}
            </button>
          ))}
      </div>
      {instances.length === 0 ? (
        <Empty
          icon="box"
          title={tr("Zatím žádná instance", "No instances yet")}
          text={tr(
            "Instance = samostatná verze Minecraftu s vlastními mody, světy a nastavením.",
            "An instance is a separate copy of Minecraft with its own mods, worlds and settings.",
          )}
        >
          <button className="btn primary" onClick={() => setCreating(true)}>
            <Icon name="plus" size={16} /> {tr("Nová instance", "New instance")}
          </button>
        </Empty>
      ) : (
        groups.map(([g, list]) => (
          <div key={g}>
            {groups.length > 1 && (
              <div className="section-title">
                <Icon name="folder" size={15} /> {g || tr("Bez složky", "No folder")}
              </div>
            )}
            <div className="icards">
              {list.map((i) => (
                <InstanceCard key={i.id} inst={i} />
              ))}
              {g === groups[groups.length - 1][0] && (
                <div className="icard new" onClick={() => setCreating(true)}>
                  <Icon name="plus" size={24} />
                  <span>{tr("Nová instance", "New instance")}</span>
                </div>
              )}
            </div>
          </div>
        ))
      )}
      {creating && <CreateInstance onClose={() => setCreating(false)} />}
    </div>
  );
}
