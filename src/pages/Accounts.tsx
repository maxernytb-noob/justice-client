import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { convertFileSrc } from "@tauri-apps/api/core";
import { SkinViewer, WalkingAnimation } from "skinview3d";
import { api, type Account, type Cape, type DeviceCode, type SkinEntry } from "../api";
import { Icon } from "../icons";
import { errText, useStore } from "../store";
import { Avatar, Empty } from "../components/common";
import { tr } from "../i18n";

export function AddAccount(_props: { compact?: boolean }) {
  const { reloadAccounts, toast } = useStore();
  const [code, setCode] = useState<DeviceCode | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);
  const [offline, setOffline] = useState(false);
  const [name, setName] = useState("");
  const nameOk = /^[A-Za-z0-9_]{3,16}$/.test(name);

  const addOffline = async () => {
    if (!nameOk) return;
    setErr("");
    try {
      await api.addOffline(name);
      await reloadAccounts();
      toast(tr(`Offline účet ${name} přidán`, `Offline account ${name} added`), "success");
      setName("");
      setOffline(false);
    } catch (e) {
      setErr(errText(e));
    }
  };

  const startMs = async () => {
    setErr("");
    setBusy(true);
    try {
      const c = await api.msLoginStart();
      setCode(c);
      await navigator.clipboard.writeText(c.userCode).catch(() => {});
      openUrl(c.verificationUri);
      const acc = await api.msLoginPoll(c.deviceCode, c.interval);
      await reloadAccounts();
      toast(`${tr("Přihlášen jako", "Signed in as")} ${acc.username}`, "success");
      setCode(null);
    } catch (e) {
      setErr(errText(e));
      setCode(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="col">
      {code ? (
        <div className="card pad col" style={{ gap: 12 }}>
          <div>
            {tr(
              "1. V prohlížeči se otevřela stránka Microsoftu. 2. Vlož tento kód (už je ve schránce):",
              "1. A Microsoft page opened in your browser. 2. Enter this code (it's already in your clipboard):",
            )}
          </div>
          <div className="code-box">{code.userCode}</div>
          <div className="row">
            <span className="spinner" />
            <span className="muted small grow">{tr("Čekám na přihlášení v prohlížeči…", "Waiting for you to sign in in the browser…")}</span>
            <button className="btn sm" onClick={() => openUrl(code.verificationUri)}>
              <Icon name="external" size={14} /> {tr("Otevřít znovu", "Open again")}
            </button>
            <button className="btn sm ghost" onClick={() => api.msLoginCancel()}>
              {tr("Zrušit", "Cancel")}
            </button>
          </div>
        </div>
      ) : (
        <button className="btn primary" style={{ height: 44 }} onClick={startMs} disabled={busy}>
          <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true">
            <path fill="#f25022" d="M1 1h10v10H1z" />
            <path fill="#7fba00" d="M13 1h10v10H13z" />
            <path fill="#00a4ef" d="M1 13h10v10H1z" />
            <path fill="#ffb900" d="M13 13h10v10H13z" />
          </svg>
          {tr("Přihlásit přes Microsoft", "Sign in with Microsoft")}
        </button>
      )}
      {!code &&
        (offline ? (
          <div className="card pad col" style={{ gap: 10 }}>
            <b>{tr("Offline účet", "Offline account")}</b>
            <span className="small muted">
              {tr(
                "Bez koupeného Minecraftu: singleplayer, LAN, sdílené světy a servery bez ověření. Přátelé a chat fungují s heslem k Justice účtu.",
                "Without owning Minecraft: singleplayer, LAN, shared worlds and servers without authentication. Friends and chat work with a Justice account password.",
              )}
            </span>
            <div className="row" style={{ gap: 8 }}>
              <input
                className="input grow"
                placeholder={tr("Herní jméno (3–16 znaků)", "Player name (3–16 characters)")}
                value={name}
                maxLength={16}
                autoFocus
                onChange={(e) => setName(e.target.value.replace(/[^A-Za-z0-9_]/g, ""))}
                onKeyDown={(e) => e.key === "Enter" && addOffline()}
              />
              <button className="btn primary" onClick={addOffline} disabled={!nameOk}>
                <Icon name="plus" size={15} /> {tr("Přidat", "Add")}
              </button>
            </div>
            <button className="btn sm ghost" style={{ alignSelf: "flex-start" }} onClick={() => setOffline(false)}>
              {tr("Zpět", "Back")}
            </button>
          </div>
        ) : (
          <button className="btn ghost" onClick={() => setOffline(true)}>
            <Icon name="user" size={15} /> {tr("Nemáš koupený Minecraft? Přidat offline účet", "Don't own Minecraft? Add an offline account")}
          </button>
        ))}
      {err && (
        <div className="callout red">
          <Icon name="alert" size={18} />
          <span>{err}</span>
        </div>
      )}
    </div>
  );
}

function Skin3D({ account }: { account: Account }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const viewer = useRef<SkinViewer | null>(null);
  const [capeUrl, setCapeUrl] = useState<string | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    const v = new SkinViewer({ canvas: ref.current, width: 260, height: 340 });
    v.animation = new WalkingAnimation();
    v.animation.speed = 0.6;
    v.autoRotate = true;
    v.autoRotateSpeed = 0.6;
    v.zoom = 0.85;
    viewer.current = v;
    return () => v.dispose();
  }, []);

  useEffect(() => {
    const v = viewer.current;
    if (!v) return;
    // By uuid, offline accounts without a Justice skin get the default skin (by name it'd be a premium player's).
    const url = account.skinUrl || `https://mc-heads.net/skin/${account.uuid.replace(/-/g, "")}`;
    api.fetchImage(url)
      .then((data) => v.loadSkin(data, { model: account.skinVariant === "SLIM" ? "slim" : "default" }))
      .catch(() => v.loadSkin(null));
  }, [account.skinUrl, account.skinVariant, account.uuid, account.username, account.kind]);

  useEffect(() => {
    const v = viewer.current;
    if (!v) return;
    if (capeUrl) api.fetchImage(capeUrl).then((d) => v.loadCape(d)).catch(() => v.loadCape(null));
    else v.loadCape(null);
  }, [capeUrl]);

  return (
    <div style={{ display: "grid", placeItems: "center", background: "radial-gradient(circle at 50% 40%, var(--red-dim), var(--bg-2) 70%)", borderRadius: 14, border: "1px solid var(--line)" }}>
      <canvas ref={ref} />
      <CapePicker account={account} onPreview={setCapeUrl} />
    </div>
  );
}

