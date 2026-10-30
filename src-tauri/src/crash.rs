//! Reads the tail of the game log / newest crash report and explains the crash in Czech.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub title: String,
    pub detail: String,
    pub suspect: Option<String>,
    pub crash_file: Option<String>,
    pub excerpt: Vec<String>,
}

struct Rule {
    re: Regex,
    /// (Czech, English)
    title: (&'static str, &'static str),
    detail: (&'static str, &'static str),
}

static RULES: Lazy<Vec<Rule>> = Lazy::new(|| {
    let r = |p: &str, title: (&'static str, &'static str), detail: (&'static str, &'static str)| Rule { re: Regex::new(p).unwrap(), title, detail };
    vec![
        r(r"OutOfMemoryError",
          ("Došla paměť (RAM)", "Out of memory (RAM)"),
          ("Hře nestačí přidělená RAM. Přidej jí víc v nastavení instance (6–8 GB pro velké modpacky), nebo ubírej shadery / Distant Horizons.",
           "The game doesn't have enough RAM. Give it more in the instance settings (6–8 GB for big modpacks), or turn down shaders / Distant Horizons.")),
        r(r"UnsupportedClassVersionError|has been compiled by a more recent version of the Java Runtime",
          ("Špatná verze Javy", "Wrong Java version"),
          ("Některý mod potřebuje novější Javu. V nastavení instance smaž vlastní cestu k Javě — launcher stáhne správnou sám.",
           "A mod needs a newer Java. Clear the custom Java path in the instance settings — the launcher downloads the right one itself.")),
        r(r"(?i)Incompatible mods? found|Mod resolution failed|requires (any version|version) .* of",
          ("Nekompatibilní mody", "Incompatible mods"),
          ("Některý mod chce jinou verzi jiného modu nebo Minecraftu. Podívej se na zvýrazněný řádek — napíše, co chybí nebo co je špatně.",
           "A mod wants a different version of another mod or of Minecraft. Check the highlighted line — it says what's missing or wrong.")),
        r(r"(?i)duplicate mod|Found duplicate mods|DuplicateModsFoundException",
          ("Stejný mod je tam dvakrát", "The same mod is installed twice"),
          ("V seznamu modů je jeden mod ve dvou verzích. Starší smaž.", "One mod is in the list in two versions. Delete the older one.")),
        r(r"(?i)Could not find required mod|requires mod|Missing or unsupported mandatory dependencies",
          ("Chybí závislost", "Missing dependency"),
          ("Některý mod potřebuje další mod. Najdi ho v prohlížeči modů (typicky Fabric API, Architectury, Cloth Config).",
           "A mod needs another mod. Find it in the mod browser (usually Fabric API, Architectury, Cloth Config).")),
        r(r"(?i)Mixin apply .* failed|MixinApplyError|InvalidInjectionException|Mixin transformation of .* failed",
          ("Konflikt modů (Mixin)", "Mod conflict (Mixin)"),
          ("Dva mody se perou o stejnou část hry. Podezřelý mod je uvedený níže — vypni ho a zkus to znovu.",
           "Two mods are fighting over the same part of the game. The suspected mod is listed below — disable it and try again.")),
        r(r"(?i)Pixel format not accelerated|GLFW error 65542|WGL: The driver does not appear to support OpenGL",
          ("Problém s grafickým ovladačem", "Graphics driver problem"),
          ("Aktualizuj ovladač grafiky (NVIDIA/AMD/Intel). Na notebooku přepni Javu na výkonnou grafiku.",
           "Update your graphics driver (NVIDIA/AMD/Intel). On a laptop, set Java to use the dedicated GPU.")),
        r(r"(?i)EXCEPTION_ACCESS_VIOLATION.*(atio6axx|nvoglv|ig\w+icd)",
          ("Pád grafického ovladače", "Graphics driver crash"),
          ("Aktualizuj ovladač grafiky, vypni shadery a zkus to znovu.", "Update your graphics driver, turn off shaders and try again.")),
        r(r"(?i)EXCEPTION_ACCESS_VIOLATION",
          ("Nativní pád hry", "Native game crash"),
          ("Často ho způsobí shadery, Distant Horizons nebo overlaye (Discord, MSI Afterburner). Zkus je vypnout.",
           "Often caused by shaders, Distant Horizons or overlays (Discord, MSI Afterburner). Try turning them off.")),
        r(r"(?i)Failed to (create|initialize) (the )?(Sodium|Iris)",
          ("Sodium / Iris se nespustil", "Sodium / Iris failed to start"),
          ("Aktualizuj ovladač grafiky nebo vypni Iris / Sodium Extra.", "Update your graphics driver or disable Iris / Sodium Extra.")),
        r(r"(?i)Failed to login: Invalid session|Invalid session \(Try restarting your game",
          ("Neplatné přihlášení", "Invalid session"),
          ("Odhlas a znovu přihlas svůj Microsoft účet v launcheru.", "Sign out and back in to your Microsoft account in the launcher.")),
    ]
});
static SUSPECT: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"Suspected Mods?: ([^\n,]+)").unwrap(),
        Regex::new(r"Mod File: .*[/\\]([^/\\]+\.jar)").unwrap(),
        Regex::new(r"Failure message: (.+?) \(").unwrap(),
        Regex::new(r"from mod ([a-z0-9_\-]+)").unwrap(),
        Regex::new(r"mixins?\.json:[^ ]* from mod ([a-z0-9_\-]+)").unwrap(),
        Regex::new(r"Mod '([^']+)' \(").unwrap(),
    ]
});

fn newest_crash_report(game_dir: &Path) -> Option<(String, String)> {
    let dir = game_dir.join("crash-reports");
    let newest = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().map(|x| x == "txt").unwrap_or(false))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())?;
    let age = newest.metadata().ok()?.modified().ok()?.elapsed().ok()?;
    if age.as_secs() > 600 {
        return None;
    }
    let text = std::fs::read_to_string(newest.path()).ok()?;
    Some((newest.path().to_string_lossy().into_owned(), text))
}

pub fn analyze(log: &[String], game_dir: &Path) -> CrashReport {
    let crash = newest_crash_report(game_dir);
    let mut text = log.join("\n");
    if let Some((_, c)) = &crash {
        text.push('\n');
        text.push_str(c);
    }
    let (title, detail) = RULES
        .iter()
        .find(|r| r.re.is_match(&text))
        .map(|r| {
            let en = crate::i18n::is_en();
            let pick = |p: (&str, &str)| if en { p.1.to_string() } else { p.0.to_string() };
            (pick(r.title), pick(r.detail))
        })
        .unwrap_or_else(|| {
            (
                l!("Hra spadla", "The game crashed"),
                l!(
                    "Nepoznal jsem přesnou příčinu. Zkus vypnout naposledy přidané mody, nebo pošli log (tlačítko Kopírovat log).",
                    "I couldn't tell the exact cause. Try disabling the most recently added mods, or share the log (Copy log button)."
                ),
            )
        });
    let suspect = SUSPECT.iter().find_map(|re| re.captures(&text).map(|c| c[1].trim().to_string()));
    let interesting = Regex::new(r"(?i)(error|exception|caused by|failed|requires|missing|incompatible)").unwrap();
    let excerpt: Vec<String> = text
        .lines()
        .filter(|l| interesting.is_match(l))
        .rev()
        .take(12)
        .map(|l| l.chars().take(300).collect())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    CrashReport { title, detail, suspect, crash_file: crash.map(|c| c.0), excerpt }
}
