//! First-run integration engine (config-authority.md, "Install and first
//! run"): detect the user's config, write the app-owned `nix-pilot/` files,
//! show the copy-paste snippet, parse-verify, and remove the app's footprint.
//! Pure and unit-testable; the GUI and any future CLI share it.
//!
//! The module travels as a copy in the config tree (embedded in the app), so
//! flake and classic layouts import the same `./nix-pilot/module.nix` and the
//! module version always matches the app.

use std::path::{Path, PathBuf};

use crate::config_source::{self, ConfigKind, ConfigRoot};
use crate::desired_state::DesiredDoc;
use crate::system::state_file;

/// The embedded NixOS module source shipped with this app.
pub const MODULE_TEXT: &str = include_str!("../nixos-module/nix-pilot.nix");

/// File name of the module copy inside the app-owned directory.
pub const MODULE_FILE: &str = "module.nix";

/// The reference that proves D33 integration: the config imports the app's
/// generated state module. Deliberately narrower than any `nix-pilot`
/// mention, so stale wiring (a pinned module input, `services.nix-pilot`
/// enablement) never counts as integrated.
const INTEGRATION_MARKER: &str = "nix-pilot/state.nix";

/// Resolve the user's configuration root, if any (see
/// [`config_source::resolve_config_root`]).
pub fn resolve_root() -> Option<ConfigRoot> {
    config_source::resolve_config_root()
}

/// Write the app-owned `nix-pilot/` files into the config tree: the module
/// copy and the rendered state module. Returns the state module path.
///
/// # Errors
/// Returns an I/O error for directory or file operations, or an
/// `InvalidData` I/O error if the desired state cannot be rendered.
pub fn write_app_files(root: &ConfigRoot, doc: &DesiredDoc) -> std::io::Result<PathBuf> {
    // The state write creates the directory; the module copy must follow.
    // (fs::write does not create parent directories.)
    let state = state_file::write_state_file(&root.path, doc)?;
    let directory = root.path.join(state_file::STATE_DIR);
    std::fs::write(directory.join(MODULE_FILE), MODULE_TEXT)?;
    Ok(state)
}

/// Whether the user's configuration already references nix-pilot. Scans the
/// `.nix` files under the root (excluding the app-owned directory) for the
/// integration marker, so the app's own files never count as a reference.
pub fn is_integrated(root: &ConfigRoot) -> bool {
    walk_nix_files(&root.path).into_iter().any(|file| {
        std::fs::read_to_string(&file)
            .map(|text| text.contains(INTEGRATION_MARKER))
            .unwrap_or(false)
    })
}

/// The copy-paste snippet connecting the user's config to nix-pilot, for the
/// detected config kind. The lines are exactly what the app wrote or would
/// write into the app-owned directory.
pub fn snippet_for(kind: ConfigKind) -> String {
    match kind {
        ConfigKind::Flake => "\
# In the NixOS module list of your host configuration, add:
modules = [
  ./nix-pilot/module.nix
  ./nix-pilot/state.nix
];

# And anywhere in that same configuration:
services.nix-pilot.enable = true;"
            .into(),
        ConfigKind::Classic => "\
# In /etc/nixos/configuration.nix, add:
imports = [ ./nix-pilot/module.nix ./nix-pilot/state.nix ];
services.nix-pilot.enable = true;"
            .into(),
    }
}

