import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// Sonolith's local API, as seen by the UI: connection status plus the live player state that
// Sonolith pushes as events (no polling). See src-tauri/src/sonolith.rs.

export interface SnlStatus {
  available: boolean;
  registered: boolean;
  connected: boolean;
  pending: boolean;
  grantedScopes: string[];
  signedIn: boolean;
  appVersion: string;
  error: string | null;
  retryAfterMs: number | null;
}

export interface SnlArtist {
  id: string | null;
  name: string;
}
export interface SnlItem {
  type: "track" | "episode" | "album" | "playlist" | "artist" | "podcast";
  id: string;
  title?: string;
  name?: string;
  artists?: SnlArtist[];
  album?: { id: string | null; title: string } | null;
  podcast?: { title: string } | null;
  author?: string | null;
  durationMs?: number | null;
  thumbnailUrl: string | null;
  rating?: "like" | "dislike" | "none" | null;
  trackCount?: number | null;
}
export interface SnlPlayer {
  status: "playing" | "paused" | "buffering" | "stopped";
  item: SnlItem | null;
  positionMs: number;
  positionAt: number;
  durationMs: number | null;
  canGoNext: boolean;
  canGoPrevious: boolean;
  shuffle: boolean;
  repeat: "off" | "all" | "one";
  queueIndex: number | null;
  queueLength: number;
}

interface Snl {
  status: SnlStatus | null;
  player: SnlPlayer | null;
  /** Index of the lyric line being sung, from lyrics.line events. */
  lyricLine: number | null;
  lyricTrack: string | null;
  /** Bumped on every queue.changed so lists can reload. */
  queueRev: number;
}

let snl: Snl = { status: null, player: null, lyricLine: null, lyricTrack: null, queueRev: 0 };
const subs = new Set<() => void>();
const set = (p: Partial<Snl>) => {
  snl = { ...snl, ...p };
  subs.forEach((f) => f());
};

// After the session opens and may read the player, pull the current state once so the UI has
// something to show before the first push event. Useful live too, not just in the dev preview.
let playerLoaded = false;
function maybeLoadPlayer(s: SnlStatus | null) {
  if (playerLoaded || !s?.connected || !s.grantedScopes.includes("player:read")) return;
  playerLoaded = true;
  snlCall<SnlPlayer>("player.getState").then((p) => set({ player: p })).catch(() => { playerLoaded = false; });
}

let started = false;
function start() {
  if (started || !("__TAURI_INTERNALS__" in window)) return;
  started = true;
  invoke<SnlStatus>("sonolith_status").then((s) => { set({ status: s }); maybeLoadPlayer(s); }).catch(() => {});
  listen<SnlStatus>("sonolith-status", (e) => { const next = { ...(snl.status ?? ({} as SnlStatus)), ...e.payload }; set({ status: next }); maybeLoadPlayer(next); });
  listen<{ method: string; params: any }>("sonolith", (e) => {
    const { method, params } = e.payload;
    switch (method) {
      case "player.stateChanged":
        set({ player: params });
        break;
      case "player.trackChanged":
        if (snl.player) set({ player: { ...snl.player, item: params.item } });
        break;
      case "player.position":
        if (snl.player) set({ player: { ...snl.player, positionMs: params.positionMs, positionAt: params.positionAt, durationMs: params.durationMs, status: params.status } });
        break;
      case "lyrics.line":
        set({ lyricLine: params.lineIndex, lyricTrack: params.trackId });
        break;
      case "queue.changed":
        set({ queueRev: snl.queueRev + 1 });
        break;
    }
  });
  // Re-read the status now and then: Sonolith may start or quit while the launcher runs.
  setInterval(() => invoke<SnlStatus>("sonolith_status").then((s) => { const next = { ...s, connected: snl.status?.connected ?? s.connected }; set({ status: next }); maybeLoadPlayer(next); }).catch(() => {}), 15000);
}

export function useSonolith(): Snl {
  start();
  return useSyncExternalStore(
    (f) => (subs.add(f), () => subs.delete(f)),
    () => snl,
  );
}

/** True when the session is open and may read the player. */
export function snlLive(s: Snl) {
  return !!s.status?.connected && !!s.status.grantedScopes.includes("player:read");
}

export const snlCall = <T = any>(method: string, params?: Record<string, unknown>) => invoke<T>("sonolith_call", { method, params: params ?? null });
export const snlConnect = () => invoke<void>("sonolith_connect");
export const snlDisconnect = () => invoke<void>("sonolith_disconnect");

export const itemTitle = (i: SnlItem) => i.title ?? i.name ?? "";
export const itemSub = (i: SnlItem) => i.artists?.map((a) => a.name).join(", ") || i.author || i.podcast?.title || "";
