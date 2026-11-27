//! Candidate activation and rollback through the fixed privileged helper.
//!
//! Under D33 the app writes its `nix-pilot/state.nix` module into the user's
//! config tree and the helper switches that same config — one config, one
//! pure rebuild. The helper remains the only root-capable entry point.

use std::path::Path;
use std::time::Duration;

use super::helper::{self, HelperAction};
use super::state_file;
use crate::config_source;
use crate::desired_state::DesiredDoc;
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

/// Apply the desired state to the user's configuration and switch, or return
/// to the previous system generation.
pub fn activate(action: ActivateAction, doc: &DesiredDoc, timeout: Duration) -> ActivateResult {
    if !can_activate() {
        return ActivateResult {
            ok: false,
            note: "System changes need the nix-pilot system module enabled".into(),
            log: String::new(),
        };
    }
    match action {
        ActivateAction::Switch => activate_switch(doc, timeout),
        ActivateAction::Rollback => run_via_helper(action, None, timeout),
    }
}

fn activate_switch(doc: &DesiredDoc, timeout: Duration) -> ActivateResult {
    let Some(root) = config_source::resolve_config_root() else {
        return ActivateResult {
            ok: false,
            note: "could not find your NixOS configuration".into(),
            log: String::new(),
        };
    };
    if let Err(error) = state_file::write_and_track(&root, doc) {
        return ActivateResult {
            ok: false,
            note: error.to_string(),
            log: error.log(),
        };
    }
    run_via_helper(ActivateAction::Switch, Some(&root.path), timeout)
}

fn run_via_helper(
    action: ActivateAction,
    config_dir: Option<&Path>,
    timeout: Duration,
) -> ActivateResult {
    match helper::run_action(action.helper_action(), config_dir, timeout) {
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
}
