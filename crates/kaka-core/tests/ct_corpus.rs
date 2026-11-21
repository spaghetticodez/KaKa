//! Opt-in: parses every `.CT` under `$KAKA_CT_CORPUS` and prints what the
//! tables use, to decide with data whether a CE-free engine is worth it.
//! The tables stay local and are never committed:
//!
//! KAKA_CT_CORPUS=~/Games/Tables cargo test -p kaka-core --test ct_corpus -- --ignored --nocapture

use std::fs;
use std::path::{Path, PathBuf};

use kaka_core::cheat_table::{self, CheatEntry, EntryKind};

fn tables(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            tables(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ct"))
        {
            out.push(path);
        }
    }
}

#[derive(Default)]
struct Count {
    values: usize,
    scripts: usize,
    /// Scripts a byte-patch engine cannot run: code caves or embedded Lua.
    complex_scripts: usize,
}

fn walk(entries: &[CheatEntry], count: &mut Count) {
    for entry in entries {
        match &entry.kind {
            EntryKind::Address(_) => count.values += 1,
            EntryKind::Script { script } => {
                count.scripts += 1;
                let s = script.to_lowercase();
                if s.contains("alloc(") || s.contains("{$lua}") || s.contains("{$c}") {
                    count.complex_scripts += 1;
                }
            }
            EntryKind::Group { .. } | EntryKind::Unsupported { .. } => {}
        }
        walk(&entry.children, count);
    }
}

#[test]
#[ignore = "needs a local folder of .CT files (KAKA_CT_CORPUS)"]
fn corpus_report() {
    let Ok(dir) = std::env::var("KAKA_CT_CORPUS") else {
        eprintln!("skipped: set KAKA_CT_CORPUS");
        return;
    };
    let mut files = Vec::new();
    tables(Path::new(&dir), &mut files);
    let (mut failed, mut table_lua, mut simple) = (0, 0, 0);
    let mut total = Count::default();
    for file in &files {
        let table = match cheat_table::load(file) {
            Ok(table) => table,
            Err(e) => {
                failed += 1;
                eprintln!("FAIL {}: {e}", file.display());
                continue;
            }
        };
        let mut count = Count::default();
        walk(&table.entries, &mut count);
        table_lua += usize::from(table.lua_script.is_some());
        simple += usize::from(count.complex_scripts == 0 && table.lua_script.is_none());
        total.values += count.values;
        total.scripts += count.scripts;
        total.complex_scripts += count.complex_scripts;
    }
    println!(
        "{} tables ({failed} unreadable): {} values, {} scripts ({} with code caves / Lua / C), \
         {table_lua} tables with a Lua script, {simple} tables a byte-patch engine could fully run",
        files.len(),
        total.values,
        total.scripts,
        total.complex_scripts
    );
    assert_eq!(failed, 0, "every table must parse");
}
