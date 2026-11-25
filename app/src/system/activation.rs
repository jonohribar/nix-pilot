//! Candidate activation and rollback through the fixed privileged helper.
//!
//! This adapter creates the exact configuration archive expected by the
//! helper, enforces the activation gate, and maps process outcomes to the
//! activation-specific result contract.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::build;
use super::helper::{self, HelperAction};
use crate::config_source;
use crate::desired_state::{DesiredDoc, PER_APP_OVERLAY_NAME};
use crate::process;

/// Activation action requested by the Updates page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivateAction {
    /// Activate the newly generated system configuration.
    Switch,
    /// Return to the previous system generation.
    Rollback,
}

impl ActivateAction {
    fn helper_action(self) -> HelperAction {
        match self {
            Self::Switch => HelperAction::Switch,
            Self::Rollback => HelperAction::Rollback,
        }
    }
}

/// Result of an activation or rollback attempt.
#[derive(Debug, Clone)]
pub struct ActivateResult {
    /// Whether activation or rollback completed successfully.
    pub ok: bool,
    /// Short plain-language outcome.
    pub note: String,
    /// Technical command output or failure detail.
    pub log: String,
}

/// Whether activation is explicitly enabled and its helper is available.
pub fn can_activate() -> bool {
    helper::can_activate()
}

