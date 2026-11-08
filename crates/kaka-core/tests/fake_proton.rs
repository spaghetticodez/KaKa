//! End-to-end launcher tests against the real /proc: a fake game process that
//! exports a Proton environment, and a fake `proton` script that records what
//! KaKa passes to it. Port of the Flutter build's `test/launcher_test.dart`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::sleep;
use std::time::Duration;

use kaka_core::launcher::{LaunchError, Launcher, LauncherEvent};
use kaka_core::model::TrainerEntry;
use kaka_core::proton::{TRAINER_MARKER_ENV, detect_running_games, scan_processes};

const FAKE_APP_ID: u64 = 987_654_321;

const FAKE_PROTON: &str = r#"#!/bin/bash
# Like real Proton: case "$1" in run|runinprefix) ... (KaKa checks for "runinprefix")
{
  echo "ARGS:$*"
  echo "PWD:$PWD"
  echo "DATA:$STEAM_COMPAT_DATA_PATH"
  echo "MARK:$KAKA_TRAINER"
  echo "ID:$KAKA_TRAINER_ID"
} > "$(dirname "$0")/record-$(basename "$2").txt"
echo "wine: hello from fake proton"
if [ "$(basename "$2")" = "Crash.exe" ]; then
  echo "err: boom" >&2
  exit 3
fi
sleep 60 &
trap 'kill $!; exit 0' TERM
wait
"#;

/// The tests share /proc (and one fake AppID), so they run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

struct Fixture {
    _serial: MutexGuard<'static, ()>,
    tmp: tempfile::TempDir,
    prefix: String,
    proton_dir: PathBuf,
    trainers: PathBuf,
    game: Child,
    launcher: Launcher,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.game.kill();
        let _ = self.game.wait();
        self.launcher.stop_all();
    }
}

fn wait_until(mut condition: impl FnMut() -> bool) -> bool {
    for _ in 0..100 {
        if condition() {
            return true;
        }
        sleep(Duration::from_millis(20));
    }
    condition()
}

fn fixture() -> Fixture {
    let serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let tmp = tempfile::Builder::new()
        .prefix("kaka_launch_")
        .tempdir()
        .unwrap();
    let proton_dir = tmp.path().join("compatibilitytools.d/Fake Proton");
    fs::create_dir_all(proton_dir.join("files/share/fonts")).unwrap();
    let script = proton_dir.join("proton");
    fs::write(&script, FAKE_PROTON).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let prefix = tmp
        .path()
        .join(format!("library/steamapps/compatdata/{FAKE_APP_ID}"));
    fs::create_dir_all(prefix.join("pfx")).unwrap();
    fs::write(prefix.join("pfx/system.reg"), "WINE REGISTRY Version 2\n").unwrap();
    // Proton writes <compatdata>/version last, when the prefix is fully set up.
    fs::write(prefix.join("version"), "11.0-100\n").unwrap();
    let pd = proton_dir.display();
    fs::write(
        prefix.join("config_info"),
        format!("fake-1\n{pd}/files/share/fonts/\n{pd}/files/lib/\n"),
    )
    .unwrap();

    let trainers = tmp.path().join("my trainers");
    fs::create_dir_all(&trainers).unwrap();
    fs::write(trainers.join("Trainer.exe"), "MZ").unwrap();
    fs::write(trainers.join("Crash.exe"), "MZ").unwrap();

    let prefix = prefix.to_string_lossy().into_owned();
    let game = Command::new("bash")
        .args(["-c", "exec -a FakeGame.exe sleep 60"])
        .env("STEAM_COMPAT_DATA_PATH", &prefix)
        .env("SteamAppId", FAKE_APP_ID.to_string())
        .spawn()
        .unwrap();
    // Wait until the exec has replaced bash, so /proc shows the game's cmdline.
    assert!(wait_until(|| detect_running_games(&scan_processes())
        .iter()
        .any(|g| g.app_id == Some(FAKE_APP_ID))));

    let launcher = Launcher::new(tmp.path().join("logs"), Duration::from_millis(800));
    Fixture {
        _serial: serial,
        tmp,
        prefix,
        proton_dir,
        trainers,
        game,
        launcher,
    }
}

impl Fixture {
    fn entry(&self, exe: &str, app_id: Option<u64>) -> TrainerEntry {
        let mut e = TrainerEntry::new(exe, exe, self.trainers.join(exe).to_string_lossy());
        e.steam_app_id = app_id;
        e
    }

    fn game_alive(&mut self) -> bool {
        self.game.try_wait().unwrap().is_none()
    }
}

fn trainer_alive(exe: &str) -> bool {
    scan_processes().iter().any(|p| {
        p.env.get(TRAINER_MARKER_ENV).is_some_and(|v| v == "1")
            && p.cmdline.iter().any(|a| a.ends_with(exe))
    })
}

#[test]
fn resolves_the_running_game_prefix_and_the_proton_build_from_config_info() {
    let f = fixture();
    let plan = f
        .launcher
        .resolve_plan(
            &f.entry("Trainer.exe", Some(FAKE_APP_ID)),
            &scan_processes(),
        )
        .unwrap();
    assert_eq!(plan.compat_data_path, f.prefix);
    assert_eq!(Path::new(&plan.proton_script), f.proton_dir.join("proton"));
    assert_eq!(plan.verb, "runinprefix");
}

#[test]
fn refuses_while_the_game_is_still_setting_up_its_prefix() {
    let f = fixture();
    fs::remove_file(Path::new(&f.prefix).join("version")).unwrap();
    assert!(Path::new(&f.prefix).join("pfx/system.reg").exists());
    assert_eq!(
        f.launcher.resolve_plan(
            &f.entry("Trainer.exe", Some(FAKE_APP_ID)),
            &scan_processes()
        ),
        Err(LaunchError::GameStarting)
    );
}

