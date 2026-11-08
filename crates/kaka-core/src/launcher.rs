//! Finds the running game for a trainer and starts the trainer in its Proton
//! prefix; tracks running trainers by their Proton wrapper's exit.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustix::process::{Pid, Signal, kill_process};

use crate::model::TrainerEntry;
use crate::proton::{
    ProcessInfo, TRAINER_ID_ENV, TRAINER_MARKER_ENV, detect_running_games, dirname,
    find_steam_root, is_trainer_process, match_game, proton_dir_from_config_info, scan_processes,
    trainer_verb,
};
use crate::storage::{home_dir, xdg_dir};

/// How long a freshly started trainer is watched for an immediate failure.
pub const DEFAULT_STARTUP_WATCH: Duration = Duration::from_secs(4);
const KEPT_LOGS: usize = 20;
const OUTPUT_TAIL_LINES: usize = 15;

/// Everything needed to run a trainer inside a game's Proton prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub proton_script: String,
    pub compat_data_path: String,
    pub client_install_path: String,
    /// Proton verb, see [`trainer_verb`].
    pub verb: &'static str,
}

/// Why a launch failed. The UI turns each variant into a translated message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    TrainerNotFound {
        path: String,
    },
    /// No running Proton game (or not the one the trainer is bound to).
    GameNotRunning,
    /// Several games run and the trainer has no AppID; display names of the games.
    MultipleGames {
        games: Vec<String>,
    },
    ProtonNotFound {
        prefix: String,
    },
    /// The game is still setting up its prefix.
    GameStarting,
    /// The trainer exited with an error within the startup watch.
    TrainerExited {
        code: i32,
        output: String,
        log_path: String,
    },
    /// The trainer of this entry already runs.
    AlreadyRunning {
        name: String,
    },
    /// The Proton script could not be started at all.
    Spawn {
        message: String,
        log_path: Option<String>,
    },
}

