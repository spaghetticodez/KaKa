//! What KaKa can fill in for a running game from local data only: the game's
//! Steam manifest and Steam's own artwork cache. Nothing is downloaded.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::proton::{RunningGame, basename, dirname};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSuggestion {
    pub name: String,
    pub app_id: Option<u64>,
    /// Local cover image from Steam's library cache, if one exists.
    pub cover_path: Option<String>,
}

/// The app name from an `appmanifest_<appid>.acf` (Valve KeyValues text): the
/// first `"name"` key followed by a quoted value, with `\"` and `\\` unescaped.
pub fn app_name_from_manifest(acf: &str) -> Option<String> {
    let mut rest = acf;
    while let Some(pos) = rest.find("\"name\"") {
        rest = &rest[pos + "\"name\"".len()..];
        let value = rest.trim_start_matches(char::is_whitespace);
        if value.len() == rest.len() || !value.starts_with('"') {
            continue; // needs whitespace, then a quoted value
        }
        let mut name = String::new();
        let mut chars = value[1..].chars();
        let mut closed = false;
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some(escaped) => name.push(escaped),
                    None => break,
                },
                '"' => {
                    closed = true;
                    break;
                }
                c => name.push(c),
            }
        }
        if closed {
            let name = name.trim();
            return (!name.is_empty()).then(|| name.to_owned());
        }
    }
    None
}

/// Portrait cover for `app_id` in `<steam_root>/appcache/librarycache`.
///
/// Steam keeps artwork either directly in `librarycache/<appid>/` or in hashed
/// sub-folders (`librarycache/<appid>/<hash>/`); the newest copy wins. The
/// 600x900 capsule matches KaKa's 2:3 cards; the landscape header is a fallback.
pub fn find_cover(steam_root: &str, app_id: u64) -> Option<String> {
    let dir = Path::new(steam_root)
        .join("appcache/librarycache")
        .join(app_id.to_string());
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(&dir).ok()?.filter_map(Result::ok) {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_file() {
            files.push(entry.path());
        } else if kind.is_dir() {
            let Ok(sub) = fs::read_dir(entry.path()) else {
                continue;
            };
            for inner in sub.filter_map(Result::ok) {
                if inner.file_type().is_ok_and(|t| t.is_file()) {
                    files.push(inner.path());
                }
            }
        }
    }
    let modified = |p: &PathBuf| {
        fs::metadata(p)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    };
    for name in [
        "library_600x900_2x.jpg",
        "library_600x900.jpg",
        "library_header.jpg",
        "header.jpg",
    ] {
        let newest = files
            .iter()
            .filter(|f| f.file_name().is_some_and(|n| n == name))
            .max_by_key(|f| modified(f));
        if let Some(path) = newest {
            return Some(path.to_string_lossy().into_owned());
        }
    }
    None
}

/// File name without its last extension, like Dart's `basenameWithoutExtension`.
fn stem(label: &str) -> &str {
    let base = basename(label);
    match base.rfind('.') {
        Some(i) if i > 0 => &base[..i],
        _ => base,
    }
}

