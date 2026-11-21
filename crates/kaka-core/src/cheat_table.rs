//! Read-only parser for Cheat Engine tables (`.CT`, XML). Nothing is ever
//! executed: Auto Assembler and Lua scripts are kept as text and reported.
//! Element names and rules follow Cheat Engine's `MemoryRecordUnit.pas`.

use std::fmt;
use std::fs;
use std::path::Path;

use roxmltree::{Document, Node};

/// Larger files are refused (tables with embedded files are a few MiB).
const MAX_TABLE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct CheatTable {
    pub version: Option<u32>,
    pub entries: Vec<CheatEntry>,
    /// Table-level Lua script; Cheat Engine offers to run it when the table opens.
    pub lua_script: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheatEntry {
    pub id: Option<i64>,
    pub description: String,
    pub kind: EntryKind,
    pub children: Vec<CheatEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryKind {
    /// A group header; `address` is set for address group headers whose
    /// children use relative (`+10`) addresses.
    Group {
        address: Option<String>,
    },
    Address(AddressRecord),
    Script {
        script: String,
    },
    /// Variable types KaKa does not handle natively (Binary, Custom, ...).
    Unsupported {
        variable_type: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AddressRecord {
    pub address: String,
    /// In file order. Cheat Engine applies the LAST one first.
    pub offsets: Vec<String>,
    pub value_type: ValueType,
    pub show_as_hex: bool,
    pub show_as_signed: bool,
    /// `(value, label)` pairs from `<DropDownList>`.
    pub dropdown: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Byte,
    Bytes2,
    Bytes4,
    Bytes8,
    Float,
    Double,
    /// `length` in characters.
    String {
        length: usize,
        utf16: bool,
    },
    ByteArray {
        length: usize,
    },
}

impl ValueType {
    /// Cheat Engine's own (untranslated) type name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Byte => "Byte",
            Self::Bytes2 => "2 Bytes",
            Self::Bytes4 => "4 Bytes",
            Self::Bytes8 => "8 Bytes",
            Self::Float => "Float",
            Self::Double => "Double",
            Self::String { .. } => "String",
            Self::ByteArray { .. } => "Array of byte",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableError {
    Io(String),
    TooLarge,
    Xml(String),
    NotACheatTable,
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) | Self::Xml(e) => f.write_str(e),
            Self::TooLarge => f.write_str("file larger than 64 MiB"),
            Self::NotACheatTable => f.write_str("not a Cheat Engine table"),
        }
    }
}

impl std::error::Error for TableError {}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TableStats {
    pub addresses: usize,
    pub scripts: usize,
    pub groups: usize,
    pub unsupported: usize,
}

impl CheatTable {
    pub fn stats(&self) -> TableStats {
        fn walk(entries: &[CheatEntry], stats: &mut TableStats) {
            for entry in entries {
                match entry.kind {
                    EntryKind::Group { .. } => stats.groups += 1,
                    EntryKind::Address(_) => stats.addresses += 1,
                    EntryKind::Script { .. } => stats.scripts += 1,
                    EntryKind::Unsupported { .. } => stats.unsupported += 1,
                }
                walk(&entry.children, stats);
            }
        }
        let mut stats = TableStats::default();
        walk(&self.entries, &mut stats);
        stats
    }

    /// Scripts Cheat Engine may run: Auto Assembler entries plus the table's
    /// own Lua script.
    pub fn script_count(&self) -> usize {
        self.stats().scripts + usize::from(self.lua_script.is_some())
    }
}

/// Reads and parses a table file (lossy UTF-8; old tables may be ANSI).
pub fn load(path: &Path) -> Result<CheatTable, TableError> {
    let meta = fs::metadata(path).map_err(|e| TableError::Io(e.to_string()))?;
    if meta.len() > MAX_TABLE_BYTES {
        return Err(TableError::TooLarge);
    }
    let bytes = fs::read(path).map_err(|e| TableError::Io(e.to_string()))?;
    parse(&String::from_utf8_lossy(&bytes))
}

pub fn parse(text: &str) -> Result<CheatTable, TableError> {
    let doc = Document::parse(text.trim_start_matches('\u{feff}'))
        .map_err(|e| TableError::Xml(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "CheatTable" {
        return Err(TableError::NotACheatTable);
    }
    Ok(CheatTable {
        version: root
            .attribute("CheatEngineTableVersion")
            .and_then(|v| v.trim().parse().ok()),
        entries: child(root, "CheatEntries")
            .map(entries_of)
            .unwrap_or_default(),
        lua_script: child(root, "LuaScript")
            .map(text_of)
            .filter(|s| !s.trim().is_empty()),
    })
}

fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}

/// Element text; roxmltree merges CDATA and adjacent text.
fn text_of(node: Node) -> String {
    node.text().unwrap_or_default().to_owned()
}

fn entries_of(list: Node) -> Vec<CheatEntry> {
    list.children()
        .filter(|n| n.is_element() && n.tag_name().name() == "CheatEntry")
        .map(entry_of)
        .collect()
}

fn entry_of(node: Node) -> CheatEntry {
    let field = |name: &str| child(node, name).map(text_of);
    let flag = |name: &str| field(name).is_some_and(|v| v.trim() == "1");
    let number = |name: &str| field(name).and_then(|v| v.trim().parse::<usize>().ok());

    let mut description = field("Description").unwrap_or_default();
    if description.len() >= 2 && description.starts_with('"') && description.ends_with('"') {
        description = description[1..description.len() - 1].to_owned();
    }
    let address = field("Address")
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty());
    let variable_type = field("VariableType").map(|v| v.trim().to_owned());
    let lower = variable_type.as_deref().map(str::to_lowercase);

    let value_type = match lower.as_deref() {
        // Cheat Engine's default when the element is missing.
        None | Some("byte") => Some(ValueType::Byte),
        Some("2 bytes") => Some(ValueType::Bytes2),
        Some("4 bytes") => Some(ValueType::Bytes4),
        Some("8 bytes") => Some(ValueType::Bytes8),
        Some("float") => Some(ValueType::Float),
        Some("double") => Some(ValueType::Double),
        Some("string") => Some(ValueType::String {
            length: number("Length").unwrap_or(10),
            utf16: flag("Unicode"),
        }),
        Some("unicode string") => Some(ValueType::String {
            length: number("Length").unwrap_or(10),
            utf16: true,
        }),
        Some("array of byte") => Some(ValueType::ByteArray {
            length: number("ByteLength").unwrap_or(4),
        }),
        _ => None,
    };

    let kind = if lower.as_deref() == Some("auto assembler script") {
        EntryKind::Script {
            script: field("AssemblerScript").unwrap_or_default(),
        }
    } else if flag("GroupHeader") || (address.is_none() && variable_type.is_none()) {
        EntryKind::Group { address }
    } else if let (Some(value_type), Some(address)) = (value_type, address) {
        EntryKind::Address(AddressRecord {
            address,
            offsets: child(node, "Offsets")
                .map(|o| {
                    o.children()
                        .filter(|n| n.is_element() && n.tag_name().name() == "Offset")
                        .map(|n| text_of(n).trim().to_owned())
                        .collect()
                })
                .unwrap_or_default(),
            value_type,
            show_as_hex: flag("ShowAsHex"),
            show_as_signed: flag("ShowAsSigned"),
            dropdown: field("DropDownList")
                .map(|list| {
                    list.lines()
                        .filter_map(|line| line.split_once(':'))
                        .map(|(v, l)| (v.trim().to_owned(), l.trim().to_owned()))
                        .collect()
                })
                .unwrap_or_default(),
        })
    } else {
        EntryKind::Unsupported {
            variable_type: variable_type.unwrap_or_default(),
        }
    };

    CheatEntry {
        id: field("ID").and_then(|v| v.trim().parse().ok()),
        description,
        kind,
        children: child(node, "CheatEntries")
            .map(entries_of)
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CheatTable {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cheat_tables/sample.CT");
        load(&path).unwrap()
    }

    #[test]
    fn parses_groups_records_pointers_and_scripts() {
        let t = sample();
        assert_eq!(t.version, Some(45));
        assert_eq!(t.entries.len(), 3);
        let player = &t.entries[0];
        assert_eq!(player.description, "Player", "CE's quotes are stripped");
        assert_eq!(player.kind, EntryKind::Group { address: None });
        assert_eq!(player.children.len(), 3);

        let EntryKind::Address(health) = &player.children[0].kind else {
            panic!("{:?}", player.children[0].kind)
        };
        assert_eq!(health.address, "\"Game.exe\"+01A2B3C0");
        assert_eq!(health.offsets, ["1C", "8"], "file order is kept");
        assert_eq!(health.value_type, ValueType::Float);

        let EntryKind::Address(gold) = &player.children[1].kind else {
            panic!()
        };
        assert!(gold.show_as_signed && !gold.show_as_hex);
        assert_eq!(gold.value_type, ValueType::Bytes4);

        let EntryKind::Address(difficulty) = &player.children[2].kind else {
            panic!()
        };
        assert_eq!(
            difficulty.dropdown,
            [
                ("0".to_owned(), "Easy".to_owned()),
                ("1".to_owned(), "Normal".to_owned()),
                ("2".to_owned(), "Hard".to_owned())
            ]
        );

        let EntryKind::Script { script } = &t.entries[1].kind else {
            panic!()
        };
        assert!(script.starts_with("[ENABLE]") && script.contains("[DISABLE]"));
        assert_eq!(
            t.entries[2].kind,
            EntryKind::Unsupported {
                variable_type: "Binary".into()
            }
        );
        assert_eq!(t.lua_script.as_deref(), Some("print(\"hello\")\n"));
    }

    #[test]
    fn counts_entries_recursively() {
        assert_eq!(
            sample().stats(),
            TableStats {
                addresses: 3,
                scripts: 1,
                groups: 1,
                unsupported: 1
            }
        );
        assert_eq!(
            sample().script_count(),
            2,
            "one AA script plus the table Lua"
        );
    }

    #[test]
    fn rejects_other_documents_and_tolerates_a_bom() {
        assert_eq!(parse("<html/>"), Err(TableError::NotACheatTable));
        assert!(matches!(parse("not xml"), Err(TableError::Xml(_))));
        let t = parse("\u{feff}<CheatTable><CheatEntries/></CheatTable>").unwrap();
        assert!(t.entries.is_empty() && t.lua_script.is_none() && t.version.is_none());
    }

    #[test]
    fn a_real_cheat_engine_table_parses() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cheat_tables/real_ce.CT");
        if !path.exists() {
            eprintln!("skipped: real_ce.CT not added yet (spike S1)");
            return;
        }
        let t = load(&path).unwrap();
        assert!(!t.entries.is_empty());
    }
}
