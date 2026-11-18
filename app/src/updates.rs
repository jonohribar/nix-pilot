//! Updates backend (v1 — safe build-first, failure-and-recovery.md).
//!
//! A system change is one transaction: explain -> evaluate/build -> apply ->
//! rollback, plus a first-class boot-recovery story (failure-and-recovery.md).
//! This module currently owns the *safe* "prepare" step: it verifies each
//! queued change can be built using the same dry-run as the install check
//! (never fetches/builds/activates). The real apply (switch) operates on the
//! app-owned desired-state/generated config, which does not exist yet, so the
//! surface's Apply control stays disabled until that layer lands; nothing here
//! ever switches a live system.
use std::time::Duration;

use crate::install;

/// A change destined for the transaction (kept decoupled from store/UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingChange {
    pub attr: String,
    pub name: String,
    pub system_wide: bool,
}

/// Per-change build-verify outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildItem {
    pub name: String,
    pub ok: bool,
    pub note: String,
}

/// Result of verifying the pending set builds.
#[derive(Debug, Default)]
pub struct BuildReport {
    pub items: Vec<BuildItem>,
}

impl BuildReport {
    pub fn all_ok(&self) -> bool {
        self.items.iter().all(|i| i.ok)
    }
}

/// Safely verify each queued change can be built (the "Prepare — changes are
/// built and checked" transaction step). Uses the dry-run availability check,
/// so it never downloads, builds or activates anything.
pub fn verify_queued(
    nix: &std::path::Path,
    changes: &[PendingChange],
    timeout: Duration,
) -> BuildReport {
    let mut report = BuildReport::default();
    for c in changes {
        let r = install::check_availability(nix, &c.attr, timeout);
        let ok = matches!(r.outcome, install::Outcome::ConfirmedAvailable { .. });
        let note = match r.outcome {
            install::Outcome::ConfirmedAvailable { note } => note,
            install::Outcome::UnfreeEvalBlocked => "unfree — blocked on this system".to_string(),
            install::Outcome::Unavailable => "not available for this system".to_string(),
            install::Outcome::CheckFailed { reason } => reason,
        };
        report.items.push(BuildItem {
            name: c.name.clone(),
            ok,
            note,
        });
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(attr: &str) -> PendingChange {
        PendingChange {
            attr: attr.into(),
            name: attr.into(),
            system_wide: false,
        }
    }

    #[test]
    fn report_is_ok_only_when_all_buildable() {
        let mut r = BuildReport::default();
        assert!(r.all_ok()); // empty is vacuously ok
        r.items.push(BuildItem {
            name: "a".into(),
            ok: true,
            note: String::new(),
        });
        r.items.push(BuildItem {
            name: "b".into(),
            ok: false,
            note: "blocked".into(),
        });
        assert!(!r.all_ok());
        assert_eq!(r.items.len(), 2);
    }

    #[test]
    fn pending_change_constructs() {
        let c = p("firefox");
        assert_eq!(c.attr, "firefox");
        assert!(!c.system_wide);
    }
}
