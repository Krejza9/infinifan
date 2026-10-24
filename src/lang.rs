//! UI language: translations live in `locales/<code>.yml` (rust-i18n) and are compiled into the exe.
//! Files in a `locales` folder next to the exe are loaded at startup and take precedence, so a
//! language can be added or fixed without rebuilding; it shows up in the Settings switcher.

use std::borrow::Cow;
use std::collections::HashMap;

/// Translations from `<exe dir>\locales\*.yml`, consulted before the embedded ones.
pub struct FolderBackend {
    langs: HashMap<String, HashMap<String, String>>,
}

impl FolderBackend {
    pub fn load() -> Self {
        let dir = crate::paths::exe_dir().join("locales");
        let mut langs = HashMap::new();
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yml")
                && let (Some(code), Ok(text)) = (path.file_stem().and_then(|s| s.to_str()), std::fs::read_to_string(&path))
            {
                langs.insert(code.to_string(), parse(&text));
            }
        }
        FolderBackend { langs }
    }
}

/// Flat `key: value` lines as in `locales/*.yml`; values may be "quoted" (JSON escapes) or plain.
fn parse(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| !k.trim().is_empty() && !k.starts_with(' ') && k.trim() != "_version")
        .map(|(k, v)| {
            let v = v.trim();
            let v = if v.starts_with('"') {
                serde_json::from_str(v).unwrap_or_else(|_| v.trim_matches('"').to_string())
            } else {
                v.trim_matches('\'').to_string()
            };
            (k.trim().to_string(), v)
        })
        .collect()
}

impl rust_i18n::Backend for FolderBackend {
    fn available_locales(&self) -> Vec<Cow<'_, str>> {
        self.langs.keys().map(|k| Cow::from(k.as_str())).collect()
    }

    fn translate(&self, locale: &str, key: &str) -> Option<Cow<'_, str>> {
        self.langs.get(locale)?.get(key).map(|v| Cow::from(v.as_str()))
    }

    fn messages_for_locale(&self, locale: &str) -> Option<Vec<(Cow<'_, str>, Cow<'_, str>)>> {
        let m = self.langs.get(locale)?;
        Some(m.iter().map(|(k, v)| (Cow::from(k.as_str()), Cow::from(v.as_str()))).collect())
    }
}

/// Translated text as `String` (all of `t!`'s forms: `tr!("key")`, `tr!("key", name = value)`).
#[macro_export]
macro_rules! tr {
    ($($t:tt)*) => {
        rust_i18n::t!($($t)*).into_owned()
    };
}

/// Available languages (file names in `locales/`).
pub fn languages() -> Vec<String> {
    let mut l: Vec<String> = rust_i18n::available_locales!().into_iter().map(|s| s.to_string()).collect();
    l.sort();
    l.dedup();
    l
}

pub fn current() -> String {
    rust_i18n::locale().to_string()
}

pub fn set(code: &str) {
    rust_i18n::set_locale(code);
}

/// Language from the config, otherwise English.
pub fn apply(configured: Option<&str>) {
    let code = configured.filter(|c| languages().iter().any(|l| l == c)).unwrap_or("en");
    set(code);
}

#[cfg(test)]
mod tests {
    /// Every language must have exactly the keys of English (no missing or stale texts).
    #[test]
    fn locales_have_same_keys() {
        let keys = |code: &str| -> Vec<String> {
            let path = format!("{}/locales/{code}.yml", env!("CARGO_MANIFEST_DIR"));
            let mut k: Vec<String> =
                std::fs::read_to_string(path).unwrap().lines().filter_map(|l| l.split_once(':').map(|(k, _)| k.trim().to_string())).collect();
            k.sort();
            k
        };
        let en = keys("en");
        for code in super::languages() {
            assert_eq!(keys(&code), en, "locales/{code}.yml differs from en.yml");
        }
    }
}
