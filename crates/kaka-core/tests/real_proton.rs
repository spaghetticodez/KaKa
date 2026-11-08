//! Opt-in tests against a REAL Proton build (`#[ignore]`, not run in CI).
//! Wine's own cmd.exe stands in for games and trainers. Set KAKA_SLR to a
//! SteamLinuxRuntime_* directory to run the fake game inside the Steam Linux
//! Runtime container, like Steam does. Port of `real_proton_test.dart`.
//!
//! ```sh
//! S=~/.local/share/Steam
//! KAKA_REAL_PROTON="$S/steamapps/common/Proton - Experimental/proton" KAKA_STEAM_ROOT="$S" \
//! KAKA_SLR="$S/steamapps/common/SteamLinuxRuntime_4" cargo test -p kaka-core --test real_proton -- --ignored
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use kaka_core::launcher::{LaunchPlan, Launcher};
use kaka_core::model::TrainerEntry;
use kaka_core::proton::{ProcessInfo, TRAINER_ID_ENV, TRAINER_MARKER_ENV, scan_processes};
use rustix::process::{Pid, Signal, kill_process};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

/// The Proton script, or `None` (test skipped) when KAKA_REAL_PROTON is unset.
fn proton() -> Option<String> {
    let proton = env("KAKA_REAL_PROTON");
    if proton.is_empty() {
        eprintln!("skipped: set KAKA_REAL_PROTON to run");
        return None;
    }
    Some(proton)
}

fn copy_cmd(proton: &str, dir: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let cmd = Path::new(proton)
        .parent()
        .unwrap()
        .join("files/lib/wine/x86_64-windows/cmd.exe");
    let target = dir.join(name);
    fs::copy(cmd, &target).unwrap();
    target
}

fn in_prefix(prefix: &str) -> Vec<ProcessInfo> {
    scan_processes()
        .into_iter()
        .filter(|p| {
            p.env
                .get("STEAM_COMPAT_DATA_PATH")
                .is_some_and(|c| c == prefix)
        })
        .collect()
}

/// A Windows path (`C:\...` or `\\...`): the argv[0] Wine gives a Windows program.
fn is_windows_path(arg: &str) -> bool {
    let b = arg.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\')
        || arg.starts_with("\\\\")
}

fn marked(p: &ProcessInfo) -> bool {
    p.env.get(TRAINER_MARKER_ENV).is_some_and(|v| v == "1")
}

/// A Wine process (argv[0] is the Windows path of `exe`) running in `prefix`.
fn wine_running(prefix: &str, exe: &str, want_marked: Option<bool>) -> bool {
    in_prefix(prefix).iter().any(|p| {
        want_marked.is_none_or(|m| marked(p) == m)
            && p.cmdline.first().is_some_and(|a0| {
                is_windows_path(a0) && a0.to_lowercase().ends_with(&exe.to_lowercase())
            })
    })
}

fn wait_for(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while !condition() && Instant::now() < deadline {
        sleep(Duration::from_millis(500));
    }
    condition()
}

