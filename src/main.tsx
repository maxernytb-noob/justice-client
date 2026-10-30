import "./styles.css";
import "./styles-v2.css";
import "./styles-v2-fx.css";
import "./styles-v3.css";
import "./styles-v2-narrow.css";

// Linux runs on WebKitGTK, which stutters with backdrop blur and endless animations: light mode there.
// ?lowfx=1 previews it anywhere.
if ((/Linux/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent)) || new URLSearchParams(location.search).has("lowfx")) {
  document.documentElement.classList.add("lowfx");
}

// No browser context menu / reload shortcuts — this is an app, not a page.
window.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement;
  if (!t.closest("input, textarea, .md, .logbox")) e.preventDefault();
});
window.addEventListener("keydown", (e) => {
  if (e.key === "F5" || (e.ctrlKey && e.key.toLowerCase() === "r")) e.preventDefault();
});

// Uncaught UI errors go to the launcher log (terminal + launcher.log) too.
function logError(msg: string) {
  import("./api").then(({ api }) => api.log("error", msg)).catch(() => {});
}
window.addEventListener("error", (e) => logError(`${e.message} @ ${e.filename}:${e.lineno}`));
window.addEventListener("unhandledrejection", (e) => logError(`unhandled: ${String(e.reason)}`));

async function boot() {
  // In a plain browser (vite dev preview) there is no Tauri backend: load sample data first.
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) await import("./devMock");
  const [{ createRoot }, { default: App }, { StoreProvider }] = await Promise.all([
    import("react-dom/client"),
    import("./App"),
    import("./store"),
  ]);
  createRoot(document.getElementById("root")!).render(
    <StoreProvider>
      <App />
    </StoreProvider>,
  );
}

// The standalone live-log window (opened by the launcher with ?window=logs) is its own tiny app:
// no sidebar, no store — just the streaming log. Keep it out of the main bundle's boot path.
async function bootLogs() {
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) await import("./devMock");
  document.documentElement.classList.add("logswin-root");
  const [{ createRoot }, { default: LogsWindow }, { api }, { setLang }] = await Promise.all([
    import("react-dom/client"),
    import("./LogsWindow"),
    import("./api"),
    import("./i18n"),
  ]);
  // This window has no store, so pick up the launcher's language ourselves — otherwise tr() stays Czech.
  try {
    setLang((await api.getSettings()).language);
  } catch {
    /* fall back to the default language */
  }
  createRoot(document.getElementById("root")!).render(<LogsWindow />);
}

if (new URLSearchParams(location.search).get("window") === "logs") bootLogs();
else boot();
