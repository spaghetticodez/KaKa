//! JSON persistence in `$XDG_DATA_HOME/io.github.spaghetticodez.kaka/`.
//!
//! Data lives in the data dir (not the cache dir, which cleaners wipe), writes
//! are atomic, corrupt files are moved aside instead of being overwritten, and a
//! LaLa library/config is imported once on first start.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::model::{AppConfig, TrainerEntry, config_to_json, library_from_json, library_to_json};

pub const APP_ID: &str = "io.github.spaghetticodez.kaka";
pub const LIBRARY_FILE_NAME: &str = "library.json";
pub const CONFIG_FILE_NAME: &str = "config.json";

/// `$<var>` when set and non-empty, else `$HOME/<fallback>` (XDG base directory rules).
pub fn xdg_dir(var: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(var) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home_dir().join(fallback),
    }
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub struct LocalStorage {
    data_dir: PathBuf,
    legacy_dir: PathBuf,
    write_lock: Mutex<()>,
    /// Writes are ignored until the corresponding file was read, so a failed or
    /// pending load can never overwrite saved data with an empty state.
    library_loaded: AtomicBool,
    config_loaded: AtomicBool,
}

impl LocalStorage {
    pub fn new(data_dir: impl Into<PathBuf>, legacy_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            legacy_dir: legacy_dir.into(),
            write_lock: Mutex::new(()),
            library_loaded: AtomicBool::new(false),
            config_loaded: AtomicBool::new(false),
        }
    }

    /// The user's real locations: the KaKa data dir, and LaLa's old cache dir for the import.
    pub fn from_env() -> Self {
        Self::new(
            xdg_dir("XDG_DATA_HOME", ".local/share").join(APP_ID),
            xdg_dir("XDG_CACHE_HOME", ".cache")
                .join("com.aironheart.lala")
                .join("config"),
        )
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    fn dir(&self) -> io::Result<&Path> {
        fs::create_dir_all(&self.data_dir)?;
        Ok(&self.data_dir)
    }

    fn own_file(&self, name: &str) -> PathBuf {
        self.data_dir.join(name)
    }

    pub fn read_library(&self) -> Vec<TrainerEntry> {
        let _ = self.dir();
        let file = self.own_file(LIBRARY_FILE_NAME);
        if file.exists() {
            let entries = read_json(&file, library_from_json, true).unwrap_or_default();
            self.library_loaded.store(true, Ordering::SeqCst);
            return entries;
        }
        let legacy = self.legacy_dir.join(LIBRARY_FILE_NAME);
        let imported = if legacy.exists() {
            read_json(&legacy, library_from_json, false).unwrap_or_default()
        } else {
            Vec::new()
        };
        self.library_loaded.store(true, Ordering::SeqCst);
        if !imported.is_empty()
            && let Err(e) = self.write_library(&imported)
        {
            eprintln!("Could not save the imported LaLa library: {e}");
        }
        imported
    }

    pub fn write_library(&self, entries: &[TrainerEntry]) -> io::Result<()> {
        if !self.library_loaded.load(Ordering::SeqCst) {
            return Ok(());
        }
        let file = self.dir()?.join(LIBRARY_FILE_NAME);
        self.atomic_write(&file, &library_to_json(entries))
    }

    pub fn read_config(&self) -> AppConfig {
        let _ = self.dir();
        let own = self.own_file(CONFIG_FILE_NAME);
        let own_exists = own.exists();
        let source = if own_exists {
            own
        } else {
            self.legacy_dir.join(CONFIG_FILE_NAME)
        };
        let config = if source.exists() {
            read_json(&source, AppConfig::from_json, own_exists)
        } else {
            None
        };
        self.config_loaded.store(true, Ordering::SeqCst);
        config.unwrap_or_default()
    }

    pub fn write_config(&self, language: &str, steam_path: &str) -> io::Result<()> {
        if !self.config_loaded.load(Ordering::SeqCst) {
            return Ok(());
        }
        let file = self.dir()?.join(CONFIG_FILE_NAME);
        self.atomic_write(&file, &config_to_json(language, steam_path))
    }

    fn atomic_write(&self, file: &Path, content: &str) -> io::Result<()> {
        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut tmp_name = file.as_os_str().to_owned();
        tmp_name.push(".tmp");
        let tmp = PathBuf::from(tmp_name);
        let mut f = fs::File::create(&tmp)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, file)
    }
}

