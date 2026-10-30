// Easter eggs. Found ones are remembered per player (localStorage) and listed in Settings.
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { tr } from "../i18n";
import { useStore } from "../store";
import { applyTheme } from "../theme";
import { McIcon, mcSvg } from "./McIcons";
import { sound } from "../sounds";
import Logo from "./Logo";

const SECRETS: Record<string, [string, string]> = {
  logo: ["Tajný zvuk při startu", "Secret startup sound"],
  konami: ["Creeper za zády", "Creeper behind you"],
  jeb: ["jeb_", "jeb_"],
  dinnerbone: ["Dinnerbone", "Dinnerbone"],
  herobrine: ["Herobrine", "Herobrine"],
  disco: ["Disko", "Disco"],
  diamonds: ["Diamantový déšť", "Diamond rain"],
  speedrun: ["Speedrunner", "Speedrunner"],
};
const HINTS: Record<string, [string, string]> = {
  logo: ["Logo vydrží hodně klikání…", "The logo can take a lot of clicks…"],
  konami: ["Staré herní kódy pořád fungují. ↑↑…", "Old cheat codes still work. ↑↑…"],
  jeb: ["Jedna ovce v Minecraftu má hodně barev.", "One sheep in Minecraft has many colours."],
  dinnerbone: ["Někteří moby chodí vzhůru nohama.", "Some mobs walk upside down."],
  herobrine: ["Prý byl odstraněn. Zkus napsat jeho jméno.", "Supposedly removed. Try typing his name."],
  disco: ["Obal písničky má rád rytmus.", "The song cover likes a beat."],
  diamonds: ["Hledej ty nejcennější rudy.", "Search for the most precious ore."],
  speedrun: ["Speedrunneři drží Shift.", "Speedrunners hold Shift."],
};

// ---------- Found list ----------
const KEY = "jc.secrets";
const subs = new Set<() => void>();
let cache = read();
function read(): string[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) || "[]");
  } catch {
    return [];
  }
}
let toastFn: ((t: string, k?: "info" | "success" | "error", icon?: string) => void) | null = null;
let theme: { accentColor: string; bgStyle: string } | null = null;
function found(id: string) {
  if (cache.includes(id)) return;
  cache = [...cache, id];
  try {
    localStorage.setItem(KEY, JSON.stringify(cache));
  } catch {}
  subs.forEach((f) => f());
  const n = Object.keys(SECRETS).length;
  setTimeout(() => sound("achievement"), 600);
  setTimeout(() => toastFn?.(`${tr("Tajemství nalezeno", "Secret found")} (${cache.length}/${n}): ${tr(...SECRETS[id])}`, "success", "trophy"), 600);
}

// ---------- Sound (Web Audio, nothing to download) ----------
let actx: AudioContext | null = null;
const ac = () => (actx ||= new AudioContext());
function tone(type: OscillatorType, f0: number, f1: number, dur: number, gain: number, delay = 0) {
  const a = ac(), t = a.currentTime + delay, o = a.createOscillator(), g = a.createGain();
  o.type = type;
  o.frequency.setValueAtTime(f0, t);
  o.frequency.exponentialRampToValueAtTime(f1, t + dur);
  g.gain.setValueAtTime(0.0001, t);
  g.gain.exponentialRampToValueAtTime(gain, t + 0.02);
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  o.connect(g).connect(a.destination);
  o.start(t);
  o.stop(t + dur + 0.05);
}
function noise(dur: number, type: BiquadFilterType, freq: number, gain: number, delay = 0) {
  const a = ac(), t = a.currentTime + delay, len = Math.floor(a.sampleRate * dur), buf = a.createBuffer(1, len, a.sampleRate), d = buf.getChannelData(0);
  for (let i = 0; i < len; i++) d[i] = Math.random() * 2 - 1;
  const src = a.createBufferSource(), f = a.createBiquadFilter(), g = a.createGain();
  src.buffer = buf;
  f.type = type;
  f.frequency.value = freq;
  g.gain.setValueAtTime(gain, t);
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  src.connect(f).connect(g).connect(a.destination);
  src.start(t);
}
function boing() {
  const a = ac(), t = a.currentTime, o = a.createOscillator(), g = a.createGain(), lfo = a.createOscillator(), lg = a.createGain();
  o.type = "triangle";
  o.frequency.setValueAtTime(140, t);
  o.frequency.exponentialRampToValueAtTime(620, t + 0.12);
  o.frequency.exponentialRampToValueAtTime(90, t + 0.7);
  lfo.frequency.value = 22;
  lg.gain.value = 45;
  lfo.connect(lg).connect(o.frequency);
  g.gain.setValueAtTime(0.0001, t);
  g.gain.exponentialRampToValueAtTime(0.5, t + 0.02);
  g.gain.exponentialRampToValueAtTime(0.0001, t + 0.75);
  o.connect(g).connect(a.destination);
  o.start(t);
  lfo.start(t);
  o.stop(t + 0.8);
  lfo.stop(t + 0.8);
  [523, 659, 784, 1047].forEach((f, i) => tone("square", f, f, i === 3 ? 0.45 : 0.12, 0.12, 0.75 + i * 0.1));
}
function wahwah() {
  [[392, 0, 0.35], [370, 0.38, 0.35], [349, 0.76, 0.35], [330, 1.14, 1.0]].forEach(([f, d, len]) => tone("sawtooth", f, len > 0.5 ? f - 18 : f, len, 0.18, d));
}

