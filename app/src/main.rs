//! Desktop entry point for nix-pilot using Rust + GTK4 + libadwaita.
//!
//! Startup initializes GTK, then supplies the UI shell with catalogue entries
//! and the discovered Nix client. Product logic lives in the non-UI modules;
//! [`ui`] composes the GTK surfaces from those domain and adapter APIs.
use glib::ExitCode;
use gtk4::prelude::*;
use log::{error, info};

mod account;
mod catalogue;
mod config_source;
mod config_value;
mod desired_state;
mod install;
mod model;
mod process;
mod search;
mod setting_value;
mod settings;
mod setup;
mod store;
mod system;
mod try_run;
mod ui;
mod updates;

/// Resolve the configured catalogue source and load its entries.
///
/// On failure, returns an empty entry list and a plain-language message for
/// the shell to show as a title-bar notice; catalogue failure does not abort
/// application startup.
pub fn index_entries() -> (Vec<model::IndexEntry>, Option<String>) {
    let source = catalogue::resolve_source();
    match catalogue::load_entries(source.as_ref()) {
        Ok(entries) => (entries, None),
        Err(message) => (vec![], Some(message)),
    }
}

/// Initialize logging and the GTK application, then run its event loop.
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
