//! The app-owned `nix-pilot/state.nix` module the user's config imports
//! (D33): rewritten on every apply. Pure flake evaluation reads the dirty
//! working tree, so the file needs `git add` only once. The per-app overlay
//! travels beside it when pinned sources exist.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::build;
use crate::desired_state::{render_per_app_overlays, DesiredDoc, PER_APP_OVERLAY_NAME};

/// Directory under the config root holding app-owned files.
pub const STATE_DIR: &str = "nix-pilot";
/// The generated desired-state module the user's config imports.
pub const STATE_MODULE: &str = "state.nix";

/// Write the desired-state module (and the per-app overlay when pinned
/// sources exist) into `<root>/nix-pilot/`. A stale overlay from an earlier
/// apply is removed when no sources remain, so the directory never carries an
/// orphaned import target. Returns the state module path.
///
/// # Errors
/// Returns an I/O error for directory or file operations, or an
/// `InvalidData` I/O error if the desired state cannot be rendered.
pub fn write_state_file(root: &Path, doc: &DesiredDoc) -> std::io::Result<PathBuf> {
    let directory = root.join(STATE_DIR);
    std::fs::create_dir_all(&directory)?;
    let module = doc
        .render_module(build::detect_user_backend())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let state_path = directory.join(STATE_MODULE);
    std::fs::write(&state_path, module)?;
    let overlay = directory.join(PER_APP_OVERLAY_NAME);
    if doc.sources.is_empty() {
        let _ = std::fs::remove_file(&overlay);
    } else {
        std::fs::write(&overlay, render_per_app_overlays(&doc.sources))?;
    }
    Ok(state_path)
}

/// Track the state module so pure flake evaluation sees it. No-op outside a
/// git working tree; an add failure means the next rebuild reads stale state.
///
/// # Errors
/// Returns an I/O error when `git add` fails.
pub fn git_add_once(root: &Path) -> std::io::Result<()> {
    if !root.join(".git").exists() {
        return Ok(());
    }
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("add")
        .arg(STATE_DIR)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("git add failed"))
    }
}

/// Why the app-owned files could not be prepared for a build or switch.
/// `Display` is the user-facing note; [`PrepareError::log`] carries the
/// underlying technical detail. Both apply-path callers (build and switch)
/// share this type so the failure wording cannot drift.
#[derive(Debug)]
pub enum PrepareError {
    /// The state module could not be rendered or written.
    Write(std::io::Error),
    /// A git working tree refused to track the app-owned directory.
    Git(std::io::Error),
}

impl fmt::Display for PrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(_) => {
                f.write_str("could not write the state module into your configuration")
            }
            Self::Git(_) => f.write_str(
                "could not track the state module in your configuration's git repository",
            ),
        }
    }
}

impl PrepareError {
    /// Technical detail for the log tail.
    pub fn log(&self) -> String {
        match self {
            Self::Write(error) => format!("writing the state module failed: {error}"),
            Self::Git(error) => format!("tracking the app-owned directory failed: {error}"),
        }
    }
}

/// Write the state module and track it, in one step. The single "apply
/// contract" shared by the build check and the switch: both must see exactly
/// the files the user's config imports.
///
/// # Errors
/// Returns a [`PrepareError`] describing the failing step in plain language.
pub fn write_and_track(
    root: &crate::config_source::ConfigRoot,
    doc: &DesiredDoc,
) -> Result<(), PrepareError> {
    write_state_file(&root.path, doc).map_err(PrepareError::Write)?;
    git_add_once(&root.path).map_err(PrepareError::Git)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desired_state::PackageScope;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "nix-pilot-state-file-{}-{name}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn write_state_file_writes_module_into_the_config_tree() {
        let fixture = Fixture::new("plain");
        let mut doc = DesiredDoc::current();
        doc.add_package("alice", "gimp".into(), PackageScope::System);
        doc.set_user("alice", vec!["firefox".into()]);

        let state_path = write_state_file(&fixture.0, &doc).unwrap();
        assert_eq!(state_path, fixture.0.join(STATE_DIR).join(STATE_MODULE));
        let text = std::fs::read_to_string(&state_path).unwrap();
        assert!(text.contains("environment.systemPackages"));
        assert!(text.contains("gimp"));
        // No sources: no overlay, and none lingers.
        assert!(!fixture
            .0
            .join(STATE_DIR)
            .join(PER_APP_OVERLAY_NAME)
            .exists());
    }

    #[test]
    fn write_state_file_carries_the_overlay_only_when_sources_exist() {
        let fixture = Fixture::new("sources");
        let mut doc = DesiredDoc::current();
        doc.sources.insert(
            "firefox".into(),
            crate::desired_state::PerAppSource {
                rev: "a".repeat(40),
                nar_hash: None,
            },
        );
        doc.add_package("alice", "firefox".into(), PackageScope::User);

        write_state_file(&fixture.0, &doc).unwrap();
        let overlay = fixture.0.join(STATE_DIR).join(PER_APP_OVERLAY_NAME);
        assert!(overlay.is_file());

        // Removing the source clears the stale overlay on the next write.
        doc.sources.clear();
        write_state_file(&fixture.0, &doc).unwrap();
        assert!(!overlay.exists());
    }

    #[test]
    fn git_add_once_is_a_noop_outside_a_git_work_tree() {
        let fixture = Fixture::new("no-git");
        assert!(git_add_once(&fixture.0).is_ok());
    }

    #[test]
    fn write_and_track_is_the_shared_apply_contract() {
        use crate::config_source::{ConfigKind, ConfigRoot};
        let fixture = Fixture::new("contract");
        let root = ConfigRoot {
            path: fixture.0.clone(),
            kind: ConfigKind::Classic,
        };
        let mut doc = DesiredDoc::current();
        doc.add_package("alice", "hello".into(), PackageScope::System);

        write_and_track(&root, &doc).unwrap();
        assert!(fixture.0.join(STATE_DIR).join(STATE_MODULE).is_file());

        // The user-facing wording is the contract the UI shows; protect it.
        let prepare_error = PrepareError::Write(std::io::Error::other("boom"));
        assert_eq!(
            prepare_error.to_string(),
            "could not write the state module into your configuration"
        );
        assert!(prepare_error.log().contains("boom"));
        let git_error = PrepareError::Git(std::io::Error::other("boom"));
        assert_eq!(
            git_error.to_string(),
            "could not track the state module in your configuration's git repository"
        );
    }
}
