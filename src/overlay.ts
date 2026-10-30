import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

type Msg = { kind: "saving" | "saved" | "error"; title: string; sub: string };

const card = document.getElementById("card")!;
const title = document.getElementById("t")!;
const sub = document.getElementById("s")!;
let outTimer = 0;
let hideTimer = 0;

function show(m: Msg) {
  clearTimeout(outTimer);
  clearTimeout(hideTimer);
  const wasVisible = !card.classList.contains("hidden") && !card.classList.contains("out");
  title.textContent = m.title;
  sub.textContent = m.sub;
  const life = m.kind === "error" ? 3800 : 2800;
  card.style.setProperty("--life", `${life}ms`);
  // Restart CSS animations (check-mark draw, progress bar) by forcing a reflow.
  card.className = "card";
  void card.offsetWidth;
  card.className = `card ${m.kind} ${wasVisible ? "" : "in"}`;
  // Already on screen (saving -> saved): swap content in place instead of sliding in again.
  card.style.opacity = wasVisible ? "1" : "";
  card.style.transform = wasVisible ? "none" : "";
  if (m.kind !== "saving") outTimer = window.setTimeout(hide, life);
}

function hide() {
  card.style.opacity = "";
  card.style.transform = "";
  card.classList.remove("in");
  card.classList.add("out");
  hideTimer = window.setTimeout(() => {
    card.className = "card hidden";
    invoke("clips_overlay_hide");
  }, 320);
}

listen<Msg>("overlay", (e) => show(e.payload));
