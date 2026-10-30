// Dev-only: lets the UI run in a normal browser (vite dev server) with sample data, so the look can
// be checked without the Tauri backend. main.tsx imports this only when Tauri isn't present.
const sampleInstances = [
  { id: "cinematic", name: "Cinematic 1.21.11", icon: "film", group: "", mcVersion: "1.21.11", loader: "fabric", loaderVersion: "", installedId: "", ramMb: null, javaPath: "", jvmArgs: "", width: null, height: null, created: 1790000000, lastPlayed: Date.now() / 1000 - 3600, playtimeSecs: 18200, template: "cinematic", modProfile: "", modProfiles: {}, recordingMode: true, lightMode: false },
  { id: "pvp18", name: "1.8.9 PvP", icon: "gamepad", group: "", mcVersion: "1.8.9", loader: "forge", loaderVersion: "", installedId: "", ramMb: null, javaPath: "", jvmArgs: "", width: null, height: null, created: 1790000000, lastPlayed: Date.now() / 1000 - 86400, playtimeSecs: 5400, template: "pvp18", modProfile: "", modProfiles: {}, recordingMode: false, lightMode: false },
];
const settings = {
  dataDir: "D:\\JusticeClient", ramMb: 6144, jvmArgs: "", onLaunch: "hide", language: new URLSearchParams(location.search).get("lang") ?? "cs", curseforgeKey: "", msClientId: "", discordRpc: true,
  discordAppId: "", streamerMode: false, backupOnExit: false, width: 1280, height: 720, highPriority: true, gc: "auto", obsEnabled: false,
  obsUrl: "ws://127.0.0.1:4455", obsPassword: "", obsAutoRecord: false, obsScene: "", onboarded: true, accentColor: "#e11d38", bgStyle: "void", quickServers: {}, justiceMod: true, ui: { shortcuts: [{ kind: "instance", id: "cinematic" }], startPage: new URLSearchParams(location.search).get("page") ?? "home" },
};

const clipsSettings = {
  clipsDir: "D:\Justice Clips", hotkey: "F8", bufferSecs: 60, postSecs: 1, fps: 60, quality: "balanced", monitor: 0, drawMouse: true, gameAudio: true,
  gameDevice: "", mic: true, micDevice: "", micVolume: 1, audioOffsetMs: 0, overlay: true, sound: true, gameFolders: true, ffmpegPath: "", enabled: true,
};
const sampleClips = [
  ["Triple kill na Justice SMP", "PvP 1.21.11", null, 38, 58e6, 600],
  ["Clutch s perlou", "PvP 1.21.11", null, 60, 91e6, 7200],
  ["Base tour", "Cinematic 1.21.11", null, 45, 70e6, 90000],
  ["Bedwars final", "1.8.9 PvP", null, 22, 31e6, 300000],
].map(([name, game, thumb, duration, size, ago]) => ({ path: `D:\Justice Clips\${name}.mp4`, name, game, thumb, duration, size, modified: Date.now() - (ago as number) * 1000, audioTracks: 3 }));

// ?shots=1 uses real screenshots from /devmock (only present while taking website screenshots).
const SHOTS = new URLSearchParams(location.search).has("shots");
if (SHOTS) {
  // Screenshots for the website: ?player=0 hides the music player, clips get real-looking thumbnails.
  localStorage.setItem("jc.player.hidden", new URLSearchParams(location.search).get("player") === "0" ? "1" : "0");
  const ART = ["wilderness-bound", "chaos-cubed", "tiny-takeover", "mounts-of-mayhem", "copper-age"];
  sampleClips.forEach((c, i) => ((c as { thumb?: string }).thumb = `https://justiceclient.online/art/${ART[i % ART.length]}.jpg`));
  const base = sampleInstances[0];
  sampleInstances.push(
    { ...base, id: "pvp", name: "PvP 1.21.11", icon: "sword", template: "pvp", lastPlayed: Date.now() / 1000 - 600, playtimeSecs: 30100, recordingMode: false },
    { ...base, id: "smp", name: "Justice SMP", icon: "crown", template: "smp", lastPlayed: Date.now() / 1000 - 7200, playtimeSecs: 52000, recordingMode: false },
    { ...base, id: "v263", name: "Vanilla 26.3", icon: "trees", loader: "vanilla", mcVersion: "26.3", template: "", lastPlayed: Date.now() / 1000 - 250000, playtimeSecs: 3000, recordingMode: false },
    { ...base, id: "burger", name: "Burger Survival", icon: "burger", mcVersion: "26.2", template: "smp", lastPlayed: Date.now() / 1000 - 400000, playtimeSecs: 8000, recordingMode: false },
  );
}
const now = Date.now() / 1000;
const me = { uuid: "0494ec66d9504e5d93016701417cfb8b", name: "Maxern_", badges: ["team", "early"], created: now - 86400, lastSeen: now, online: true, presence: null,
  profile: { status: "Točím nové video 🎬", banner: "red", bio: "", stats: { playtimeSecs: 19 * 3600, instances: 7, clips: 12, friends: 4, pvp: { kills: 142, deaths: 61, bestCombo: 17, hits: 3400 } }, cape: "justice", achievements: ["first", "h10", "inst5", "clip1", "friend1"] } };
