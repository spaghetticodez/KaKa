//! Add / edit a trainer (`AdwDialog`), with "Use running game".

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use kaka_core::game_info::GameSuggestion;
use kaka_core::model::TrainerEntry;

use crate::dialogs::response_label;
use crate::tr;
use crate::ui::Ui;

const MAX_NAME_CHARS: i32 = 80;

#[derive(Default)]
struct Paths {
    trainer: String,
    cover: String,
}

fn file_name(path: &str) -> String {
    if path.is_empty() {
        "—".to_owned()
    } else {
        Path::new(path)
            .file_name()
            .map_or(path.into(), |n| n.to_string_lossy().into_owned())
    }
}

fn row(title: &str) -> adw::ActionRow {
    // Rows show user paths: never interpret them as Pango markup.
    let row = adw::ActionRow::builder()
        .title(title)
        .use_markup(false)
        .build();
    row.set_subtitle_selectable(false);
    row
}

fn pick_button() -> gtk::Button {
    gtk::Button::builder().valign(gtk::Align::Center).build()
}

/// The validated entry, or the message to show.
pub fn validate(
    initial: Option<&TrainerEntry>,
    name: &str,
    trainer_path: &str,
    cover: &str,
    app_id: &str,
) -> Result<TrainerEntry, String> {
    let name = name.trim();
    if name.is_empty() || trainer_path.is_empty() {
        return Err(tr!("requiredFields"));
    }
    let app_id = app_id.trim();
    let steam_app_id = if app_id.is_empty() {
        None
    } else {
        match app_id.parse::<u64>() {
            Ok(n) if n > 0 => Some(n),
            _ => return Err(tr!("invalidAppId")),
        }
    };
    if !Path::new(trainer_path).is_file() {
        return Err(tr!("trainerPathNotFound", trainerPath = trainer_path));
    }
    let mut entry = TrainerEntry::new(
        initial.map_or_else(TrainerEntry::new_id, |e| e.id.clone()),
        name,
        trainer_path,
    );
    entry.cover_image_path = cover.to_owned();
    entry.steam_app_id = steam_app_id;
    // Editing keeps where the trainer last ran (for "Reset trainer data").
    entry.last_prefix = initial.map(|e| e.last_prefix.clone()).unwrap_or_default();
    Ok(entry)
}