/// Name, AppID and cover for `game`. The manifest is looked up next to the
/// game's own prefix (`<library>/steamapps/compatdata/<id>` →
/// `<library>/steamapps/appmanifest_<id>.acf`), so games on any drive work;
/// games without one (non-Steam shortcuts, Heroic, ...) use the exe name.
pub fn suggestion_for(game: &RunningGame, steam_root: Option<&str>) -> GameSuggestion {
    let name = game.app_id.and_then(|id| {
        let steamapps = dirname(&dirname(&game.compat_data_path));
        let manifest = Path::new(&steamapps).join(format!("appmanifest_{id}.acf"));
        app_name_from_manifest(&fs::read_to_string(manifest).ok()?)
    });
    let root = game
        .client_install_path
        .as_deref()
        .or(steam_root)
        .filter(|r| !r.is_empty());
    GameSuggestion {
        name: name.unwrap_or_else(|| stem(&game.label).to_owned()),
        app_id: game.app_id,
        cover_path: match (game.app_id, root) {
            (Some(id), Some(root)) => find_cover(root, id),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::time::Duration;

    const MANIFEST: &str = "\"AppState\"\n{\n\t\"appid\"\t\t\"1245620\"\n\t\"universe\"\t\t\"1\"\n\
        \t\"LauncherPath\"\t\t\"/home/u/.local/share/Steam/ubuntu12_32/steam\"\n\
        \t\"name\"\t\t\"ELDEN RING \\\"Deluxe\\\" ®\"\n\t\"StateFlags\"\t\t\"4\"\n\
        \t\"UserConfig\"\n\t{\n\t\t\"name\"\t\t\"not this one\"\n\t}\n}\n";

    fn touch(path: &Path, year: Option<i32>) -> String {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x").unwrap();
        if let Some(year) = year {
            // Seconds since the epoch at the start of `year` (close enough for ordering).
            let secs = ((year - 1970) as u64) * 365 * 24 * 3600;
            File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
                .unwrap();
        }
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn app_name_from_manifest_reads_the_top_level_name_with_escapes_and_non_ascii() {
        assert_eq!(
            app_name_from_manifest(MANIFEST).as_deref(),
            Some("ELDEN RING \"Deluxe\" ®")
        );
        assert_eq!(
            app_name_from_manifest("\"AppState\" { \"appid\" \"1\" }"),
            None
        );
        assert_eq!(app_name_from_manifest("\"name\" \"\""), None);
    }

    #[test]
    fn find_cover_flat_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Steam");
        let cache = root.join("appcache/librarycache/1245620");
        touch(&cache.join("header.jpg"), None);
        let capsule = touch(&cache.join("library_600x900.jpg"), None);
        assert_eq!(find_cover(root.to_str().unwrap(), 1245620), Some(capsule));
    }

    #[test]
    fn find_cover_hashed_sub_folders_the_newest_portrait_capsule_wins_over_headers() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Steam");
        let cache = root.join("appcache/librarycache/1245620");
        touch(&cache.join("aaa/library_600x900.jpg"), Some(2024));
        let newest = touch(&cache.join("bbb/library_600x900.jpg"), Some(2026));
        touch(&cache.join("ccc/library_header.jpg"), Some(2027));
        assert_eq!(find_cover(root.to_str().unwrap(), 1245620), Some(newest));
    }

    #[test]
    fn find_cover_falls_back_to_the_header_none_without_any_artwork() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Steam");
        let header = touch(
            &root.join("appcache/librarycache/1245620/ddd/library_header.jpg"),
            None,
        );
        assert_eq!(find_cover(root.to_str().unwrap(), 1245620), Some(header));
        assert_eq!(find_cover(root.to_str().unwrap(), 1), None);
    }

    #[test]
    fn steam_game_on_a_second_drive_name_from_that_library_cover_from_the_steam_root() {
        let tmp = tempfile::tempdir().unwrap();
        let library = tmp.path().join("mnt/games/SteamLibrary/steamapps");
        fs::create_dir_all(&library).unwrap();
        fs::write(library.join("appmanifest_1245620.acf"), MANIFEST).unwrap();
        let root = tmp.path().join("Steam");
        let cover = touch(
            &root.join("appcache/librarycache/1245620/library_600x900.jpg"),
            None,
        );
        let game = RunningGame {
            compat_data_path: library
                .join("compatdata/1245620")
                .to_string_lossy()
                .into_owned(),
            client_install_path: Some(root.to_string_lossy().into_owned()),
            app_id: Some(1245620),
            proton_script: None,
            label: "eldenring.exe".into(),
        };

        let s = suggestion_for(&game, Some("/elsewhere"));
        assert_eq!(s.name, "ELDEN RING \"Deluxe\" ®");
        assert_eq!(s.app_id, Some(1245620));
        assert_eq!(
            s.cover_path,
            Some(cover),
            "the game's own Steam root wins over the configured one"
        );
    }

    #[test]
    fn no_manifest_exe_name_without_extension_no_cover_without_appid() {
        let tmp = tempfile::tempdir().unwrap();
        let heroic = RunningGame {
            compat_data_path: tmp
                .path()
                .join("Heroic/Prefixes/Cyberpunk")
                .to_string_lossy()
                .into_owned(),
            client_install_path: None,
            app_id: None,
            proton_script: None,
            label: "Cyberpunk2077.exe".into(),
        };
        let s = suggestion_for(&heroic, Some(tmp.path().join("Steam").to_str().unwrap()));
        assert_eq!(s.name, "Cyberpunk2077");
        assert_eq!(s.app_id, None);
        assert_eq!(s.cover_path, None);
    }
}