/// Parses `file`; on unreadable or corrupt content the file is renamed aside
/// (when `quarantine`) so the next save does not destroy it.
fn read_json<T>(file: &Path, parse: impl Fn(&Value) -> Option<T>, quarantine: bool) -> Option<T> {
    let parsed = fs::read_to_string(file)
        .map_err(|e| e.to_string())
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string()))
        .and_then(|json| parse(&json).ok_or_else(|| "unexpected JSON structure".to_owned()));
    match parsed {
        Ok(value) => Some(value),
        Err(e) => {
            eprintln!("Could not read {}: {e}", file.display());
            if quarantine {
                let millis = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or_default();
                let mut aside = file.as_os_str().to_owned();
                aside.push(format!(".corrupt-{millis}"));
                // If this fails, the file stays in place and the next save replaces it.
                let _ = fs::rename(file, aside);
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    struct Dirs {
        _tmp: TempDir,
        data: PathBuf,
        legacy: PathBuf,
    }

    fn dirs() -> Dirs {
        let tmp = tempfile::Builder::new()
            .prefix("kaka_storage_")
            .tempdir()
            .unwrap();
        let data = tmp.path().join("data").join(APP_ID);
        let legacy = tmp
            .path()
            .join("cache")
            .join("com.aironheart.lala")
            .join("config");
        fs::create_dir_all(&legacy).unwrap();
        Dirs {
            _tmp: tmp,
            data,
            legacy,
        }
    }

    fn storage(d: &Dirs) -> LocalStorage {
        LocalStorage::new(&d.data, &d.legacy)
    }

    fn names(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn first_start_imports_lala_custom_trainers_and_config_once() {
        let d = dirs();
        fs::write(
            d.legacy.join("library.json"),
            json!([
                {"name": "Online", "app_id": 1, "cover_image_url": "https://x", "trainers": []},
                {"name": "Mine", "app_id": 2000000000u64, "cover_image_path": "", "trainer_path": "/t/mine.exe"}
            ])
            .to_string(),
        )
        .unwrap();
        fs::write(
            d.legacy.join("config.json"),
            json!({"language": "German", "custom_steam_path": "/s"}).to_string(),
        )
        .unwrap();

        let s = storage(&d);
        let entries = s.read_library();
        assert_eq!(
            entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["Mine"]
        );
        assert!(d.data.join("library.json").exists(), "import is persisted");

        let config = s.read_config();
        assert_eq!(config.language.as_deref(), Some("German"));
        assert_eq!(config.steam_path, "/s");
        assert!(
            d.legacy.join("library.json").exists(),
            "the legacy files are left untouched"
        );
    }

    #[test]
    fn writes_are_ignored_before_load_and_are_atomic_after() {
        let d = dirs();
        let s = storage(&d);
        let entry = TrainerEntry::new("x", "A", "/t/a.exe");
        s.write_library(std::slice::from_ref(&entry)).unwrap();
        assert!(!d.data.join("library.json").exists());

        s.read_library();
        s.write_library(&[entry]).unwrap();
        assert_eq!(storage(&d).read_library()[0].name, "A");
        assert!(!names(&d.data).contains(&"library.json.tmp".to_owned()));
    }

    #[test]
    fn a_corrupt_library_is_moved_aside_instead_of_being_overwritten() {
        let d = dirs();
        fs::create_dir_all(&d.data).unwrap();
        fs::write(d.data.join("library.json"), "{broken").unwrap();
        let s = storage(&d);
        assert!(s.read_library().is_empty());
        s.write_library(&[TrainerEntry::new("n", "New", "/t/n.exe")])
            .unwrap();
        let names = names(&d.data);
        assert_eq!(
            names
                .iter()
                .filter(|n| n.starts_with("library.json.corrupt-"))
                .count(),
            1
        );
        assert!(names.contains(&"library.json".to_owned()));
    }

    #[test]
    fn a_library_that_is_not_a_list_is_quarantined_too() {
        let d = dirs();
        fs::create_dir_all(&d.data).unwrap();
        fs::write(d.data.join("library.json"), "{\"a\": 1}").unwrap();
        assert!(storage(&d).read_library().is_empty());
        assert!(
            names(&d.data)
                .iter()
                .any(|n| n.starts_with("library.json.corrupt-"))
        );
    }

    #[test]
    fn config_round_trip() {
        let d = dirs();
        let s = storage(&d);
        s.read_config();
        s.write_config("中文", "/steam").unwrap();
        let config = storage(&d).read_config();
        assert_eq!(config.language.as_deref(), Some("中文"));
        assert_eq!(config.steam_path, "/steam");
    }

    /// Files written by the Flutter build (KaKa 1.x) load unchanged and re-save byte-identical.
    #[test]
    fn flutter_written_files_round_trip_byte_identical() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/flutter");
        let d = dirs();
        fs::create_dir_all(&d.data).unwrap();
        for name in [LIBRARY_FILE_NAME, CONFIG_FILE_NAME] {
            fs::copy(fixtures.join(name), d.data.join(name)).unwrap();
        }
        let s = storage(&d);
        let library = s.read_library();
        let config = s.read_config();
        assert_eq!(library.len(), 3);
        assert_eq!(library[0].name, "ELDEN RING \"Deluxe\" ® – 艾尔登法环");
        assert_eq!(library[0].steam_app_id, Some(1245620));
        assert_eq!(library[1].steam_app_id, None);
        assert_eq!(
            library[2].steam_app_id,
            Some(3123456789),
            "non-Steam shortcut ids exceed i32"
        );
        assert_eq!(config.language.as_deref(), Some("中文"));

        s.write_library(&library).unwrap();
        s.write_config(config.language.as_deref().unwrap(), &config.steam_path)
            .unwrap();
        for name in [LIBRARY_FILE_NAME, CONFIG_FILE_NAME] {
            assert_eq!(
                fs::read(d.data.join(name)).unwrap(),
                fs::read(fixtures.join(name)).unwrap(),
                "{name} differs"
            );
        }
    }
}