// ---------- Visual bits ----------
export function burst(x: number, y: number, n = 18) {
  const cs = getComputedStyle(document.documentElement);
  const colors = [cs.getPropertyValue("--red").trim() || "#e11d38", "#fff", cs.getPropertyValue("--accent-2").trim() || "#ff6b80"];
  for (let i = 0; i < n; i++) {
    const c = document.createElement("i");
    c.className = "fx-conf";
    const a = Math.random() * Math.PI * 2, d = 60 + Math.random() * 90;
    c.style.cssText = `left:${x}px;top:${y}px;background:${colors[i % 3]};--dx:${Math.cos(a) * d}px;--dy:${Math.sin(a) * d - 40}px;--rot:${Math.random() * 720 - 360}deg`;
    document.body.append(c);
    setTimeout(() => c.remove(), 1000);
  }
}
export function burstAt(el: Element, n = 18) {
  const r = el.getBoundingClientRect();
  burst(r.left + r.width / 2, r.top + r.height / 2, n);
}
export function playNotes(el: Element) {
  const r = el.getBoundingClientRect();
  [0, 1, 2].forEach((i) => {
    const s = document.createElement("span");
    s.className = "fx-note";
    s.innerHTML = mcSvg("note", 18);
    s.style.cssText = `left:${r.left + r.width / 2 - 8}px;top:${r.top}px;--dx:${(i - 1) * 34}px;--rot:${(i - 1) * 25}deg;animation-delay:${i * 0.08}s`;
    document.body.append(s);
    setTimeout(() => s.remove(), 1400);
  });
}

// ---------- Logo: 10 clicks toggle a sound that plays at every start ----------
const SOUND_KEY = "jc.egg.sound";
const soundOn = () => {
  try {
    return localStorage.getItem(SOUND_KEY) === "1";
  } catch {
    return false;
  }
};
export default function LogoButton() {
  const [on, setOn] = useState(soundOn);
  const [n, setN] = useState(0);
  const btn = useRef<HTMLButtonElement>(null);
  const img = useRef<HTMLDivElement>(null);
  const reset = useRef(0);
  const anim = (c: string) => {
    const el = img.current;
    if (!el) return;
    el.classList.remove("squish", "boom", "sad");
    void el.offsetWidth;
    el.classList.add(c);
  };
  useEffect(() => {
    if (!on) return;
    // Startup sound; the webview may hold audio until the first click.
    const play = () => {
      boing();
      anim("boom");
    };
    ac().resume().then(() => (actx!.state === "running" ? play() : addEventListener("pointerdown", () => ac().resume().then(play), { once: true })));
  }, []);
  const click = () => {
    clearTimeout(reset.current);
    const next = n + 1;
    if (next < 10) {
      setN(next);
      anim("squish");
      tone("sine", 420 + next * 70, 420 + next * 70, 0.08, 0.08);
      reset.current = window.setTimeout(() => setN(0), 1600);
      return;
    }
    setN(0);
    const v = !on;
    setOn(v);
    try {
      localStorage.setItem(SOUND_KEY, v ? "1" : "0");
    } catch {}
    if (v) {
      anim("boom");
      boing();
      if (btn.current) {
        burstAt(btn.current, 40);
        const r = btn.current.getBoundingClientRect(), w = document.createElement("i");
        w.className = "fx-shock";
        w.style.left = `${r.left + r.width / 2}px`;
        w.style.top = `${r.top + r.height / 2}px`;
        document.body.append(w);
        setTimeout(() => w.remove(), 700);
      }
      toastFn?.(tr("Tajný zvuk zapnutý – zazní při každém startu", "Secret sound on – it plays at every start"), "info", "note");
      found("logo");
    } else {
      anim("sad");
      wahwah();
      toastFn?.(tr("Tajný zvuk vypnutý", "Secret sound off"), "info", "lock");
    }
  };
  return (
    <button ref={btn} className={`logo-btn ${n ? "counting" : ""} ${n >= 7 ? "shake" : ""} ${on ? "egg-on" : ""}`} onClick={click} aria-label="Justice Client">
      <svg className="ring" viewBox="0 0 60 60">
        <circle cx="30" cy="30" r="27" style={{ strokeDashoffset: 170 - (170 * n) / 10 }} />
      </svg>
      <div className="logo-img" ref={img}>
        <Logo size={44} />
      </div>
      <span key={n} className="count">
        {10 - n}
      </span>
    </button>
  );
}

