//! Library entries and settings, with the on-disk JSON format of KaKa 1.x
//! (and the one-time import rules for LaLa data).

use std::fs::File;
use std::io::Read;

use serde::Serialize;
use serde_json::Value;

/// A user-added trainer. KaKa never downloads trainers: every entry points at a local `.exe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerEntry {
    pub id: String,
    pub name: String,
    pub trainer_path: String,
    /// Empty when the entry has no cover image.
    pub cover_image_path: String,
    /// Optional Steam AppID of the target game. Picks the right game when several
    /// Proton games are running; the game itself must already run.
    pub steam_app_id: Option<u64>,
    /// Proton prefix (`.../compatdata/<id>`) of the last successful launch, so
    /// trainer data can be reset there while the game is not running. Empty if
    /// the trainer was never launched.
    pub last_prefix: String,
}

/// Field order and names of `library.json` entries (KaKa 1.x compatible).
#[derive(Serialize)]
struct EntryJson<'a> {
    id: &'a str,
    name: &'a str,
    trainer_path: &'a str,
    cover_image_path: &'a str,
    steam_app_id: Option<u64>,
    last_prefix: &'a str,
}

impl TrainerEntry {
    pub fn new(id: impl Into<String>, name: impl Into<String>, trainer_path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            trainer_path: trainer_path.into(),
            cover_image_path: String::new(),
            steam_app_id: None,
            last_prefix: String::new(),
        }
    }

    /// 16 random hex characters from the kernel's CSPRNG.
    pub fn new_id() -> String {
        let mut bytes = [0u8; 8];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .expect("/dev/urandom must be readable on Linux");
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Returns `None` for entries that are unusable offline, such as LaLa
    /// online-catalogue games (they have no local `trainer_path`).
    pub fn from_json(json: &Value) -> Option<Self> {
        let obj = json.as_object()?;
        let name = obj.get("name")?.as_str()?;
        let trainer_path = obj.get("trainer_path")?.as_str().filter(|p| !p.is_empty())?;
        let id = match obj.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()) {
            Some(id) => id.to_owned(),
            // Legacy LaLa custom games have a random numeric app_id instead of an id.
            None => format!(
                "lala-{}",
                match obj.get("app_id") {
                    None | Some(Value::Null) => Self::new_id(),
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                }
            ),
        };
        let string = |key: &str| obj.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        Some(Self {
            id,
            name: name.to_owned(),
            trainer_path: trainer_path.to_owned(),
            cover_image_path: string("cover_image_path"),
            steam_app_id: obj.get("steam_app_id").and_then(Value::as_u64).filter(|&n| n > 0),
            last_prefix: string("last_prefix"),
        })
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self.json_view()).expect("entry serializes")
    }

    fn json_view(&self) -> EntryJson<'_> {
        EntryJson {
            id: &self.id,
            name: &self.name,
            trainer_path: &self.trainer_path,
            cover_image_path: &self.cover_image_path,
            steam_app_id: self.steam_app_id,
            last_prefix: &self.last_prefix,
        }
    }
}

/// `library.json` content: a pretty-printed array (2-space indent), as KaKa 1.x wrote it.
pub fn library_to_json(entries: &[TrainerEntry]) -> String {
    let views: Vec<_> = entries.iter().map(TrainerEntry::json_view).collect();
    serde_json::to_string_pretty(&views).expect("library serializes")
}

/// Parses a library; invalid items are skipped. `None` when the document is not a list.
pub fn library_from_json(json: &Value) -> Option<Vec<TrainerEntry>> {
    Some(json.as_array()?.iter().filter_map(TrainerEntry::from_json).collect())
}

/// Settings stored in `config.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppConfig {
    /// Display name from [`crate::language::LANGUAGE_OPTIONS`], if one was saved.
    pub language: Option<String>,
    /// Steam install root chosen in Settings; empty means auto-detect.
    pub steam_path: String,
}

#[derive(Serialize)]
struct ConfigJson<'a> {
    language: &'a str,
    custom_steam_path: &'a str,
}

impl AppConfig {
    /// `None` when the document is not an object.
    pub fn from_json(json: &Value) -> Option<Self> {
        let obj = json.as_object()?;
        Some(Self {
            language: obj.get("language").and_then(Value::as_str).map(str::to_owned),
            steam_path: obj.get("custom_steam_path").and_then(Value::as_str).unwrap_or_default().to_owned(),
        })
    }
}

/// `config.json` content, as KaKa 1.x wrote it.
pub fn config_to_json(language: &str, steam_path: &str) -> String {
    serde_json::to_string_pretty(&ConfigJson {
        language,
        custom_steam_path: steam_path,
    })
    .expect("config serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn survives_a_json_round_trip() {
        let entry = TrainerEntry {
            id: "abc".into(),
            name: "Test Game".into(),
            trainer_path: "/t/trainer.exe".into(),
            cover_image_path: "/t/cover.png".into(),
            steam_app_id: Some(1245620),
            last_prefix: "/c/1245620".into(),
        };
        let text = serde_json::to_string(&entry.to_json()).unwrap();
        let restored = TrainerEntry::from_json(&serde_json::from_str(&text).unwrap()).unwrap();
        assert_eq!(restored, entry);
    }

    #[test]
    fn imports_a_lala_custom_game_and_rejects_lala_online_games() {
        let custom =
            TrainerEntry::from_json(&json!({"name": "Old", "app_id": 1999999999u64, "cover_image_path": "", "trainer_path": "/t/old.exe"})).unwrap();
        assert_eq!(custom.id, "lala-1999999999");
        assert_eq!(custom.steam_app_id, None, "LaLa app_id of custom games was random, not a Steam AppID");
        assert!(TrainerEntry::from_json(&json!({"name": "Online", "app_id": 1245620, "cover_image_url": "https://x", "trainers": []})).is_none());
    }

    #[test]
    fn new_id_is_random_hex() {
        let id = TrainerEntry::new_id();
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_ne!(TrainerEntry::new_id(), TrainerEntry::new_id());
    }

    #[test]
    fn tolerant_parsing_skips_bad_items_and_bad_fields() {
        let lib = library_from_json(&json!([
            {"name": "Ok", "trainer_path": "/t/a.exe", "steam_app_id": 0, "cover_image_path": 5},
            {"name": "No path", "trainer_path": ""},
            "not an object",
            {"name": 7, "trainer_path": "/t/b.exe"},
            {"id": "", "name": "No id", "trainer_path": "/t/c.exe", "steam_app_id": -3, "last_prefix": null}
        ]))
        .unwrap();
        assert_eq!(lib.len(), 2);
        assert_eq!(lib[0].steam_app_id, None, "0 is not a valid AppID");
        assert_eq!(lib[0].cover_image_path, "");
        assert!(lib[1].id.starts_with("lala-"), "missing id gets a generated one");
        assert_eq!(lib[1].steam_app_id, None);
        assert!(library_from_json(&json!({"not": "a list"})).is_none());
    }
}
