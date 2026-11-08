//! KaKa: runs user-supplied Windows trainers inside the Proton prefix of the
//! game that is running. Linux only, local only. GTK4 + libadwaita UI over
//! `kaka-core`.

mod dialogs;
mod i18n;
mod settings;
mod trainer_dialog;
mod ui;

use adw::prelude::*;
use gtk::glib;

pub const APP_ID: &str = "io.github.spaghetticodez.kaka";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SOURCE_URL: &str = "https://github.com/spaghetticodez/KaKa";
pub const UPSTREAM_URL: &str = "https://github.com/wyyadd/LaLa";

const CSS: &str = "
.kaka-cover { border-radius: 10px; }
.kaka-badge { background-color: alpha(black, 0.65); color: white; border-radius: 10px; padding: 3px 8px; font-size: 12px; }
.kaka-badge-dot { color: #3dffe9; }
";

fn main() -> glib::ExitCode {
    // X11 WM_CLASS comes from the program name; match the .desktop file / app id.
    glib::set_prgname(Some(APP_ID));
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| {
        // Dark unless the desktop asks for light, like the old navy theme.
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::PreferDark);
        let css = gtk::CssProvider::new();
        css.load_from_string(CSS);
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        gtk::Window::set_default_icon_name(APP_ID);
    });
    app.connect_activate(|app| {
        // A second start of KaKa activates the running instance.
        match app.active_window() {
            Some(window) => window.present(),
            None => ui::Ui::build(app),
        }
    });
    app.set_accels_for_action("win.add", &["<Ctrl>n"]);
    app.set_accels_for_action("window.close", &["<Ctrl>q", "<Ctrl>w"]);
    app.run()
}
