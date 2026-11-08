//! The main window: header bar, card grid, toasts, and the glue between the
//! UI thread and the (blocking) core operations.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use kaka_core::launcher::LaunchError;
use kaka_core::library::Library;
use kaka_core::model::TrainerEntry;
use kaka_core::trainer_data::{CleanupError, TrainerDataCleanup};

use crate::{dialogs, i18n, settings, tr, trainer_dialog};

const DEFAULT_COVER: &[u8] = include_bytes!("../../../image/default_game_cover.jpg");
const LOGO: &[u8] = include_bytes!("../../../image/kaka.png");
const ORPHAN_POLL_SECONDS: u32 = 5;
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// One trainer card in the grid.
struct Card {
    id: String,
    root: gtk::Box,
    badge: gtk::Box,
    button: gtk::Button,
}

pub struct Ui {
    pub library: Arc<Library>,
    pub window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    title: adw::WindowTitle,
    add_button: gtk::Button,
    settings_button: gtk::Button,
    quit_button: gtk::Button,
    stack: gtk::Stack,
    empty: adw::StatusPage,
    flow: gtk::FlowBox,
    cards: RefCell<Vec<Card>>,
    /// What the grid currently shows, to rebuild it only when needed.
    rendered: RefCell<Option<(Vec<TrainerEntry>, String)>>,
    /// Entries with a launch or stop in progress.
    busy: RefCell<HashSet<String>>,
    covers: RefCell<HashMap<String, gdk::Texture>>,
    default_cover: gdk::Texture,
    orphan_poll: RefCell<Option<glib::SourceId>>,
    closing: Cell<bool>,
}

fn texture(bytes: &'static [u8]) -> gdk::Texture {
    gdk::Texture::from_bytes(&glib::Bytes::from_static(bytes)).expect("embedded image decodes")
}

fn icon_button(icon: &str) -> gtk::Button {
    gtk::Button::builder().icon_name(icon).build()
}