const pvpOf = (k: number) => ({ playtimeSecs: k * 900, instances: 3, clips: 2, friends: 3, pvp: { kills: k, deaths: Math.round(k * 0.7) + 3, bestCombo: 5 + (k % 13), hits: k * 20 } });
const friend = (uuid: string, name: string, online: boolean, presence: unknown, unread = 0) => ({ uuid, name, online, presence, unread, badges: [], created: now - 3600, lastSeen: now - 5400, profile: {} });
const friends = [
  friend("069a79f444e94726a5befca90e38aaf5", "Notch", true, { state: "playing", instance: "PvP 1.21.11", mc: "1.21.11", loader: "fabric", server: "justicesmp.online" }, 2),
  friend("853c80ef3c3749fdaa49938b674adae6", "jeb_", true, { state: "online" }),
  friend("61699b2ed3274a019f1e0ea8c3f06bc6", "Dinnerbone", false, null),
];
friends.forEach((f, i) => ((f as { profile: object }).profile = { stats: pvpOf([188, 97, 40][i] ?? 10), cape: ["gold", "", "void"][i] ?? "" }));
// ?offline=1 previews the offline-account sign-in on the Friends page.
const OFFLINE = new URLSearchParams(location.search).has("offline");
// Preview of the standalone logs window (?window=logs): pretend a game is running with output.
const LOGWIN = new URLSearchParams(location.search).get("window") === "logs";
// Preview of the download manager queue (?dl=1): a few sample downloads in the top bar.
const DLDEMO = new URLSearchParams(location.search).has("dl");
const SAMPLE_LOG = [
  "[19:52:48] [main/INFO] Loading Minecraft 1.21.11 with Fabric Loader 0.17.2",
  "[19:52:49] [main/INFO] Loading 18 mods:\n\t- fabric-api 0.137.0\n\t- justicemod 1.8.8\n\t- sodium 0.6.13",
  "[19:52:50] [Render thread/INFO] [Justice] Performance module enabled (tier: mid)",
  "[19:52:50] [Render thread/INFO] Setting user: Maxern_",
  "[19:52:51] [Render thread/INFO] [Indigo] Registering Indigo renderer!",
  "[19:52:53] [Render thread/INFO] Backend library: LWJGL version 3.3.3",
  "[19:52:55] [Render thread/WARN] Ignoring unknown attribute 'glow' in particle config",
  "[19:52:57] [Render thread/INFO] Reloading ResourceManager: vanilla, fabric-api, justicemod",
  "[19:53:00] [Render thread/INFO] OpenAL initialized on device OpenAL Soft on Sluchátka",
  "[19:53:00] [Render thread/INFO] Sound engine started",
  "[19:53:00] [Render thread/INFO] Created: 2048x2048x4 minecraft:textures/atlas/blocks.png-atlas",
  "[19:53:01] [Render thread/INFO] [Justice] Now Playing bridge connected",
  "[19:53:02] [Render thread/INFO] Time elapsed: 14032 ms",
];
const party = { id: "p1", leader: me.uuid, members: [me, friends[0]], invited: [friends[1]], created: now - 60 };
const msgs = [
  { id: 1, from: friends[0].uuid, to: me.uuid, kind: "text", body: "ahoj, jdeš na Justice SMP?", meta: {}, created: now - 900, read: true },
  { id: 2, from: me.uuid, to: friends[0].uuid, kind: "text", body: "jj, dej mi 5 minut, dotáčím klip", meta: {}, created: now - 840, read: true },
  { id: 3, from: me.uuid, to: friends[0].uuid, kind: "world", body: "Svět hráče Maxern_", meta: { world: "x", mc: "1.21.11", loader: "fabric" }, created: now - 300, read: true },
  { id: 4, from: friends[0].uuid, to: me.uuid, kind: "pack", body: "PvP 1.21.11", meta: { name: "PvP 1.21.11", mc: "1.21.11", loader: "fabric", code: "x" }, created: now - 120, read: false },
  { id: 5, from: friends[0].uuid, to: me.uuid, kind: "text", body: "tady máš můj pack, má i voicechat 🔥", meta: {}, created: now - 110, read: false },
];