// ---------- Keyboard secrets, the cover and search ----------
const KONAMI = ["arrowup", "arrowup", "arrowdown", "arrowdown", "arrowleft", "arrowright", "arrowleft", "arrowright", "b", "a"];
let rainbow = 0;
function creeper() {
  const app = document.querySelector(".shell");
  const face = document.createElement("div");
  face.className = "fx-creeper";
  face.innerHTML = "<i></i><i></i><b></b>";
  document.body.append(face);
  noise(1.5, "bandpass", 3200, 0.25);
  app?.classList.add("fx-fuse");
  setTimeout(() => {
    app?.classList.remove("fx-fuse");
    face.classList.add("blow");
    noise(1.4, "lowpass", 600, 0.9);
    tone("sine", 120, 30, 0.9, 0.6);
    document.body.classList.add("fx-flash");
    burstAt(face, 70);
    setTimeout(() => {
      document.body.classList.remove("fx-flash");
      face.remove();
    }, 900);
    found("konami");
  }, 1500);
}
function jeb() {
  const st = document.documentElement.style;
  if (rainbow) {
    clearInterval(rainbow);
    rainbow = 0;
    if (theme) applyTheme(theme.accentColor, theme.bgStyle);
    toastFn?.("jeb_ " + tr("odešel", "left"), "info", "sheep");
    return;
  }
  let h = 0;
  rainbow = window.setInterval(() => {
    h = (h + 4) % 360;
    st.setProperty("--red", `hsl(${h} 90% 55%)`);
    st.setProperty("--red-2", `hsl(${h} 90% 48%)`);
    st.setProperty("--accent-2", `hsl(${h} 95% 70%)`);
    st.setProperty("--accent-glow", `hsl(${h} 90% 55% / .35)`);
    st.setProperty("--accent-soft", `hsl(${h} 90% 55% / .14)`);
  }, 80);
  tone("square", 400, 900, 0.25, 0.08);
  tone("square", 600, 1200, 0.25, 0.08, 0.12);
  toastFn?.("jeb_ " + tr("je tady", "is here"), "info", "sheep");
  found("jeb");
}
function dinnerbone() {
  const app = document.querySelector(".shell");
  app?.classList.add("fx-flip");
  tone("triangle", 300, 150, 0.6, 0.15);
  setTimeout(() => {
    app?.classList.remove("fx-flip");
    tone("triangle", 150, 300, 0.6, 0.15);
  }, 4000);
  found("dinnerbone");
}
function herobrine() {
  const o = document.createElement("div");
  o.className = "fx-herobrine";
  o.innerHTML = `<div class="eyes"><i></i><i></i></div><div class="msg">${tr("Herobrine byl odstraněn.", "Removed Herobrine.")}</div>`;
  document.body.append(o);
  tone("sawtooth", 55, 50, 3.5, 0.18);
  tone("sine", 82, 78, 3.5, 0.12);
  noise(3, "lowpass", 300, 0.15);
  setTimeout(() => o.classList.add("out"), 3200);
  setTimeout(() => o.remove(), 3900);
  found("herobrine");
}

let coverClicks: number[] = [];
let disco = 0;
/** 5 quick clicks on the song cover start a 10-second disco. */
export function coverClicked() {
  const now = Date.now();
  coverClicks = coverClicks.filter((t) => now - t < 1500).concat(now);
  if (coverClicks.length < 5 || disco) return;
  coverClicks = [];
  document.body.classList.add("fx-disco");
  let beat = 0;
  disco = window.setInterval(() => {
    tone("sine", 150, 45, 0.18, 0.5);
    if (beat % 2) noise(0.06, "highpass", 7000, 0.18);
    beat++;
    document.body.style.setProperty("--disco", `${(beat * 47) % 360}deg`);
  }, 250);
  setTimeout(() => {
    clearInterval(disco);
    disco = 0;
    document.body.classList.remove("fx-disco");
  }, 10000);
  toastFn?.(tr("Disko!", "Disco!"), "info", "disc");
  found("disco");
}

