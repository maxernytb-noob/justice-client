import { invoke } from "@tauri-apps/api/core";

export type Loader = "vanilla" | "fabric" | "quilt" | "forge" | "neoforge";
export type ContentKind = "mod" | "resourcepack" | "shader" | "modpack";
export type LinuxPackage = "appimage" | "deb" | "arch" | "other";

export interface Settings {
  dataDir: string;
  ramMb: number;
  jvmArgs: string;
  onLaunch: "hide" | "close" | "keep";
  language: "cs" | "en";
  curseforgeKey: string;
  msClientId: string;
  discordRpc: boolean;
  discordAppId: string;
  streamerMode: boolean;
  backupOnExit: boolean;
  width: number;
  height: number;
  highPriority: boolean;
  gc: "auto" | "zgc" | "g1";
  obsEnabled: boolean;
  obsUrl: string;
  obsPassword: string;
  obsAutoRecord: boolean;
  obsScene: string;
  onboarded: boolean;
  accentColor: string;
  bgStyle: "oled" | "dark" | "graphite" | "void" | "midnight" | "aurora" | "sakura" | "mono";
  quickServers: Record<string, string>;
  justiceMod: boolean;
  /** Pop the standalone live-log window when a game launches (Lunar-style). */
  logWindowOnLaunch: boolean;
  /** Look & layout preferences, see ui.ts. */
  ui: unknown;
}

export interface SystemInfo {
  totalRamMb: number;
  recommendedRamMb: number;
  version: string;
  hasCfKey: boolean;
}

export interface Instance {
  id: string;
  name: string;
  icon: string;
  group: string;
  mcVersion: string;
  loader: Loader;
  loaderVersion: string;
  installedId: string;
  ramMb: number | null;
  javaPath: string;
  jvmArgs: string;
  width: number | null;
  height: number | null;
  created: number;
  lastPlayed: number;
  playtimeSecs: number;
  template: string;
  modProfile: string;
  modProfiles: Record<string, string[]>;
  recordingMode: boolean;
  lightMode: boolean;
  noJusticeMod: boolean;
}

export interface GameVersion {
  id: string;
  kind: string;
  releaseTime: string;
}
export interface LoaderVersion {
  version: string;
  stable: boolean;
}

export interface Account {
  id: string;
  kind: "msa" | "offline";
  username: string;
  uuid: string;
  skinUrl: string;
  skinVariant: string;
  active: boolean;
}
export interface DeviceCode {
  userCode: string;
  verificationUri: string;
  deviceCode: string;
  interval: number;
  expiresIn: number;
}
export interface Cape {
  id: string;
  alias: string;
  url: string;
  active: boolean;
}

export interface Hit {
  source: "modrinth" | "curseforge";
  id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  iconUrl: string;
  downloads: number;
  updated: string;
  categories: string[];
  url: string;
}
export interface ProjectVersion {
  id: string;
  name: string;
  versionNumber: string;
  gameVersions: string[];
  loaders: string[];
  date: string;
  kind: string;
}
export interface ProjectDetails {
  hit: Hit;
  body: string;
  gallery: string[];
  versions: ProjectVersion[];
}
export interface OptimizeChange {
  id: string;
  group: "mods" | "conflicts" | "video" | "sodium" | "memory";
  title: string;
  detail: string;
  from: string | null;
  to: string | null;
}
export interface OptimizePlan {
  tier: "low" | "mid" | "high";
  hardware: string;
  changes: OptimizeChange[];
  undoAvailable: boolean;
}
export interface OptimizeReport {
  applied: string[];
  installed: string[];
  unavailable: string[];
}
export interface InstallReport {
  installed: string[];
  skipped: string[];
  manual: [string, string][];
}
export interface Installed {
  file: string;
  kind: ContentKind;
  enabled: boolean;
  title: string;
  version: string;
  iconUrl: string;
  source: string;
  projectId: string;
  size: number;
  pinned: boolean;
}
export interface Update {
  file: string;
  kind: ContentKind;
  title: string;
  source: string;
  projectId: string;
  newVersionId: string;
  newVersion: string;
}

export interface Conflict {
  declarer: string;
  target: string;
  kind: "breaks" | "depends";
  text: string;
}

