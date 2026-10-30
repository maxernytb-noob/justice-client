import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useStore } from "../store";
import { Icon } from "../icons";
import { AddAccount } from "../pages/Accounts";
import Logo from "./Logo";
import { ACCENT_PRESETS } from "../theme";
import { tr } from "../i18n";

export default function Onboarding() {
  const { settings, saveSettings, accounts } = useStore();
  const [step, setStep] = useState(0);
  if (!settings) return null;

  const pickDir = async () => {
    const d = await open({ directory: true, defaultPath: settings.dataDir });
    if (d) saveSettings({ dataDir: d as string });
  };

  return (
    <div className="overlay" style={{ background: "rgba(4,4,5,.92)" }}>
      <div className="modal" style={{ width: 560 }}>
        <div className="modal-body" style={{ padding: 28 }}>
          {step === 0 && (
            <div className="col" style={{ gap: 18 }}>
              <div className="row" style={{ gap: 16 }}>
                <Logo size={64} />
                <div>
                  <h2 style={{ margin: 0, fontSize: 24 }}>
                    {tr("Vítej v", "Welcome to")} Justice <span style={{ color: "var(--red)" }}>Client</span>
                  </h2>
                  <div className="muted">
                    {tr("Launcher pro natáčení — čistý, rychlý, s maximem FPS.", "A launcher for content creators — clean, fast, with maximum FPS.")}
                  </div>
                </div>
              </div>
              <div className="field">
                <label>Jazyk / Language</label>
                <div className="chips">
                  <button className={`chip ${settings.language === "cs" ? "on" : ""}`} onClick={() => saveSettings({ language: "cs" })}>Čeština</button>
                  <button className={`chip ${settings.language === "en" ? "on" : ""}`} onClick={() => saveSettings({ language: "en" })}>English</button>
                </div>
              </div>
              <div className="field">
                <label>{tr("Barva launcheru (později i vlastní hex v Nastavení)", "Launcher colour (a custom hex is available later in Settings)")}</label>
                <div className="row wrap" style={{ gap: 8 }}>
                  {ACCENT_PRESETS.map((p) => (
                    <button
                      key={p.hex}
                      title={tr(p.name, p.nameEn)}
                      aria-label={tr(p.name, p.nameEn)}
                      onClick={() => saveSettings({ accentColor: p.hex })}
                      style={{
                        width: 30, height: 30, borderRadius: 9, cursor: "pointer", background: p.hex,
                        border: settings.accentColor === p.hex ? "2px solid var(--text)" : "2px solid transparent",
                      }}
                    />
                  ))}
                </div>
              </div>
              <div className="field">
                <label>{tr("Kam ukládat hru, mody a světy", "Where to store the game, mods and worlds")}</label>
                <div className="row">
                  <input className="input mono" value={settings.dataDir} readOnly style={{ fontSize: 12 }} />
                  <button className="btn" onClick={pickDir}>
                    <Icon name="folder" size={16} /> {tr("Změnit", "Change")}
                  </button>
                </div>
                <span className="hint">
                  {tr(
                    "Počítej s pár GB na každou verzi. Pokud máš málo místa na systémovém disku, vyber jiný.",
                    "Expect a few GB per game version. If your system drive is low on space, pick another one.",
                  )}
                </span>
              </div>
              <div className="row" style={{ justifyContent: "flex-end" }}>
                <button className="btn primary" onClick={() => setStep(1)}>
                  {tr("Pokračovat", "Continue")} <Icon name="right" size={16} />
                </button>
              </div>
            </div>
          )}
          {step === 1 && (
            <div className="col" style={{ gap: 18 }}>
              <div>
                <h2 style={{ margin: 0 }}>{tr("Přidej účet", "Add an account")}</h2>
                <div className="muted">
                  {tr(
                    "Přihlas se svým Microsoft účtem, se kterým hraješ Minecraft.",
                    "Sign in with the Microsoft account you play Minecraft with.",
                  )}
                </div>
              </div>
              <AddAccount compact />
              {accounts.length > 0 && (
                <div className="callout">
                  <Icon name="check" size={18} style={{ color: "var(--green)" }} />
                  <span>
                    {tr("Účet", "Account")} <b style={{ color: "var(--text)" }}>{accounts.find((a) => a.active)?.username}</b>{" "}
                    {tr("je připravený.", "is ready.")}
                  </span>
                </div>
              )}
              <div className="row" style={{ justifyContent: "space-between" }}>
                <button className="btn ghost" onClick={() => setStep(0)}>
                  <Icon name="left" size={16} /> {tr("Zpět", "Back")}
                </button>
                <button className="btn primary" onClick={() => saveSettings({ onboarded: true })}>
                  {accounts.length ? tr("Hotovo", "Done") : tr("Přeskočit", "Skip")} <Icon name="check" size={16} />
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
