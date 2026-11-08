//! Pure Steam / Proton discovery helpers: `/proc` parsing, running-game
//! detection, Proton install and Steam root lookup, trainer process matching.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Environment marker set on every trainer KaKa starts. It keeps trainers out
/// of game detection and lets KaKa stop exactly the processes it started.
pub const TRAINER_MARKER_ENV: &str = "KAKA_TRAINER";

/// Library entry id of the trainer a KaKa-started process belongs to.
pub const TRAINER_ID_ENV: &str = "KAKA_TRAINER_ID";

const PROTON_VERBS: [&str; 3] = ["waitforexitandrun", "run", "runinprefix"];

/// Wine/Proton helper executables; never used as a game's name.
const WINE_HELPERS: [&str; 13] = [
    "steam.exe",
    "services.exe",
    "winedevice.exe",
    "plugplay.exe",
    "explorer.exe",
    "rpcss.exe",
    "svchost.exe",
    "tabtip.exe",
    "conhost.exe",
    "start.exe",
    "wineboot.exe",
    "winemenubuilder.exe",
    "steamwebhelper.exe",
];

/// A snapshot of one process from `/proc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: i32,
    pub env: HashMap<String, String>,
    pub cmdline: Vec<String>,
}

impl ProcessInfo {
    pub fn new<K: Into<String>, V: Into<String>>(pid: i32, env: impl IntoIterator<Item = (K, V)>, cmdline: &[&str]) -> Self {
        Self {
            pid,
            env: env.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
            cmdline: cmdline.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn env(&self, key: &str) -> Option<&str> {
        self.env.get(key).map(String::as_str)
    }
}

/// A game currently running under Proton, grouped by its compat-data prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningGame {
    /// `STEAM_COMPAT_DATA_PATH` of the game (the directory that contains `pfx/`), normalized.
    pub compat_data_path: String,
    /// `STEAM_COMPAT_CLIENT_INSTALL_PATH` (the Steam install root), if the launcher set one.
    pub client_install_path: Option<String>,
    /// Steam AppID, if the launcher exposed one.
    pub app_id: Option<u64>,
    /// Absolute path of the `proton` script the game was started with, if seen.
    pub proton_script: Option<String>,
    /// Human readable name (game executable name, or the prefix folder name).
    pub label: String,
}

impl RunningGame {
    pub fn display_name(&self) -> String {
        match self.app_id {
            Some(id) => format!("{} (AppID {id})", self.label),
            None => self.label.clone(),
        }
    }
}

/// Parses a NUL separated `/proc/<pid>/environ` blob.
pub fn parse_environ(bytes: &[u8]) -> HashMap<String, String> {
    String::from_utf8_lossy(bytes)
        .split('\0')
        .filter_map(|entry| {
            let eq = entry.find('=')?;
            (eq > 0).then(|| (entry[..eq].to_owned(), entry[eq + 1..].to_owned()))
        })
        .collect()
}

/// Parses a NUL separated `/proc/<pid>/cmdline` blob.
pub fn parse_cmdline(bytes: &[u8]) -> Vec<String> {
    let mut parts: Vec<String> = String::from_utf8_lossy(bytes).split('\0').map(str::to_owned).collect();
    if parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

/// Reads every process of the current user that is readable under `proc_root`
/// (normally `/proc`). Processes that vanish or belong to other users are skipped.
pub fn read_processes(proc_root: &Path) -> Vec<ProcessInfo> {
    let Ok(entries) = fs::read_dir(proc_root) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let pid: i32 = entry.file_name().to_str()?.parse().ok()?;
            let env = fs::read(entry.path().join("environ")).ok()?;
            let cmdline = fs::read(entry.path().join("cmdline")).ok()?;
            Some(ProcessInfo {
                pid,
                env: parse_environ(&env),
                cmdline: parse_cmdline(&cmdline),
            })
        })
        .collect()
}

/// [`read_processes`] on the real `/proc`.
pub fn scan_processes() -> Vec<ProcessInfo> {
    read_processes(Path::new("/proc"))
}

fn positive_int(value: Option<&str>) -> Option<u64> {
    value?.parse::<u64>().ok().filter(|&n| n > 0)
}

/// Lexical POSIX normalization (like Dart's `package:path`): removes `.`,
/// resolves `..`, collapses and trims slashes. Does not touch the filesystem.
pub fn normalize(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push("..");
                }
            }
            p => parts.push(p),
        }
    }
    let joined = parts.join("/");
    match (absolute, joined.is_empty()) {
        (true, _) => format!("/{joined}"),
        (false, true) => ".".to_owned(),
        (false, false) => joined,
    }
}

