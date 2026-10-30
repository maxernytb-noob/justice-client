import { useEffect, useState } from "react";
import { useStore } from "../store";
import { tr } from "../i18n";

/** Animated logo shown while the launcher loads its data; fades out once ready. */
export default function Splash() {
  const { ready } = useStore();
  const [gone, setGone] = useState(false);
  useEffect(() => {
    if (!ready) return;
    const h = setTimeout(() => setGone(true), 450);
    return () => clearTimeout(h);
  }, [ready]);
  if (gone) return null;
  return (
    <div className={`splash ${ready ? "out" : ""}`} aria-busy={!ready} aria-label={tr("Načítám Justice Client", "Loading Justice Client")}>
      <div className="splash-inner">
        <div className="splash-logo">
          <svg width="96" height="96" viewBox="0 0 1024 1024" aria-hidden="true">
            <rect x="32" y="32" width="960" height="960" rx="200" fill="var(--bg-2)" />
            <path className="j" d="M300 232H760V352H300Z" />
            <path className="j" d="M560 232H760V600C760 750 660 824 520 824C400 824 308 764 284 654L464 610C474 654 500 672 528 672C548 672 560 660 560 630Z" />
          </svg>
        </div>
        <div className="splash-title">
          Justice <b>Client</b>
        </div>
        <div className="splash-bar">
          <div />
        </div>
      </div>
    </div>
  );
}