fn describe(prefix: &str) -> String {
    in_prefix(prefix)
        .iter()
        .map(|p| {
            format!(
                "  {}{}: {}",
                p.pid,
                if marked(p) { " [kaka]" } else { "" },
                p.cmdline.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn signal(pid: i32, sig: Signal) {
    if let Some(pid) = Pid::from_raw(pid) {
        let _ = kill_process(pid, sig);
    }
}

fn kill_prefix(prefix: &str) {
    for p in in_prefix(prefix) {
        signal(p.pid, Signal::TERM);
    }
    if !wait_for(Duration::from_secs(15), || in_prefix(prefix).is_empty()) {
        for p in in_prefix(prefix) {
            signal(p.pid, Signal::KILL);
        }
        wait_for(Duration::from_secs(5), || in_prefix(prefix).is_empty());
    }
}

fn only_log(dir: &Path) -> String {
    fs::read_dir(dir)
        .ok()
        .and_then(|mut entries| entries.next())
        .and_then(|e| e.ok())
        .and_then(|e| fs::read_to_string(e.path()).ok())
        .unwrap_or_default()
}

/// Kills everything in the prefix (and the game wrapper) even when an assertion fails.
struct Cleanup {
    prefix: String,
    game: Option<Child>,
    _tmp: tempfile::TempDir,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(game) = &mut self.game {
            let _ = game.kill();
            let _ = game.wait();
        }
        kill_prefix(&self.prefix);
    }
}

#[test]
#[ignore = "needs a real Proton build (KAKA_REAL_PROTON)"]
fn fresh_prefix_run_creates_it_the_trainer_runs_and_kaka_can_stop_it() {
    let Some(proton) = proton() else { return };
    let tmp = tempfile::Builder::new()
        .prefix("kaka_real_")
        .tempdir()
        .unwrap();
    let prefix = tmp
        .path()
        .join("compatdata/424242")
        .to_string_lossy()
        .into_owned();
    fs::create_dir_all(&prefix).unwrap();
    let trainer = copy_cmd(&proton, &tmp.path().join("trainers"), "cmd.exe");
    let logs = tmp.path().join("logs");
    let _cleanup = Cleanup {
        prefix: prefix.clone(),
        game: None,
        _tmp: tmp,
    };

    let entry = TrainerEntry::new("real", "Real Proton", trainer.to_string_lossy());
    let launcher = Launcher::new(&logs, Duration::from_secs(4));
    let plan = LaunchPlan {
        proton_script: proton,
        compat_data_path: prefix.clone(),
        client_install_path: env("KAKA_STEAM_ROOT"),
        verb: "run",
    };
    launcher.start(&entry, &plan).unwrap();

    let up = wait_for(Duration::from_secs(180), || {
        wine_running(&prefix, "cmd.exe", Some(true))
    });
    assert!(up, "log:\n{}", only_log(&logs));
    assert!(Path::new(&prefix).join("pfx/system.reg").exists());

    launcher.stop_all();
    assert!(wait_for(Duration::from_secs(20), || !wine_running(
        &prefix, "cmd.exe", None
    )));
}

#[test]
#[ignore = "needs a real Proton build (KAKA_REAL_PROTON)"]
fn running_game_kaka_detects_it_joins_its_prefix_and_stops_only_the_trainer() {
    let Some(proton) = proton() else { return };
    let (steam_root, slr) = (env("KAKA_STEAM_ROOT"), env("KAKA_SLR"));
    eprintln!(
        "Steam Linux Runtime: {}",
        if slr.is_empty() { "no" } else { &slr }
    );
    let tmp = tempfile::Builder::new()
        .prefix("kaka_real_game_")
        .tempdir()
        .unwrap();
    let prefix = tmp
        .path()
        .join("compatdata/434343")
        .to_string_lossy()
        .into_owned();
    fs::create_dir_all(&prefix).unwrap();
    let game_exe = copy_cmd(&proton, &tmp.path().join("game"), "FakeGame.exe");
    let trainer = copy_cmd(&proton, &tmp.path().join("trainers"), "FakeTrainer.exe");
    let logs = tmp.path().join("logs");

    let mut args: Vec<String> = Vec::new();
    if !slr.is_empty() {
        args.extend([
            format!("{slr}/_v2-entry-point"),
            "--verb=waitforexitandrun".into(),
            "--".into(),
        ]);
    }
    args.extend([
        proton.clone(),
        "waitforexitandrun".into(),
        game_exe.to_string_lossy().into_owned(),
    ]);
    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..])
        .env("STEAM_COMPAT_DATA_PATH", &prefix)
        .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &steam_root)
        .env("SteamAppId", "434343")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if !slr.is_empty() {
        let proton_dir = Path::new(&proton).parent().unwrap().display().to_string();
        cmd.env("STEAM_COMPAT_TOOL_PATHS", format!("{proton_dir}:{slr}"));
    }
    let game = cmd.spawn().unwrap();
    let _cleanup = Cleanup {
        prefix: prefix.clone(),
        game: Some(game),
        _tmp: tmp,
    };
    assert!(
        wait_for(Duration::from_secs(180), || wine_running(
            &prefix,
            "FakeGame.exe",
            None
        )),
        "game did not start"
    );

    let mut entry = TrainerEntry::new("game", "Fake Trainer", trainer.to_string_lossy());
    entry.steam_app_id = Some(434343);
    let launcher = Launcher::new(&logs, Duration::from_secs(4));
    let plan = launcher
        .resolve_plan(&entry, &scan_processes())
        .unwrap_or_else(|e| panic!("{e:?}\n{}", describe(&prefix)));
    assert_eq!(plan.compat_data_path, prefix);
    assert_eq!(
        plan.verb,
        "runinprefix",
        "proton={}\n{}",
        plan.proton_script,
        describe(&prefix)
    );

    launcher.start(&entry, &plan).unwrap();
    let up = wait_for(Duration::from_secs(60), || {
        wine_running(&prefix, "FakeTrainer.exe", Some(true))
    });
    eprintln!("processes in the prefix:\n{}", describe(&prefix));
    assert!(up, "log:\n{}", only_log(&logs));
    let wineservers = in_prefix(&prefix)
        .iter()
        .filter(|p| p.cmdline.iter().any(|a| a.ends_with("/wineserver")))
        .count();
    assert_eq!(
        wineservers, 1,
        "the trainer must join the game's wineserver, not start a second one"
    );

    // The trainer's own Wine process must carry the entry id, so KaKa can show
    // it as running and stop exactly this trainer.
    let is_trainer_wine = |p: &ProcessInfo| {
        p.env.get(TRAINER_ID_ENV) == Some(&entry.id) && p.cmdline.iter().any(|a| a.contains('\\'))
    };
    assert!(
        in_prefix(&prefix).iter().any(|p| is_trainer_wine(p)
            && p.cmdline
                .iter()
                .any(|a| a.to_lowercase().ends_with("faketrainer.exe"))),
        "{}",
        describe(&prefix)
    );
    assert_eq!(
        launcher.running_ids(std::slice::from_ref(&entry), &scan_processes()),
        [entry.id.clone()].into()
    );

    // Running state is event-driven: the Proton wrapper KaKa started must live
    // exactly as long as the trainer. Not shorter...
    sleep(Duration::from_secs(5));
    assert!(wine_running(&prefix, "FakeTrainer.exe", None));
    assert_eq!(
        launcher.live_ids(),
        [entry.id.clone()].into(),
        "the wrapper must not exit while the trainer runs"
    );

    // ...and not longer: closing only the trainer's own Wine process (like the
    // user closing the trainer window) must end the wrapper too.
    let trainer_wine: Vec<i32> = in_prefix(&prefix)
        .iter()
        .filter(|p| is_trainer_wine(p))
        .map(|p| p.pid)
        .collect();
    assert!(!trainer_wine.is_empty(), "{}", describe(&prefix));
    for pid in trainer_wine {
        signal(pid, Signal::TERM);
    }
    assert!(
        wait_for(Duration::from_secs(20), || launcher.live_ids().is_empty()),
        "the wrapper outlived its trainer:\n{}",
        describe(&prefix)
    );
    assert!(wait_for(Duration::from_secs(20), || !wine_running(
        &prefix,
        "FakeTrainer.exe",
        None
    )));
    assert!(
        launcher
            .running_ids(std::slice::from_ref(&entry), &scan_processes())
            .is_empty()
    );
    assert!(
        wine_running(&prefix, "FakeGame.exe", None),
        "stopping trainers must not touch the game"
    );
}
