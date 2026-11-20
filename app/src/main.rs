// nix-pilot desktop client (D19: Rust + GTK4 + libadwaita). Thin startup and
// composition: load the catalogue, resolve the nix client, hand both to the UI
// shell. Product logic lives in the non-UI modules (model/search/store/install/
// updates) and the GTK surface in `ui`. Coding-style.md.
use glib::ExitCode;
use gtk4::prelude::*;
use log::{error, info};

mod apply;
mod catalogue;
mod config_value;
mod desired_state;
mod install;
mod model;
mod process;
mod search;
mod settings;
mod store;
mod ui;
mod updates;

// ------- index loading ----------------------------------------------------------
/// Resolve and load the catalogue entries, returning a plain-language error
/// when they cannot be obtained (surface: title-bar notice).
pub fn index_entries() -> (Vec<model::IndexEntry>, Option<String>) {
    let source = catalogue::resolve_source();
    match catalogue::load_entries(source.as_ref()) {
        Ok(entries) => (entries, None),
        Err(message) => (vec![], Some(message)),
    }
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("nix-pilot starting");

    if libadwaita::init().is_err() {
        error!("failed to initialise libadwaita");
        return ExitCode::FAILURE;
    }

    let app = gtk4::Application::builder()
        .application_id("dev.nixpilot.app")
        .build();

    app.connect_activate(|app| {
        let (entries, error) = index_entries();
        let nix = install::discover_nix();
        ui::run(app, entries, error, nix);
    });

    app.run()
}