/// Last path component, ignoring trailing slashes.
pub fn basename(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return if path.is_empty() { "" } else { "/" };
    }
    trimmed.rsplit('/').next().unwrap_or(trimmed)
}

/// Parent directory: `/a/b` → `/a`, `/a` → `/`, `a` → `.`.
pub fn dirname(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) => "/".to_owned(),
        Some(i) => trimmed[..i].to_owned(),
        None if path.starts_with('/') => "/".to_owned(),
        None => ".".to_owned(),
    }
}

/// File name of a Linux or Windows (`C:\...`) path.
pub fn exe_name(arg: &str) -> String {
    basename(&arg.replace('\\', "/")).to_owned()
}

/// Groups Proton game processes by `STEAM_COMPAT_DATA_PATH`.
///
/// Works for Steam games on any library drive, non-Steam shortcuts and other
/// launchers that use Proton (Heroic, umu-launcher, ...), because all of them
/// export the prefix in the environment. Trainers started by KaKa are ignored.
pub fn detect_running_games(processes: &[ProcessInfo]) -> Vec<RunningGame> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<&ProcessInfo>> = HashMap::new();
    for proc in processes {
        if proc.env.contains_key(TRAINER_MARKER_ENV) {
            continue;
        }
        let Some(compat) = proc.env("STEAM_COMPAT_DATA_PATH").filter(|c| !c.is_empty()) else {
            continue;
        };
        let key = normalize(compat);
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push(proc);
    }

    let mut games = Vec::new();
    for compat in order {
        let procs = &groups[&compat];
        let mut app_id = None;
        let mut client = None;
        let mut proton = None;
        let mut exe = None;
        for proc in procs {
            app_id = app_id
                .or_else(|| positive_int(proc.env("STEAM_COMPAT_APP_ID")))
                .or_else(|| positive_int(proc.env("SteamAppId")))
                .or_else(|| positive_int(proc.env("SteamGameId")));
            if client.is_none() {
                client = proc.env("STEAM_COMPAT_CLIENT_INSTALL_PATH").filter(|c| !c.is_empty()).map(str::to_owned);
            }
            let cmd = &proc.cmdline;
            for (i, arg) in cmd.iter().enumerate() {
                if proton.is_none() && basename(arg) == "proton" && cmd.get(i + 1).is_some_and(|verb| PROTON_VERBS.contains(&verb.as_str())) {
                    proton = Some(arg.clone());
                }
                if exe.is_none() && arg.to_lowercase().ends_with(".exe") {
                    let name = exe_name(arg);
                    // Skip Wine/Proton helper executables; we want the game's name.
                    if !WINE_HELPERS.contains(&name.to_lowercase().as_str()) {
                        exe = Some(name);
                    }
                }
            }
        }
        // A prefix with neither a proton invocation nor a Windows executable is not a running game.
        if proton.is_none() && exe.is_none() {
            continue;
        }
        let label = exe.unwrap_or_else(|| basename(&compat).to_owned());
        games.push(RunningGame {
            compat_data_path: compat,
            client_install_path: client,
            app_id,
            proton_script: proton,
            label,
        });
    }
    games
}

/// Picks the running game for a trainer bound to `app_id`. Matches the AppID
/// the launcher exported, or the prefix folder name (`compatdata/<app_id>`).
pub fn match_game(games: &[RunningGame], app_id: u64) -> Option<&RunningGame> {
    let folder = app_id.to_string();
    games.iter().find(|g| g.app_id == Some(app_id) || basename(&g.compat_data_path) == folder)
}

/// Finds the Proton installation directory from a prefix's `config_info`.
///
/// `config_info` lines 2+ are paths *inside* the Proton install
/// (e.g. `.../GE-Proton9-20/files/share/fonts/`). Walk up from each path until a
/// directory containing a `proton` script is found. This handles Steam,
/// compatibilitytools.d, Heroic (`.../tools/proton/GE-Proton-latest/...`) and
/// paths where an unrelated parent folder happens to contain "proton".
pub fn proton_dir_from_config_info(content: &str, is_file: impl Fn(&str) -> bool) -> Option<String> {
    for raw in content.lines() {
        let line = raw.trim();
        if !line.starts_with('/') {
            continue;
        }
        let mut dir = normalize(line);
        while dir != "/" && !dir.is_empty() && dir != "." {
            if is_file(&format!("{dir}/proton")) {
                return Some(dir);
            }
            dir = dirname(&dir);
        }
    }
    None
}

