import { useEffect, useState } from "react";
import { Icon, instanceIcon } from "../icons";
import { LOADER_LABEL, useStore } from "../store";
import { customIconSrc, ProgressBar, parseIcon, InstGlyph } from "./common";
import { McIcon } from "./McIcons";
import { tr } from "../i18n";

const TIPS: [string, string][] = [
  ["Zapni Režim natáčení v nastavení instance — vypne houpání kamery a zkreslení obrazu.", "Turn on Recording mode in the instance settings — it removes view bobbing and screen distortion."],
  ["Lehký režim vypne Distant Horizons a další náročné mody, když jen hraješ.", "Light mode turns off Distant Horizons and other heavy mods when you're just playing."],
  ["Soubory .jar a .zip můžeš do launcheru rovnou přetáhnout myší.", "You can drag .jar and .zip files straight into the launcher."],
  ["Profily modů přepnou celou sadu modů jedním klikem.", "Mod profiles switch a whole set of mods with one click."],
  ["Flashback replaye najdeš na stránce Natáčení.", "Your Flashback replays are on the Recording page."],
  ["Kamarádovi pošleš stejné mody přes sdílecí kód v nastavení instance.", "Send a friend the same mods with a share code from the instance settings."],
  ["Když hra spadne, zkus tlačítko Automaticky opravit.", "If the game crashes, try the Fix automatically button."],
];

/** Full-window screen while an instance is being prepared and started. */
export default function LaunchOverlay() {
  const { launching, instances, progress, dismissLaunch } = useStore();
  const [tip, setTip] = useState(() => Math.floor(Math.random() * TIPS.length));
  useEffect(() => {
    if (!launching) return;
    const h = setInterval(() => setTip((t) => (t + 1) % TIPS.length), 5000);
    return () => clearInterval(h);
  }, [launching]);
  if (!launching) return null;
  const inst = instances.find((i) => i.id === launching.id);
  const p = progress[launching.id];
  const bytes = p && p.total > 100000;

  return (
    <div className="launch-overlay" role="status" aria-live="polite">
      <div className="launch-card">
        {launching.started ? (
          <div className="done">
            <Icon name="check" size={52} stroke={3} />
          </div>
        ) : (
          <div className="ring">
            {inst && customIconSrc(parseIcon(inst.icon).base) ? (
              <img className="custom-big" src={customIconSrc(parseIcon(inst.icon).base)!} alt="" />
            ) : (
              <span style={{ color: "var(--red)", display: "grid", placeItems: "center" }}>{inst ? <InstGlyph icon={inst.icon} template={inst.template} size={40} /> : <Icon name="play" size={40} />}</span>
            )}
          </div>
        )}
        <div>
          <h2>{launching.started ? tr("Hra běží!", "Game started!") : inst?.name ?? tr("Spouštím", "Starting")}</h2>
          {inst && (
            <div className="muted small">
              {LOADER_LABEL[inst.loader]} {inst.mcVersion}
            </div>
          )}
        </div>
        {!launching.started && (
          <>
            <div className="step">
              {p?.label ?? tr("Připravuji", "Preparing")}…
              {p && p.total > 1 && (
                <span className="muted">
                  {" "}
                  {bytes ? `${(p.done / 1048576).toFixed(0)} / ${(p.total / 1048576).toFixed(0)} MB` : `${p.done} / ${p.total}`}
                </span>
              )}
            </div>
            <ProgressBar done={p?.done ?? 0} total={p?.total ?? 1} />
            <div className="tip" key={tip}>
              <McIcon name="torch" size={16} /> {tr(TIPS[tip][0], TIPS[tip][1])}
            </div>
            <button className="btn ghost sm" onClick={dismissLaunch}>
              {tr("Skrýt", "Hide")}
            </button>
          </>
        )}
      </div>
    </div>
  );
}
