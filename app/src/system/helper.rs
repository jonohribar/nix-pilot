//! Fixed-action interface to the installed privileged helper.
//!
//! This module owns helper discovery, the explicit activation gate, and
//! bounded pkexec invocation. Callers select only a closed action enum; no
//! caller-provided command or argument string reaches the helper.

use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::process::{self, Output};

#[derive(Debug, Clone, Copy)]
pub(super) enum HelperAction {
    Switch,
    Rollback,
    Gc,
    Optimise,
    GcEstimate,
}

impl HelperAction {
    pub(super) fn subcommand(self) -> &'static str {
        match self {
            Self::Switch => "switch",
            Self::Rollback => "rollback",
            Self::Gc => "gc",
            Self::Optimise => "optimise",
            Self::GcEstimate => "gc-estimate",
        }
    }
}

#[derive(Debug)]
pub(super) enum HelperError {
    Disabled,
    MissingHelper,
    Process(process::Error),
}

impl fmt::Display for HelperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => {
                f.write_str("System changes need the nix-pilot system module enabled")
            }
            Self::MissingHelper => f.write_str("nix-pilot-apply helper not found"),
            Self::Process(error) => error.fmt(f),
        }
    }
}

/// Whether privileged operations are explicitly enabled and the helper exists.
pub(super) fn can_activate() -> bool {
    std::env::var_os("NIX_PILOT_ALLOW_SWITCH").is_some() && find_helper().is_some()
}

fn find_helper() -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join("nix-pilot-apply"))
        .find(|path| path.is_file())
        .or_else(|| {
            let path = PathBuf::from("/run/current-system/sw/bin/nix-pilot-apply");
            path.is_file().then_some(path)
        })
}

pub(super) fn run_action(
    action: HelperAction,
    stdin: &[u8],
    timeout: Duration,
) -> Result<Output, HelperError> {
    if std::env::var_os("NIX_PILOT_ALLOW_SWITCH").is_none() {
        return Err(HelperError::Disabled);
    }
    let helper = find_helper().ok_or(HelperError::MissingHelper)?;
    let mut command = Command::new("pkexec");
    command.arg(helper).arg(action.subcommand());
    process::run_with_stdin(&mut command, stdin, timeout).map_err(HelperError::Process)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_actions_have_only_fixed_subcommands() {
        assert_eq!(HelperAction::Switch.subcommand(), "switch");
        assert_eq!(HelperAction::Rollback.subcommand(), "rollback");
        assert_eq!(HelperAction::Gc.subcommand(), "gc");
        assert_eq!(HelperAction::Optimise.subcommand(), "optimise");
        assert_eq!(HelperAction::GcEstimate.subcommand(), "gc-estimate");
    }

    #[test]
    fn activation_gate_requires_explicit_enablement() {
        std::env::remove_var("NIX_PILOT_ALLOW_SWITCH");
        assert!(!can_activate());
        assert!(matches!(
            run_action(HelperAction::Switch, &[], Duration::from_secs(1)),
            Err(HelperError::Disabled)
        ));
    }
}
