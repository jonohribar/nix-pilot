//! Writing the app-owned `nix-pilot/state.nix` module into the user's config
//! tree (D33). The user's config imports this one file; the app owns it and
//! rewrites it on every apply. Pure flake evaluation reads the dirty working
//! tree, so the file only needs `git add` once — afterwards it can just
//! change. The per-app overlay travels beside it when pinned sources exist.

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

/// Track the state module so pure flake evaluation sees it. No-op when the
/// root is not inside a git working tree. An error means a git repo whose
/// add failed, in which case the next rebuild would read stale state.
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
}