impl Ui {
    pub fn build(app: &adw::Application) {
        let library = Arc::new(Library::from_env());
        library.load();
        i18n::set_locale(kaka_core::language::language_to_locale(&library.language()));

        let logo = gtk::Image::from_paintable(Some(&texture(LOGO)));
        logo.set_pixel_size(24);
        let header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new("", "");
        header.set_title_widget(Some(&title));
        header.pack_start(&logo);
        let (add_button, settings_button, quit_button) = (
            icon_button("list-add-symbolic"),
            icon_button("emblem-system-symbolic"),
            icon_button("application-exit-symbolic"),
        );
        header.pack_end(&quit_button);
        header.pack_end(&settings_button);
        header.pack_end(&add_button);

        let flow = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .valign(gtk::Align::Start)
            .column_spacing(10)
            .row_spacing(20)
            .max_children_per_line(64)
            .margin_top(30)
            .margin_bottom(30)
            .margin_start(30)
            .margin_end(30)
            .build();
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&flow)
            .build();
        let empty = adw::StatusPage::builder()
            .icon_name("list-add-symbolic")
            .build();
        let stack = gtk::Stack::new();
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&scroller, Some("grid"));

        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&stack));
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&toasts));
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .default_width(1100)
            .default_height(720)
            .content(&toolbar)
            .build();

        let ui = Rc::new(Ui {
            library,
            window,
            toasts,
            title,
            add_button,
            settings_button,
            quit_button,
            stack,
            empty,
            flow,
            cards: RefCell::default(),
            rendered: RefCell::default(),
            busy: RefCell::default(),
            covers: RefCell::default(),
            default_cover: texture(DEFAULT_COVER),
            orphan_poll: RefCell::default(),
            closing: Cell::new(false),
        });
        ui.connect();
        ui.refresh();
        ui.window.present();
        // One scan at startup shows trainers an earlier session left running.
        ui.background(|l| l.start_monitoring(), |ui, ()| ui.refresh());
    }

    fn connect(self: &Rc<Self>) {
        // Core changes (also from launcher watcher threads) reach the UI thread through a channel.
        let (tx, rx) = async_channel::unbounded::<()>();
        self.library.set_listener(Some(Arc::new(move || {
            let _ = tx.try_send(());
        })));
        let ui = Rc::clone(self);
        glib::spawn_future_local(async move {
            while rx.recv().await.is_ok() {
                while rx.try_recv().is_ok() {} // coalesce bursts
                ui.refresh();
            }
        });

        let ui = Rc::clone(self);
        self.add_button.connect_clicked(move |_| ui.add_trainer());
        let ui = Rc::clone(self);
        self.settings_button
            .connect_clicked(move |_| settings::open(&ui));
        let window = self.window.clone();
        self.quit_button.connect_clicked(move |_| window.close());

        let ui = Rc::clone(self);
        self.window.connect_close_request(move |_| {
            ui.shutdown();
            glib::Propagation::Proceed
        });

        let add = gio::SimpleAction::new("add", None);
        let ui = Rc::clone(self);
        add.connect_activate(move |_, _| ui.add_trainer());
        self.window.add_action(&add);
        type Handler = fn(&Rc<Ui>, TrainerEntry);
        let per_entry: [(&str, Handler); 3] = [
            ("edit", Ui::edit_trainer),
            ("reset", Ui::reset_trainer_data),
            ("remove", Ui::remove_trainer),
        ];
        for (name, handler) in per_entry {
            let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
            let ui = Rc::clone(self);
            action.connect_activate(move |_, param| {
                let entry = param
                    .and_then(|v| v.get::<String>())
                    .and_then(|id| ui.library.trainer(&id));
                if let Some(entry) = entry {
                    handler(&ui, entry);
                }
            });
            self.window.add_action(&action);
        }
    }

    /// Runs `work` on a worker thread, then `done` on the UI thread.
    pub fn background<T: Send + 'static>(
        self: &Rc<Self>,
        work: impl FnOnce(&Library) -> T + Send + 'static,
        done: impl FnOnce(&Rc<Ui>, T) + 'static,
    ) {
        let library = Arc::clone(&self.library);
        let ui = Rc::clone(self);
        glib::spawn_future_local(async move {
            match gio::spawn_blocking(move || work(&library)).await {
                Ok(value) => done(&ui, value),
                Err(_) => eprintln!("A background task panicked"),
            }
        });
    }

    pub fn toast(&self, text: &str, seconds: u32) {
        self.toasts.add_toast(
            adw::Toast::builder()
                .title(text)
                .use_markup(false)
                .timeout(seconds)
                .build(),
        );
    }

    /// Brings the window in line with the library: texts, cards, running state.
    pub fn refresh(self: &Rc<Self>) {
        if self.closing.get() {
            return;
        }
        let language = self.library.language();
        let language_changed = self
            .rendered
            .borrow()
            .as_ref()
            .is_none_or(|(_, l)| *l != language);
        if language_changed {
            i18n::set_locale(kaka_core::language::language_to_locale(&language));
            self.retranslate();
        }

        let trainers = self.library.trainers();
        self.stack
            .set_visible_child_name(if trainers.is_empty() { "empty" } else { "grid" });
        let changed = language_changed
            || self
                .rendered
                .borrow()
                .as_ref()
                .is_none_or(|(t, _)| *t != trainers);
        if changed {
            self.rebuild_cards(&trainers);
            *self.rendered.borrow_mut() = Some((trainers, language));
        }
        for card in self.cards.borrow().iter() {
            self.update_card(card);
        }
        self.update_orphan_poll();
    }

    fn retranslate(&self) {
        let title = tr!("title");
        self.window.set_title(Some(&title));
        self.title.set_title(&title);
        self.add_button.set_tooltip_text(Some(&tr!("addTrainer")));
        self.settings_button.set_tooltip_text(Some(&tr!("setting")));
        self.quit_button.set_tooltip_text(Some(&tr!("quit")));
        let text = tr!("libraryEmpty");
        let (first, rest) = text.split_once('\n').unwrap_or((&text, ""));
        self.empty.set_title(first);
        self.empty
            .set_description(Some(rest.trim()).filter(|r| !r.is_empty()));
    }

    fn cover(&self, path: &str) -> gdk::Texture {
        if path.is_empty() {
            return self.default_cover.clone();
        }
        let mut covers = self.covers.borrow_mut();
        if let Some(texture) = covers.get(path) {
            return texture.clone();
        }
        match gdk::Texture::from_filename(path) {
            Ok(texture) => {
                covers.insert(path.to_owned(), texture.clone());
                texture
            }
            Err(_) => self.default_cover.clone(),
        }
    }

    fn rebuild_cards(self: &Rc<Self>, trainers: &[TrainerEntry]) {
        self.flow.remove_all();
        let cards: Vec<Card> = trainers.iter().map(|entry| self.card(entry)).collect();
        for card in &cards {
            self.flow.append(&card.root);
        }
        *self.cards.borrow_mut() = cards;
    }

    fn card(self: &Rc<Self>, entry: &TrainerEntry) -> Card {
        let picture = gtk::Picture::for_paintable(&self.cover(&entry.cover_image_path));
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_size_request(200, 300);
        picture.set_can_shrink(true);

        let dot = gtk::Label::new(Some("●"));
        dot.add_css_class("kaka-badge-dot");
        let badge = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        badge.append(&dot);
        badge.append(&gtk::Label::new(Some(&tr!("running"))));
        badge.add_css_class("kaka-badge");
        badge.set_halign(gtk::Align::Start);
        badge.set_valign(gtk::Align::End);
        badge.set_margin_start(8);
        badge.set_margin_bottom(8);

        let cover = gtk::Overlay::new();
        cover.set_child(Some(&picture));
        cover.add_overlay(&badge);
        cover.add_css_class("kaka-cover");
        cover.set_overflow(gtk::Overflow::Hidden);
        cover.set_cursor_from_name(Some("pointer"));
        cover.set_tooltip_text(Some(&entry.trainer_path));
        let click = gtk::GestureClick::new();
        let (ui, id) = (Rc::clone(self), entry.id.clone());
        click.connect_released(move |_, _, _, _| ui.launch(&id));
        cover.add_controller(click);

        let name = gtk::Label::builder()
            .label(&entry.name)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .justify(gtk::Justification::Center)
            .lines(2)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(20)
            .build();

        let button = gtk::Button::builder().hexpand(true).build();
        let (ui, id) = (Rc::clone(self), entry.id.clone());
        button.connect_clicked(move |_| ui.launch_or_stop(&id));

        let menu = gio::Menu::new();
        let target = entry.id.to_variant();
        let mut items = vec![(tr!("edit"), "win.edit")];
        if !entry.last_prefix.is_empty() {
            items.push((tr!("resetTrainerData"), "win.reset"));
        }
        items.push((tr!("remove"), "win.remove"));
        for (label, action) in items {
            let item = gio::MenuItem::new(Some(&label), None);
            item.set_action_and_target_value(Some(action), Some(&target));
            menu.append_item(&item);
        }
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .menu_model(&menu)
            .build();
        more.add_css_class("flat");

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions.append(&button);
        actions.append(&more);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.set_halign(gtk::Align::Center);
        root.set_size_request(200, -1);
        root.append(&cover);
        root.append(&name);
        root.append(&actions);
        Card {
            id: entry.id.clone(),
            root,
            badge,
            button,
        }
    }

    fn update_card(&self, card: &Card) {
        let running = self.library.is_running(&card.id);
        let busy = self.busy.borrow().contains(&card.id);
        card.badge.set_visible(running);
        card.button.set_sensitive(!busy);
        if busy {
            let spinner = gtk::Spinner::new();
            spinner.start();
            card.button.set_child(Some(&spinner));
        } else {
            card.button
                .set_label(&if running { tr!("stop") } else { tr!("launch") });
        }
        let (add, remove) = if running {
            ("destructive-action", "suggested-action")
        } else {
            ("suggested-action", "destructive-action")
        };
        card.button.remove_css_class(remove);
        card.button.add_css_class(add);
    }

    /// Trainers left running by an earlier session have no exit event: re-check
    /// them every few seconds, but only while there are any.
    fn update_orphan_poll(self: &Rc<Self>) {
        let needed = self.library.needs_orphan_poll() && !self.closing.get();
        let mut poll = self.orphan_poll.borrow_mut();
        if needed && poll.is_none() {
            let ui = Rc::clone(self);
            *poll = Some(glib::timeout_add_seconds_local(
                ORPHAN_POLL_SECONDS,
                move || {
                    ui.background(|l| drop(l.rescan()), |ui, ()| ui.refresh());
                    glib::ControlFlow::Continue
                },
            ));
        } else if !needed && let Some(source) = poll.take() {
            source.remove();
        }
    }

    fn launch_or_stop(self: &Rc<Self>, id: &str) {
        if self.library.is_running(id) {
            self.stop(id)
        } else {
            self.launch(id)
        }
    }

    fn set_busy(self: &Rc<Self>, id: &str, busy: bool) {
        if busy {
            self.busy.borrow_mut().insert(id.to_owned());
        } else {
            self.busy.borrow_mut().remove(id);
        }
        self.refresh();
    }

    fn launch(self: &Rc<Self>, id: &str) {
        let Some(entry) = self.library.trainer(id) else {
            return;
        };
        if self.busy.borrow().contains(id) {
            return;
        }
        if self.library.is_running(id) {
            self.toast(&tr!("trainerAlreadyRunning", name = entry.name.as_str()), 2);
            return;
        }
        self.set_busy(id, true);
        let (id, name) = (id.to_owned(), entry.name.clone());
        self.background(
            move |l| l.launch(&entry),
            move |ui, result| {
                ui.set_busy(&id, false);
                match result {
                    Ok(()) => ui.toast(&tr!("launched", name = name.as_str()), 2),
                    Err(LaunchError::AlreadyRunning { name }) => {
                        ui.toast(&tr!("trainerAlreadyRunning", name = name.as_str()), 2)
                    }
                    // Not modal for the card: it leaves its busy state while the dialog is open.
                    Err(error) => dialogs::launch_error(ui, &error),
                }
            },
        );
    }

    fn stop(self: &Rc<Self>, id: &str) {
        let Some(entry) = self.library.trainer(id) else {
            return;
        };
        if self.busy.borrow().contains(id) {
            return;
        }
        self.set_busy(id, true);
        let (id, name) = (id.to_owned(), entry.name.clone());
        self.background(
            move |l| l.stop(&entry, STOP_TIMEOUT),
            move |ui, stopped| {
                ui.set_busy(&id, false);
                let text = if stopped {
                    tr!("trainerStopped", name = name.as_str())
                } else {
                    tr!("trainerStopFailed", name = name.as_str())
                };
                ui.toast(&text, 3);
            },
        );
    }

    fn add_trainer(self: &Rc<Self>) {
        let ui = Rc::clone(self);
        trainer_dialog::open(self, None, move |entry| match ui.library.add(entry) {
            Ok(()) => ui.toast(&tr!("trainerAdded"), 2),
            Err(e) => ui.toast(&e.to_string(), 4),
        });
    }

    fn edit_trainer(self: &Rc<Self>, entry: TrainerEntry) {
        let ui = Rc::clone(self);
        trainer_dialog::open(self, Some(entry), move |updated| {
            match ui.library.update(updated) {
                Ok(()) => ui.toast(&tr!("trainerUpdated"), 2),
                Err(e) => ui.toast(&e.to_string(), 4),
            }
        });
    }

    fn remove_trainer(self: &Rc<Self>, entry: TrainerEntry) {
        let ui = Rc::clone(self);
        dialogs::confirm(
            self,
            None,
            &tr!("removeTrainerConfirm", name = entry.name.as_str()),
            &tr!("remove"),
            move || {
                if let Err(e) = ui.library.remove(&entry.id) {
                    ui.toast(&e.to_string(), 4);
                }
            },
        );
    }

    fn reset_trainer_data(self: &Rc<Self>, entry: TrainerEntry) {
        let paths: Vec<String> = TrainerDataCleanup::new(&entry.last_prefix)
            .targets()
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        let ui = Rc::clone(self);
        dialogs::confirm(
            self,
            Some(&tr!("resetTrainerData")),
            &tr!("resetTrainerDataConfirm", paths = paths.join("\n")),
            &tr!("reset"),
            move || {
                let entry = entry.clone();
                ui.background(
                    move |l| l.reset_trainer_data(&entry),
                    |ui, result| {
                        let text = match result {
                            Ok(count) => tr!("resetTrainerDataDone", count = count),
                            Err(CleanupError::PrefixInUse) => tr!("prefixInUse"),
                            Err(CleanupError::OutsidePrefix) => tr!("cleanupOutsidePrefix"),
                            Err(CleanupError::Io(e)) => e.to_string(),
                        };
                        ui.toast(&text, 4);
                    },
                );
            },
        );
    }

    /// Saves everything and stops this session's trainers; runs once.
    fn shutdown(&self) {
        if self.closing.replace(true) {
            return;
        }
        if let Some(source) = self.orphan_poll.borrow_mut().take() {
            source.remove();
        }
        self.library.set_listener(None);
        self.library.shutdown();
    }
}
