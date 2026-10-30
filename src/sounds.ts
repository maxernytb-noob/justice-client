// Short UI sounds, synthesized with Web Audio (nothing to download). Only for moments that matter:
// starting the game, something finishing, the game crashing, a friend writing, an achievement.
// Not on every click or toggle.

export type UiSound = "launch" | "done" | "error" | "notify" | "achievement" | "open";

let enabled = true;
let volume = 0.5;
let ctx: AudioContext | null = null;

/** Called by applyUi whenever the preferences change. */
export function setSoundPrefs(on: boolean, vol: number) {
  enabled = on;
  volume = vol;
}

function ac(): AudioContext | null {
  try {
    ctx ||= new AudioContext();
    if (ctx.state === "suspended") ctx.resume().catch(() => {});
    return ctx;
  } catch {
    return null;
  }
}

/** One soft note: sine/triangle with a quick attack and an exponential tail. */
function note(a: AudioContext, freq: number, at: number, len: number, gain: number, type: OscillatorType = "sine") {
  const o = a.createOscillator();
  const g = a.createGain();
  o.type = type;
  o.frequency.setValueAtTime(freq, a.currentTime + at);
  const t0 = a.currentTime + at;
  g.gain.setValueAtTime(0.0001, t0);
  g.gain.exponentialRampToValueAtTime(Math.max(0.0002, gain * volume), t0 + 0.012);
  g.gain.exponentialRampToValueAtTime(0.0001, t0 + len);
  o.connect(g).connect(a.destination);
  o.start(t0);
  o.stop(t0 + len + 0.05);
}

export function sound(kind: UiSound, force = false) {
  if ((!enabled && !force) || volume <= 0) return;
  if (document.documentElement.classList.contains("snd-off")) return;
  const a = ac();
  if (!a) return;
  switch (kind) {
    case "launch": // rising major chord, like a door opening
      [523.25, 659.25, 783.99, 1046.5].forEach((f, i) => note(a, f, i * 0.06, 0.45, 0.16, "triangle"));
      break;
    case "done":
      note(a, 880, 0, 0.18, 0.14);
      note(a, 1318.5, 0.09, 0.32, 0.14);
      break;
    case "error":
      note(a, 196, 0, 0.32, 0.18, "triangle");
      note(a, 164.8, 0.12, 0.42, 0.16, "triangle");
      break;
    case "notify":
      note(a, 1174.7, 0, 0.22, 0.1);
      note(a, 1568, 0.07, 0.3, 0.08);
      break;
    case "achievement":
      [784, 988, 1175, 1568].forEach((f, i) => note(a, f, i * 0.09, 0.5, 0.13, "square"));
      break;
    case "open":
      note(a, 1760, 0, 0.06, 0.05);
      break;
  }
}
