//! Alert dialogs: confirmations, launch errors; opening local files.

use std::path::Path;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use kaka_core::launcher::LaunchError;

use crate::tr;
use crate::ui::Ui;

/// Response labels are mnemonics: a literal `_` must be doubled.
pub fn response_label(text: &str) -> String {
    text.replace('_', "__")
}

/// A selectable, wrapping text block (paths and messages can be copied).
pub fn selectable(text: &str) -> gtk::ScrolledWindow {
    let label = gtk::Label::builder()
        .label(text)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .selectable(true)
        .xalign(0.0)
        .build();
    gtk::ScrolledWindow::builder()
        .child(&label)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(360)
        .build()
}

/// Cancel / `action` (destructive) confirmation; `on_confirm` runs on `action`.
pub fn confirm(
    ui: &Rc<Ui>,
    heading: Option<&str>,
    body: &str,
    action: &str,
    on_confirm: impl Fn() + 'static,
) {
    let dialog = adw::AlertDialog::new(heading, None);
    dialog.set_extra_child(Some(&selectable(body)));
    dialog.add_response("cancel", &response_label(&tr!("cancel")));
    dialog.add_response("confirm", &response_label(action));
    dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.connect_response(Some("confirm"), move |_, _| on_confirm());
    dialog.present(Some(&ui.window));
}

/// User-facing text for a failed launch.
pub fn launch_error_text(error: &LaunchError) -> String {
    match error {
        LaunchError::TrainerNotFound { path } => {
            tr!("trainerPathNotFound", trainerPath = path.as_str())
        }
        LaunchError::GameNotRunning => tr!("gameNotRunning"),
        LaunchError::MultipleGames { games } => {
            let list = games
                .iter()
                .map(|g| format!("• {g}"))
                .collect::<Vec<_>>()
                .join("\n");
            tr!("multipleGamesRunning", games = list)
        }
        LaunchError::ProtonNotFound { prefix } => {
            tr!("protonPathNotFound", prefix = prefix.as_str())
        }
        LaunchError::GameStarting => tr!("gameStarting"),
        LaunchError::TrainerExited {
            code,
            output,
            log_path,
        } => {
            tr!(
                "trainerExited",
                code = code.to_string(),
                output = output.as_str(),
                logPath = log_path.as_str()
            )
        }
        LaunchError::AlreadyRunning { name } => tr!("trainerAlreadyRunning", name = name.as_str()),
        LaunchError::Spawn { message, .. } => message.clone(),
    }
}

/// Why the launch failed, with "Open log" when there is a log.
pub fn launch_error(ui: &Rc<Ui>, error: &LaunchError) {
    let dialog = adw::AlertDialog::new(Some(&tr!("launchFail")), None);
    dialog.set_extra_child(Some(&selectable(&launch_error_text(error))));
    if let Some(log) = error.log_path() {
        dialog.add_response("log", &response_label(&tr!("openLog")));
        let (window, log) = (ui.window.clone(), log.to_owned());
        dialog.connect_response(Some("log"), move |_, _| open_path(&window, Path::new(&log)));
    }
    dialog.add_response("ok", &response_label(&tr!("ok")));
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("ok");
    dialog.present(Some(&ui.window));
}

/// Opens a local file in its default application, or a folder in the file manager.
pub fn open_path(window: &impl IsA<gtk::Window>, path: &Path) {
    let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(path)));
    let shown = path.display().to_string();
    launcher.launch(Some(window), gio::Cancellable::NONE, move |result| {
        if let Err(e) = result {
            eprintln!("Could not open {shown}: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_launch_error_has_a_message_with_its_details() {
        let _guard = crate::i18n::test_lock();
        crate::i18n::set_locale("en");
        let cases = [
            (
                LaunchError::TrainerNotFound {
                    path: "/t/x.exe".into(),
                },
                "/t/x.exe",
            ),
            (LaunchError::GameNotRunning, "Start the game first"),
            (
                LaunchError::MultipleGames {
                    games: vec!["a.exe (AppID 1)".into(), "b.exe".into()],
                },
                "• a.exe (AppID 1)\n• b.exe",
            ),
            (
                LaunchError::ProtonNotFound {
                    prefix: "/c/7".into(),
                },
                "Prefix: /c/7",
            ),
            (LaunchError::GameStarting, "still starting"),
            (
                LaunchError::TrainerExited {
                    code: -15,
                    output: "boom".into(),
                    log_path: "/l/x.log".into(),
                },
                "(exit code -15)",
            ),
            (
                LaunchError::AlreadyRunning {
                    name: "Elden".into(),
                },
                "Elden is already running.",
            ),
            (
                LaunchError::Spawn {
                    message: "No such file".into(),
                    log_path: None,
                },
                "No such file",
            ),
        ];
        for (error, expected) in cases {
            let text = launch_error_text(&error);
            assert!(text.contains(expected), "{error:?}: {text}");
        }
        let exited = launch_error_text(&LaunchError::TrainerExited {
            code: 3,
            output: "boom".into(),
            log_path: "/l/x.log".into(),
        });
        assert!(
            exited.contains("boom") && exited.ends_with("Full log: /l/x.log"),
            "{exited}"
        );
    }

    #[test]
    fn response_labels_keep_underscores() {
        assert_eq!(response_label("my_game.exe"), "my__game.exe");
    }
}
