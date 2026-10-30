import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { check, type Update as AppUpdate } from "@tauri-apps/plugin-updater";

/** A newer launcher version. `manual` = can't self-install (Linux packages), the dialog explains how to update. */
export interface UpdateInfo {
  version: string;
  body?: string;
  manual?: { url: string; kind: LinuxPackage };
  raw?: AppUpdate;
}
const IS_LINUX = navigator.userAgent.includes("Linux");
import { relaunch } from "@tauri-apps/plugin-process";
import { api, type Account, type Download, type GameExit, type Instance, type LinuxPackage, type Progress, type Settings, type SystemInfo } from "./api";
import { locale, setLang, tr } from "./i18n";
import { applyTheme } from "./theme";
import { sound } from "./sounds";
import { applyUi, uiPrefs } from "./ui";

export type Page = "home" | "instances" | "instance" | "browse" | "recording" | "clips" | "friends" | "servers" | "settings" | "accounts";
export interface Nav {
  page: Page;
  id?: string;
  tab?: string;
  /** Search text to start with (Browse). */
  q?: string;
}
interface Toast {
  id: number;
  text: string;
  kind: "info" | "error" | "success";
  /** Pixel icon name (components/McIcons) shown instead of an emoji. */
  icon?: string;
}
export interface Notice extends Toast {
  at: number;
  read: boolean;
}

interface Store {
  settings: Settings | null;
  saveSettings: (patch: Partial<Settings>) => Promise<void>;
  system: SystemInfo | null;
  accounts: Account[];
  activeAccount: Account | undefined;
  reloadAccounts: () => Promise<void>;
  instances: Instance[];
  reloadInstances: () => Promise<void>;
  running: string[];
  progress: Record<string, Progress>;
  downloads: Download[];
  nav: Nav;
  go: (n: Nav) => void;
  back: () => void;
  forward: () => void;
  canBack: boolean;
  canForward: boolean;
  toast: (text: string, kind?: Toast["kind"], icon?: string) => void;
  toasts: Toast[];
  /** Everything that was toasted lately, newest first, for the bell in the sidebar. */
  notifications: Notice[];
  markNotificationsRead: () => void;
  crash: GameExit | null;
  clearCrash: () => void;
  play: (id: string, server?: string) => Promise<void>;
  /** Initial data (settings, instances, accounts) has loaded — the splash can go. */
  ready: boolean;
  /** Instance currently being launched from the UI, and whether the game already started. */
  launching: { id: string; started: boolean } | null;
  dismissLaunch: () => void;
  update: UpdateInfo | null;
  updateProgress: { done: number; total: number } | null;
  checkUpdate: (manual?: boolean) => Promise<void>;
  installUpdate: () => Promise<void>;
}

const Ctx = createContext<Store>(null!);
export const useStore = () => useContext(Ctx);

export function errText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