#[test]
fn falls_back_to_run_for_a_proton_build_without_runinprefix() {
    let f = fixture();
    fs::write(
        f.proton_dir.join("proton"),
        FAKE_PROTON.replace("runinprefix", "oldverb"),
    )
    .unwrap();
    assert_eq!(
        f.launcher
            .resolve_plan(
                &f.entry("Trainer.exe", Some(FAKE_APP_ID)),
                &scan_processes()
            )
            .unwrap()
            .verb,
        "run"
    );
}

#[test]
fn starts_the_trainer_inside_the_prefix_with_the_marker_then_stops_it_on_exit() {
    let mut f = fixture();
    let e = f.entry("Trainer.exe", Some(FAKE_APP_ID));
    f.launcher.launch(&e, &scan_processes()).unwrap();

    let record = fs::read_to_string(f.proton_dir.join("record-Trainer.exe.txt")).unwrap();
    let lines: Vec<&str> = record.lines().collect();
    assert!(
        lines.contains(&format!("ARGS:runinprefix {}", e.trainer_path).as_str()),
        "{record}"
    );
    assert!(
        lines.contains(&format!("PWD:{}", f.trainers.display()).as_str()),
        "{record}"
    );
    assert!(
        lines.contains(&format!("DATA:{}", f.prefix).as_str()),
        "{record}"
    );
    assert!(lines.contains(&"MARK:1"));
    assert!(lines.contains(&format!("ID:{}", e.id).as_str()));

    assert!(trainer_alive("Trainer.exe"));
    let games = detect_running_games(&scan_processes());
    assert_eq!(
        games
            .iter()
            .filter(|g| g.compat_data_path == f.prefix)
            .count(),
        1,
        "the trainer itself must not show up as a second game"
    );

    f.launcher.stop_all();
    assert!(wait_until(|| !trainer_alive("Trainer.exe")));
    assert!(f.game_alive(), "the game process must survive stop_all");

    let logs: Vec<_> = fs::read_dir(f.tmp.path().join("logs"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert_eq!(logs.len(), 1);
    assert!(
        fs::read_to_string(logs[0].path())
            .unwrap()
            .contains("hello from fake proton")
    );
}

#[test]
fn a_running_trainer_is_found_per_entry_and_stop_ends_only_that_one() {
    let mut f = fixture();
    let events: Arc<Mutex<Vec<LauncherEvent>>> = Arc::default();
    let sink = Arc::clone(&events);
    f.launcher
        .set_event_handler(Arc::new(move |ev| sink.lock().unwrap().push(ev)));

    let e = f.entry("Trainer.exe", Some(FAKE_APP_ID));
    let mut twin = TrainerEntry::new("twin", "Same exe, other entry", e.trainer_path.clone());
    twin.steam_app_id = Some(FAKE_APP_ID);
    f.launcher.launch(&e, &scan_processes()).unwrap();

    assert_eq!(
        f.launcher.live_ids(),
        [e.id.clone()].into(),
        "tracked from its own process, no scan needed"
    );
    assert_eq!(
        events.lock().unwrap().as_slice(),
        [LauncherEvent::Started { id: e.id.clone() }]
    );
    assert_eq!(
        f.launcher
            .running_ids(&[e.clone(), twin.clone()], &scan_processes()),
        [e.id.clone()].into()
    );

    assert_eq!(
        f.launcher.stop(&twin, &scan_processes()),
        0,
        "another entry with the same exe matches nothing"
    );
    assert!(f.launcher.stop(&e, &scan_processes()) > 0);
    assert!(
        wait_until(|| f.launcher.live_ids().is_empty()),
        "the exit itself clears the running state"
    );
    assert!(
        matches!(events.lock().unwrap().last(), Some(LauncherEvent::Exited { id, .. }) if *id == e.id)
    );
    assert!(f.launcher.running_ids(&[e], &scan_processes()).is_empty());
    assert!(f.game_alive(), "the game process must survive stop");
}

#[test]
fn a_trainer_that_exits_immediately_reports_the_exit_code_and_output() {
    let f = fixture();
    let err = f
        .launcher
        .launch(&f.entry("Crash.exe", Some(FAKE_APP_ID)), &scan_processes())
        .unwrap_err();
    let LaunchError::TrainerExited {
        code,
        output,
        log_path,
    } = err
    else {
        panic!("unexpected {err:?}")
    };
    assert_eq!(code, 3);
    assert!(
        output.contains("err: boom") && output.contains("hello from fake proton"),
        "{output}"
    );
    assert!(!output.contains("# "), "KaKa's own log lines are not shown");
    assert!(Path::new(&log_path).is_file());
    assert!(
        fs::read_to_string(&log_path)
            .unwrap()
            .contains("# exit code 3")
    );
}

#[test]
fn missing_trainer_file_fails_clearly_a_bound_game_that_is_not_running_is_never_started_into() {
    let f = fixture();
    let missing = f.entry("Missing.exe", Some(FAKE_APP_ID));
    assert!(matches!(
        f.launcher.resolve_plan(&missing, &scan_processes()),
        Err(LaunchError::TrainerNotFound { .. })
    ));
    // Starting a trainer before the game would make Steam's waitforexitandrun hang the game.
    assert_eq!(
        f.launcher
            .resolve_plan(&f.entry("Trainer.exe", Some(1)), &scan_processes()),
        Err(LaunchError::GameNotRunning)
    );
}