/// Parse-check the root's entry file without evaluating anything. A flake
/// root parses `flake.nix`; a classic root parses `configuration.nix`.
///
/// # Errors
/// Returns an error string when `nix-instantiate` is unavailable or the file
/// does not parse.
pub fn verify_parse(root: &ConfigRoot) -> Result<(), String> {
    let entry = match root.kind {
        ConfigKind::Flake => root.path.join("flake.nix"),
        ConfigKind::Classic => root.path.join("configuration.nix"),
    };
    let output = std::process::Command::new("nix-instantiate")
        .arg("--parse")
        .arg(&entry)
        .output()
        .map_err(|_| "nix-instantiate is not available on this system".to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} does not parse: {}",
            entry.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Remove the app-owned `nix-pilot/` directory (module, state, overlay). The
/// user's import line and `services.nix-pilot.enable` stay in their config —
/// the returned text tells them exactly what to remove.
///
/// # Errors
/// Returns an I/O error when the directory cannot be removed.
pub fn uninstall(root: &ConfigRoot) -> std::io::Result<String> {
    let directory = root.path.join(state_file::STATE_DIR);
    if directory.exists() {
        std::fs::remove_dir_all(&directory)?;
    }
    Ok("\
nix-pilot files were removed from your configuration.

To finish removing nix-pilot, delete these lines from your configuration:
  - the import or module entry for ./nix-pilot/module.nix (and ./nix-pilot/state.nix)
  - services.nix-pilot.enable = true;"
        .into())
}

/// All `.nix` files under `root`, excluding the app-owned directory and git
/// metadata, in walk order (bounded: a configuration repo is small).
fn walk_nix_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".git" || name == state_file::STATE_DIR {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if name.ends_with(".nix") {
                files.push(path);
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired_state::PackageScope;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("nix-pilot-setup-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, name: &str, text: &str) {
            std::fs::write(self.0.join(name), text).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn module_text_is_the_shipped_module() {
        assert!(MODULE_TEXT.starts_with("# nix-pilot NixOS module"));
        assert!(MODULE_TEXT.contains("services.nix-pilot"));
        // Brace balance is a cheap sanity proxy; the verify gate parses Nix.
        assert_eq!(
            MODULE_TEXT.matches('{').count(),
            MODULE_TEXT.matches('}').count()
        );
    }

    #[test]
    fn snippet_matches_the_owned_file_names() {
        let flake = snippet_for(ConfigKind::Flake);
        assert!(flake.contains("./nix-pilot/module.nix"));
        assert!(flake.contains("./nix-pilot/state.nix"));
        assert!(flake.contains("services.nix-pilot.enable = true;"));

        let classic = snippet_for(ConfigKind::Classic);
        assert!(classic.contains("imports = [ ./nix-pilot/module.nix ./nix-pilot/state.nix ];"));
        assert!(classic.contains("services.nix-pilot.enable = true;"));
    }

    #[test]
    fn write_app_files_places_module_and_state_side_by_side() {
        let fixture = Fixture::new("files");
        let root = ConfigRoot {
            path: fixture.0.clone(),
            kind: ConfigKind::Classic,
        };
        let mut doc = DesiredDoc::current();
        doc.add_package("alice", "hello".into(), PackageScope::System);

        let state_path = write_app_files(&root, &doc).unwrap();
        assert!(state_path.ends_with("nix-pilot/state.nix"));
        let module = fixture.0.join(state_file::STATE_DIR).join(MODULE_FILE);
        assert_eq!(std::fs::read_to_string(&module).unwrap(), MODULE_TEXT);
    }

    #[test]
    fn integration_detection_ignores_the_app_owned_directory() {
        let fixture = Fixture::new("integrated");
        let root = ConfigRoot {
            path: fixture.0.clone(),
            kind: ConfigKind::Classic,
        };
        fixture.write(
            "configuration.nix",
            "{ ... }: { imports = [ ./nix-pilot/state.nix ]; }",
        );
        assert!(is_integrated(&root));

        // The app's own files must not count as a reference.
        let mut doc = DesiredDoc::current();
        doc.add_package("alice", "hello".into(), PackageScope::System);
        write_app_files(&root, &doc).unwrap();
        std::fs::write(fixture.0.join("configuration.nix"), "{ ... }: { }").unwrap();
        assert!(!is_integrated(&root));
    }

    #[test]
    fn stale_wiring_never_counts_as_integrated() {
        // The old module model left a pinned input and an enable flag in the
        // config; only the D33 state-module import proves integration.
        let fixture = Fixture::new("stale-wiring");
        let root = ConfigRoot {
            path: fixture.0.clone(),
            kind: ConfigKind::Flake,
        };
        fixture.write(
            "flake.nix",
            "{ inputs = { nix-pilot.url = \"github:jonohribar/nix-pilot/5f5791c?dir=app/nixos-module\"; }; outputs = { ... }: { }; }",
        );
        fixture.write("host.nix", "{ ... }: { services.nix-pilot.enable = true; }");
        assert!(!is_integrated(&root));

        // The real import flips it to integrated.
        fixture.write(
            "flake.nix",
            "{ modules = [ ./nix-pilot/module.nix ./nix-pilot/state.nix ]; }",
        );
        assert!(is_integrated(&root));
    }

    #[test]
    fn uninstall_removes_the_app_owned_directory() {
        let fixture = Fixture::new("uninstall");
        let root = ConfigRoot {
            path: fixture.0.clone(),
            kind: ConfigKind::Classic,
        };
        let mut doc = DesiredDoc::current();
        doc.add_package("alice", "hello".into(), PackageScope::System);
        write_app_files(&root, &doc).unwrap();
        let directory = fixture.0.join(state_file::STATE_DIR);
        assert!(directory.is_dir());

        let text = uninstall(&root).unwrap();
        assert!(!directory.exists());
        assert!(text.contains("./nix-pilot/module.nix"));
        assert!(text.contains("services.nix-pilot.enable = true;"));
    }

    #[test]
    fn verify_parse_checks_the_entry_file_only() {
        let fixture = Fixture::new("verify");
        fixture.write("configuration.nix", "{ config, pkgs, ... }: { }");
        let root = ConfigRoot {
            path: fixture.0.clone(),
            kind: ConfigKind::Classic,
        };
        match verify_parse(&root) {
            Ok(()) => {}
            Err(message) if message.contains("nix-instantiate is not available") => {}
            Err(message) => panic!("unexpected verify failure: {message}"),
        }
    }
}