function CapePicker({ account, onPreview }: { account: Account; onPreview: (url: string | null) => void }) {
  const { toast } = useStore();
  const [capes, setCapes] = useState<Cape[]>([]);
  useEffect(() => {
    // Capes of the previous account mustn't stay after switching (an offline account has none).
    setCapes([]);
    onPreview(null);
    if (account.kind !== "msa") return;
    api.getCapes(account.id)
      .then((c) => {
        setCapes(c ?? []);
        onPreview(c?.find((x) => x.active)?.url ?? null);
      })
      .catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [account.id]);
  if (!capes.length) return <div style={{ height: 12 }} />;
  const choose = async (c: Cape | null) => {
    try {
      await api.setCape(account.id, c?.id ?? null);
      setCapes((all) => all.map((x) => ({ ...x, active: x.id === c?.id })));
      onPreview(c?.url ?? null);
      toast(c ? `Cape: ${c.alias}` : tr("Cape skryta", "Cape hidden"), "success");
    } catch (e) {
      toast(errText(e), "error");
    }
  };
  return (
    <div className="chips" style={{ padding: 12, justifyContent: "center" }}>
      <button className={`chip ${!capes.some((c) => c.active) ? "on" : ""}`} onClick={() => choose(null)}>{tr("Bez cape", "No cape")}</button>
      {capes.map((c) => (
        <button key={c.id} className={`chip ${c.active ? "on" : ""}`} onClick={() => choose(c)}>{c.alias}</button>
      ))}
    </div>
  );
}

function SkinEditor({ account }: { account: Account }) {
  const { reloadAccounts, toast } = useStore();
  const [variant, setVariant] = useState(account.skinVariant === "SLIM" ? "slim" : "classic");
  const [player, setPlayer] = useState("");
  const [busy, setBusy] = useState(false);
  const offline = account.kind !== "msa";

  const run = async (f: () => Promise<void>, done: string) => {
    setBusy(true);
    try {
      await f();
      await reloadAccounts();
      toast(done, "success");
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const uploadSkin = async () => {
    const path = await open({ filters: [{ name: "Skin", extensions: ["png"] }] });
    if (path) run(() => api.uploadSkin(account.id, path as string, variant), tr("Skin změněn", "Skin changed"));
  };
  const copySkin = () => {
    if (player.length < 3) return;
    run(() => api.skinFromPlayer(account.id, player), tr(`Máš skin hráče ${player}`, `You now wear ${player}'s skin`)).then(() => setPlayer(""));
  };

  return (
    <div className="card pad col" style={{ gap: 10 }}>
      <b>{tr("Změnit skin", "Change skin")}</b>
      {offline && (
        <span className="small muted">
          {tr(
            "Skin offline účtu uvidíš ty a ostatní hráči s Justice Clientem (když jsi přihlášený do přátel). Ostatní klienti ukazují výchozí skin.",
            "Your offline skin is shown to you and other Justice Client players (while you're signed in to friends). Other clients show the default skin.",
          )}
        </span>
      )}
      <div className="chips">
        <button className={`chip ${variant === "classic" ? "on" : ""}`} onClick={() => setVariant("classic")}>{tr("Klasický (Steve)", "Classic (Steve)")}</button>
        <button className={`chip ${variant === "slim" ? "on" : ""}`} onClick={() => setVariant("slim")}>Slim (Alex)</button>
      </div>
      <button className="btn" onClick={uploadSkin} disabled={busy}>
        <Icon name="upload" size={16} /> {tr("Nahrát .png skin", "Upload .png skin")}
      </button>
      <div className="row" style={{ gap: 8 }}>
        <input
          className="input grow"
          placeholder={tr("Skin podle jména hráče", "Skin from a player name")}
          value={player}
          maxLength={16}
          onChange={(e) => setPlayer(e.target.value.replace(/[^A-Za-z0-9_]/g, ""))}
          onKeyDown={(e) => e.key === "Enter" && copySkin()}
        />
        <button className="btn" onClick={copySkin} disabled={busy || player.length < 3}>
          {tr("Použít", "Use")}
        </button>
      </div>
      {account.skinUrl && (
        <button className="btn sm ghost" style={{ alignSelf: "flex-start" }} disabled={busy} onClick={() => run(() => api.resetSkin(account.id), tr("Skin vrácen na výchozí", "Skin reset to default"))}>
          {tr("Vrátit výchozí skin", "Reset to the default skin")}
        </button>
      )}
    </div>
  );
}

/** The face of a skin texture (8x8 at 8,8), scaled up with crisp pixels. */
function SkinFace({ file, size = 56 }: { file: string; size?: number }) {
  const s = size / 8;
  return (
    <div
      className="skinface"
      style={{
        width: size,
        height: size,
        backgroundImage: `url("${convertFileSrc(file)}")`,
        backgroundSize: `${64 * s}px ${64 * s}px`,
        backgroundPosition: `${-8 * s}px ${-8 * s}px`,
        imageRendering: "pixelated",
      }}
    />
  );
}

/** Saved skins: keep several and switch any account between them. */
function SkinLibrary({ account }: { account: Account }) {
  const { toast, reloadAccounts } = useStore();
  const [skins, setSkins] = useState<SkinEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const reload = () => api.skinlibList().then(setSkins).catch(() => {});
  useEffect(() => { reload(); }, []);

  const run = async (f: () => Promise<unknown>, done?: string) => {
    setBusy(true);
    try {
      await f();
      reload();
      if (done) toast(done, "success");
    } catch (e) {
      toast(errText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const addFile = async () => {
    const path = await open({ filters: [{ name: "Skin", extensions: ["png"] }] });
    if (path) run(() => api.skinlibAddFile(path as string, "", account.skinVariant === "SLIM" ? "slim" : "classic"), tr("Přidáno do knihovny", "Added to the library"));
  };
  const addPlayer = async () => {
    const name = prompt(tr("Jméno hráče, jehož skin chceš uložit:", "Player whose skin to save:"))?.trim();
    if (name) run(() => api.skinlibAddPlayer(name, ""), tr("Přidáno do knihovny", "Added to the library"));
  };
  const saveCurrent = () => run(() => api.skinlibAddCurrent(account.id, tr("Můj skin", "My skin")), tr("Aktuální skin uložen", "Current skin saved"));
  const apply = (s: SkinEntry) => run(async () => { await api.skinlibApply(account.id, s.id); await reloadAccounts(); }, tr(`Skin „${s.name}" nasazen`, `Skin "${s.name}" applied`));
  const rename = (s: SkinEntry) => {
    const name = prompt(tr("Nový název:", "New name:"), s.name)?.trim();
    if (name && name !== s.name) run(() => api.skinlibRename(s.id, name));
  };
  const remove = (s: SkinEntry) => run(() => api.skinlibRemove(s.id));

  return (
    <div className="card pad col" style={{ gap: 10 }}>
      <div className="row">
        <b className="grow">{tr("Knihovna skinů", "Skin library")}</b>
        <button className="btn sm" onClick={saveCurrent} disabled={busy} title={tr("Uložit skin tohoto účtu do knihovny", "Save this account's skin to the library")}>
          <Icon name="plus" size={14} /> {tr("Uložit aktuální", "Save current")}
        </button>
      </div>
      <div className="row" style={{ gap: 8 }}>
        <button className="btn sm ghost" onClick={addFile} disabled={busy}><Icon name="upload" size={14} /> {tr("Z PC", "From file")}</button>
        <button className="btn sm ghost" onClick={addPlayer} disabled={busy}><Icon name="user" size={14} /> {tr("Podle jména", "From a name")}</button>
      </div>
      {skins.length === 0 ? (
        <span className="small muted">{tr("Zatím prázdné. Ulož si sem skiny a přepínej mezi nimi jedním klikem.", "Empty for now. Save skins here and switch between them in one click.")}</span>
      ) : (
        <div className="skinlib-grid">
          {skins.map((s) => (
            <div key={s.id} className="skinlib-item">
              <button className="skinlib-face" onClick={() => apply(s)} disabled={busy} title={tr("Nasadit tenhle skin", "Apply this skin")}>
                <SkinFace file={s.file} />
              </button>
              <div className="skinlib-name ellipsis" onClick={() => rename(s)} title={tr("Přejmenovat", "Rename")}>{s.name}</div>
              <button className="skinlib-del" onClick={() => remove(s)} disabled={busy} title={tr("Smazat", "Delete")}><Icon name="trash" size={13} /></button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default function Accounts() {
  const { accounts, activeAccount, reloadAccounts, settings } = useStore();

  return (
    <div className="page">
      <div className="page-head">
        <h1>{tr("Účty a skiny", "Accounts and skins")}</h1>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "300px 1fr", gap: 20, alignItems: "start" }}>
        <div className="col">
          {activeAccount ? <Skin3D account={activeAccount} /> : <Empty icon="user" title={tr("Žádný účet", "No account")} />}
          {activeAccount && <SkinEditor key={activeAccount.id} account={activeAccount} />}
          {activeAccount && <SkinLibrary key={`lib-${activeAccount.id}`} account={activeAccount} />}
        </div>
        <div className="col">
          <div className="section-title">{tr("Tvoje účty", "Your accounts")}</div>
          {accounts.length === 0 ? (
            <div className="muted">{tr("Zatím žádný účet.", "No accounts yet.")}</div>
          ) : (
            <div className="list">
              {accounts.map((a) => (
                <div key={a.id} className="list-row">
                  <Avatar account={a} size={34} />
                  <div className="grow">
                    <div style={{ fontWeight: 600 }}>{a.username}</div>
                    <div className="small muted">
                      {a.kind === "msa" ? tr("Microsoft účet", "Microsoft account") : tr("Offline účet", "Offline account")}
                      {!settings?.streamerMode && a.kind === "msa" ? ` · ${a.uuid.slice(0, 8)}…` : ""}
                    </div>
                  </div>
                  {a.active ? (
                    <span className="badge green">{tr("Aktivní", "Active")}</span>
                  ) : (
                    <button className="btn sm" onClick={() => api.setActiveAccount(a.id).then(reloadAccounts)}>
                      {tr("Použít", "Use")}
                    </button>
                  )}
                  <button
                    className="btn sm ghost icon danger"
                    title={tr("Odebrat", "Remove")}
                    onClick={() => api.removeAccount(a.id).then(reloadAccounts)}
                  >
                    <Icon name="logout" size={15} />
                  </button>
                </div>
              ))}
            </div>
          )}
          <div className="section-title">{tr("Přidat účet", "Add account")}</div>
          <AddAccount />
        </div>
      </div>
    </div>
  );
}