export interface CrashReport {
  title: string;
  detail: string;
  suspect: string | null;
  crashFile: string | null;
  excerpt: string[];
}
export interface RepairAction {
  id: string;
  group: "duplicate" | "missing" | "conflict" | "crash";
  title: string;
  detail: string;
  severity: "error" | "warn";
  checked: boolean;
}
export interface RepairPlan {
  actions: RepairAction[];
  crash: CrashReport | null;
  undoAvailable: boolean;
  supported: boolean;
}
export interface FixReport {
  fixed: string[];
  remaining: Conflict[];
}
export interface ModCard {
  id: string;
  slug: string;
  title: string;
  description: string;
  iconUrl: string;
  downloads: number;
  reason: string;
}
export interface MaxReply {
  reply: string;
  limited: boolean;
  mods: ModCard[];
}
export interface SkinEntry {
  id: string;
  name: string;
  variant: "slim" | "classic";
  added: number;
  file: string;
}
export interface GameExit {
  instance: string;
  code: number | null;
  seconds: number;
  crash: CrashReport | null;
}
export interface Download {
  id: number;
  name: string;
  kind: "modpack" | "content";
  done: number;
  total: number;
  speed: number;
  etaSecs: number;
  status: "running" | "paused" | "done" | "error" | "canceled";
  error: string | null;
}
export interface Progress {
  task: string;
  label: string;
  done: number;
  total: number;
}

export interface MediaFile {
  instance: string;
  instanceName: string;
  path: string;
  name: string;
  kind: "replay" | "video" | "screenshot";
  size: number;
  modified: number;
  durationSecs: number | null;
  world: string;
}
export interface World {
  name: string;
  path: string;
  modified: number;
  size: number;
  icon: string | null;
}
export interface Backup {
  file: string;
  kind: "world" | "snapshot";
  label: string;
  created: string;
  size: number;
}
export interface Server {
  id: string;
  name: string;
  address: string;
  instance: string;
  pinned: boolean;
}
export interface ServerStatus {
  online: number;
  max: number;
  motd: string;
  version: string;
  icon: string | null;
  pingMs: number;
}

export interface ClipsSettings {
  clipsDir: string;
  hotkey: string;
  bufferSecs: number;
  postSecs: number;
  fps: number;
  quality: "small" | "balanced" | "high";
  monitor: number;
  drawMouse: boolean;
  gameAudio: boolean;
  gameDevice: string;
  mic: boolean;
  micDevice: string;
  micVolume: number;
  audioOffsetMs: number;
  overlay: boolean;
  sound: boolean;
  gameFolders: boolean;
  ffmpegPath: string;
  enabled: boolean;
}
export interface Clip {
  path: string;
  name: string;
  game: string;
  size: number;
  modified: number;
  duration: number;
  audioTracks: number;
  thumb: string | null;
}
export interface ClipsStatus {
  state: "starting" | "recording" | "paused" | "error";
  error: string | null;
  bufferBytes: number;
  bufferSecs: number;
  ffmpeg: string | null;
  hotkeyError: string | null;
  gameRunning: boolean;
  platform: "windows" | "linux";
  /** Linux: gpu-screen-recorder isn't installed. */
  recorderMissing: boolean;
}
export interface ClipsDevices {
  outputs: string[];
  inputs: string[];
  defaultOutput: string | null;
  defaultInput: string | null;
}
export interface Monitor {
  index: number;
  name: string;
  width: number;
  height: number;
  primary: boolean;
}

export interface Presence {
  state: "online" | "playing";
  instance?: string;
  mc?: string;
  loader?: string;
  server?: string | null;
}
export interface PvpStats {
  kills: number;
  deaths: number;
  bestCombo: number;
  hits: number;
}
export interface SocialProfile {
  bio?: string;
  status?: string;
  banner?: string;
  stats?: { playtimeSecs: number; instances: number; clips: number; friends: number; pvp?: PvpStats } | null;
  achievements?: string[];
  /** Justice cape id ("" = none). */
  cape?: string;
  /** Justice pet id ("" = none). */
  pet?: string;
}
export interface SocialUser {
  uuid: string;
  name: string;
  profile: SocialProfile;
  badges: string[];
  created: number;
  lastSeen: number;
  online: boolean;
  presence: Presence | null;
  unread?: number;
  /** Offline (non-premium) account with a Justice password. */
  offline?: boolean;
}
export interface Party {
  id: string;
  leader: string;
  members: SocialUser[];
  invited: SocialUser[];
  created: number;
}
/** Where the party leader wants everyone: a server, or a world they share through Justice. */
export type WarpTarget =
  | { kind: "server"; address: string; mc?: string; loader?: string; instance?: string }
  | { kind: "world"; world: string; name: string; mc?: string; loader?: string };
