//! App state without UI: the trainer library, settings, launch/stop and the
//! running-trainer bookkeeping. Port of the Flutter build's `LibraryController`.
//!
//! [`Library`] is `Send + Sync` and meant to be shared as `Arc<Library>`. The
//! blocking operations ([`Library::launch`], [`Library::stop`],
//! [`Library::rescan`], ...) scan `/proc` or wait for a process, so the UI
//! calls them on a worker thread; the state lock is never held across them.
//! Timers stay in the UI: after a scan it asks [`Library::needs_orphan_poll`].

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use crate::game_info::{GameSuggestion, suggestion_for};
use crate::language::LANGUAGE_OPTIONS;
use crate::launcher::{EventHandler, LaunchError, LaunchPlan, Launcher};
use crate::model::{AppConfig, TrainerEntry};
use crate::proton::{ProcessInfo, detect_running_games, prefix_in_use, scan_processes};
use crate::storage::LocalStorage;
use crate::trainer_data::{CleanupError, TrainerDataCleanup};

/// Persistence of the library and settings.
pub trait Store: Send + Sync {
    fn read_library(&self) -> Vec<TrainerEntry>;
    fn write_library(&self, entries: &[TrainerEntry]) -> io::Result<()>;
    fn read_config(&self) -> AppConfig;
    fn write_config(&self, language: &str, steam_path: &str) -> io::Result<()>;
}

impl Store for LocalStorage {
    fn read_library(&self) -> Vec<TrainerEntry> {
        LocalStorage::read_library(self)
    }
    fn write_library(&self, entries: &[TrainerEntry]) -> io::Result<()> {
        LocalStorage::write_library(self, entries)
    }
    fn read_config(&self) -> AppConfig {
        LocalStorage::read_config(self)
    }
    fn write_config(&self, language: &str, steam_path: &str) -> io::Result<()> {
        LocalStorage::write_config(self, language, steam_path)
    }
}

/// Starting, finding and stopping trainers (the real one is [`Launcher`]).
pub trait Backend: Send + Sync {
    fn launch(
        &self,
        entry: &TrainerEntry,
        processes: &[ProcessInfo],
    ) -> Result<LaunchPlan, LaunchError>;
    /// Trainers this session started whose Proton wrapper still runs.
    fn live_ids(&self) -> HashSet<String>;
    fn running_ids(&self, entries: &[TrainerEntry], processes: &[ProcessInfo]) -> HashSet<String>;
    fn stop(&self, entry: &TrainerEntry, processes: &[ProcessInfo]) -> usize;
    fn stop_all(&self);
    fn steam_path(&self) -> String;
    fn set_steam_path(&self, path: &str);
    fn steam_root(&self) -> Option<String>;
    fn log_dir(&self) -> PathBuf;
    /// Called from a watcher thread when a trainer this session started starts or exits.
    fn set_event_handler(&self, handler: EventHandler);
}

impl Backend for Launcher {
    fn launch(
        &self,
        entry: &TrainerEntry,
        processes: &[ProcessInfo],
    ) -> Result<LaunchPlan, LaunchError> {
        Launcher::launch(self, entry, processes)
    }
    fn live_ids(&self) -> HashSet<String> {
        Launcher::live_ids(self)
    }
    fn running_ids(&self, entries: &[TrainerEntry], processes: &[ProcessInfo]) -> HashSet<String> {
        Launcher::running_ids(self, entries, processes)
    }
    fn stop(&self, entry: &TrainerEntry, processes: &[ProcessInfo]) -> usize {
        Launcher::stop(self, entry, processes)
    }
    fn stop_all(&self) {
        Launcher::stop_all(self)
    }
    fn steam_path(&self) -> String {
        Launcher::steam_path(self)
    }
    fn set_steam_path(&self, path: &str) {
        Launcher::set_steam_path(self, path)
    }
    fn steam_root(&self) -> Option<String> {
        Launcher::steam_root(self)
    }
    fn log_dir(&self) -> PathBuf {
        Launcher::log_dir(self).to_path_buf()
    }
    fn set_event_handler(&self, handler: EventHandler) {
        Launcher::set_event_handler(self, handler)
    }
}

/// A snapshot of the processes of the current user.
pub trait ProcessSource: Send + Sync {
    fn scan(&self) -> Vec<ProcessInfo>;
}

/// The real `/proc`.
pub struct ProcScan;

