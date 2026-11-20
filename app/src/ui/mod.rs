//! GTK UI surface: the app shell (window, header, store list + search) and its
//! focused components (detail + install, updates review dialog). Pure logic and
//! the nix adapter live in the sibling non-UI modules; this layer translates
//! user actions into store/updates/install actions and renders their outcomes.
pub mod detail;
pub mod settings_page;
pub mod updates_page;
pub mod widgets;
pub mod your_system;

mod shell;

use crate::model::IndexEntry;

/// Build and present the application window (thin composition entry point).
/// `entries` is the loaded catalogue; `nix` is the resolved client used for
/// install/update checks. The shell owns the window and all widgets.
pub fn run(
    app: &gtk4::Application,
    entries: Vec<IndexEntry>,
    load_error: Option<String>,
    nix: Option<std::path::PathBuf>,
) {
    shell::run(app, entries, load_error, nix);
}