impl LaunchError {
    /// The launch log, when one was written.
    pub fn log_path(&self) -> Option<&str> {
        match self {
            Self::TrainerExited { log_path, .. } => Some(log_path),
            Self::Spawn { log_path, .. } => log_path.as_deref(),
            _ => None,
        }
    }
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for LaunchError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LauncherEvent {
    Started { id: String },
    Exited { id: String, code: i32 },
}

pub type EventHandler = Arc<dyn Fn(LauncherEvent) + Send + Sync>;

/// `$XDG_STATE_HOME/kaka/logs` (default `~/.local/state/kaka/logs`).
pub fn default_log_dir() -> PathBuf {
    xdg_dir("XDG_STATE_HOME", ".local/state")
        .join("kaka")
        .join("logs")
}

#[derive(Default)]
struct Shared {
    /// Proton wrapper pid per entry id while it runs. `proton run/runinprefix`
    /// waits for the trainer, so the wrapper's exit is the trainer's exit and
    /// running state needs no polling.
    live: HashMap<String, u32>,
    /// Trainers started in this session (entry id -> trainer path), stopped on exit.
    started: HashMap<String, String>,
    handler: Option<EventHandler>,
}

pub struct Launcher {
    steam_path: Mutex<String>,
    log_dir: PathBuf,
    startup_watch: Duration,
    home: String,
    shared: Arc<Mutex<Shared>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Launcher {
    pub fn new(log_dir: impl Into<PathBuf>, startup_watch: Duration) -> Self {
        Self {
            steam_path: Mutex::new(String::new()),
            log_dir: log_dir.into(),
            startup_watch,
            home: home_dir().to_string_lossy().into_owned(),
            shared: Arc::default(),
        }
    }

    /// Real log dir and startup watch.
    pub fn from_env() -> Self {
        Self::new(default_log_dir(), DEFAULT_STARTUP_WATCH)
    }

    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    /// Steam install root chosen in Settings. Empty means auto-detect.
    pub fn steam_path(&self) -> String {
        lock(&self.steam_path).clone()
    }

    pub fn set_steam_path(&self, path: &str) {
        *lock(&self.steam_path) = path.to_owned();
    }

    /// Called (from a watcher thread) when a trainer starts or exits.
    pub fn set_event_handler(&self, handler: EventHandler) {
        lock(&self.shared).handler = Some(handler);
    }

    /// Ids of trainers started by this launcher whose Proton wrapper still runs.
    pub fn live_ids(&self) -> HashSet<String> {
        lock(&self.shared).live.keys().cloned().collect()
    }

    /// Steam install root: the Settings override, else the first detected one.
    pub fn steam_root(&self) -> Option<String> {
        let root = find_steam_root(&self.home, &self.steam_path(), |p| Path::new(p).is_dir())?;
        Some(
            fs::canonicalize(&root)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or(root),
        )
    }

    /// Resolves the plan from `processes` (a fresh scan) and starts the trainer.
    /// Blocks for up to the startup watch: call it off the UI thread.
    pub fn launch(
        &self,
        entry: &TrainerEntry,
        processes: &[ProcessInfo],
    ) -> Result<LaunchPlan, LaunchError> {
        let plan = self.resolve_plan(entry, processes)?;
        self.start(entry, &plan)?;
        Ok(plan)
    }

    /// Works out which prefix and Proton build to use.
    pub fn resolve_plan(
        &self,
        entry: &TrainerEntry,
        processes: &[ProcessInfo],
    ) -> Result<LaunchPlan, LaunchError> {
        if !Path::new(&entry.trainer_path).is_file() {
            return Err(LaunchError::TrainerNotFound {
                path: entry.trainer_path.clone(),
            });
        }

        let games = detect_running_games(processes);
        // The game must already run. Starting a trainer first does not work with
        // Steam: its 'waitforexitandrun' waits for the prefix's wineserver (kept alive
        // by the trainer) to exit, so the game would hang until the trainer closes.
        let game = match entry.steam_app_id {
            Some(app_id) => match_game(&games, app_id).ok_or(LaunchError::GameNotRunning)?,
            None => match games.as_slice() {
                [single] => single,
                [] => return Err(LaunchError::GameNotRunning),
                _ => {
                    return Err(LaunchError::MultipleGames {
                        games: games.iter().map(|g| g.display_name()).collect(),
                    });
                }
            },
        };

        // Prefer what the game's own launcher exported: always the right drive and Steam root.
        let prefix = game.compat_data_path.clone();
        let client = game
            .client_install_path
            .clone()
            .or_else(|| self.steam_root())
            .unwrap_or_default();

        let proton = match game
            .proton_script
            .clone()
            .filter(|p| Path::new(p).is_file())
        {
            Some(script) => script,
            None => {
                let config_info =
                    fs::read_to_string(Path::new(&prefix).join("config_info")).unwrap_or_default();
                let dir = proton_dir_from_config_info(&config_info, |p| Path::new(p).is_file())
                    .ok_or_else(|| LaunchError::ProtonNotFound {
                        prefix: prefix.clone(),
                    })?;
                format!("{dir}/proton")
            }
        };

        // Proton writes <compatdata>/version as the last step of prefix setup (under
        // pfx.lock) and deletes it before an upgrade; pfx/system.reg appears much
        // earlier. Without it the game is still setting up the prefix, and a second
        // Proton instance racing into it could break it.
        if !Path::new(&prefix).join("version").is_file() {
            return Err(LaunchError::GameStarting);
        }

        let script = fs::read_to_string(&proton).unwrap_or_default();
        Ok(LaunchPlan {
            proton_script: proton,
            compat_data_path: prefix,
            client_install_path: client,
            verb: trainer_verb(&script),
        })
    }

    /// Starts the trainer through Proton and watches it for the startup watch.
    /// Fails with the log tail if it exits with an error in that time.
    pub fn start(&self, entry: &TrainerEntry, plan: &LaunchPlan) -> Result<(), LaunchError> {
        let (log_path, mut log) =
            self.new_log_file(&entry.name)
                .map_err(|e| LaunchError::Spawn {
                    message: e.to_string(),
                    log_path: None,
                })?;
        let log_str = log_path.to_string_lossy().into_owned();
        let spawn_error = |e: io::Error| LaunchError::Spawn {
            message: e.to_string(),
            log_path: Some(log_str.clone()),
        };

        let (_, iso) = utc_stamp(SystemTime::now());
        writeln!(
            log,
            "# KaKa trainer launch {iso}\n# trainer: {}\n# proton:  {} {}\n# prefix:  {}\n# steam:   {}",
            entry.trainer_path, plan.proton_script, plan.verb, plan.compat_data_path, plan.client_install_path
        )
        .map_err(spawn_error)?;

        let stdout = log.try_clone().map_err(spawn_error)?;
        let stderr = log.try_clone().map_err(spawn_error)?;
        let mut child = match Command::new(&plan.proton_script)
            .arg(plan.verb)
            .arg(&entry.trainer_path)
            // Trainers often load files next to themselves.
            .current_dir(dirname(&entry.trainer_path))
            .env(
                "STEAM_COMPAT_CLIENT_INSTALL_PATH",
                &plan.client_install_path,
            )
            .env("STEAM_COMPAT_DATA_PATH", &plan.compat_data_path)
            .env(TRAINER_MARKER_ENV, "1")
            .env(TRAINER_ID_ENV, &entry.id)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                let _ = writeln!(log, "# failed to start: {e}");
                return Err(spawn_error(e));
            }
        };

