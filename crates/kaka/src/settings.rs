//! Settings (`AdwPreferencesDialog`): language, Steam path, logs, About.

use std::fs;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use kaka_core::language::LANGUAGE_OPTIONS;

use crate::dialogs::open_path;
use crate::trainer_dialog::pick_folder;
use crate::ui::Ui;
use crate::{APP_ID, SOURCE_URL, UPSTREAM_URL, VERSION, tr};

fn row(title: &str, subtitle: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .use_markup(false)
        .build()
}

fn suffix_button(label: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(label)
        .valign(gtk::Align::Center)
        .build()
}

pub fn open(ui: &Rc<Ui>) {
    let dialog = adw::PreferencesDialog::builder()
        .title(tr!("setting"))
        .search_enabled(false)
        .build();
    let group = adw::PreferencesGroup::new();

    // Language: switching re-translates the window; the dialog reopens in the new language.
    let languages = gtk::StringList::new(&LANGUAGE_OPTIONS);
    let current = LANGUAGE_OPTIONS
        .iter()
        .position(|l| *l == ui.library.language())
        .unwrap_or(0);
    let language = adw::ComboRow::builder()
        .title(tr!("language"))
        .model(&languages)
        .selected(current as u32)
        .build();
    {
        let (ui, dialog) = (ui.clone(), dialog.clone());
        language.connect_selected_notify(move |row| {
            let Some(choice) = LANGUAGE_OPTIONS.get(row.selected() as usize) else {
                return;
            };
            if let Err(e) = ui.library.set_language(choice) {
                dialog.add_toast(
                    adw::Toast::builder()
                        .title(e.to_string())
                        .use_markup(false)
                        .build(),
                );
                return;
            }
            ui.refresh();
            let (ui, dialog) = (ui.clone(), dialog.clone());
            glib::idle_add_local_once(move || {
                dialog.close();
                open(&ui);
            });
        });
    }
    group.add(&language);

    // Steam path: empty = auto-detect.
    let steam = row(&tr!("steamPath"), "");
    let reset = suffix_button(&tr!("reset"));
    reset.add_css_class("flat");
    let set = suffix_button(&tr!("set"));
    steam.add_suffix(&reset);
    steam.add_suffix(&set);
    let sync_steam = {
        let (ui, steam, reset) = (ui.clone(), steam.clone(), reset.clone());
        Rc::new(move || {
            let path = ui.library.steam_path();
            steam.set_subtitle(&if path.is_empty() {
                tr!("steamPathAuto")
            } else {
                path.clone()
            });
            reset.set_visible(!path.is_empty());
        })
    };
    sync_steam();
    {
        let (ui, dialog, sync) = (ui.clone(), dialog.clone(), sync_steam.clone());
        set.connect_clicked(move |_| {
            let (ui2, dialog, sync) = (ui.clone(), dialog.clone(), sync.clone());
            pick_folder(&ui.window, &tr!("steamPath"), move |path| {
                let text = match ui2.library.set_steam_path(&path) {
                    Ok(()) => tr!("steamPathSet"),
                    Err(e) => e.to_string(),
                };
                sync();
                dialog.add_toast(adw::Toast::builder().title(text).use_markup(false).build());
            });
        });
    }
    {
        let (ui, sync) = (ui.clone(), sync_steam.clone());
        reset.connect_clicked(move |_| {
            if let Err(e) = ui.library.set_steam_path("") {
                eprintln!("Could not save the settings: {e}");
            }
            sync();
        });
    }
    group.add(&steam);

    // Trainer logs.
    let log_dir = ui.library.log_dir();
    let logs = row(&tr!("trainerLogs"), &log_dir.display().to_string());
    logs.set_subtitle_selectable(true);
    let open_folder = suffix_button(&tr!("openFolder"));
    {
        let window = ui.window.clone();
        open_folder.connect_clicked(move |_| {
            // Created on demand: no trainer may have run yet.
            if let Err(e) = fs::create_dir_all(&log_dir) {
                eprintln!("Could not create {}: {e}", log_dir.display());
            }
            open_path(&window, &log_dir);
        });
    }
    logs.add_suffix(&open_folder);
    group.add(&logs);

    // About.
    let about = row(&tr!("about"), &format!("KaKa {VERSION}"));
    about.set_activatable(true);
    about.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    {
        let dialog = dialog.clone();
        about.connect_activated(move |_| about_dialog().present(Some(&dialog)));
    }
    let info = adw::PreferencesGroup::new();
    info.add(&about);

    let page = adw::PreferencesPage::new();
    page.add(&group);
    page.add(&info);
    dialog.add(&page);
    dialog.present(Some(&ui.window));
}

fn about_dialog() -> adw::AboutDialog {
    let about = adw::AboutDialog::builder()
        .application_name("KaKa")
        .application_icon(APP_ID)
        .version(VERSION)
        .developer_name("spaghetticodez")
        .website(SOURCE_URL)
        .issue_url(format!("{SOURCE_URL}/issues"))
        .license_type(gtk::License::Agpl30)
        .comments(tr!("title"))
        .build();
    about.add_link(&tr!("sourceCode"), SOURCE_URL);
    about.add_credit_section(Some(&tr!("basedOn")), &[&format!("wyyadd {UPSTREAM_URL}")]);
    about
}