/// Generate and submit the desired configuration to the fixed helper action.
pub fn activate(action: ActivateAction, doc: &DesiredDoc, timeout: Duration) -> ActivateResult {
    if !can_activate() {
        return ActivateResult {
            ok: false,
            note: "System changes need the nix-pilot system module enabled".into(),
            log: "Open Settings → System to enable activation.".into(),
        };
    }

    let (preserved, hardware) = match config_source::discover() {
        Ok(sources) => (Some(sources.configuration), sources.hardware_configuration),
        Err(_) => (None, None),
    };
    let state_version = build::state_version_or_default();
    let directory =
        std::env::temp_dir().join(format!("nix-pilot-activation-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    if let Err(error) = std::fs::create_dir_all(&directory) {
        return ActivateResult {
            ok: false,
            note: "could not write the activation configuration".into(),
            log: error.to_string(),
        };
    }

    let result = activate_in_directory(
        action,
        doc,
        &directory,
        &state_version,
        preserved.as_deref(),
        hardware.as_deref(),
        timeout,
    );
    let _ = std::fs::remove_dir_all(&directory);
    result
}

fn activate_in_directory(
    action: ActivateAction,
    doc: &DesiredDoc,
    directory: &Path,
    state_version: &str,
    preserved: Option<&Path>,
    hardware: Option<&Path>,
    timeout: Duration,
) -> ActivateResult {
    if let Err(error) = build::write_generation(directory, doc, state_version, preserved, hardware)
    {
        return ActivateResult {
            ok: false,
            note: "could not write the activation configuration".into(),
            log: error.to_string(),
        };
    }

    let archive = match create_tar_gz(directory, timeout) {
        Ok(archive) => archive,
        Err(error) => {
            return ActivateResult {
                ok: false,
                note: "could not package the activation configuration".into(),
                log: error,
            };
        }
    };

    match helper::run_action(action.helper_action(), &archive, timeout) {
        Ok(output) => {
            let ok = output.status.success();
            let note = if ok {
                match action {
                    ActivateAction::Switch => "the change was applied",
                    ActivateAction::Rollback => "the previous system was restored",
                }
            } else {
                match action {
                    ActivateAction::Switch => "applying the change failed",
                    ActivateAction::Rollback => "rollback failed",
                }
            };
            ActivateResult {
                ok,
                note: note.into(),
                log: process::tail(&format!("{}\n{}", output.stdout, output.stderr), 12),
            }
        }
        Err(error) => ActivateResult {
            ok: false,
            note: match action {
                ActivateAction::Switch => "applying the change failed".into(),
                ActivateAction::Rollback => "rollback failed".into(),
            },
            log: error.to_string(),
        },
    }
}

fn create_tar_gz(directory: &Path, timeout: Duration) -> Result<Vec<u8>, String> {
    let archive = directory.join("activation.tar.gz");
    // The generated module rel-imports `./per-app-overlays.nix` when per-app
    // sources exist; that file must travel with the module so the switch,
    // which runs from the extracted tar, can resolve it.
    let mut files = vec![
        build::OUTER_CONFIG_NAME.to_string(),
        build::DESIRED_MODULE_NAME.to_string(),
    ];
    if directory.join(PER_APP_OVERLAY_NAME).is_file() {
        files.push(PER_APP_OVERLAY_NAME.to_string());
    }
    let mut command = Command::new("tar");
    command
        .arg("-czf")
        .arg(&archive)
        .args(&files)
        .current_dir(directory);
    let result = match process::run(&mut command, timeout) {
        Ok(output) if output.status.success() => {
            std::fs::read(&archive).map_err(|error| error.to_string())
        }
        Ok(output) => Err(format!("tar failed: {}", process::tail(&output.stderr, 12))),
        Err(error) => Err(error.to_string()),
    };
    let _ = std::fs::remove_file(archive);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_action_maps_only_to_fixed_subcommands() {
        assert_eq!(
            ActivateAction::Switch.helper_action().subcommand(),
            "switch"
        );
        assert_eq!(
            ActivateAction::Rollback.helper_action().subcommand(),
            "rollback"
        );
    }

    #[test]
    fn activation_fails_closed_when_not_enabled() {
        std::env::remove_var("NIX_PILOT_ALLOW_SWITCH");
        let result = activate(
            ActivateAction::Switch,
            &DesiredDoc::current(),
            Duration::from_secs(5),
        );
        assert!(!result.ok);
        assert_eq!(
            result.note,
            "System changes need the nix-pilot system module enabled"
        );
    }

    #[test]
    fn tar_carries_the_per_app_overlay_when_sources_exist() {
        let mut doc = DesiredDoc::current();
        doc.sources.insert(
            "firefox".into(),
            crate::desired_state::PerAppSource {
                rev: "a".repeat(40),
                nar_hash: None,
            },
        );
        doc.add_package(
            "alice",
            "hello".into(),
            crate::desired_state::PackageScope::System,
        );
        let (preserved, hardware) = config_source::discover()
            .map(|sources| (Some(sources.configuration), sources.hardware_configuration))
            .unwrap_or((None, None));

        let dir = std::env::temp_dir().join(format!("np-tar-overlay-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        build::write_generation(
            &dir,
            &doc,
            "26.05",
            preserved.as_deref(),
            hardware.as_deref(),
        )
        .unwrap();
        // The overlay file is produced beside the generated module.
        assert!(dir.join(PER_APP_OVERLAY_NAME).is_file());

        let archive = create_tar_gz(&dir, Duration::from_secs(10)).unwrap();
        let path = std::env::temp_dir().join(format!("np-tar-overlay-list-{}", std::process::id()));
        std::fs::write(&path, &archive).unwrap();
        let listing = Command::new("tar")
            .args(["-tzf"])
            .arg(&path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(listing.status.success());
        let files: Vec<String> = String::from_utf8_lossy(&listing.stdout)
            .lines()
            .map(str::to_string)
            .collect();
        assert!(files.contains(&PER_APP_OVERLAY_NAME.to_string()));
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn tar_contains_exactly_two_files_and_is_deterministic() {
        let mut doc = DesiredDoc::current();
        doc.add_package(
            "alice",
            "gimp".into(),
            crate::desired_state::PackageScope::System,
        );
        doc.set_user("alice", vec!["firefox".into()]);
        let (preserved, hardware) = config_source::discover()
            .map(|sources| (Some(sources.configuration), sources.hardware_configuration))
            .unwrap_or((None, None));

        let dir1 = std::env::temp_dir().join(format!("np-tar1-{}", std::process::id()));
        let dir2 = std::env::temp_dir().join(format!("np-tar2-{}", std::process::id()));
        for dir in [&dir1, &dir2] {
            let _ = std::fs::remove_dir_all(dir);
            build::write_generation(
                dir,
                &doc,
                "26.05",
                preserved.as_deref(),
                hardware.as_deref(),
            )
            .unwrap();
        }

        let archive1 = create_tar_gz(&dir1, Duration::from_secs(10)).unwrap();
        let archive2 = create_tar_gz(&dir2, Duration::from_secs(10)).unwrap();
        assert_eq!(archive1, archive2, "tar output must be deterministic");

        for (index, archive) in [archive1, archive2].iter().enumerate() {
            let path =
                std::env::temp_dir().join(format!("np-tar-list-{}-{index}", std::process::id()));
            std::fs::write(&path, archive).unwrap();
            let listing = Command::new("tar")
                .args(["-tzf"])
                .arg(&path)
                .output()
                .unwrap();
            let _ = std::fs::remove_file(&path);
            assert!(listing.status.success());
            let files: Vec<_> = String::from_utf8_lossy(&listing.stdout)
                .lines()
                .map(str::to_string)
                .collect();
            assert_eq!(files.len(), 2);
            assert!(files.contains(&build::OUTER_CONFIG_NAME.to_string()));
            assert!(files.contains(&build::DESIRED_MODULE_NAME.to_string()));
        }
        let _ = std::fs::remove_dir_all(&dir1);
        let _ = std::fs::remove_dir_all(&dir2);
    }
}