        let pid = child.id();
        let handler = {
            let mut shared = lock(&self.shared);
            shared
                .started
                .insert(entry.id.clone(), entry.trainer_path.clone());
            shared.live.insert(entry.id.clone(), pid);
            shared.handler.clone()
        };
        if let Some(h) = handler {
            h(LauncherEvent::Started {
                id: entry.id.clone(),
            });
        }

        let (code_tx, code_rx) = mpsc::channel();
        let shared = Arc::clone(&self.shared);
        let id = entry.id.clone();
        thread::Builder::new()
            .name(format!("kaka-watch-{pid}"))
            .spawn(move || {
                let code = match child.wait() {
                    Ok(status) => status
                        .code()
                        .unwrap_or_else(|| -status.signal().unwrap_or(0)),
                    Err(_) => -1,
                };
                let _ = writeln!(log, "# exit code {code}");
                let handler = {
                    let mut shared = lock(&shared);
                    // A newer launch of the same entry owns the slot now.
                    if shared.live.get(&id) == Some(&pid) {
                        shared.live.remove(&id);
                        shared.handler.clone()
                    } else {
                        None
                    }
                };
                if let Some(h) = handler {
                    h(LauncherEvent::Exited { id, code });
                }
                let _ = code_tx.send(code);
            })
            .map_err(spawn_error)?;

        match code_rx.recv_timeout(self.startup_watch) {
            Ok(code) if code != 0 => Err(LaunchError::TrainerExited {
                code,
                output: output_tail(&log_path),
                log_path: log_str,
            }),
            _ => Ok(()),
        }
    }

    /// Creates the log file for a new launch, deleting the oldest logs so that
    /// at most [`KEPT_LOGS`] remain. Opened in append mode, so the trainer's
    /// output and KaKa's own lines never overwrite each other.
    fn new_log_file(&self, name: &str) -> io::Result<(PathBuf, File)> {
        fs::create_dir_all(&self.log_dir)?;
        let mut logs: Vec<PathBuf> = fs::read_dir(&self.log_dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "log"))
            .collect();
        logs.sort();
        let excess = (logs.len() + 1).saturating_sub(KEPT_LOGS);
        for old in logs.iter().take(excess) {
            let _ = fs::remove_file(old); // best effort only
        }
        let (stamp, _) = utc_stamp(SystemTime::now());
        let path = self
            .log_dir
            .join(format!("{stamp}-{}.log", safe_log_name(name)));
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok((path, file))
    }

    /// Ids of `entries` with a live trainer process (see [`is_trainer_process`]).
    /// Also finds trainers a previous KaKa session left running.
    pub fn running_ids(
        &self,
        entries: &[TrainerEntry],
        processes: &[ProcessInfo],
    ) -> HashSet<String> {
        let marked: Vec<&ProcessInfo> = processes
            .iter()
            .filter(|p| p.env.get(TRAINER_MARKER_ENV).is_some_and(|v| v == "1"))
            .collect();
        entries
            .iter()
            .filter(|e| {
                marked
                    .iter()
                    .any(|p| is_trainer_process(p, &e.id, &e.trainer_path))
            })
            .map(|e| e.id.clone())
            .collect()
    }

    /// Sends SIGTERM to the processes of `entry`'s trainer; returns how many.
    pub fn stop(&self, entry: &TrainerEntry, processes: &[ProcessInfo]) -> usize {
        processes
            .iter()
            .filter(|p| is_trainer_process(p, &entry.id, &entry.trainer_path))
            .filter(|p| terminate(p.pid))
            .count()
    }

    /// Stops trainers started by this launcher. Only processes carrying the KaKa
    /// marker, the entry id *and* the trainer's executable name are signalled, so
    /// Wine services shared with a running game (wineserver, explorer.exe, ...) survive.
    pub fn stop_all(&self) {
        let started = lock(&self.shared).started.clone();
        if started.is_empty() {
            return;
        }
        for proc in scan_processes() {
            let Some(id) = proc.env.get(TRAINER_ID_ENV) else {
                continue;
            };
            if let Some(path) = started.get(id)
                && is_trainer_process(&proc, id, path)
            {
                terminate(proc.pid);
            }
        }
    }
}