export interface ChatMessage {
  id: number;
  from: string;
  to: string;
  kind: "text" | "clip" | "pack" | "invite" | "world";
  body: string;
  meta: Record<string, any>;
  created: number;
  read: boolean;
}

export const social = {
  login: () => invoke<{ token: string; uuid: string; name: string }>("social_login"),
  /** Offline accounts: sign in to (register = create) the Justice account. */
  loginPassword: (password: string, register: boolean) => invoke<{ token: string; uuid: string; name: string }>("social_login_password", { password, register }),
  changePassword: (oldPassword: string, newPassword: string) => invoke<void>("social_post", { path: "/password", body: { oldPassword, newPassword } }),
  party: () => invoke<{ party: Party | null; invites: Party[] }>("social_get", { path: "/party" }),
  partyInvite: (uuid: string) => invoke<void>("social_post", { path: "/party/invite", body: { uuid } }),
  partyAccept: (id: string) => invoke<void>("social_post", { path: "/party/accept", body: { id } }),
  partyDecline: (id: string) => invoke<void>("social_post", { path: "/party/decline", body: { id } }),
  partyLeave: () => invoke<void>("social_post", { path: "/party/leave", body: {} }),
  partyKick: (uuid: string) => invoke<void>("social_post", { path: "/party/kick", body: { uuid } }),
  /** Leader: bring the party to the server/world we're on (the launcher knows where that is). */
  partyWarpHere: () => invoke<{ sent: number }>("party_warp_here"),
  pvpStats: () => invoke<PvpStats>("pvp_stats"),
  logout: () => invoke<void>("social_logout"),
  status: () => invoke<{ signedIn: boolean; uuid: string | null }>("social_status"),
  me: () => invoke<SocialUser>("social_get", { path: "/me" }),
  saveProfile: (p: SocialProfile) => invoke<SocialUser>("social_put", { path: "/me", body: p }),
  user: (name: string) => invoke<SocialUser>("social_get", { path: `/users/${encodeURIComponent(name)}` }),
  friends: () => invoke<{ friends: SocialUser[]; incoming: SocialUser[]; outgoing: SocialUser[] }>("social_get", { path: "/friends" }),
  request: (name: string) => invoke<{ status: string }>("social_post", { path: "/friends/request", body: { name } }),
  respond: (uuid: string, accept: boolean) => invoke<void>("social_post", { path: "/friends/respond", body: { uuid, accept } }),
  remove: (uuid: string) => invoke<void>("social_post", { path: "/friends/remove", body: { uuid } }),
  history: (uuid: string, before?: number) =>
    invoke<{ messages: ChatMessage[] }>("social_get", { path: `/messages/${uuid}${before ? `?before=${before}` : ""}` }),
  send: (to: string, kind: ChatMessage["kind"], body: string, meta: Record<string, unknown> = {}) =>
    invoke<ChatMessage>("social_post", { path: "/messages", body: { to, kind, body, meta } }),
  upload: (path: string) => invoke<{ url: string; size: number; expires: number }>("social_upload", { path }),
  openClip: (url: string) => invoke<void>("social_open_clip", { url }),
  typing: (to: string) => invoke<void>("social_typing", { to }),
  joinWorld: (world: string) => invoke<number>("world_join", { world }),
};

