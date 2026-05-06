use std::collections::BTreeMap;
use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};

type LocaleMap = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    En,
    Es,
    Fr,
    Hi,
    Zh,
}

impl Language {
    pub fn native_name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Es => "Espanol",
            Self::Fr => "Francais",
            Self::Hi => "हिन्दी",
            Self::Zh => "中文",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Es => "es",
            Self::Fr => "fr",
            Self::Hi => "hi",
            Self::Zh => "zh",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::En, Self::Es, Self::Fr, Self::Hi, Self::Zh]
    }
}

const LOCALE_EN: &str = include_str!("i18n/locales/en.json");
const LOCALE_ES: &str = include_str!("i18n/locales/es.json");
const LOCALE_FR: &str = include_str!("i18n/locales/fr.json");
const LOCALE_HI: &str = include_str!("i18n/locales/hi.json");
const LOCALE_ZH: &str = include_str!("i18n/locales/zh.json");

#[derive(Debug, Clone)]
struct I18nState {
    language: Language,
    current: LocaleMap,
    fallback_en: LocaleMap,
}

impl I18nState {
    fn new(language: Language) -> Self {
        let fallback_en = parse_locale(LOCALE_EN);
        let current = load_locale(language);

        Self {
            language,
            current,
            fallback_en,
        }
    }
}

static CURRENT: OnceLock<RwLock<I18nState>> = OnceLock::new();

fn state_lock() -> &'static RwLock<I18nState> {
    CURRENT.get_or_init(|| RwLock::new(I18nState::new(Language::default())))
}

fn locale_src(language: Language) -> &'static str {
    match language {
        Language::En => LOCALE_EN,
        Language::Es => LOCALE_ES,
        Language::Fr => LOCALE_FR,
        Language::Hi => LOCALE_HI,
        Language::Zh => LOCALE_ZH,
    }
}

fn parse_locale(json: &str) -> LocaleMap {
    serde_json::from_str(json).unwrap_or_default()
}

fn load_locale(language: Language) -> LocaleMap {
    parse_locale(locale_src(language))
}

pub fn init(language: Language) {
    if let Ok(mut guard) = state_lock().write() {
        *guard = I18nState::new(language);
    }
}

pub fn set_language(language: Language) {
    init(language);
}

pub fn current_language() -> Language {
    state_lock()
        .read()
        .map(|guard| guard.language)
        .unwrap_or_default()
}

pub fn tr(key: &str) -> String {
    if let Ok(guard) = state_lock().read()
        && let Some(value) = guard
            .current
            .get(key)
            .or_else(|| guard.fallback_en.get(key))
    {
        return value.clone();
    }

    key.to_owned()
}

pub fn trf(key: &str, vars: &[(&str, &str)]) -> String {
    let mut value = tr(key);
    for (name, replacement) in vars {
        value = value.replace(&format!("{{{name}}}"), replacement);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_locales_parse() {
        for language in Language::all() {
            let locale = load_locale(*language);
            assert!(
                !locale.is_empty(),
                "locale {} should not be empty",
                language.code()
            );
        }
    }

    #[test]
    fn missing_key_falls_back_to_literal_key() {
        assert_eq!(tr("Missing literal key"), "Missing literal key");
    }
}