fn terminate(pid: i32) -> bool {
    Pid::from_raw(pid).is_some_and(|pid| kill_process(pid, Signal::TERM).is_ok())
}

/// The last lines of the trainer's own output (KaKa's `# ` lines excluded).
fn output_tail(log: &Path) -> String {
    let text = fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.starts_with("# ")).collect();
    lines[lines.len().saturating_sub(OUTPUT_TAIL_LINES)..].join("\n")
}

/// Runs of characters outside `[A-Za-z0-9._-]` become `_`; at most 40 characters.
fn safe_log_name(name: &str) -> String {
    let mut safe = String::new();
    let mut in_run = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            safe.push(c);
            in_run = false;
        } else if !in_run {
            safe.push('_');
            in_run = true;
        }
    }
    safe.truncate(40);
    safe
}

/// (`YYYY-MM-DDTHH-MM-SS` for file names, `YYYY-MM-DDTHH:MM:SSZ`) in UTC.
fn utc_stamp(t: SystemTime) -> (String, String) {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default() as i64;
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (s / 3600, s % 3600 / 60, s % 60);
    (
        format!("{y:04}-{m:02}-{d:02}T{hh:02}-{mm:02}-{ss:02}"),
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z"),
    )
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian (H. Hinnant).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(10_957), (2000, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        let (file, iso) = utc_stamp(UNIX_EPOCH + Duration::from_secs(19_723 * 86_400 + 3_723));
        assert_eq!(file, "2024-01-01T01-02-03");
        assert_eq!(iso, "2024-01-01T01:02:03Z");
    }

    #[test]
    fn log_names_are_sanitized_and_short() {
        assert_eq!(safe_log_name("Elden Ring: Deluxe!"), "Elden_Ring_Deluxe_");
        assert_eq!(safe_log_name("艾尔登法环 v1.0"), "_v1.0");
        assert_eq!(safe_log_name(&"x".repeat(60)).len(), 40);
    }

    #[test]
    fn keeps_the_newest_logs_only() {
        let tmp = tempfile::tempdir().unwrap();
        let launcher = Launcher::new(tmp.path(), DEFAULT_STARTUP_WATCH);
        for i in 0..25 {
            fs::write(
                tmp.path().join(format!("2020-01-01T00-00-{i:02}-old.log")),
                "x",
            )
            .unwrap();
        }
        fs::write(tmp.path().join("notes.txt"), "keep").unwrap();
        let (path, _) = launcher.new_log_file("new").unwrap();
        let logs: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "log"))
            .collect();
        assert_eq!(logs.len(), KEPT_LOGS, "19 old ones plus the new one");
        assert!(path.exists());
        assert!(
            !tmp.path().join("2020-01-01T00-00-00-old.log").exists(),
            "oldest go first"
        );
        assert!(
            tmp.path().join("notes.txt").exists(),
            "only .log files are rotated"
        );
    }

    #[test]
    fn output_tail_skips_kaka_lines_and_keeps_the_last_15() {
        let tmp = tempfile::tempdir().unwrap();
        let log = tmp.path().join("x.log");
        let mut text = String::from("# header\n");
        for i in 0..20 {
            text.push_str(&format!("line {i}\n"));
        }
        text.push_str("# exit code 3\n");
        fs::write(&log, text).unwrap();
        let tail = output_tail(&log);
        assert_eq!(tail.lines().count(), 15);
        assert!(tail.starts_with("line 5") && tail.ends_with("line 19"));
    }
}