export const clips = {
  settings: () => invoke<ClipsSettings>("clips_get_settings"),
  saveSettings: (s: ClipsSettings) => invoke<ClipsSettings>("clips_set_settings", { s }),
  status: () => invoke<ClipsStatus>("clips_status"),
  save: () => invoke<void>("clips_save"),
  setRecording: (on: boolean) => invoke<void>("clips_set_recording", { on }),
  list: () => invoke<Clip[]>("clips_list"),
  strip: (path: string) => invoke<string>("clips_strip", { path }),
  remove: (path: string) => invoke<void>("clips_delete", { path }),
  rename: (path: string, name: string) => invoke<Clip>("clips_rename", { path, name }),
  reveal: (path: string) => invoke<void>("clips_reveal", { path }),
  openDir: () => invoke<void>("clips_open_dir"),
  trim: (req: { path: string; start: number; end: number; audio: string; precise: boolean; replace: boolean; speed?: number; text?: string }) => invoke<Clip>("clips_trim", { req }),
  devices: () => invoke<ClipsDevices>("clips_devices"),
  monitors: () => invoke<Monitor[]>("clips_monitors"),
  log: () => invoke<string[]>("clips_log"),
  testNotification: () => invoke<void>("clips_test_notification"),
  detectFfmpeg: (path: string) => invoke<string | null>("clips_detect_ffmpeg", { path }),
  publish: (path: string, title: string, description: string, tags: string[], hashtags: string[], durationMs: number, captured: number) =>
    invoke<FeedClip>("clips_publish", { path, title, description, tags, hashtags, durationMs, captured }),
  feed: (offset: number, hashtag: string, player: string, mine: boolean) => invoke<{ clips: FeedClip[] }>("clips_feed", { offset, hashtag, player, mine }),
  like: (id: string) => invoke<{ likes: number; liked: boolean }>("clips_like", { id }),
  report: (id: string, reason: string) => invoke<void>("clips_report", { id, reason }),
  feedDelete: (id: string) => invoke<void>("clips_feed_delete", { id }),
};

export interface FeedClip {
  id: string;
  owner: string;
  ownerName: string;
  url: string;
  thumb: string;
  title: string;
  description: string;
  tags: string[];
  hashtags: string[];
  durationMs: number;
  width: number;
  height: number;
  captured: number;
  created: number;
  likes: number;
  liked: boolean;
  mine: boolean;
}

/** Where the player was last in an instance (from its latest log / saves). */
export interface LastPlace {
  kind: "server" | "world";
  name: string;
  image: string | null;
}

/** Now playing from Windows' media controls (Sonolith, Spotify, a browser…). */
export interface MediaSession { id: string; source: string; title: string; playing: boolean }
export interface MediaState {
  id: string;
  source: string;
  title: string;
  artist: string;
  album: string;
  thumb: string | null;
  thumbKey: string;
  position: number;
  duration: number;
  playing: boolean;
  canNext: boolean;
  canPrev: boolean;
  canSeek: boolean;
  sessions: MediaSession[];
}
export const media = {
  state: (prefer?: string | null, knownThumb?: string | null) => invoke<MediaState | null>("media_state", { prefer: prefer ?? null, knownThumb: knownThumb ?? null }),
  command: (command: "playPause" | "next" | "previous" | "seek", value?: number, prefer?: string | null) => invoke<void>("media_command", { command, value: value ?? null, prefer: prefer ?? null }),
  openMusicApp: () => invoke<void>("open_music_app"),
};

/** Official key art of a Minecraft version (Mojang's launcher content). */
export interface VersionArt {
  name: string;
  image: string;
  wide: string | null;
  /** Full-HD key art for drops we host one for. */
  hd: string | null;
}

