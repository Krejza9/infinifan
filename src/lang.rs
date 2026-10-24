//! UI language. Translations are plain files in the `locales` folder next to the exe
//! (`<code>.yml`, flat `key: "text"` lines, `%{name}` placeholders); nothing is built into the exe.
//! `en.yml` is the base language and must be present; other languages fall back to it key by key.
//! Adding a language = adding a file; it shows up in the Settings switcher after a restart.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

const BASE: &str = "en";

static LANGS: OnceLock<HashMap<String, HashMap<String, String>>> = OnceLock::new();
static CURRENT: RwLock<String> = RwLock::new(String::new());

/// Translated text: `tr!("key")` or `tr!("key", name = value, …)` for `%{name}` placeholders.
#[macro_export]
macro_rules! tr {
    ($key:expr) => {
        $crate::lang::translate($key, &[])
    };
    ($key:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::lang::translate($key, &[$((stringify!($name), $value.to_string())),+])
    };
}

pub fn locales_dir() -> PathBuf {
    crate::paths::exe_dir().join("locales")
}

/// Loads all `locales\*.yml`. Fails when the base language file is missing or empty.
pub fn load() -> Result<(), String> {
    let dir = locales_dir();
    let mut langs = HashMap::new();
    for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "yml")
            && let (Some(code), Ok(text)) = (path.file_stem().and_then(|s| s.to_str()), std::fs::read_to_string(&path))
        {
            langs.insert(code.to_string(), parse(&text));
        }
    }
    if langs.get(BASE).is_none_or(|m| m.is_empty()) {
        return Err(format!(
            "The base language file is missing:\n{}\n\nInfiniFan cannot start without it. Download the release again \
             or put en.yml back into the locales folder.",
            dir.join(format!("{BASE}.yml")).display()
        ));
    }
    let _ = LANGS.set(langs);
    set(BASE);
    Ok(())
}

/// Flat `key: value` lines; values may be "quoted" (JSON escapes) or plain.
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

fn langs() -> &'static HashMap<String, HashMap<String, String>> {
    static EMPTY: OnceLock<HashMap<String, HashMap<String, String>>> = OnceLock::new();
    LANGS.get().unwrap_or_else(|| EMPTY.get_or_init(HashMap::new))
}

/// Text for `key` in the current language, then English, then the key itself.
pub fn translate(key: &str, args: &[(&str, String)]) -> String {
    let current = current();
    let text = [current.as_str(), BASE]
        .iter()
        .find_map(|l| langs().get(*l).and_then(|m| m.get(key)))
        .cloned()
        .unwrap_or_else(|| key.to_string());
    args.iter().fold(text, |t, (name, value)| t.replace(&format!("%{{{name}}}"), value))
}

/// Available languages (file names in `locales`), sorted.
pub fn languages() -> Vec<String> {
    let mut l: Vec<String> = langs().keys().cloned().collect();
    l.sort();
    l
}

/// Name of a language written in that language (`lang.name` of its file).
pub fn name_of(code: &str) -> String {
    langs().get(code).and_then(|m| m.get("lang.name")).cloned().unwrap_or_else(|| code.to_string())
}

pub fn current() -> String {
    CURRENT.read().map(|c| c.clone()).unwrap_or_default()
}

pub fn set(code: &str) {
    if let Ok(mut c) = CURRENT.write() {
        *c = code.to_string();
    }
}

/// Language from the config, otherwise English.
pub fn apply(configured: Option<&str>) {
    let code = configured.filter(|c| langs().contains_key(*c)).unwrap_or(BASE);
    set(code);
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    fn keys(path: &Path) -> Vec<String> {
        let mut k: Vec<String> = super::parse(&std::fs::read_to_string(path).unwrap()).into_keys().collect();
        k.sort();
        k
    }

    /// Every language must have exactly the keys of English (no missing or stale texts).
    #[test]
    fn locales_have_same_keys() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
        let en = keys(&dir.join("en.yml"));
        assert!(!en.is_empty());
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yml") {
                assert_eq!(keys(&path), en, "{} differs from en.yml", path.display());
            }
        }
    }

    #[test]
    fn placeholders_are_filled() {
        assert_eq!(super::translate("no.such.key", &[]), "no.such.key");
        let t = "Group %{n}".to_string();
        assert_eq!([("n", "2".to_string())].iter().fold(t, |t, (n, v)| t.replace(&format!("%{{{n}}}"), v)), "Group 2");
    }
}