pub fn open(ui: &Rc<Ui>, initial: Option<TrainerEntry>, on_save: impl Fn(TrainerEntry) + 'static) {
    let paths = Rc::new(RefCell::new(Paths {
        trainer: initial
            .as_ref()
            .map(|e| e.trainer_path.clone())
            .unwrap_or_default(),
        cover: initial
            .as_ref()
            .map(|e| e.cover_image_path.clone())
            .unwrap_or_default(),
    }));
    let title = if initial.is_some() {
        tr!("editTrainer")
    } else {
        tr!("addTrainer")
    };
    let dialog = adw::Dialog::builder()
        .title(&title)
        .content_width(500)
        .build();

    let cancel = gtk::Button::with_label(&tr!("cancel"));
    let save = gtk::Button::with_label(&tr!("save"));
    save.add_css_class("suggested-action");
    let header = adw::HeaderBar::builder()
        .show_start_title_buttons(false)
        .show_end_title_buttons(false)
        .build();
    header.pack_start(&cancel);
    header.pack_end(&save);

    // Trainer file and "Use running game".
    let trainer_row = row(&tr!("trainerFile"));
    let trainer_button = pick_button();
    trainer_row.add_suffix(&trainer_button);
    let running_row = row(&tr!("useRunningGame"));
    running_row.add_prefix(&gtk::Image::from_icon_name("input-gaming-symbolic"));
    running_row.set_activatable(true);
    let files = adw::PreferencesGroup::new();
    files.add(&trainer_row);
    files.add(&running_row);

    // Name, cover.
    let name_row = adw::EntryRow::builder().title(tr!("gameName")).build();
    if let Some(text) = name_row
        .delegate()
        .and_then(|d| d.downcast::<gtk::Text>().ok())
    {
        text.set_max_length(MAX_NAME_CHARS);
    }
    let cover_row = row(&tr!("gameCover"));
    let clear_cover = gtk::Button::builder()
        .icon_name("edit-clear-symbolic")
        .valign(gtk::Align::Center)
        .build();
    clear_cover.add_css_class("flat");
    let cover_button = pick_button();
    cover_row.add_suffix(&clear_cover);
    cover_row.add_suffix(&cover_button);
    let game = adw::PreferencesGroup::new();
    game.add(&name_row);
    game.add(&cover_row);

    // AppID (digits only).
    let app_id_row = adw::EntryRow::builder()
        .title(tr!("steamAppId"))
        .input_purpose(gtk::InputPurpose::Digits)
        .build();
    app_id_row.connect_changed(|row| {
        let text = row.text();
        if text.chars().any(|c| !c.is_ascii_digit()) {
            row.set_text(
                &text
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>(),
            );
        }
    });
    let app_id_group = adw::PreferencesGroup::builder()
        .description(tr!("steamAppIdHelp"))
        .build();
    app_id_group.add(&app_id_row);

    let error = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .visible(false)
        .selectable(true)
        .build();
    error.add_css_class("error");
    let error_group = adw::PreferencesGroup::new();
    error_group.add(&error);

    if let Some(e) = &initial {
        name_row.set_text(&e.name);
        if let Some(id) = e.steam_app_id {
            app_id_row.set_text(&id.to_string());
        }
    }

    let page = adw::PreferencesPage::new();
    for group in [&files, &game, &app_id_group, &error_group] {
        page.add(group);
    }
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    dialog.set_child(Some(&toolbar));

    // Shows the current paths on the rows.
    let sync: Rc<dyn Fn()> = {
        let (paths, trainer_row, trainer_button, cover_row, cover_button, clear_cover) = (
            paths.clone(),
            trainer_row.clone(),
            trainer_button.clone(),
            cover_row.clone(),
            cover_button.clone(),
            clear_cover.clone(),
        );
        Rc::new(move || {
            let p = paths.borrow();
            trainer_row.set_subtitle(&file_name(&p.trainer));
            trainer_row.set_tooltip_text(
                Some(&p.trainer)
                    .filter(|t| !t.is_empty())
                    .map(String::as_str),
            );
            trainer_button.set_label(&if p.trainer.is_empty() {
                tr!("select")
            } else {
                tr!("change")
            });
            cover_row.set_subtitle(&file_name(&p.cover));
            cover_row
                .set_tooltip_text(Some(&p.cover).filter(|c| !c.is_empty()).map(String::as_str));
            cover_button.set_label(&if p.cover.is_empty() {
                tr!("select")
            } else {
                tr!("change")
            });
            clear_cover.set_visible(!p.cover.is_empty());
        })
    };
    sync();
    let show_error = {
        let error = error.clone();
        move |text: Option<String>| {
            error.set_visible(text.is_some());
            error.set_label(text.as_deref().unwrap_or_default());
        }
    };

    {
        let (window, paths, sync, name_row) = (
            ui.window.clone(),
            paths.clone(),
            sync.clone(),
            name_row.clone(),
        );
        trainer_button.connect_clicked(move |_| {
            let filter = gtk::FileFilter::new();
            filter.set_name(Some(&tr!("trainerFile")));
            filter.add_suffix("exe"); // case-insensitive
            let (paths, sync, name_row) = (paths.clone(), sync.clone(), name_row.clone());
            pick_file(&window, &tr!("trainerFile"), filter, move |path| {
                if name_row.text().trim().is_empty()
                    && let Some(stem) = Path::new(&path).file_stem()
                {
                    name_row.set_text(&stem.to_string_lossy());
                }
                paths.borrow_mut().trainer = path;
                sync();
            });
        });
    }
    {
        let (window, paths, sync) = (ui.window.clone(), paths.clone(), sync.clone());
        cover_button.connect_clicked(move |_| {
            let filter = gtk::FileFilter::new();
            filter.set_name(Some(&tr!("gameCover")));
            filter.add_pixbuf_formats();
            let (paths, sync) = (paths.clone(), sync.clone());
            pick_file(&window, &tr!("gameCover"), filter, move |path| {
                paths.borrow_mut().cover = path;
                sync();
            });
        });
    }
    {
        let (paths, sync) = (paths.clone(), sync.clone());
        clear_cover.connect_clicked(move |_| {
            paths.borrow_mut().cover.clear();
            sync();
        });
    }
    {
        let (ui2, paths, sync, name_row, app_id_row, show_error) = (
            ui.clone(),
            paths.clone(),
            sync.clone(),
            name_row.clone(),
            app_id_row.clone(),
            show_error.clone(),
        );
        running_row.connect_activated(move |row| {
            row.set_sensitive(false);
            let (row, paths, sync, name_row, app_id_row, show_error) = (
                row.clone(),
                paths.clone(),
                sync.clone(),
                name_row.clone(),
                app_id_row.clone(),
                show_error.clone(),
            );
            ui2.background(
                |l| l.running_games(),
                move |ui, games| {
                    row.set_sensitive(true);
                    let not_running = show_error.clone();
                    let apply = move |game: &GameSuggestion| {
                        show_error(None);
                        name_row.set_text(&game.name);
                        app_id_row
                            .set_text(&game.app_id.map(|id| id.to_string()).unwrap_or_default());
                        if let Some(cover) = &game.cover_path {
                            paths.borrow_mut().cover = cover.clone();
                        }
                        sync();
                    };
                    match games.as_slice() {
                        [] => not_running(Some(tr!("gameNotRunning"))),
                        [game] => apply(game),
                        _ => choose_game(ui, games, apply),
                    }
                },
            );
        });
    }
    {
        let dialog = dialog.clone();
        cancel.connect_clicked(move |_| {
            dialog.close();
        });
    }
    {
        let dialog2 = dialog.clone();
        save.connect_clicked(move |_| {
            let p = paths.borrow();
            match validate(
                initial.as_ref(),
                &name_row.text(),
                &p.trainer,
                &p.cover,
                &app_id_row.text(),
            ) {
                Ok(entry) => {
                    on_save(entry);
                    dialog2.close();
                }
                Err(message) => show_error(Some(message)),
            }
        });
    }
    dialog.present(Some(&ui.window));
}