export const api = {
  versionArt: () => invoke<Record<string, VersionArt>>("version_art"),
  lastPlace: (id: string) => invoke<LastPlace | null>("instance_last_place", { id }),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  systemInfo: () => invoke<SystemInfo>("system_info"),
  hardwareInfo: () => invoke<{ cpu: string; threads: number; gpus: string[]; totalRamMb: number }>("hardware_info"),
  freeSpace: (path: string) => invoke<number | null>("free_space", { path }),
  moveDataDir: (dest: string) => invoke<void>("move_data_dir", { dest }),

  gameVersions: () => invoke<GameVersion[]>("game_versions"),
  loaderVersions: (loader: string, game: string) => invoke<LoaderVersion[]>("loader_versions", { loader, game }),

  listInstances: () => invoke<Instance[]>("list_instances"),
  getInstance: (id: string) => invoke<Instance>("get_instance", { id }),
  createInstance: (instance: Partial<Instance>, packs: string[], optimize: boolean) =>
    invoke<Instance>("create_instance", { instance, packs, optimize }),
  updateInstance: (instance: Instance) => invoke<Instance>("update_instance", { instance }),
  setInstanceIcon: (id: string, path: string) => invoke<Instance>("set_instance_icon", { id, path }),
  deleteInstance: (id: string) => invoke<void>("delete_instance", { id }),
  duplicateInstance: (id: string) => invoke<Instance>("duplicate_instance", { id }),
  instancePath: (id: string, sub?: string) => invoke<string>("instance_path", { id, sub: sub ?? null }),

  launch: (id: string, server?: string) => invoke<void>("launch_instance", { id, server: server ?? null }),
  kill: (id: string) => invoke<void>("kill_game", { id }),
  running: () => invoke<string[]>("running_games"),
  logs: (id: string) => invoke<string[]>("game_logs", { id }),
  openLogsWindow: () => invoke<void>("open_logs_window"),
  analyzeCrash: (id: string) => invoke<CrashReport>("analyze_crash", { id }),

  listAccounts: () => invoke<Account[]>("list_accounts"),
  addOffline: (name: string) => invoke<void>("add_offline_account", { name }),
  msLoginStart: () => invoke<DeviceCode>("ms_login_start"),
  msLoginPoll: (deviceCode: string, interval: number) => invoke<Account>("ms_login_poll", { deviceCode, interval }),
  msLoginCancel: () => invoke<void>("ms_login_cancel"),
  setActiveAccount: (id: string) => invoke<void>("set_active_account", { id }),
  removeAccount: (id: string) => invoke<void>("remove_account", { id }),
  getCapes: (id: string) => invoke<Cape[]>("get_capes", { id }),
  setCape: (id: string, cape: string | null) => invoke<void>("set_cape", { id, cape }),
  uploadSkin: (id: string, path: string, variant: string) => invoke<void>("upload_skin", { id, path, variant }),
  /** Copies a premium player's skin onto the account. */
  skinFromPlayer: (id: string, name: string) => invoke<void>("skin_from_player", { id, name }),
  resetSkin: (id: string) => invoke<void>("reset_skin", { id }),
  skinlibList: () => invoke<SkinEntry[]>("skinlib_list"),
  skinlibAddFile: (path: string, name: string, variant: string) => invoke<string>("skinlib_add_file", { path, name, variant }),
  skinlibAddPlayer: (player: string, name: string) => invoke<string>("skinlib_add_player", { player, name }),
  skinlibAddCurrent: (account: string, name: string) => invoke<string>("skinlib_add_current", { account, name }),
  skinlibApply: (account: string, id: string) => invoke<void>("skinlib_apply", { account, id }),
  skinlibRename: (id: string, name: string) => invoke<void>("skinlib_rename", { id, name }),
  skinlibRemove: (id: string) => invoke<void>("skinlib_remove", { id }),
  fetchImage: (url: string) => invoke<string>("fetch_image", { url }),

  search: (a: {
    source: string;
    query: string;
    kind: ContentKind;
    gameVersion: string;
    loader: string;
    sort: string;
    offset: number;
    category?: string;
  }) => invoke<{ hits: Hit[]; total: number }>("search_content", a),
  projectDetails: (source: string, id: string, kind: ContentKind, instance: string) =>
    invoke<ProjectDetails>("project_details", { source, id, kind, instance }),
  installContent: (instance: string, source: string, project: string, kind: ContentKind, version?: string) =>
    invoke<InstallReport>("install_content", { instance, source, project, kind, version: version ?? null }),
  listContent: (instance: string) => invoke<Installed[]>("list_content", { instance }),
  toggleContent: (instance: string, kind: string, file: string, enabled: boolean) =>
    invoke<void>("toggle_content", { instance, kind, file, enabled }),
  removeContent: (instance: string, kind: string, file: string) => invoke<void>("remove_content", { instance, kind, file }),
  addLocalFiles: (instance: string, paths: string[]) => invoke<number>("add_local_files", { instance, paths }),
  checkCompat: (instance: string) => invoke<Conflict[]>("check_compat", { instance }),
  repairCrash: (instance: string) => invoke<{ fixed: string[]; remaining: Conflict[] }>("repair_crash", { instance }),
  setPinned: (instance: string, kind: string, file: string, pinned: boolean) => invoke<void>("set_pinned", { instance, kind, file, pinned }),
  fixCompat: (instance: string) => invoke<{ fixed: string[]; remaining: Conflict[] }>("fix_compat", { instance }),
  repairPlan: (instance: string) => invoke<RepairPlan>("repair_plan", { instance }),
  repairApply: (instance: string, ids: string[]) => invoke<FixReport>("repair_apply", { instance, ids }),
  repairUndo: (instance: string) => invoke<void>("repair_undo", { instance }),
  checkUpdates: (instance: string) => invoke<Update[]>("check_updates", { instance }),
  applyUpdates: (instance: string, updates: Update[]) => invoke<string[]>("apply_updates", { instance, updates }),
  optimize: (instance: string) => invoke<InstallReport>("optimize_instance", { instance }),
  optimizePlan: (instance: string) => invoke<OptimizePlan>("optimize_plan", { instance }),
  optimizeApply: (instance: string, ids: string[]) => invoke<OptimizeReport>("optimize_apply", { instance, ids }),
  optimizeUndo: (instance: string) => invoke<void>("optimize_undo", { instance }),

  saveModProfile: (instance: string, name: string) => invoke<void>("save_mod_profile", { instance, name }),
  applyModProfile: (instance: string, name: string) => invoke<void>("apply_mod_profile", { instance, name }),
  deleteModProfile: (instance: string, name: string) => invoke<void>("delete_mod_profile", { instance, name }),
  listMedia: (kind: string) => invoke<MediaFile[]>("list_media", { kind }),
  deleteMedia: (path: string) => invoke<void>("delete_media", { path }),
  obsTest: () => invoke<string[]>("obs_test"),

  listWorlds: (instance: string) => invoke<World[]>("list_worlds", { instance }),
  backupWorld: (instance: string, world: string) => invoke<string>("backup_world", { instance, world }),
  snapshot: (instance: string) => invoke<string>("snapshot_instance", { instance }),
  listBackups: (instance: string) => invoke<Backup[]>("list_backups", { instance }),
  restoreBackup: (instance: string, file: string) => invoke<void>("restore_backup", { instance, file }),
  deleteBackup: (instance: string, file: string) => invoke<void>("delete_backup", { instance, file }),

  shareCode: (instance: string) => invoke<string>("share_code", { instance }),
  importCode: (code: string) => invoke<Instance>("import_code", { code }),
  exportPack: (instance: string, dest: string) => invoke<void>("export_pack", { instance, dest }),
  importPack: (path: string) => invoke<Instance>("import_pack", { path }),
  /** Modrinth / CurseForge modpack -> new instance. */
  installModpack: (source: string, project: string, version?: string) => invoke<Instance>("install_modpack", { source, project, version }),
  downloadModpack: (source: string, project: string, name: string, version?: string) => invoke<number>("download_modpack", { source, project, name, version: version ?? null }),
  downloadList: () => invoke<Download[]>("download_list"),
  downloadPause: (id: number) => invoke<void>("download_pause", { id }),
  downloadResume: (id: number) => invoke<void>("download_resume", { id }),
  downloadCancel: (id: number) => invoke<void>("download_cancel", { id }),
  downloadClear: () => invoke<void>("download_clear"),
  modbotChat: (mc: string, loader: string, messages: { role: string; content: string }[]) => invoke<MaxReply>("modbot_chat", { mc, loader, messages }),
  modbotBuild: (mc: string, loader: string, name: string, slugs: string[]) => invoke<Instance>("modbot_build", { mc, loader, name, slugs }),
  modbotAdd: (instance: string, slugs: string[]) => invoke<InstallReport>("modbot_add", { instance, slugs }),

  listServers: () => invoke<Server[]>("list_servers"),
  saveServer: (server: Partial<Server>) => invoke<Server>("save_server", { server }),
  removeServer: (id: string) => invoke<void>("remove_server", { id }),
  pingServer: (address: string) => invoke<ServerStatus>("ping_server", { address }),

  log: (level: "debug" | "info" | "warn" | "error", message: string) => invoke<void>("log_frontend", { level, message }).catch(() => {}),
  logPath: () => invoke<string>("log_path"),
  checkUpdateManual: () => invoke<{ version: string; notes: string; url: string; kind: LinuxPackage } | null>("check_update_manual"),
};
