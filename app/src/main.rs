// nix-pilot desktop client (D19: Rust + GTK4 + libadwaita). Thin startup and
// composition: load the catalogue, resolve the nix client, hand both to the UI
// shell. Product logic lives in the non-UI modules (model/search/store/install/
// updates) and the GTK surface in `ui`. Coding-style.md.
use glib::ExitCode;
use gtk4::prelude::*;
use std::fmt;
use std::path::Path;

mod apply;
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

use model::IndexEntry;

// ------- index loading ----------------------------------------------------------
fn resolve_index_path() -> Option<String> {
    if let Some(p) = std::env::args().nth(1) {
        return Some(p);
    }
    for cand in [
        "experiments/catalogue/results/catalogue-index.json", // repo root cwd
        "results/catalogue-index.json",                       // catalogue cwd
        "../experiments/catalogue/results/catalogue-index.json",
    ] {
        if std::path::Path::new(cand).exists() {
            return Some(cand.to_string());
        }
    }
    None
}

#[derive(Debug)]
enum IndexLoadError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Parse(serde_json::Error),
}

impl fmt::Display for IndexLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "cannot read {path}: {source}"),
            Self::Parse(source) => write!(f, "cannot parse catalogue index: {source}"),
        }
    }
}

impl std::error::Error for IndexLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse(source) => Some(source),
        }
    }
}

fn load_index(path: &Path) -> Result<Vec<IndexEntry>, IndexLoadError> {
    let raw = std::fs::read_to_string(path).map_err(|source| IndexLoadError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let idx: model::CatalogueIndex = serde_json::from_str(&raw).map_err(IndexLoadError::Parse)?;
    Ok(idx.entries)
}

fn index_entries() -> (Vec<IndexEntry>, Option<String>) {
    match resolve_index_path() {
        Some(p) => match load_index(Path::new(&p)) {
            Ok(entries) => (entries, None),
            Err(e) => (vec![], Some(e.to_string())),
        },
        None => (
            vec![],
            Some("catalogue index not found — pass a path or run from the repo root".into()),
        ),
    }
}

fn main() -> ExitCode {
    if libadwaita::init().is_err() {
        eprintln!("failed to initialise libadwaita");
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