// Fake music app (Windows media controls): a small queue that advances in real time.
const D = "/packaging/design/img/";
const PLACES: Record<string, unknown> = {
  cinematic: { kind: "world", name: "My World", image: D + "shaders.webp" },
  pvp: { kind: "server", name: "mc.hypixel.net", image: D + "pvp2.webp" },
  smp: { kind: "server", name: "justicesmp.online", image: D + "vanilla.webp" },
  v263: { kind: "world", name: "Builds", image: D + "pvp.webp" },
  burger: { kind: "world", name: "New World", image: D + "nihher.webp" },
};
const musicQueue = [
  ["Blinding Lights", "The Weeknd", 200], ["Levitating", "Dua Lipa", 203], ["STAY", "The Kid LAROI, Justin Bieber", 141], ["Heat Waves", "Glass Animals", 238], ["As It Was", "Harry Styles", 167],
].map(([title, author, d], i) => ({ title, author, duration: `${Math.floor((d as number) / 60)}:${String((d as number) % 60).padStart(2, "0")}`, d: d as number, videoId: `v${i}`, thumbnails: [{ url: D + ["pvp2", "vanilla", "shaders", "nihher", "pvp"][i] + ".webp", width: 544, height: 544 }] }));
const musicS = { i: 0, pos: 78, playing: true, vol: 70, t: Date.now() };
const musicTick = () => {
  const now = Date.now();
  if (musicS.playing) musicS.pos += (now - musicS.t) / 1000;
  musicS.t = now;
  if (musicS.pos >= musicQueue[musicS.i].d) { musicS.i = (musicS.i + 1) % musicQueue.length; musicS.pos = 0; }
};

