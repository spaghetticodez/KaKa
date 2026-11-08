//! UI strings: Fluent files in `i18n/<locale>/kaka.ftl`, embedded in the binary.
//! `fl!` checks message ids and arguments against the English file at compile time.

use std::sync::LazyLock;

use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::unic_langid::LanguageIdentifier;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader = fluent_language_loader!();
    load(&loader, "en".parse().expect("valid id"));
    loader
});

/// Loads `id` (English fills gaps). The isolation setting is per loaded bundle,
/// so it must be applied after every load: no Unicode isolation marks around
/// arguments, they would end up in copied paths and log text.
fn load(loader: &FluentLanguageLoader, id: LanguageIdentifier) {
    if let Err(e) = i18n_embed::select(loader, &Localizations, &[id]) {
        eprintln!("Could not load the UI strings: {e}");
    }
    loader.set_use_isolating(false);
}

/// Translated text: `tr!("key")` or `tr!("key", name = value)`.
#[macro_export]
macro_rules! tr {
    ($($args:tt)*) => {
        i18n_embed_fl::fl!($crate::i18n::LOADER, $($args)*)
    };
}

/// Switches to `locale` (`en`, `de`, `zh`); English fills missing messages.
pub fn set_locale(locale: &str) {
    let id: LanguageIdentifier = locale
        .parse()
        .unwrap_or_else(|_| "en".parse().expect("valid id"));
    load(&LOADER, id);
}

/// The locale is global: tests that switch it run one at a time.
#[cfg(test)]
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn keys(locale: &str) -> BTreeSet<String> {
        let file = Localizations::get(&format!("{locale}/kaka.ftl")).expect("embedded");
        std::str::from_utf8(&file.data)
            .unwrap()
            .lines()
            .filter_map(|l| {
                l.split_once(" =")
                    .map(|(k, _)| k)
                    .filter(|k| !k.starts_with([' ', '#']) && !k.is_empty())
            })
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn every_language_has_exactly_the_english_keys() {
        let en = keys("en");
        assert_eq!(en.len(), 56);
        for locale in ["de", "zh"] {
            assert_eq!(keys(locale), en, "{locale}");
        }
    }

    #[test]
    fn switching_languages_formats_arguments_without_isolation_marks() {
        let _guard = test_lock();
        set_locale("de");
        let de = crate::tr!("launched", name = "/a b.exe");
        set_locale("zh");
        let zh = crate::tr!("launched", name = "/a b.exe");
        set_locale("en");
        assert_eq!(
            crate::tr!("launched", name = "/a b.exe"),
            "Trainer started: /a b.exe"
        );
        for text in [&de, &zh] {
            assert!(
                text.contains("/a b.exe") && !text.contains('\u{2068}'),
                "{text}"
            );
        }
        assert_ne!(de, zh);
    }
}