/// Steam install locations, in lookup order. A non-empty `override_path` (set by
/// the user in Settings) is the only candidate.
pub fn steam_root_candidates(home: &str, override_path: &str) -> Vec<String> {
    if !override_path.is_empty() {
        return vec![override_path.to_owned()];
    }
    [
        ".steam/root",
        ".local/share/Steam",
        ".steam/steam",
        ".steam/debian-installation",
        ".var/app/com.valvesoftware.Steam/.local/share/Steam",
        "snap/steam/common/.local/share/Steam",
    ]
    .iter()
    .map(|rel| format!("{home}/{rel}"))
    .collect()
}

/// First Steam root candidate that has a `steamapps` directory.
pub fn find_steam_root(home: &str, override_path: &str, dir_exists: impl Fn(&str) -> bool) -> Option<String> {
    steam_root_candidates(home, override_path)
        .into_iter()
        .find(|c| dir_exists(&format!("{c}/steamapps")))
}

/// Chooses the Proton verb used to start a trainer next to a running game.
///
/// `run` goes through Proton's `steam.exe` helper. With GE-Proton 11 and Proton
/// Experimental 11 (tested without a running Steam client) that helper did not
/// start a second program while the game's own `steam.exe` ran in the same
/// prefix: the trainer never appeared. `runinprefix` runs the exe with Wine
/// directly and joins the game's wineserver, also when the game runs inside the
/// Steam Linux Runtime container. It needs an already set-up prefix (callers
/// ensure the game finished setting it up) and a Proton build that knows the
/// verb; older builds fall back to `run`.
pub fn trainer_verb(proton_script: &str) -> &'static str {
    if proton_script.contains("\"runinprefix\"") {
        "runinprefix"
    } else {
        "run"
    }
}

/// Whether `proc` belongs to trainer `id` that KaKa started from `trainer_path`:
/// it carries KaKa's marker and the entry id, and its command line names the
/// trainer executable. Shared Wine services never match, so stopping matched
/// processes cannot hit the game.
pub fn is_trainer_process(proc: &ProcessInfo, id: &str, trainer_path: &str) -> bool {
    if proc.env(TRAINER_MARKER_ENV) != Some("1") || proc.env(TRAINER_ID_ENV) != Some(id) {
        return false;
    }
    let exe = exe_name(trainer_path).to_lowercase();
    proc.cmdline.iter().any(|arg| exe_name(arg).to_lowercase() == exe)
}

