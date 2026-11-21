//! Storage measurement and upkeep operations.
//!
//! Store-size queries are read-only and unprivileged. Cleanup actions use the
//! fixed-action helper adapter and retain operation-specific user summaries.

use std::process::Command;
use std::time::Duration;

use super::helper::{self, HelperAction, HelperError};
use crate::process;

/// Storage upkeep action requested by the Updates page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupAction {
    /// Delete old generations and garbage-collect while retaining recent ones.
    Gc,
    /// Hard-link identical store files to save space.
    Optimise,
    /// Report what garbage collection would remove without changing anything.
    GcEstimate,
}

impl CleanupAction {
    fn helper_action(self) -> HelperAction {
        match self {
            Self::Gc => HelperAction::Gc,
            Self::Optimise => HelperAction::Optimise,
            Self::GcEstimate => HelperAction::GcEstimate,
        }
    }
}

/// Outcome of one storage-upkeep operation.
#[derive(Debug, Clone)]
pub struct CleanupResult {
    /// Whether the requested storage operation succeeded.
    pub ok: bool,
    /// Short outcome suitable for a user-facing summary.
    pub note: String,
    /// Technical command output or failure detail.
    pub log: String,
    /// Parsed dry-run summary when available.
    pub summary: Option<String>,
}

/// Run a fixed storage action through the privileged helper.
pub fn cleanup(action: CleanupAction, timeout: Duration) -> CleanupResult {
    match helper::run_action(action.helper_action(), &[], timeout) {
        Ok(output) => {
            let combined = format!("{}\n{}", output.stdout, output.stderr);
            let ok = output.status.success();
            CleanupResult {
                ok,
                note: if ok {
                    match action {
                        CleanupAction::Gc => "old versions were cleaned up".into(),
                        CleanupAction::Optimise => "storage was reclaimed".into(),
                        CleanupAction::GcEstimate => "here is what cleaning would free".into(),
                    }
                } else {
                    "storage upkeep failed".into()
                },
                log: tail(&combined),
                summary: parse_gc_estimate(&combined),
            }
        }
        Err(error) => CleanupResult {
            ok: false,
            note: match &error {
                HelperError::Disabled => "storage upkeep is not enabled on this system".into(),
                HelperError::MissingHelper => "nix-pilot-apply helper not found".into(),
                HelperError::Process(_) => "storage upkeep failed".into(),
            },
            log: match &error {
                HelperError::Disabled => {
                    "set NIX_PILOT_ALLOW_SWITCH only with the nix-pilot system module installed."
                        .into()
                }
                HelperError::MissingHelper => String::new(),
                HelperError::Process(error) => error.to_string(),
            },
            summary: None,
        },
    }
}

/// Read the Nix store size without privilege; return `None` on failure.
pub fn store_usage_bytes() -> Option<u64> {
    let output = process::run(
        Command::new("du").args(["-s", "-b", "/nix/store"]),
        Duration::from_secs(30),
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    output.stdout.split_whitespace().next()?.parse().ok()
}

/// Format a byte count in a short human unit (B, KiB, MiB, or GiB).
pub fn human_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Extract the summary line from `nix-collect-garbage --dry-run` output.
pub fn parse_gc_estimate(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| line.ends_with("would be deleted"))
        .map(str::to_string)
}

fn tail(output: &str) -> String {
    output
        .lines()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gc_estimate_summary() {
        assert_eq!(
            parse_gc_estimate("removing stale link...\n1056 store paths would be deleted"),
            Some("1056 store paths would be deleted".into())
        );
        assert_eq!(
            parse_gc_estimate("determining live/dead paths...\n0 store paths would be deleted\n"),
            Some("0 store paths would be deleted".into())
        );
        assert_eq!(parse_gc_estimate("nothing here"), None);
        assert_eq!(parse_gc_estimate(""), None);
    }

    #[test]
    fn cleanup_actions_have_fixed_subcommands() {
        assert_eq!(CleanupAction::Gc.helper_action().subcommand(), "gc");
        assert_eq!(
            CleanupAction::Optimise.helper_action().subcommand(),
            "optimise"
        );
        assert_eq!(
            CleanupAction::GcEstimate.helper_action().subcommand(),
            "gc-estimate"
        );
    }

    #[test]
    fn formats_human_bytes() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1536), "1.5 KiB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(human_bytes(2 * 1024 * 1024 * 1024), "2.0 GiB");
    }
}
