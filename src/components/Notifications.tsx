import { useEffect, useRef, useState } from "react";
import { Icon } from "../icons";
import { tr } from "../i18n";
import { McIcon } from "./McIcons";
import { fmtAgo, useStore } from "../store";

const KIND_ICON = { info: "bell", success: "check", error: "alert" } as const;

/** Bell next to the profile: the last notifications, so a toast you missed isn't gone for good. */
export default function NotificationBell() {
  const { notifications, markNotificationsRead } = useStore();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const unread = notifications.filter((n) => !n.read).length;

  useEffect(() => {
    if (!open) return;
    markNotificationsRead();
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    addEventListener("mousedown", close);
    addEventListener("keydown", esc);
    return () => {
      removeEventListener("mousedown", close);
      removeEventListener("keydown", esc);
    };
  }, [open, notifications.length]);

  return (
    <div className="bell" ref={ref}>
      <button className={`bell-btn ${unread ? "has" : ""}`} onClick={() => setOpen((o) => !o)} aria-label={tr("Oznámení", "Notifications")}>
        <Icon name="bell" size={18} />
        {unread > 0 && <span className="bell-dot">{unread > 9 ? "9+" : unread}</span>}
      </button>
      {open && (
        <div className="bell-pop">
          <div className="bell-head">{tr("Oznámení", "Notifications")}</div>
          {notifications.length === 0 ? (
            <div className="bell-empty">
              <Icon name="bell" size={20} />
              {tr("Zatím nic nového", "Nothing new yet")}
            </div>
          ) : (
            <div className="bell-list">
              {notifications.map((n) => (
                <div key={n.id} className={`bell-item ${n.kind}`}>
                  {n.icon ? <McIcon name={n.icon} size={18} /> : <Icon name={KIND_ICON[n.kind]} size={15} />}
                  <div>
                    <p>{n.text}</p>
                    <small>{fmtAgo(n.at / 1000)}</small>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
