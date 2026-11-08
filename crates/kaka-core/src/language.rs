//! UI languages. `config.json` stores the display name (KaKa 1.x format).

/// Display names, in the order the Settings list shows them.
pub const LANGUAGE_OPTIONS: [&str; 3] = ["English", "中文", "German"];

/// Locale code (`en`, `zh`, `de`) for a display name; unknown names map to English.
pub fn language_to_locale(language: &str) -> &'static str {
    match language {
        "中文" => "zh",
        "German" => "de",
        _ => "en",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_map_to_locales_unknown_falls_back_to_english() {
        assert_eq!(LANGUAGE_OPTIONS.map(language_to_locale), ["en", "zh", "de"]);
        assert_eq!(language_to_locale("Klingon"), "en");
    }
}