// --- Sonolith sample data (dev náhled; naostro přichází z named pipe / unix socketu) ---
const snlImg = (i: number) => D + ["pvp2", "vanilla", "shaders", "nihher", "pvp"][i % 5] + ".webp";
const snlTrack = (i: number, title: string, artist: string, d: number) => ({
  type: "track" as const, id: `t${i}`, title, artists: [{ id: `a${i}`, name: artist }],
  album: { id: `al${i}`, title: "—" }, durationMs: d * 1000, thumbnailUrl: snlImg(i), rating: "none" as const,
});
const snlCatalog = [
  snlTrack(0, "Blinding Lights", "The Weeknd", 200), snlTrack(1, "Levitating", "Dua Lipa", 203),
  snlTrack(2, "STAY", "The Kid LAROI, Justin Bieber", 141), snlTrack(3, "Heat Waves", "Glass Animals", 238),
  snlTrack(4, "As It Was", "Harry Styles", 167), snlTrack(5, "Starboy", "The Weeknd", 230),
  snlTrack(6, "Don't Start Now", "Dua Lipa", 183), snlTrack(7, "Sunflower", "Post Malone", 158),
];
const snlPlayerState = () => {
  musicTick();
  const cur = snlCatalog[musicS.i % snlCatalog.length];
  return {
    status: musicS.playing ? "playing" : "paused", item: cur,
    positionMs: Math.round(musicS.pos * 1000), positionAt: Date.now(), durationMs: cur.durationMs,
    canGoNext: true, canGoPrevious: true, shuffle: false, repeat: "off",
    queueIndex: musicS.i % snlCatalog.length, queueLength: snlCatalog.length,
  };
};
const snlStatus = {
  available: true, registered: true, connected: true, pending: false,
  grantedScopes: ["player:read", "player:control", "queue:read", "queue:write", "search", "library:read", "lyrics:read"],
  signedIn: true, appVersion: "1.4.0", error: null, retryAfterMs: null,
};
const snlDispatch: Record<string, (p: any) => unknown> = {
  "player.getState": () => snlPlayerState(),
  "player.toggle": () => { musicS.playing = !musicS.playing; return null; },
  "player.next": () => { musicS.i = (musicS.i + 1) % snlCatalog.length; musicS.pos = 0; return null; },
  "player.previous": () => { if (musicS.pos > 5) musicS.pos = 0; else musicS.i = (musicS.i + snlCatalog.length - 1) % snlCatalog.length; musicS.pos = 0; return null; },
  "player.seek": (p) => { musicS.pos = (p?.positionMs ?? 0) / 1000; return null; },
  "player.playItem": (p) => { const idx = snlCatalog.findIndex((t) => t.id === p?.item?.id); if (idx >= 0) { musicS.i = idx; musicS.pos = 0; musicS.playing = true; } return null; },
  "queue.get": (p) => ({ items: snlCatalog.slice(p?.fromIndex ?? 0, (p?.fromIndex ?? 0) + (p?.limit ?? 40)).map((item, k) => ({ index: (p?.fromIndex ?? 0) + k, entryId: `q${item.id}`, item })) }),
  "queue.add": () => ({ added: 1 }),
  "search.query": (p) => { const s = String(p?.query ?? "").toLowerCase(); return { items: snlCatalog.filter((t) => t.title!.toLowerCase().includes(s) || t.artists![0].name.toLowerCase().includes(s)) }; },
  "library.playlists": () => ({ items: [
    { type: "playlist" as const, id: "pl1", title: "Liked Music", thumbnailUrl: snlImg(2), trackCount: 128 },
    { type: "playlist" as const, id: "pl2", title: "Gaming Mix", thumbnailUrl: snlImg(0), trackCount: 54 },
    { type: "playlist" as const, id: "pl3", title: "Focus", thumbnailUrl: snlImg(3), trackCount: 31 },
  ] }),
  "lyrics.get": () => ({ status: "ok", lyrics: { lines: [
    { startMs: 0, text: "Yeah" }, { startMs: 2200, text: "I've been tryna call" },
    { startMs: 5400, text: "I've been on my own for long enough" }, { startMs: 9800, text: "Maybe you can show me how to love, maybe" },
    { startMs: 14200, text: "I'm going through withdrawals" }, { startMs: 17600, text: "You don't even have to do too much" },
    { startMs: 21000, text: "You can turn me on with just a touch, baby" }, { startMs: 25400, text: "I look around and Sin City's cold and empty" },
    { startMs: 29800, text: "No one's around to judge me" }, { startMs: 33200, text: "I can't see clearly when you're gone" },
  ] } }),
};