export function StoreProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [instances, setInstances] = useState<Instance[]>([]);
  const [running, setRunning] = useState<string[]>([]);
  const [progress, setProgress] = useState<Record<string, Progress>>({});
  const [downloads, setDownloads] = useState<Download[]>([]);
  // Browser-like history: back moves the index, a new page drops what was ahead of it.
  const [hist, setHist] = useState<{ items: Nav[]; i: number }>({ items: [{ page: "home" }], i: 0 });
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [notifications, setNotifications] = useState<Notice[]>([]);
  const [crash, setCrash] = useState<GameExit | null>(null);
  const [ready, setReady] = useState(false);
  const [launching, setLaunching] = useState<{ id: string; started: boolean } | null>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [updateProgress, setUpdateProgress] = useState<{ done: number; total: number } | null>(null);
  const toastId = useRef(0);

  const toast = useCallback((text: string, kind: Toast["kind"] = "info", icon?: string) => {
    const id = ++toastId.current;
    setToasts((t) => [...t, { id, text, kind, icon }]);
    setNotifications((n) => [{ id, text, kind, icon, at: Date.now(), read: false }, ...n].slice(0, 30));
    if (kind === "error") api.log("error", text);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), kind === "error" ? 9000 : 4000);
  }, []);

  const reloadAccounts = useCallback(async () => setAccounts(await api.listAccounts()), []);
  const reloadInstances = useCallback(async () => setInstances(await api.listInstances()), []);

  useEffect(() => {
    const settingsLoaded = api.getSettings().then((s) => {
      setLang(s.language);
      applyTheme(s.accentColor, s.bgStyle);
      applyUi(uiPrefs(s.ui));
      const start = uiPrefs(s.ui).startPage;
      if (start && start !== "home") setHist({ items: [{ page: start }], i: 0 });
      setSettings(s);
    });
    api.systemInfo().then(setSystem);
    // Keep the splash up for at least a moment so it doesn't just flash.
    const minSplash = new Promise((r) => setTimeout(r, 1200));
    Promise.allSettled([settingsLoaded, reloadAccounts(), reloadInstances(), minSplash]).then(() => setReady(true));
    api.running().then(setRunning);
    api.downloadList().then(setDownloads).catch(() => {});

    const un = [
      listen<Download[]>("downloads", ({ payload }) => {
        setDownloads((prev) => {
          const wasActive = prev.some((d) => d.status === "running" || d.status === "paused");
          const nowActive = payload.some((d) => d.status === "running" || d.status === "paused");
          // A modpack that just finished downloading: refresh the instance list.
          if (wasActive && !nowActive) reloadInstances();
          return payload;
        });
      }),
      listen<Progress>("progress", ({ payload }) => {
        setProgress((p) => {
          const next = { ...p };
          if (payload.total === 0 && payload.done === 1) delete next[payload.task];
          else next[payload.task] = payload;
          return next;
        });
      }),
      listen<{ instance: string; fixed: string[] }>("compat-fixed", ({ payload }) =>
        toast(`${tr("Opraveny nekompatibilní mody", "Fixed incompatible mods")}:\n${payload.fixed.join("\n")}`, "success"),
      ),
      listen<number>("low-disk", ({ payload }) =>
        toast(
          tr(
            `Na disku dochází místo (zbývá ${(payload / 1024 ** 3).toFixed(1)} GB). Přesuň data v Nastavení → Přesunout data.`,
            `Your disk is running out of space (${(payload / 1024 ** 3).toFixed(1)} GB left). Move your data in Settings → Move data.`,
          ),
          "error",
        ),
      ),
      listen<string>("game-start", ({ payload }) => {
        setRunning((r) => [...new Set([...r, payload])]);
        setLaunching((l) => (l && l.id === payload ? { ...l, started: true } : l));
        setTimeout(() => setLaunching((l) => (l && l.id === payload ? null : l)), 1600);
      }),
      // A modpack installed from the in-game chat.
      listen("instances-changed", () => reloadInstances()),
      listen<{ text: string; kind: Toast["kind"] }>("toast", ({ payload }) => toast(payload.text, payload.kind)),
      listen<GameExit>("game-exit", ({ payload }) => {
        setRunning((r) => r.filter((x) => x !== payload.instance));
        reloadInstances();
        if (payload.crash) {
          sound("error");
          setCrash(payload);
        }
      }),
    ];
    return () => un.forEach((p) => p.then((f) => f()));
  }, [reloadAccounts, reloadInstances, toast]);

  const saveSettings = useCallback(
    async (patch: Partial<Settings>) => {
      if (!settings) return;
      const next = { ...settings, ...patch };
      setSettings(next);
      setLang(next.language);
      applyTheme(next.accentColor, next.bgStyle);
      applyUi(uiPrefs(next.ui));
      await api.saveSettings(next);
    },
    [settings],
  );

  const play = useCallback(
    async (id: string, server?: string) => {
      try {
        setLaunching({ id, started: false });
        sound("launch");
        setProgress((p) => ({ ...p, [id]: { task: id, label: tr("Spouštím", "Starting"), done: 0, total: 1 } }));
        await api.launch(id, server);
      } catch (e) {
        setLaunching(null);
        setProgress((p) => {
          const n = { ...p };
          delete n[id];
          return n;
        });
        toast(errText(e), "error");
      }
    },
    [toast],
  );

  const checkUpdate = useCallback(
    async (manual = false) => {
      try {
        let u: UpdateInfo | null = null;
        if (IS_LINUX) {
          const m = await api.checkUpdateManual();
          if (m) u = { version: m.version, body: m.notes, manual: { url: m.url, kind: m.kind } };
        } else {
          const raw = await check();
          if (raw) u = { version: raw.version, body: raw.body, raw };
        }
        api.log("info", `update check: ${u ? "new version " + u.version : "up to date"}`);
        setUpdate(u);
        if (manual && !u) toast(tr("Máš nejnovější verzi", "You're on the latest version"), "success");
      } catch (e) {
        api.log("warn", `update check failed: ${errText(e)}`);
        // Update feed unreachable (offline).
        if (manual) toast(`${tr("Kontrola aktualizací selhala", "Update check failed")}: ${errText(e)}`, "error");
      }
    },
    [toast],
  );

  const installUpdate = useCallback(async () => {
    if (!update?.raw) return;
    try {
      let total = 0;
      let done = 0;
      setUpdateProgress({ done: 0, total: 0 });
      await update.raw.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        if (ev.event === "Progress") done += ev.data.chunkLength;
        setUpdateProgress({ done, total });
      });
      await relaunch();
    } catch (e) {
      setUpdateProgress(null);
      toast(`${tr("Aktualizace selhala", "Update failed")}: ${errText(e)}`, "error");
    }
  }, [update, toast]);

  useEffect(() => {
    checkUpdate();
    const h = setInterval(() => checkUpdate(), 1000 * 60 * 60 * 3);
    return () => clearInterval(h);
  }, [checkUpdate]);

  const nav = hist.items[hist.i];
  const value = useMemo<Store>(
    () => ({
      settings,
      saveSettings,
      system,
      accounts,
      activeAccount: accounts.find((a) => a.active),
      reloadAccounts,
      instances,
      reloadInstances,
      running,
      progress,
      downloads,
      nav,
      go: (n) =>
        setHist((h) => {
          const items = [...h.items.slice(0, h.i + 1), n].slice(-30);
          return { items, i: items.length - 1 };
        }),
      back: () => setHist((h) => ({ ...h, i: Math.max(0, h.i - 1) })),
      forward: () => setHist((h) => ({ ...h, i: Math.min(h.items.length - 1, h.i + 1) })),
      canBack: hist.i > 0,
      canForward: hist.i < hist.items.length - 1,
      toast,
      toasts,
      notifications,
      markNotificationsRead: () => setNotifications((n) => (n.some((x) => !x.read) ? n.map((x) => ({ ...x, read: true })) : n)),
      crash,
      clearCrash: () => setCrash(null),
      play,
      ready,
      launching,
      dismissLaunch: () => setLaunching(null),
      update,
      updateProgress,
      checkUpdate,
      installUpdate,
    }),
    [settings, saveSettings, system, accounts, reloadAccounts, instances, reloadInstances, running, progress, downloads, nav, hist, toast, toasts, notifications, crash, play, ready, launching, update, updateProgress, checkUpdate, installUpdate],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function fmtDuration(secs: number): string {
  if (secs < 60) return `${secs} s`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  return `${h} h ${m % 60} min`;
}

export function fmtAgo(ts: number): string {
  if (!ts) return tr("nikdy", "never");
  const d = Date.now() / 1000 - ts;
  if (d < 60) return tr("právě teď", "just now");
  if (d < 3600) return tr(`před ${Math.floor(d / 60)} min`, `${Math.floor(d / 60)} min ago`);
  if (d < 86400) return tr(`před ${Math.floor(d / 3600)} h`, `${Math.floor(d / 3600)} h ago`);
  const days = Math.floor(d / 86400);
  if (days === 1) return tr("včera", "yesterday");
  if (days < 30) return tr(`před ${days} dny`, `${days} days ago`);
  return new Date(ts * 1000).toLocaleDateString(locale());
}

export function fmtSize(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 ** 2) return `${(b / 1024).toFixed(0)} KB`;
  if (b < 1024 ** 3) return `${(b / 1024 ** 2).toFixed(1)} MB`;
  return `${(b / 1024 ** 3).toFixed(2)} GB`;
}

export function fmtNum(n: number): string {
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(n >= 1e4 ? 0 : 1)}k`;
  return String(n);
}

export const LOADER_LABEL: Record<string, string> = {
  vanilla: "Vanilla",
  fabric: "Fabric",
  quilt: "Quilt",
  forge: "Forge",
  neoforge: "NeoForge",
};