/// Whether any process (game, trainer, wineserver, ...) uses the Proton prefix
/// at `compat_data_path`. Proton exports STEAM_COMPAT_DATA_PATH to everything it
/// starts and WINEPREFIX (`<compatdata>/pfx`) to Wine processes.
pub fn prefix_in_use(processes: &[ProcessInfo], compat_data_path: &str) -> bool {
    let compat = normalize(compat_data_path);
    let pfx = format!("{}/pfx", compat.trim_end_matches('/'));
    processes.iter().any(|proc| {
        proc.env("STEAM_COMPAT_DATA_PATH")
            .is_some_and(|c| !c.is_empty() && normalize(c) == compat)
            || proc.env("WINEPREFIX").is_some_and(|w| !w.is_empty() && normalize(w) == pfx)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nul(parts: &[&str]) -> Vec<u8> {
        format!("{}\0", parts.join("\0")).into_bytes()
    }

    fn proc(pid: i32, env: &[(&str, &str)], cmdline: &[&str]) -> ProcessInfo {
        ProcessInfo::new(pid, env.iter().copied(), cmdline)
    }

    const STEAM_GAME_ENV: [(&str, &str); 3] = [
        ("STEAM_COMPAT_DATA_PATH", "/mnt/games/SteamLibrary/steamapps/compatdata/1245620/"),
        ("STEAM_COMPAT_CLIENT_INSTALL_PATH", "/home/u/.local/share/Steam"),
        ("SteamAppId", "1245620"),
    ];

    // parse_environ / parse_cmdline

    #[test]
    fn splits_nul_separated_blobs_keeps_equals_inside_values() {
        let env = parse_environ(&nul(&["A=1", "STEAM_COMPAT_DATA_PATH=/x/compatdata/42", "WEIRD=a=b", "NOEQUALS"]));
        let expected: HashMap<String, String> = [("A", "1"), ("STEAM_COMPAT_DATA_PATH", "/x/compatdata/42"), ("WEIRD", "a=b")]
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect();
        assert_eq!(env, expected);
        assert_eq!(
            parse_cmdline(&nul(&["python3", "/p/proton", "waitforexitandrun", "game.exe"])),
            ["python3", "/p/proton", "waitforexitandrun", "game.exe"]
        );
    }

    #[test]
    fn handles_non_utf8_bytes_without_panicking() {
        assert!(parse_environ(&[0x41, 0x3d, 0xff, 0x00]).contains_key("A"));
    }

    // detect_running_games

    #[test]
    fn steam_game_on_a_second_drive_prefix_steam_root_appid_and_proton_script_come_from_the_environment() {
        let games = detect_running_games(&[
            proc(
                10,
                &STEAM_GAME_ENV,
                &["/home/u/.local/share/Steam/ubuntu12_32/reaper", "SteamLaunch", "AppId=1245620"],
            ),
            proc(
                11,
                &STEAM_GAME_ENV,
                &[
                    "python3",
                    "/home/u/.local/share/Steam/compatibilitytools.d/GE-Proton9-20/proton",
                    "waitforexitandrun",
                    "/mnt/games/x/eldenring.exe",
                ],
            ),
            proc(
                12,
                &STEAM_GAME_ENV,
                &["C:\\windows\\system32\\steam.exe", "Z:\\mnt\\games\\x\\eldenring.exe"],
            ),
            proc(13, &STEAM_GAME_ENV, &["Z:\\mnt\\games\\x\\eldenring.exe"]),
        ]);
        assert_eq!(games.len(), 1);
        let g = &games[0];
        assert_eq!(g.compat_data_path, "/mnt/games/SteamLibrary/steamapps/compatdata/1245620");
        assert_eq!(g.client_install_path.as_deref(), Some("/home/u/.local/share/Steam"));
        assert_eq!(g.app_id, Some(1245620));
        assert_eq!(
            g.proton_script.as_deref(),
            Some("/home/u/.local/share/Steam/compatibilitytools.d/GE-Proton9-20/proton")
        );
        assert_eq!(g.label, "eldenring.exe");
    }

    #[test]
    fn heroic_umu_game_without_a_steam_appid_is_still_detected() {
        let games = detect_running_games(&[proc(
            20,
            &[
                ("STEAM_COMPAT_DATA_PATH", "/home/u/Games/Heroic/Prefixes/default/Cyberpunk 2077"),
                ("STEAM_COMPAT_CLIENT_INSTALL_PATH", ""),
            ],
            &[
                "python3",
                "/home/u/.var/app/com.heroicgameslauncher.hgl/config/heroic/tools/proton/GE-Proton-latest/proton",
                "run",
                "/home/u/Games/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe",
            ],
        )]);
        assert_eq!(games[0].app_id, None);
        assert_eq!(games[0].client_install_path, None, "empty value is treated as unset");
        assert_eq!(games[0].label, "Cyberpunk2077.exe");
    }

    #[test]
    fn ignores_processes_started_by_kaka_and_processes_without_a_prefix() {
        let mut marked = STEAM_GAME_ENV.to_vec();
        marked.push((TRAINER_MARKER_ENV, "1"));
        let games = detect_running_games(&[
            proc(30, &marked, &["Z:\\trainers\\EldenRingTrainer.exe"]),
            proc(31, &[("HOME", "/home/u")], &["/usr/bin/bash"]),
            proc(32, &[("STEAM_COMPAT_DATA_PATH", "/x/compatdata/7")], &["/usr/bin/sleep", "100"]),
        ]);
        assert!(games.is_empty());
    }

    #[test]
    fn several_games_are_reported_separately_match_game_picks_by_appid_or_prefix_folder() {
        let games = detect_running_games(&[
            proc(40, &STEAM_GAME_ENV, &["Z:\\a\\eldenring.exe"]),
            proc(
                41,
                &[("STEAM_COMPAT_DATA_PATH", "/home/u/.local/share/Steam/steamapps/compatdata/3123456789")],
                &["C:\\x\\shortcut.exe"],
            ),
        ]);
        assert_eq!(games.len(), 2);
        assert_eq!(match_game(&games, 1245620).map(|g| g.label.as_str()), Some("eldenring.exe"));
        assert_eq!(match_game(&games, 3123456789).map(|g| g.label.as_str()), Some("shortcut.exe"));
        assert!(match_game(&games, 999).is_none());
    }

    #[test]
    fn wine_helper_executables_are_not_used_as_the_game_label() {
        let games = detect_running_games(&[
            proc(
                50,
                &[("STEAM_COMPAT_DATA_PATH", "/c/compatdata/5")],
                &["C:\\windows\\system32\\services.exe"],
            ),
            proc(51, &[("STEAM_COMPAT_DATA_PATH", "/c/compatdata/5")], &["C:\\Games\\Real.exe"]),
        ]);
        assert_eq!(games[0].label, "Real.exe");
    }

    // proton_dir_from_config_info

    fn files_at<'a>(files: &'a [&'a str]) -> impl Fn(&str) -> bool + 'a {
        move |path| files.contains(&path)
    }

    #[test]
    fn steam_compatibilitytools_build_with_a_space_in_its_name() {
        let info = "11.0-100\n/home/u/.local/share/Steam/compatibilitytools.d/Proton-GE Latest/files/share/fonts/\n\
                    /home/u/.local/share/Steam/compatibilitytools.d/Proton-GE Latest/files/lib/\n";
        assert_eq!(
            proton_dir_from_config_info(
                info,
                files_at(&["/home/u/.local/share/Steam/compatibilitytools.d/Proton-GE Latest/proton"])
            )
            .as_deref(),
            Some("/home/u/.local/share/Steam/compatibilitytools.d/Proton-GE Latest")
        );
    }

    #[test]
    fn valve_proton_in_steamapps_common() {
        let info = "11.0-100\n/s/steamapps/common/Proton - Experimental/files/share/fonts/\n";
        assert_eq!(
            proton_dir_from_config_info(info, files_at(&["/s/steamapps/common/Proton - Experimental/proton"])).as_deref(),
            Some("/s/steamapps/common/Proton - Experimental")
        );
    }

    #[test]
    fn heroic_parent_folder_named_proton_is_not_mistaken_for_the_install_upstream_88() {
        let base = "/var/home/u/.var/app/com.heroicgameslauncher.hgl/config/heroic/tools/proton/GE-Proton-latest";
        let info = format!("GE-Proton10-4\n{base}/files/share/fonts/\n");
        let script = format!("{base}/proton");
        assert_eq!(proton_dir_from_config_info(&info, files_at(&[&script])).as_deref(), Some(base));
    }

    #[test]
    fn home_directory_containing_proton_does_not_confuse_the_lookup() {
        let info = "x\n/home/protonfan/.steam/steam/compatibilitytools.d/GE-Proton9-1/files/share/fonts/\n";
        assert_eq!(
            proton_dir_from_config_info(info, files_at(&["/home/protonfan/.steam/steam/compatibilitytools.d/GE-Proton9-1/proton"])).as_deref(),
            Some("/home/protonfan/.steam/steam/compatibilitytools.d/GE-Proton9-1")
        );
    }

    #[test]
    fn returns_none_when_the_proton_build_was_removed_or_for_empty_input() {
        assert!(proton_dir_from_config_info("x\n/gone/GE-Proton8-1/files/share/fonts/\n", files_at(&[])).is_none());
        assert!(proton_dir_from_config_info("", files_at(&[])).is_none());
    }

    // Steam root

    #[test]
    fn find_steam_root_user_override_wins_and_is_not_silently_replaced() {
        let dirs = ["/home/u/.local/share/Steam/steamapps"];
        assert_eq!(
            find_steam_root("/home/u", "", |p| dirs.contains(&p)).as_deref(),
            Some("/home/u/.local/share/Steam")
        );
        assert!(find_steam_root("/home/u", "/custom", |p| dirs.contains(&p)).is_none());
        assert_eq!(
            find_steam_root("/home/u", "/custom", |p| p == "/custom/steamapps").as_deref(),
            Some("/custom")
        );
    }

    #[test]
    fn find_steam_root_finds_flatpak_steam() {
        let dir = "/home/u/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps";
        assert_eq!(
            find_steam_root("/home/u", "", |p| p == dir).as_deref(),
            Some("/home/u/.var/app/com.valvesoftware.Steam/.local/share/Steam")
        );
    }

    // trainer_verb

    #[test]
    fn runinprefix_when_the_proton_build_supports_it_run_for_older_builds() {
        assert_eq!(trainer_verb("elif sys.argv[1] == \"runinprefix\":"), "runinprefix");
        assert_eq!(trainer_verb("if sys.argv[1] == \"run\":"), "run");
        assert_eq!(trainer_verb(""), "run");
    }

    // is_trainer_process

    const ID: &str = "abc123";
    const PATH: &str = "/home/u/Trainers/Elden Ring Trainer.exe";
    const MARKED: [(&str, &str); 2] = [(TRAINER_MARKER_ENV, "1"), (TRAINER_ID_ENV, ID)];

    #[test]
    fn matches_the_proton_wrapper_and_the_wine_process_of_this_trainer_entry() {
        assert!(is_trainer_process(
            &proc(1, &MARKED, &["python3", "/p/proton", "runinprefix", PATH]),
            ID,
            PATH
        ));
        assert!(is_trainer_process(
            &proc(2, &MARKED, &["Z:\\home\\u\\Trainers\\ELDEN RING TRAINER.EXE"]),
            ID,
            PATH
        ));
    }

    #[test]
    fn never_matches_another_entry_an_unmarked_process_or_a_shared_wine_service() {
        assert!(!is_trainer_process(
            &proc(3, &[(TRAINER_MARKER_ENV, "1"), (TRAINER_ID_ENV, "other")], &[PATH]),
            ID,
            PATH
        ));
        assert!(!is_trainer_process(&proc(4, &[(TRAINER_ID_ENV, ID)], &[PATH]), ID, PATH));
        assert!(!is_trainer_process(&proc(5, &MARKED, &["/p/files/bin/wineserver"]), ID, PATH));
        assert!(!is_trainer_process(&proc(6, &MARKED, &["C:\\windows\\system32\\explorer.exe"]), ID, PATH));
    }

    // prefix_in_use

    const COMPAT: &str = "/games/steamapps/compatdata/1245620";

    #[test]
    fn matches_steam_compat_data_path_any_spelling_and_wineprefix_including_kaka_trainers() {
        assert!(prefix_in_use(
            &[proc(1, &[("STEAM_COMPAT_DATA_PATH", &format!("{COMPAT}/"))], &["game.exe"])],
            COMPAT
        ));
        assert!(prefix_in_use(
            &[proc(2, &[("WINEPREFIX", &format!("{COMPAT}/pfx"))], &["wineserver"])],
            COMPAT
        ));
        assert!(prefix_in_use(
            &[proc(
                3,
                &[("STEAM_COMPAT_DATA_PATH", COMPAT), (TRAINER_MARKER_ENV, "1")],
                &["Trainer.exe"]
            )],
            COMPAT
        ));
    }

    #[test]
    fn other_prefixes_and_unrelated_processes_do_not_count() {
        let other = format!("{COMPAT}0");
        assert!(!prefix_in_use(
            &[
                proc(4, &[("STEAM_COMPAT_DATA_PATH", &other)], &["x"]),
                proc(5, &[("WINEPREFIX", COMPAT)], &["y"]),
                proc(6, &[], &["z"])
            ],
            COMPAT
        ));
    }

    // read_processes

    #[test]
    fn reads_numeric_entries_skips_unreadable_ones_and_non_pid_folders() {
        let root = tempfile::Builder::new().prefix("kaka_proc_").tempdir().unwrap();
        let good = root.path().join("123");
        fs::create_dir(&good).unwrap();
        fs::write(good.join("environ"), nul(&["STEAM_COMPAT_DATA_PATH=/c/compatdata/9"])).unwrap();
        fs::write(good.join("cmdline"), nul(&["C:\\g\\game.exe"])).unwrap();
        fs::create_dir(root.path().join("456")).unwrap(); // vanished process: no files
        fs::create_dir(root.path().join("self")).unwrap();

        let procs = read_processes(root.path());
        assert_eq!(procs.iter().map(|p| p.pid).collect::<Vec<_>>(), [123]);
        assert_eq!(detect_running_games(&procs)[0].label, "game.exe");
    }

    #[test]
    fn works_against_the_real_proc_of_this_machine() {
        let me = std::process::id() as i32;
        assert!(scan_processes().iter().any(|p| p.pid == me));
    }

    // path helpers (parity with Dart's package:path for the cases KaKa relies on)

    #[test]
    fn path_helpers_match_package_path() {
        assert_eq!(normalize("/a/b/../c/./d/"), "/a/c/d");
        assert_eq!(normalize("/a//b"), "/a/b");
        assert_eq!(normalize("/.."), "/");
        assert_eq!(basename("/a/b/"), "b");
        assert_eq!(basename("/"), "/");
        assert_eq!(dirname("/a/b"), "/a");
        assert_eq!(dirname("/a"), "/");
        assert_eq!(exe_name("Z:\\x\\Game.EXE"), "Game.EXE");
    }
}