const handlers: Record<string, (a: any) => unknown> = {
  optimize_plan: () =>
    new Promise((r) =>
      setTimeout(
        () =>
          r({
            tier: "mid",
            hardware: "8 vláken CPU, 16 GB RAM",
            undoAvailable: false,
            changes: [
              { id: "mods", group: "mods", title: "Doinstalovat 17 výkonových modů", detail: "C2ME, ScalableLux, Ixeris, Gnetum, VMP, Async Logger, Particle Core, Cull Fewer Leaves, Structure Layout Optimizer, Fast Noise, FastQuit, Force Close Loading Screen, Better Mipmaps, OptiPainting Reloaded, Sciophobia, OBE, Windows Priority Boost", from: null, to: null },
              { id: "disable:OptiFine_1.21.11_HD_U_J7.jar", group: "conflicts", title: "Vypnout OptiFine_1.21.11_HD_U_J7.jar", detail: "OptiFine se Sodiem padá nebo zpomaluje; Sodium dělá totéž lépe.", from: null, to: null },
              { id: "opt:renderDistance=12", group: "video", title: "Dohled", detail: "", from: "18 chunků", to: "12 chunků" },
              { id: "opt:graphicsMode=1", group: "video", title: "Grafika", detail: "", from: "Úžasná", to: "Pěkná" },
              { id: "sodium", group: "sodium", title: "Nastavení Sodia pro tvůj procesor", detail: "Méně vláken na stavbu chunků a bez pokročilých staging bufferů: při načítání světa nebere procesor hře.", from: null, to: null },
              { id: "ram:6144", group: "memory", title: "Paměť pro hru", detail: "Na 97 modů je to málo, hra by čistila paměť příliš často.", from: "2 GB", to: "6 GB" },
            ],
          }),
        900,
      ),
    ),
  optimize_apply: (a) => new Promise((r) => setTimeout(() => r({ applied: a.ids, installed: ["C2ME", "ScalableLux", "Ixeris", "Gnetum"], unavailable: ["sciophobia"] }), 1500)),
  optimize_undo: () => null,
  repair_plan: () => ({
    supported: true,
    undoAvailable: false,
    crash: { title: "Konflikt modů (Mixin)", detail: "Dva mody se perou o stejnou část hry. Podezřelý mod je uvedený níže – vypni ho a zkus to znovu.", suspect: "sodium-extra", crashFile: null, excerpt: [] },
    actions: [
      { id: "repair-crash", group: "crash", title: "Opravit konflikt z posledního pádu", detail: "Log ukazuje selhání mixinu; sladím verze modů, které se perou.", severity: "error", checked: true },
      { id: "disable:sodium-0.5.3.jar", group: "duplicate", title: "Vypnout starší sodium 0.5.3", detail: "sodium je tam dvakrát; nechávám 0.6.13 a vypínám tuhle starší verzi.", severity: "error", checked: true },
      { id: "install:cloth-config", group: "missing", title: "Doinstalovat cloth-config", detail: "Potřebuje ho modmenu – stáhnu ho z Modrinthu pro tvou verzi.", severity: "error", checked: true },
      { id: "install:fabric-api", group: "missing", title: "Doinstalovat fabric-api", detail: "Potřebuje ho sodium – stáhnu ho z Modrinthu pro tvou verzi.", severity: "error", checked: true },
      { id: "fix-conflicts", group: "conflict", title: "Vyřešit konflikty verzí (2)", detail: "iris 1.7.0 potřebuje sodium 0.5.x, je tam 0.6.13\nindium 1.0.3 nejde s sodium 0.6.13", severity: "warn", checked: true },
    ],
  }),
  repair_apply: (a) => new Promise((r) => setTimeout(() => r({ fixed: (a.ids as string[]).map((id: string) => `Hotovo: ${id}`), remaining: [] }), 1400)),
  repair_undo: () => null,
  download_list: () => DLDEMO ? [
    { id: 1, name: "Fabulously Optimized", kind: "modpack", done: 182 * 1024 ** 2, total: 410 * 1024 ** 2, speed: 11.4 * 1024 ** 2, etaSecs: 20, status: "running", error: null },
    { id: 2, name: "Cobblemon", kind: "modpack", done: 64 * 1024 ** 2, total: 540 * 1024 ** 2, speed: 0, etaSecs: 0, status: "paused", error: null },
    { id: 3, name: "Create: Astral", kind: "modpack", done: 320 * 1024 ** 2, total: 320 * 1024 ** 2, speed: 0, etaSecs: 0, status: "done", error: null },
  ] : [],
  download_modpack: () => 1,
  download_pause: () => null,
  download_resume: () => null,
  download_cancel: () => null,
  download_clear: () => null,
  modbot_chat: (a) => new Promise((r) => setTimeout(() => r({
    reply: `Jasně! Pro ${a.mc} na ${a.loader} bych pro plynulost a pěkný survival zvolil tyhle mody. Zaškrtni, co chceš, a sestav:`,
    limited: false,
    mods: [
      { id: "AANobbMI", slug: "sodium", title: "Sodium", description: "Rychlé vykreslování", iconUrl: D + "shaders.webp", downloads: 48_000_000, reason: "Výrazně víc FPS" },
      { id: "gvQqBUqZ", slug: "lithium", title: "Lithium", description: "Optimalizace logiky", iconUrl: D + "vanilla.webp", downloads: 41_000_000, reason: "Plynulejší herní tick" },
      { id: "YL57xq9U", slug: "iris", title: "Iris Shaders", description: "Shadery", iconUrl: D + "pvp2.webp", downloads: 22_000_000, reason: "Podpora shaderů" },
      { id: "1u6JkXh5", slug: "modmenu", title: "Mod Menu", description: "Nastavení modů", iconUrl: D + "nihher.webp", downloads: 35_000_000, reason: "Správa a nastavení modů" },
      { id: "nfn13YXA", slug: "xaeros-minimap", title: "Xaero's Minimap", description: "Minimapa", iconUrl: D + "pvp.webp", downloads: 18_000_000, reason: "Minimapa do rohu" },
    ],
  }), 1200)),
  modbot_build: (a) => new Promise((r) => setTimeout(() => r({ id: "cinematic", name: `MaxPack ${a.mc}`, mcVersion: a.mc, loader: a.loader }), 1500)),
  modbot_add: (a) => ({ installed: a.slugs, skipped: [], manual: [] }),
  sonolith_status: () => snlStatus,
  sonolith_connect: () => null,
  sonolith_disconnect: () => null,
  sonolith_call: (a) => { const h = snlDispatch[a.method]; if (h) return h(a.params ?? {}); console.debug("[devMock] unhandled sonolith", a.method, a.params); return null; },
  instance_last_place: (a) => PLACES[a.id] ?? null,
  version_art: () => {
    const M = "https://launchercontent.mojang.com/v2/images/";
    const H = "https://justiceclient.online/art/";
    return {
      "26.3": { name: "Wilderness Bound", image: M + "dappledcamp540x540.jpg", wide: M + "MinecraftWildernessBoundLauncher772x350.png", hd: H + "wilderness-bound.jpg" },
      "26.2": { name: "Chaos Cubed", image: M + "26.2540x540.jpg", wide: M + "summerdropaddon772x350.png", hd: H + "chaos-cubed.jpg" },
      "1.21.11": { name: "Mounts of Mayhem", image: M + "1.21.11540x540.jpg", wide: M + "MinecraftMountsofMayhemDungeonDescentLauncher772x350.png", hd: H + "mounts-of-mayhem.jpg" },
      "26.1.2": { name: "Tiny Takeover", image: M + "26.1540x540.jpg", wide: M + "TinyTakeoverMiniManiaLauncher772x350.png", hd: H + "tiny-takeover.jpg" },
      "1.21.10": { name: "The Copper Age", image: M + "1.21.9540x540.jpg", wide: M + "MinecraftFallDropCampaignKeyArtMinecraftLauncher772x350.png", hd: H + "copper-age.jpg" },
    };
  },
  media_state: () => {
    musicTick();
    const q = musicQueue[musicS.i];
    return { id: "msedgewebview2.exe", source: "Sonolith", title: q.title, artist: q.author, album: "", thumb: q.thumbnails[0].url, thumbKey: q.title, position: musicS.pos, duration: q.d, playing: musicS.playing, canNext: true, canPrev: true, canSeek: true,
      sessions: [{ id: "msedgewebview2.exe", source: "Sonolith", title: q.title, playing: musicS.playing }, { id: "308046B0AF4A39CB", source: "Firefox", title: "Rating the VIRAL Spicy Bowl", playing: false }] };
  },
  media_command: (a) => {
    musicTick();
    const c = a.command;
    if (c === "playPause") musicS.playing = !musicS.playing;
    else if (c === "next") { musicS.i = (musicS.i + 1) % musicQueue.length; musicS.pos = 0; }
    else if (c === "previous") { if (musicS.pos > 5) musicS.pos = 0; else { musicS.i = (musicS.i + musicQueue.length - 1) % musicQueue.length; musicS.pos = 0; } }
    else if (c === "seek") musicS.pos = a.value;
  },
  open_music_app: () => null,
  get_settings: () => settings,
  save_settings: (a) => Object.assign(settings, a.settings),
  system_info: () => ({ totalRamMb: 16384, recommendedRamMb: 6144, version: "0.2.1-dev", hasCfKey: true }),
  hardware_info: () => ({ cpu: "12th Gen Intel(R) Core(TM) i3-12100F", threads: 8, gpus: ["NVIDIA GeForce GTX 1660 SUPER"], totalRamMb: 16384 }),
  list_instances: () => sampleInstances,
  get_instance: (a) => sampleInstances.find((i) => i.id === a.id),
  game_versions: () => ["26.3", "26.2", "26.1.2", "26.1", "1.21.11", "1.21.10", "1.21.9", "1.21.8", "1.21.4", "1.21.1", "1.20.6", "1.20.4", "1.20.1", "1.19.4", "1.18.2", "1.16.5", "1.12.2", "1.8.9"].map((id) => ({ id, kind: "release", releaseTime: "" })),
  loader_versions: () => [{ version: "0.17.2", stable: true }, { version: "0.17.1", stable: true }],
  instance_path: () => "D:/JusticeClient/instances/x",
  list_accounts: () => [{ id: "a", kind: OFFLINE ? "offline" : "msa", username: OFFLINE ? "Pepa_CZ" : "Maxern_", uuid: "069a79f444e94726a5befca90e38aaf5", skinUrl: "", skinVariant: "CLASSIC", active: true }],
  running_games: () => (LOGWIN ? ["cinematic"] : []),
  game_logs: () => LOGWIN ? SAMPLE_LOG : [],
  open_logs_window: () => null,
  list_servers: () => [
    { id: "s1", name: "Justice SMP", address: "justicesmp.online", instance: "smp", pinned: true },
    { id: "s2", name: "Ertenix", address: "ertenix.net", instance: "pvp", pinned: true },
    { id: "s3", name: "Hypixel", address: "mc.hypixel.net", instance: "pvp", pinned: false },
    { id: "s4", name: "Starý server", address: "old.example.cz", instance: "", pinned: false },
  ],
  ping_server: (a) => a.address.startsWith("old.") ? Promise.reject("offline") : a.address.includes("hypixel") ? { online: 41203, max: 200000, motd: "§aHypixel Network §c[1.8-1.21] §6§lSKYBLOCK §e§lUPDATE", version: "1.21", icon: null, pingMs: 98 } : ({ online: a.address.includes("30000") ? 12 : 57, max: a.address.includes("30000") ? 69 : 500, motd: "§c-=- JUSTICE SMP -=-", version: "1.21.11", icon: null, pingMs: 24 }),
  free_space: () => 48 * 1024 ** 3,
  list_content: () => [{ projectId: "AANobbMI", title: "Sodium" }],
  search_content: () => ({
    total: 6,
    hits: [
      ["AANobbMI", "Sodium", "jellysquid3", "Moderní renderer, výrazně víc FPS bez změny vzhledu.", 82e6, ["optimization"]],
      ["YL57xq9U", "Iris Shaders", "coderbot", "Shadery pro Fabric, funguje s OptiFine packy a Sodiem.", 51e6, ["decoration", "optimization"]],
      ["gvQqBUqZ", "Lithium", "jellysquid3", "Optimalizace herní logiky: AI, fyzika, chunky.", 40e6, ["optimization"]],
      ["uXXizFIs", "Distant Horizons", "James Seibel", "Obrovská dohlednost díky úrovním detailu.", 9e6, ["optimization"]],
      ["4cXLbBGw", "Flashback", "Moulberry", "Nahrávání a střih záznamů ze hry, cinematic kamera.", 3e6, ["utility"]],
      ["w7ThoJFB", "Zoomify", "isXander", "Plynulý zoom jako v OptiFine, se spoustou nastavení.", 12e6, ["utility"]],
    ].map(([id, title, author, description, downloads, categories]) => ({ id, slug: String(title).toLowerCase(), title, author, description, downloads, categories, iconUrl: null, url: "", source: "modrinth", updated: new Date(Date.now() - 86400e3 * 5).toISOString() })),
  }),
  clips_get_settings: () => clipsSettings,
  clips_set_settings: (a) => Object.assign(clipsSettings, a.s),
  clips_status: () => ({ state: "recording", error: null, bufferBytes: 90e6, bufferSecs: 58.4, ffmpeg: "C:\ffmpeg\ffmpeg.exe", hotkeyError: null, gameRunning: true }),
  clips_list: () => sampleClips,
  clips_publish: () => ({ id: "new", owner: me.uuid, ownerName: "Maxern_", url: "", thumb: "", title: "Nový klip", description: "", tags: [], hashtags: [], durationMs: 12000, width: 0, height: 720, captured: 0, created: Math.floor(Date.now() / 1000), likes: 0, liked: false, mine: true }),
  clips_feed: () => ({ clips: [
    { id: "f1", owner: me.uuid, ownerName: "Maxern_", url: "", thumb: D + "pvp2.webp", title: "Insane clutch 1v4", description: "Poslední hráč, 2 HP, žádný heal 😮‍💨", tags: ["jeb_"], hashtags: ["pvp", "clutch"], durationMs: 18000, width: 0, height: 720, captured: 0, created: Math.floor(Date.now() / 1000) - 3600, likes: 42, liked: true, mine: true },
    { id: "f2", owner: "x", ownerName: "Notch", url: "", thumb: D + "shaders.webp", title: "Sunset nad vesnicí", description: "Complementary shaders jsou nádhera", tags: [], hashtags: ["shaders", "build"], durationMs: 9000, width: 0, height: 720, captured: 0, created: Math.floor(Date.now() / 1000) - 86400, likes: 128, liked: false, mine: false },
    { id: "f3", owner: "y", ownerName: "jeb_", url: "", thumb: D + "nihher.webp", title: "Creeper mě dostal", description: "", tags: ["Maxern_"], hashtags: ["fail", "funny"], durationMs: 6000, width: 0, height: 720, captured: 0, created: Math.floor(Date.now() / 1000) - 172800, likes: 7, liked: false, mine: false },
  ] }),
  clips_like: (a) => ({ likes: 43, liked: true, id: a.id }),
  clips_report: () => null,
  clips_feed_delete: () => null,
  social_status: () => ({ signedIn: !OFFLINE, uuid: OFFLINE ? null : me.uuid }),
  social_login: () => Promise.reject("JUSTICE_NEEDS_PASSWORD"),
  social_login_password: () => ({ token: "t", uuid: me.uuid, name: "Pepa_CZ" }),
  pvp_stats: () => me.profile.stats.pvp,
  get_capes: () => [],
  skinlib_list: () => [
    { id: "s1", name: "Můj skin", variant: "classic", added: 0, file: "https://mc-heads.net/skin/MHF_Steve" },
    { id: "s2", name: "Alex", variant: "slim", added: 0, file: "https://mc-heads.net/skin/MHF_Alex" },
    { id: "s3", name: "Notch", variant: "classic", added: 0, file: "https://mc-heads.net/skin/Notch" },
    { id: "s4", name: "Ninja", variant: "classic", added: 0, file: "https://mc-heads.net/skin/jeb_" },
  ],
  skinlib_add_file: () => "new",
  skinlib_add_player: () => "new",
  skinlib_add_current: () => "new",
  skinlib_apply: () => null,
  skinlib_rename: () => null,
  skinlib_remove: () => null,
  party_warp_here: () => ({ sent: 1 }),
  social_get: (a) =>
    a.path === "/me" ? me : a.path === "/friends" ? { friends, incoming: [], outgoing: [] } : a.path.startsWith("/messages/") ? { messages: msgs } : a.path === "/party" ? { party, invites: [] } : {},
  social_put: () => me,
  clips_devices: () => ({ outputs: ["Sluchátka (TUF GAMING H1)"], inputs: ["Mikrofon (TUF GAMING H1)"], defaultOutput: "Sluchátka (TUF GAMING H1)", defaultInput: "Mikrofon (TUF GAMING H1)" }),
  clips_monitors: () => [{ index: 0, name: "", width: 1920, height: 1080, primary: true }, { index: 1, name: "", width: 1920, height: 1080, primary: false }],
  "plugin:updater|check": () => SHOTS ? null : ({ rid: 1, currentVersion: "0.2.0", version: "0.2.1", date: new Date().toISOString(), body: "- Rychlé připojení na Justice SMP a Ertenix", rawJson: {} }),
};

(window as any).__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
  transformCallback: () => Math.floor(Math.random() * 1e9),
  convertFileSrc: (p: string) => p,
  invoke: async (cmd: string, args: any) => {
    if (cmd.startsWith("plugin:event|")) return 1;
    const h = handlers[cmd];
    if (h) return h(args ?? {});
    console.debug("[devMock] unhandled", cmd, args);
    return null;
  },
};
(window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };

export {};