/** Search box hook: returns true when the query was a secret (and was handled). */
export function secretSearch(q: string): boolean {
  if (!/^(diamond|diamonds|diamant|diamanty)$/i.test(q)) return false;
  for (let i = 0; i < 60; i++) {
    const d = document.createElement("span");
    d.className = "fx-diamond";
    const sz = Math.round(16 + Math.random() * 22);
    d.innerHTML = mcSvg("diamond", sz);
    d.style.cssText = `left:${Math.random() * 100}vw;font-size:${sz}px;animation-duration:${1.6 + Math.random() * 1.8}s;animation-delay:${Math.random() * 1.2}s;--rot:${Math.random() * 720 - 360}deg`;
    document.body.append(d);
    setTimeout(() => d.remove(), 4800);
    if (i % 6 === 0) tone("sine", 1400 + Math.random() * 900, 1800 + Math.random() * 900, 0.15, 0.05, Math.random() * 1.8);
  }
  toastFn?.(tr("Našel jsi diamanty!", "You found diamonds!"), "success", "diamond");
  found("diamonds");
  return true;
}

let srRaf = 0;
/** Shift + click on Play: a speedrun timer (again to stop). */
export function speedrun(name: string) {
  let el = document.querySelector<HTMLDivElement>(".fx-speedrun");
  if (el) {
    cancelAnimationFrame(srRaf);
    el.classList.add("stop");
    tone("square", 880, 880, 0.15, 0.08);
    tone("square", 1320, 1320, 0.3, 0.08, 0.15);
    const done = el;
    setTimeout(() => done.remove(), 2500);
    return;
  }
  el = document.createElement("div");
  el.className = "fx-speedrun";
  document.body.append(el);
  [0, 0.5, 1].forEach((d) => tone("square", 440, 440, 0.12, 0.08, d));
  tone("square", 880, 880, 0.3, 0.1, 1.5);
  const start = performance.now() + 1500;
  const box = el;
  const loop = () => {
    const ms = Math.max(0, performance.now() - start);
    box.innerHTML = `⏱ <b>${String(Math.floor(ms / 60000)).padStart(2, "0")}:${String(Math.floor(ms / 1000) % 60).padStart(2, "0")}.${String(Math.floor(ms % 1000)).padStart(3, "0")}</b> <small>Any% · ${name}</small>`;
    srRaf = requestAnimationFrame(loop);
  };
  loop();
  found("speedrun");
}

/** Keyboard secrets, ripples on buttons, and the toast hook. Call once in App. */
export function useSecrets() {
  const { toast, settings } = useStore();
  toastFn = toast;
  theme = settings;
  useEffect(() => {
    let k = 0, typed = "";
    const key = (e: KeyboardEvent) => {
      const inField = (e.target as HTMLElement)?.closest?.("input, textarea, [contenteditable]");
      if (inField) return;
      const kk = e.key.toLowerCase();
      k = kk === KONAMI[k] ? k + 1 : kk === KONAMI[0] ? 1 : 0;
      if (k === KONAMI.length) {
        k = 0;
        creeper();
      }
      if (e.key.length !== 1) return;
      typed = (typed + kk).slice(-12);
      if (typed.endsWith("jeb_")) jeb();
      else if (typed.endsWith("dinnerbone")) dinnerbone();
      else if (typed.endsWith("herobrine")) herobrine();
    };
    // Ripple from the click point on buttons.
    const down = (e: PointerEvent) => {
      const b = (e.target as HTMLElement).closest?.(".btn, .navbtn, .mp-tabs button, .mp-ctrl button, .seg button, .chip");
      if (!b) return;
      const r = b.getBoundingClientRect(), d = Math.max(r.width, r.height) * 2.2, s = document.createElement("span");
      s.className = "v2-rip";
      s.style.cssText = `width:${d}px;height:${d}px;left:${e.clientX - r.left - d / 2}px;top:${e.clientY - r.top - d / 2}px`;
      b.append(s);
      setTimeout(() => s.remove(), 650);
    };
    addEventListener("keydown", key);
    addEventListener("pointerdown", down);
    return () => {
      removeEventListener("keydown", key);
      removeEventListener("pointerdown", down);
    };
  }, []);
}

/** "Found secrets x/8" with hints for the rest (Settings). */
export function SecretsCard() {
  const f = useSyncExternalStore((cb) => (subs.add(cb), () => subs.delete(cb)), () => cache);
  const ids = Object.keys(SECRETS);
  return (
    <div className="card pad secret-card">
      <div className="sc-head">
        <b className="sc-title">
          <McIcon name="trophy" size={20} /> {tr("Nalezená tajemství", "Secrets found")}
        </b>
        <span>
          {f.length}/{ids.length}
        </span>
      </div>
      <div className="sc-bar">
        <i style={{ width: `${(f.length / ids.length) * 100}%` }} />
      </div>
      <div className="sc-list">
        {ids.map((id) =>
          f.includes(id) ? (
            <div key={id} className="ok">
              <McIcon name="check" size={16} /> {tr(...SECRETS[id])}
            </div>
          ) : (
            <div key={id} className="lock">
              <McIcon name="lock" size={16} /> {tr(...HINTS[id])}
            </div>
          ),
        )}
      </div>
    </div>
  );
}
