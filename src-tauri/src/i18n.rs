//! Messages the backend shows to the player are written in both languages side by side:
//! `l!("Stahuji Javu {major}", "Downloading Java {major}")`.

pub fn is_en() -> bool {
    crate::settings::get().language == "en"
}

/// Czech or English static text by the UI language.
pub fn tr(cs: &'static str, en: &'static str) -> &'static str {
    if is_en() { en } else { cs }
}

/// Pick the Czech or English format string by the UI language; extra arguments go to `format!`.
macro_rules! l {
    ($cs:literal, $en:literal $(,)?) => {
        if $crate::i18n::is_en() { format!($en) } else { format!($cs) }
    };
    ($cs:literal, $en:literal, $($arg:tt)+) => {
        if $crate::i18n::is_en() { format!($en, $($arg)+) } else { format!($cs, $($arg)+) }
    };
}