/// "Which game?" when several Proton games run.
fn choose_game(ui: &Rc<Ui>, games: Vec<GameSuggestion>, apply: impl Fn(&GameSuggestion) + 'static) {
    let dialog = adw::AlertDialog::new(Some(&tr!("chooseGame")), None);
    for (i, game) in games.iter().enumerate() {
        let label = match game.app_id {
            Some(id) => format!("{} (AppID {id})", game.name),
            None => game.name.clone(),
        };
        dialog.add_response(&format!("game{i}"), &response_label(&label));
    }
    dialog.add_response("cancel", &response_label(&tr!("cancel")));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, response| {
        if let Some(game) = response
            .strip_prefix("game")
            .and_then(|i| i.parse::<usize>().ok())
            .and_then(|i| games.get(i))
        {
            apply(game);
        }
    });
    dialog.present(Some(&ui.window));
}

/// Native file chooser (portal when available); `done` gets a local path.
fn pick_file(
    window: &adw::ApplicationWindow,
    title: &str,
    filter: gtk::FileFilter,
    done: impl FnOnce(String) + 'static,
) {
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let chooser = gtk::FileDialog::builder()
        .title(title)
        .modal(true)
        .filters(&filters)
        .default_filter(&filter)
        .build();
    chooser.open(Some(window), gio::Cancellable::NONE, move |result| {
        if let Some(path) = result.ok().and_then(|f| f.path()) {
            done(path.to_string_lossy().into_owned());
        }
    });
}

/// Folder chooser; `done` gets a local path.
pub fn pick_folder(
    window: &adw::ApplicationWindow,
    title: &str,
    done: impl FnOnce(String) + 'static,
) {
    let chooser = gtk::FileDialog::builder().title(title).modal(true).build();
    chooser.select_folder(Some(window), gio::Cancellable::NONE, move |result| {
        if let Some(path) = result.ok().and_then(|f| f.path()) {
            done(path.to_string_lossy().into_owned());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_rules_match_the_flutter_dialog() {
        let _guard = crate::i18n::test_lock();
        crate::i18n::set_locale("en");
        let tmp = std::env::temp_dir().join(format!("kaka_dialog_{}.exe", std::process::id()));
        std::fs::write(&tmp, "MZ").unwrap();
        let path = tmp.to_str().unwrap();

        assert_eq!(
            validate(None, "  ", path, "", ""),
            Err(tr!("requiredFields"))
        );
        assert_eq!(
            validate(None, "Game", "", "", ""),
            Err(tr!("requiredFields"))
        );
        assert_eq!(
            validate(None, "Game", path, "", "0"),
            Err(tr!("invalidAppId"))
        );
        assert_eq!(
            validate(None, "Game", "/missing/t.exe", "", ""),
            Err(tr!("trainerPathNotFound", trainerPath = "/missing/t.exe"))
        );

        let added = validate(None, " Game ", path, "/c.png", "1245620").unwrap();
        assert_eq!(
            (
                added.name.as_str(),
                added.steam_app_id,
                added.cover_image_path.as_str()
            ),
            ("Game", Some(1245620), "/c.png")
        );
        assert_eq!(added.id.len(), 16);

        let mut old = TrainerEntry::new("keep", "Old", path);
        old.last_prefix = "/compat/1".into();
        let edited = validate(Some(&old), "New", path, "", "").unwrap();
        assert_eq!(
            (
                edited.id.as_str(),
                edited.last_prefix.as_str(),
                edited.steam_app_id
            ),
            ("keep", "/compat/1", None)
        );
        std::fs::remove_file(tmp).unwrap();
    }
}