impl ProcessSource for ProcScan {
    fn scan(&self) -> Vec<ProcessInfo> {
        scan_processes()
    }
}

pub type ChangeListener = Arc<dyn Fn() + Send + Sync>;

/// The listener lives behind its own lock so launcher events (from watcher
/// threads) can notify without touching the library state.
#[derive(Default)]
struct Notifier {
    listener: Mutex<Option<ChangeListener>>,
}

impl Notifier {
    fn notify(&self) {
        let listener = lock(&self.listener).clone();
        if let Some(listener) = listener {
            listener();
        }
    }
}

struct State {
    trainers: Vec<TrainerEntry>,
    loading: bool,
    language: String,
    /// Running trainers this session did not start (found by a scan).
    orphans: HashSet<String>,
    /// Entries between the running check and the end of their launch.
    launching: HashSet<String>,
}

pub struct Library {
    store: Arc<dyn Store>,
    backend: Arc<dyn Backend>,
    processes: Arc<dyn ProcessSource>,
    notifier: Arc<Notifier>,
    state: Mutex<State>,
    stop_poll: Duration,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Library {
    pub fn new(
        store: Arc<dyn Store>,
        backend: Arc<dyn Backend>,
        processes: Arc<dyn ProcessSource>,
    ) -> Self {
        let notifier = Arc::new(Notifier::default());
        let events = Arc::clone(&notifier);
        backend.set_event_handler(Arc::new(move |_| events.notify()));
        Self {
            store,
            backend,
            processes,
            notifier,
            state: Mutex::new(State {
                trainers: Vec::new(),
                loading: true,
                language: LANGUAGE_OPTIONS[0].to_owned(),
                orphans: HashSet::new(),
                launching: HashSet::new(),
            }),
            stop_poll: Duration::from_millis(300),
        }
    }

    /// The real storage, launcher and `/proc`.
    pub fn from_env() -> Self {
        Self::new(
            Arc::new(LocalStorage::from_env()),
            Arc::new(Launcher::from_env()),
            Arc::new(ProcScan),
        )
    }

    /// Called after every change, possibly from a worker or watcher thread.
    /// `None` detaches the UI (on close); later events are ignored.
    pub fn set_listener(&self, listener: Option<ChangeListener>) {
        *lock(&self.notifier.listener) = listener;
    }

    fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    pub fn trainers(&self) -> Vec<TrainerEntry> {
        self.state().trainers.clone()
    }

    pub fn trainer(&self, id: &str) -> Option<TrainerEntry> {
        self.state().trainers.iter().find(|e| e.id == id).cloned()
    }

    pub fn loading(&self) -> bool {
        self.state().loading
    }

    /// Display name from [`LANGUAGE_OPTIONS`].
    pub fn language(&self) -> String {
        self.state().language.clone()
    }

    /// Steam install root chosen in Settings; empty means auto-detect.
    pub fn steam_path(&self) -> String {
        self.backend.steam_path()
    }

    /// Folder with the per-launch trainer logs.
    pub fn log_dir(&self) -> PathBuf {
        self.backend.log_dir()
    }

    /// Whether entry `id`'s trainer runs: started by this session and its
    /// Proton wrapper still alive (event-driven), or found by the last scan.
    pub fn is_running(&self, id: &str) -> bool {
        self.backend.live_ids().contains(id) || self.state().orphans.contains(id)
    }

    /// Whether trainers left by an earlier session exist. They have no exit
    /// event, so the UI re-checks them with [`Self::rescan`] only while this is true.
    pub fn needs_orphan_poll(&self) -> bool {
        !self.state().orphans.is_empty()
    }

    pub fn load(&self) {
        let config = self.store.read_config();
        self.backend.set_steam_path(&config.steam_path);
        let trainers = self.store.read_library();
        {
            let mut state = self.state();
            if let Some(language) = config
                .language
                .filter(|l| LANGUAGE_OPTIONS.contains(&l.as_str()))
            {
                state.language = language;
            }
            state.trainers = trainers;
            state.loading = false;
        }
        self.notifier.notify();
    }

    /// Applies `change` to the list, notifies and saves.
    fn edit(&self, change: impl FnOnce(&mut Vec<TrainerEntry>) -> bool) -> io::Result<()> {
        let snapshot = {
            let mut state = self.state();
            if !change(&mut state.trainers) {
                return Ok(());
            }
            state.trainers.clone()
        };
        self.notifier.notify();
        self.store.write_library(&snapshot)
    }

    pub fn add(&self, entry: TrainerEntry) -> io::Result<()> {
        self.edit(|list| {
            list.insert(0, entry);
            true
        })
    }

    pub fn update(&self, entry: TrainerEntry) -> io::Result<()> {
        self.edit(|list| match list.iter_mut().find(|e| e.id == entry.id) {
            Some(slot) => {
                *slot = entry;
                true
            }
            None => false,
        })
    }

    pub fn remove(&self, id: &str) -> io::Result<()> {
        self.edit(|list| {
            let before = list.len();
            list.retain(|e| e.id != id);
            list.len() != before
        })
    }

    pub fn set_language(&self, language: &str) -> io::Result<()> {
        {
            let mut state = self.state();
            if !LANGUAGE_OPTIONS.contains(&language) || state.language == language {
                return Ok(());
            }
            state.language = language.to_owned();
        }
        self.notifier.notify();
        self.save_config()
    }

    pub fn set_steam_path(&self, path: &str) -> io::Result<()> {
        self.backend.set_steam_path(path);
        self.notifier.notify();
        self.save_config()
    }

    /// Moves `entry` to the front and launches it. A trainer that is already
    /// running (or being launched) is not started a second time: FLiNG
    /// trainers fail on a second instance. Blocks for the startup watch.
    pub fn launch(&self, entry: &TrainerEntry) -> Result<(), LaunchError> {
        let processes = self.rescan();
        let already = || LaunchError::AlreadyRunning {
            name: entry.name.clone(),
        };
        if self.is_running(&entry.id) {
            return Err(already());
        }
        let snapshot = {
            let mut state = self.state();
            if !state.launching.insert(entry.id.clone()) {
                return Err(already());
            }
            state.trainers.retain(|e| e.id != entry.id);
            state.trainers.insert(0, entry.clone());
            state.trainers.clone()
        };
        self.notifier.notify();
        self.save_library(&snapshot);

        let result = self.backend.launch(entry, &processes);
        let snapshot = {
            let mut state = self.state();
            state.launching.remove(&entry.id);
            match &result {
                Ok(plan) => {
                    if let Some(slot) = state.trainers.iter_mut().find(|e| e.id == entry.id) {
                        slot.last_prefix = plan.compat_data_path.clone();
                    }
                    Some(state.trainers.clone())
                }
                Err(_) => None,
            }
        };
        self.notifier.notify();
        if let Some(snapshot) = snapshot {
            self.save_library(&snapshot);
        }
        result.map(|_| ())
    }

    /// One scan at startup, to show trainers an earlier session left running.
    /// Trainers started from now on are tracked through their process exit.
    pub fn start_monitoring(&self) {
        self.rescan();
    }

    /// Scans the process list once and updates the set of running trainers
    /// this session did not start. Returns the scanned processes.
    pub fn rescan(&self) -> Vec<ProcessInfo> {
        let processes = self.processes.scan();
        let trainers = self.trainers();
        let live = self.backend.live_ids();
        let orphans: HashSet<String> = self
            .backend
            .running_ids(&trainers, &processes)
            .difference(&live)
            .cloned()
            .collect();
        let changed = {
            let mut state = self.state();
            let changed = state.orphans != orphans;
            state.orphans = orphans;
            changed
        };
        if changed {
            self.notifier.notify();
        }
        processes
    }

    /// Stops `entry`'s trainer and waits up to `timeout` for it to exit.
    /// Returns whether it is gone.
    pub fn stop(&self, entry: &TrainerEntry, timeout: Duration) -> bool {
        self.backend.stop(entry, &self.processes.scan());
        let deadline = Instant::now() + timeout;
        loop {
            self.rescan();
            if !self.is_running(&entry.id) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(self.stop_poll);
        }
    }

    /// Proton games running right now, with name, AppID and cover filled in
    /// from local Steam data where available ("use running game" when adding).
    pub fn running_games(&self) -> Vec<GameSuggestion> {
        let games = detect_running_games(&self.processes.scan());
        if games.is_empty() {
            return Vec::new();
        }
        let root = self.backend.steam_root();
        games
            .iter()
            .map(|g| suggestion_for(g, root.as_deref()))
            .collect()
    }

    /// Resets FLiNG trainer state in the prefix `entry` last ran in (see
    /// [`TrainerDataCleanup`]). Returns the number of removed items; refuses
    /// while anything still runs in that prefix.
    pub fn reset_trainer_data(&self, entry: &TrainerEntry) -> Result<usize, CleanupError> {
        if entry.last_prefix.is_empty() {
            return Ok(0);
        }
        let in_use = prefix_in_use(&self.processes.scan(), &entry.last_prefix);
        TrainerDataCleanup::new(Path::new(&entry.last_prefix)).run(in_use)
    }

    /// Saves everything and stops the trainers this session started.
    pub fn shutdown(&self) {
        if let Err(e) = self.save_config() {
            eprintln!("Could not save the settings: {e}");
        }
        self.save_library(&self.trainers());
        self.backend.stop_all();
    }

    fn save_library(&self, entries: &[TrainerEntry]) {
        if let Err(e) = self.store.write_library(entries) {
            eprintln!("Could not save the library: {e}");
        }
    }

    fn save_config(&self) -> io::Result<()> {
        let language = self.language();
        self.store
            .write_config(&language, &self.backend.steam_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::LauncherEvent;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct MemoryStore {
        library: Mutex<Vec<TrainerEntry>>,
        config: Mutex<AppConfig>,
        library_writes: AtomicUsize,
        config_writes: AtomicUsize,
    }

    impl MemoryStore {
        fn ids(&self) -> Vec<String> {
            lock(&self.library).iter().map(|e| e.id.clone()).collect()
        }
    }

    impl Store for MemoryStore {
        fn read_library(&self) -> Vec<TrainerEntry> {
            lock(&self.library).clone()
        }
        fn write_library(&self, entries: &[TrainerEntry]) -> io::Result<()> {
            *lock(&self.library) = entries.to_vec();
            self.library_writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn read_config(&self) -> AppConfig {
            lock(&self.config).clone()
        }
        fn write_config(&self, language: &str, steam_path: &str) -> io::Result<()> {
            *lock(&self.config) = AppConfig {
                language: Some(language.to_owned()),
                steam_path: steam_path.to_owned(),
            };
            self.config_writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    /// Records launches instead of starting Proton.
    #[derive(Default)]
    struct FakeBackend {
        failure: Mutex<Option<LaunchError>>,
        prefix: Mutex<String>,
        launched: Mutex<Vec<String>>,
        stop_all_calls: AtomicUsize,
        /// Started this "session" (event-driven). Launch adds, stop removes unless `stubborn`.
        live: Mutex<HashSet<String>>,
        /// Only a process scan finds these (left over from an earlier session).
        orphan_procs: Mutex<HashSet<String>>,
        stubborn: Mutex<bool>,
        steam_path: Mutex<String>,
        handler: Mutex<Option<EventHandler>>,
    }

    impl FakeBackend {
        fn emit(&self) {
            let handler = lock(&self.handler).clone();
            if let Some(h) = handler {
                h(LauncherEvent::Started { id: String::new() });
            }
        }
    }

    impl Backend for FakeBackend {
        fn launch(
            &self,
            entry: &TrainerEntry,
            _: &[ProcessInfo],
        ) -> Result<LaunchPlan, LaunchError> {
            lock(&self.launched).push(entry.id.clone());
            if let Some(e) = lock(&self.failure).clone() {
                return Err(e);
            }
            lock(&self.live).insert(entry.id.clone());
            self.emit();
            Ok(LaunchPlan {
                proton_script: "/proton/proton".into(),
                compat_data_path: lock(&self.prefix).clone(),
                client_install_path: "/steam".into(),
                verb: "runinprefix",
            })
        }
        fn live_ids(&self) -> HashSet<String> {
            lock(&self.live).clone()
        }
        fn running_ids(&self, entries: &[TrainerEntry], _: &[ProcessInfo]) -> HashSet<String> {
            let (live, orphans) = (lock(&self.live), lock(&self.orphan_procs));
            entries
                .iter()
                .filter(|e| live.contains(&e.id) || orphans.contains(&e.id))
                .map(|e| e.id.clone())
                .collect()
        }
        fn stop(&self, entry: &TrainerEntry, _: &[ProcessInfo]) -> usize {
            if *lock(&self.stubborn) {
                return 1;
            }
            let removed =
                lock(&self.live).remove(&entry.id) | lock(&self.orphan_procs).remove(&entry.id);
            if removed {
                self.emit();
            }
            usize::from(removed)
        }
        fn stop_all(&self) {
            self.stop_all_calls.fetch_add(1, Ordering::SeqCst);
        }
        fn steam_path(&self) -> String {
            lock(&self.steam_path).clone()
        }
        fn set_steam_path(&self, path: &str) {
            *lock(&self.steam_path) = path.to_owned();
        }
        fn steam_root(&self) -> Option<String> {
            None
        }
        fn log_dir(&self) -> PathBuf {
            PathBuf::from("/logs")
        }
        fn set_event_handler(&self, handler: EventHandler) {
            *lock(&self.handler) = Some(handler);
        }
    }

    /// Counts how often the library scans /proc.
    #[derive(Default)]
    struct CountingScan {
        processes: Mutex<Vec<ProcessInfo>>,
        calls: AtomicUsize,
    }

    impl CountingScan {
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl ProcessSource for CountingScan {
        fn scan(&self) -> Vec<ProcessInfo> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            lock(&self.processes).clone()
        }
    }

    fn trainer(id: &str) -> TrainerEntry {
        TrainerEntry::new(id, format!("Game {id}"), format!("/t/{id}.exe"))
    }

    struct Fixture {
        store: Arc<MemoryStore>,
        backend: Arc<FakeBackend>,
        scan: Arc<CountingScan>,
        library: Library,
    }

    fn fixture() -> Fixture {
        let store = Arc::new(MemoryStore::default());
        *lock(&store.library) = vec![trainer("a"), trainer("b")];
        *lock(&store.config) = AppConfig {
            language: Some("German".into()),
            steam_path: "/steam".into(),
        };
        let backend = Arc::new(FakeBackend::default());
        *lock(&backend.prefix) = "/compat/1245620".into();
        let scan = Arc::new(CountingScan::default());
        let mut library = Library::new(store.clone(), backend.clone(), scan.clone());
        library.stop_poll = Duration::from_millis(10);
        library.load();
        Fixture {
            store,
            backend,
            scan,
            library,
        }
    }

    fn count_notifications(library: &Library) -> Arc<AtomicUsize> {
        let notified = Arc::new(AtomicUsize::new(0));
        let n = Arc::clone(&notified);
        library.set_listener(Some(Arc::new(move || {
            n.fetch_add(1, Ordering::SeqCst);
        })));
        notified
    }

    #[test]
    fn load_reads_the_library_and_settings_and_hands_the_steam_path_to_the_launcher() {
        let f = fixture();
        assert!(!f.library.loading());
        assert_eq!(
            f.library
                .trainers()
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(f.library.language(), "German");
        assert_eq!(f.backend.steam_path(), "/steam");
    }

    #[test]
    fn an_unknown_saved_language_falls_back_to_english() {
        let store = Arc::new(MemoryStore::default());
        *lock(&store.config) = AppConfig {
            language: Some("Klingon".into()),
            steam_path: String::new(),
        };
        let library = Library::new(
            store,
            Arc::new(FakeBackend::default()),
            Arc::new(CountingScan::default()),
        );
        library.load();
        assert_eq!(library.language(), "English");
    }

    #[test]
    fn add_update_and_remove_are_saved_immediately() {
        let f = fixture();
        f.library.add(trainer("c")).unwrap();
        assert_eq!(f.store.ids(), ["c", "a", "b"]);

        let mut renamed = trainer("a");
        renamed.name = "Renamed".into();
        f.library.update(renamed).unwrap();
        assert_eq!(lock(&f.store.library)[1].name, "Renamed");

        f.library.remove("b").unwrap();
        assert_eq!(f.store.ids(), ["c", "a"]);
        assert_eq!(f.store.library_writes.load(Ordering::SeqCst), 3);

        f.library.update(trainer("unknown")).unwrap();
        f.library.remove("unknown").unwrap();
        assert_eq!(
            f.store.library_writes.load(Ordering::SeqCst),
            3,
            "no-op edits are not saved"
        );
    }

    #[test]
    fn launch_moves_the_entry_to_the_front() {
        let f = fixture();
        let last = f.library.trainers().pop().unwrap();
        f.library.launch(&last).unwrap();
        assert_eq!(*lock(&f.backend.launched), ["b"]);
        assert_eq!(f.store.ids(), ["b", "a"]);
    }

    #[test]
    fn a_successful_launch_records_the_prefix_a_failed_one_keeps_the_old_value() {
        let f = fixture();
        let first = f.library.trainers()[0].clone();
        f.library.launch(&first).unwrap();
        assert_eq!(lock(&f.store.library)[0].last_prefix, "/compat/1245620");

        lock(&f.backend.live).clear();
        *lock(&f.backend.prefix) = "/compat/other".into();
        *lock(&f.backend.failure) = Some(LaunchError::GameNotRunning);
        assert!(f.library.launch(&f.library.trainers()[0]).is_err());
        assert_eq!(lock(&f.store.library)[0].last_prefix, "/compat/1245620");
    }

    #[test]
    fn reset_trainer_data_refuses_while_something_runs_in_the_prefix_then_cleans_it() {
        let tmp = tempfile::Builder::new()
            .prefix("kaka_ctl_reset_")
            .tempdir()
            .unwrap();
        let compat = tmp
            .path()
            .join("compatdata/7")
            .to_string_lossy()
            .into_owned();
        let fling =
            Path::new(&compat).join("pfx/drive_c/users/steamuser/AppData/Local/FLiNGTrainer");
        fs::create_dir_all(&fling).unwrap();
        let running = Arc::new(CountingScan::default());
        *lock(&running.processes) = vec![ProcessInfo::new(
            1,
            [("STEAM_COMPAT_DATA_PATH", compat.as_str())],
            &["game.exe"],
        )];
        let library = Library::new(
            Arc::new(MemoryStore::default()),
            Arc::new(FakeBackend::default()),
            running.clone(),
        );
        let mut entry = trainer("x");
        entry.last_prefix = compat.clone();

        assert!(matches!(
            library.reset_trainer_data(&entry),
            Err(CleanupError::PrefixInUse)
        ));
        assert!(fling.exists());

        lock(&running.processes).clear();
        assert_eq!(library.reset_trainer_data(&entry).unwrap(), 1);
        assert!(!fling.exists());
        assert_eq!(
            library
                .reset_trainer_data(&trainer("never-launched"))
                .unwrap(),
            0
        );
    }

    #[test]
    fn a_running_trainer_is_not_launched_twice_stop_clears_it() {
        let f = fixture();
        let entry = f.library.trainers()[0].clone();
        f.library.launch(&entry).unwrap();
        assert!(f.library.is_running(&entry.id));

        assert_eq!(
            f.library.launch(&entry),
            Err(LaunchError::AlreadyRunning {
                name: entry.name.clone()
            })
        );
        assert_eq!(
            *lock(&f.backend.launched),
            vec![entry.id.clone()],
            "no second process"
        );

        assert!(f.library.stop(&entry, Duration::from_secs(5)));
        assert!(!f.library.is_running(&entry.id));
    }

    #[test]
    fn the_launch_guard_also_catches_a_trainer_only_a_scan_can_see() {
        let f = fixture();
        lock(&f.backend.orphan_procs).insert("a".into());
        assert_eq!(
            f.library.launch(&f.library.trainers()[0]),
            Err(LaunchError::AlreadyRunning {
                name: "Game a".into()
            })
        );
        assert!(lock(&f.backend.launched).is_empty());
    }

    #[test]
    fn concurrent_launches_of_one_entry_start_a_single_trainer() {
        let f = fixture();
        // A slow launch: the second click arrives while the first is still in the startup watch.
        struct Slow(Arc<FakeBackend>);
        impl Backend for Slow {
            fn launch(
                &self,
                e: &TrainerEntry,
                p: &[ProcessInfo],
            ) -> Result<LaunchPlan, LaunchError> {
                thread::sleep(Duration::from_millis(200));
                let plan = self.0.launch(e, p);
                lock(&self.0.live).remove(&e.id); // not visible as running via live_ids
                plan
            }
            fn live_ids(&self) -> HashSet<String> {
                HashSet::new()
            }
            fn running_ids(&self, _: &[TrainerEntry], _: &[ProcessInfo]) -> HashSet<String> {
                HashSet::new()
            }
            fn stop(&self, e: &TrainerEntry, p: &[ProcessInfo]) -> usize {
                self.0.stop(e, p)
            }
            fn stop_all(&self) {}
            fn steam_path(&self) -> String {
                String::new()
            }
            fn set_steam_path(&self, _: &str) {}
            fn steam_root(&self) -> Option<String> {
                None
            }
            fn log_dir(&self) -> PathBuf {
                PathBuf::new()
            }
            fn set_event_handler(&self, _: EventHandler) {}
        }
        let library = Arc::new(Library::new(
            f.store.clone(),
            Arc::new(Slow(f.backend.clone())),
            f.scan.clone(),
        ));
        library.load();
        let entry = library.trainers()[0].clone();
        let (l2, e2) = (Arc::clone(&library), entry.clone());
        let first = thread::spawn(move || l2.launch(&e2));
        thread::sleep(Duration::from_millis(50));
        assert_eq!(
            library.launch(&entry),
            Err(LaunchError::AlreadyRunning {
                name: entry.name.clone()
            })
        );
        first.join().unwrap().unwrap();
        assert_eq!(lock(&f.backend.launched).len(), 1);
    }

    #[test]
    fn stop_gives_up_after_the_timeout_when_the_trainer_does_not_exit() {
        let f = fixture();
        lock(&f.backend.live).insert("a".into());
        *lock(&f.backend.stubborn) = true;
        assert!(
            !f.library
                .stop(&f.library.trainers()[0], Duration::from_millis(100))
        );
        assert!(f.library.is_running("a"));
    }

    #[test]
    fn idle_one_scan_at_startup_then_none_while_no_orphan_trainer_exists() {
        let f = fixture();
        f.library.start_monitoring();
        assert_eq!(f.scan.calls(), 1);
        assert!(
            !f.library.needs_orphan_poll(),
            "no periodic /proc scans when nothing needs them"
        );

        // A trainer this session starts and its exit are events, not scans.
        let notified = count_notifications(&f.library);
        lock(&f.backend.live).insert("b".into());
        f.backend.emit();
        assert!(f.library.is_running("b"));
        lock(&f.backend.live).remove("b");
        f.backend.emit();
        assert!(!f.library.is_running("b"));
        assert_eq!(notified.load(Ordering::SeqCst), 2);
        assert_eq!(f.scan.calls(), 1);
    }

    #[test]
    fn a_trainer_left_by_an_earlier_session_is_re_checked_only_until_it_is_gone() {
        let f = fixture();
        lock(&f.backend.orphan_procs).insert("b".into());
        f.library.start_monitoring();
        assert!(f.library.is_running("b"));
        assert!(f.library.needs_orphan_poll());

        lock(&f.backend.orphan_procs).clear();
        f.library.rescan();
        assert!(!f.library.is_running("b"));
        assert!(
            !f.library.needs_orphan_poll(),
            "polling stops with the last orphan"
        );
    }

    #[test]
    fn a_detached_listener_ignores_launcher_events() {
        let f = fixture();
        let notified = count_notifications(&f.library);
        f.backend.emit();
        f.library.set_listener(None);
        f.backend.emit();
        assert_eq!(notified.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn launch_returns_the_launch_error_with_its_log_path() {
        let f = fixture();
        let error = LaunchError::TrainerExited {
            code: 3,
            output: "boom".into(),
            log_path: "/logs/x.log".into(),
        };
        *lock(&f.backend.failure) = Some(error.clone());
        let got = f.library.launch(&f.library.trainers()[0]).unwrap_err();
        assert_eq!(got, error);
        assert_eq!(got.log_path(), Some("/logs/x.log"));
    }

    #[test]
    fn settings_are_saved_unknown_languages_are_rejected() {
        let f = fixture();
        f.library.set_language("中文").unwrap();
        f.library.set_steam_path("").unwrap();
        assert_eq!(lock(&f.store.config).language.as_deref(), Some("中文"));
        assert_eq!(lock(&f.store.config).steam_path, "");
        assert_eq!(f.backend.steam_path(), "");

        f.library.set_language("Klingon").unwrap();
        assert_eq!(f.library.language(), "中文");
    }

    #[test]
    fn shutdown_saves_everything_and_stops_this_sessions_trainers() {
        let f = fixture();
        f.library.shutdown();
        assert_eq!(f.store.library_writes.load(Ordering::SeqCst), 1);
        assert_eq!(f.store.config_writes.load(Ordering::SeqCst), 1);
        assert_eq!(f.backend.stop_all_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn listeners_are_notified_on_every_change() {
        let f = fixture();
        let notified = count_notifications(&f.library);
        f.library.add(trainer("c")).unwrap();
        f.library.remove("c").unwrap();
        f.library.set_language("English").unwrap();
        assert_eq!(notified.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn running_games_is_empty_without_proton_processes() {
        let f = fixture();
        assert!(f.library.running_games().is_empty());
    }
}
