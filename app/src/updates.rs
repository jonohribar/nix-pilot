//! Updates backend (v1 — safe build-first, failure-and-recovery.md).
//!
//! This module owns the transaction transition table and safe "prepare" step.
//! `verify_queued` dry-runs each queued package, then the Updates page builds a
//! candidate generation. The transition table also models future apply and
//! recovery states; those states do not themselves perform system operations.
//! The current app is build-only, so activation remains disabled.
use std::time::Duration;

use crate::install;
pub use crate::store::PendingChange;

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

/// One transaction step. The full transition model follows
/// failure-and-recovery.md. Apply and recovery labels describe the intended
/// user-facing states; performing those operations belongs to the adapter/UI.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxStep {
    Ready,
    Explaining,
    BuildCheck,
    Applying,
    Done,
    BuildFailed,
    ApplyFailed,
    RolledBack,
    BootRecovery,
}

impl TxStep {
    /// Plain-language label for the step, shown verbatim on the Updates page.
    pub fn label(self) -> &'static str {
        use TxStep::*;
        match self {
            Ready => "Changes are ready — nothing has been applied yet",
            Explaining => "Explaining the change…",
            BuildCheck => "Prepared — the changes are built and checked",
            Applying => "Applying…",
            Done => "The change was applied",
            BuildFailed => "Nothing was changed — building the change failed",
            ApplyFailed => "Applying failed — returning to the previous system",
            RolledBack => "The previous system was restored",
            BootRecovery => "Recovery — this may be an earlier system",
        }
    }
}

/// Events that advance the transaction. Apply and recovery events are model
/// transitions; callers remain responsible for performing and verifying I/O.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxEvent {
    Prepare,
    Prepared,
    BuildOk,
    BuildFailed,
    Apply,
    ApplyOk,
    ApplyFailed,
    Rollback,
    Recovered,
}

/// The pure transition table. Returns `None` when the event is not valid for
/// the current step, so a stray UI action can never jump the transaction.
pub fn advance(step: TxStep, event: TxEvent) -> Option<TxStep> {
    use TxEvent as Ev;
    use TxStep as St;
    Some(match (step, event) {
        (St::Ready, Ev::Prepare) => St::Explaining,
        (St::Explaining, Ev::Prepared) => St::BuildCheck,
        (St::Explaining, Ev::BuildFailed) => St::BuildFailed,
        (St::BuildCheck, Ev::BuildOk) => St::Applying,
        (St::BuildCheck, Ev::BuildFailed) => St::BuildFailed,
        (St::BuildFailed, Ev::Prepare) => St::Ready,
        (St::Applying, Ev::ApplyOk) => St::Done,
        (St::Applying, Ev::ApplyFailed) => St::ApplyFailed,
        (St::ApplyFailed, Ev::Rollback) => St::RolledBack,
        (St::Done, Ev::Rollback) => St::RolledBack,
        (St::RolledBack, Ev::Prepare) => St::Ready,
        (St::BootRecovery, Ev::Recovered) => St::Done,
        _ => return None,
    })
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

    #[test]
    fn happy_path_reaches_done() {
        use TxStep::*;
        let mut s = Ready;
        s = advance(s, TxEvent::Prepare).unwrap();
        s = advance(s, TxEvent::Prepared).unwrap();
        assert_eq!(s, BuildCheck);
        s = advance(s, TxEvent::BuildOk).unwrap();
        assert_eq!(s, Applying);
        s = advance(s, TxEvent::ApplyOk).unwrap();
        assert_eq!(s, Done);
    }

    #[test]
    fn build_failure_leaves_nothing_changed() {
        use TxStep::*;
        let s = advance(Explaining, TxEvent::BuildFailed);
        assert_eq!(s, Some(BuildFailed));
        let s = advance(BuildCheck, TxEvent::BuildFailed);
        assert_eq!(s, Some(BuildFailed));
        // retry goes back to the start, nothing was applied
        assert_eq!(advance(BuildFailed, TxEvent::Prepare), Some(Ready));
    }

    #[test]
    fn apply_failure_rolls_back_and_recovers() {
        use TxStep::*;
        let s = advance(Applying, TxEvent::ApplyFailed);
        assert_eq!(s, Some(ApplyFailed));
        let s = advance(ApplyFailed, TxEvent::Rollback);
        assert_eq!(s, Some(RolledBack));
        // rollback is also available after a clean apply
        assert_eq!(advance(Done, TxEvent::Rollback), Some(RolledBack));
        assert_eq!(advance(RolledBack, TxEvent::Prepare), Some(Ready));
        // boot recovery returns to a usable state
        assert_eq!(advance(BootRecovery, TxEvent::Recovered), Some(Done));
    }

    #[test]
    fn invalid_events_are_none() {
        use TxStep::*;
        assert_eq!(advance(Ready, TxEvent::ApplyOk), None);
        assert_eq!(advance(Ready, TxEvent::Rollback), None);
        assert_eq!(advance(Done, TxEvent::Prepare), None);
        assert_eq!(advance(Applying, TxEvent::Prepared), None);
        assert_eq!(advance(BuildFailed, TxEvent::Apply), None);
        assert_eq!(advance(RolledBack, TxEvent::BuildOk), None);
    }

    #[test]
    fn step_labels_are_distinct_and_plain() {
        use TxStep::*;
        let steps = [
            Ready,
            Explaining,
            BuildCheck,
            Applying,
            Done,
            BuildFailed,
            ApplyFailed,
            RolledBack,
            BootRecovery,
        ];
        let mut seen = std::collections::HashSet::new();
        for s in steps {
            let l = s.label();
            assert!(!l.is_empty());
            assert!(seen.insert(l), "duplicate label for {s:?}");
            assert!(!l.contains("nix"), "label must be plain language: {l}");
        }
    }
}
