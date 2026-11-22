//! Updates backend (v1 — safe build-first, failure-and-recovery.md).
//!
//! This module owns the transaction transition table, its derived action view,
//! and the safe package-verification step. The Updates UI uses these pure
//! contracts to coordinate candidate builds and the separately gated apply
//! adapter; this module itself performs no system activation or recovery I/O.
use std::time::Duration;

use crate::desired_state::PackageIntent;
use crate::install;

/// Per-change build-verify outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildItem {
    /// Package attribute checked by the availability probe.
    pub attr: String,
    /// Whether the package was confirmed available for this system.
    pub ok: bool,
    /// Plain-language result or reason the check could not confirm availability.
    pub note: String,
}

/// Result of verifying each selected package intent.
#[derive(Debug, Default)]
pub struct BuildReport {
    /// One result for each requested package intent, in input order.
    pub items: Vec<BuildItem>,
}

impl BuildReport {
    /// Whether every checked item succeeded; an empty report is successful.
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
    /// No prepare operation is currently in progress.
    Ready,
    /// The prepare operation has begun and its checks are running.
    Explaining,
    /// Package checks passed and the candidate is ready for the next step.
    BuildCheck,
    /// An activation operation is in progress (modeled, not performed here).
    Applying,
    /// The requested change was applied (modeled state only).
    Done,
    /// A rollback operation is in progress.
    RollingBack,
    /// Candidate preparation failed before activation.
    BuildFailed,
    /// Activation failed and recovery may be required.
    ApplyFailed,
    /// A rollback attempt failed; it may be retried.
    RollbackFailed,
    /// The previous system state was restored.
    RolledBack,
    /// Startup detected a system requiring the recovery flow.
    BootRecovery,
}

impl TxStep {
    /// Plain-language label for the step, shown verbatim on the Updates page.
    pub fn label(self) -> &'static str {
        use TxStep::*;
        match self {
            Ready => "Changes are ready — nothing has been applied yet",
            Explaining => "Checking the changes…",
            BuildCheck => "Checked and built — ready to apply",
            Applying => "Applying…",
            Done => "The change was applied",
            RollingBack => "Restoring the previous system…",
            BuildFailed => "Nothing was changed — building the change failed",
            ApplyFailed => "Applying failed — the previous system can be restored",
            RollbackFailed => "Could not restore the previous system",
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
    /// Begin preparing the pending changes.
    Prepare,
    /// Verification and candidate preparation completed successfully.
    Prepared,
    /// Preparation or candidate build failed.
    BuildFailed,
    /// Begin applying the prepared candidate.
    Apply,
    /// Applying completed successfully.
    ApplyOk,
    /// Applying failed.
    ApplyFailed,
    /// Begin restoring the previous system state.
    Rollback,
    /// Rollback completed successfully.
    RollbackOk,
    /// Rollback failed.
    RollbackFailed,
    /// Prepared inputs changed before apply.
    InputsChanged,
    /// Recovery completed.
    Recovered,
}

/// Operation represented by the Updates page's primary action button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryAction {
    /// No primary operation is available in the current transaction state.
    None,
    /// Apply the prepared candidate system generation.
    Apply,
    /// Restore the previous system generation.
    Rollback,
}

/// GTK-independent controls derived from the transaction state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TxView {
    /// Whether the UI should enable the Prepare action.
    pub prepare_enabled: bool,
    /// Operation selected for the primary action, if any.
    pub primary_action: PrimaryAction,
    /// Whether that operation is currently allowed by system policy.
    pub primary_enabled: bool,
    /// User-facing label for the primary action button.
    pub primary_label: &'static str,
    /// Optional explanation for the primary action's availability.
    pub primary_tooltip: Option<&'static str>,
}

/// Derive available Updates actions from transaction state and system policy.
pub fn view_for(step: TxStep, activation_allowed: bool) -> TxView {
    use PrimaryAction as Action;
    use TxStep as St;

    let prepare_enabled = matches!(
        step,
        St::Ready | St::BuildCheck | St::BuildFailed | St::RollbackFailed | St::RolledBack
    );
    let primary_action = match step {
        St::BuildCheck => Action::Apply,
        St::Done | St::ApplyFailed | St::RollbackFailed => Action::Rollback,
        _ => Action::None,
    };
    let primary_label = match primary_action {
        Action::Apply | Action::None => "Apply changes",
        Action::Rollback => "Go back to the previous system",
    };
    let primary_tooltip = if !activation_allowed {
        Some("Activation requires NIX_PILOT_ALLOW_SWITCH to be set and the nix-pilot-apply helper installed (nixos-module).")
    } else if primary_action == Action::Rollback {
        Some("Return to the system generation before this change was applied.")
    } else {
        None
    };

    TxView {
        prepare_enabled,
        primary_action,
        primary_enabled: activation_allowed && primary_action != Action::None,
        primary_label,
        primary_tooltip,
    }
}

/// The pure transition table. Returns `None` when the event is not valid for
/// the current step, so a stray UI action can never jump the transaction.
pub fn advance(step: TxStep, event: TxEvent) -> Option<TxStep> {
    use TxEvent as Ev;
    use TxStep as St;
    Some(match (step, event) {
        (St::Ready, Ev::Prepare)
        | (St::BuildCheck, Ev::Prepare)
        | (St::BuildFailed, Ev::Prepare)
        | (St::RollbackFailed, Ev::Prepare)
        | (St::RolledBack, Ev::Prepare) => St::Explaining,
        (St::Explaining, Ev::Prepared) => St::BuildCheck,
        (St::Explaining, Ev::BuildFailed) => St::BuildFailed,
        (St::BuildCheck, Ev::Apply) => St::Applying,
        (St::BuildCheck, Ev::BuildFailed) => St::BuildFailed,
        (St::BuildCheck, Ev::InputsChanged) => St::Ready,
        (St::Applying, Ev::ApplyOk) => St::Done,
        (St::Applying, Ev::ApplyFailed) => St::ApplyFailed,
        (St::ApplyFailed, Ev::Rollback)
        | (St::Done, Ev::Rollback)
        | (St::RollbackFailed, Ev::Rollback) => St::RollingBack,
        (St::RollingBack, Ev::RollbackOk) => St::RolledBack,
        (St::RollingBack, Ev::RollbackFailed) => St::RollbackFailed,
        (St::BootRecovery, Ev::Recovered) => St::Done,
        _ => return None,
    })
}

/// Safely verify each selected package can be built (the "Prepare — changes are
/// built and checked" transaction step). Uses the dry-run availability check,
/// so it never downloads, builds or activates anything.
pub fn verify_package_intents(
    nix: &std::path::Path,
    changes: &[PackageIntent],
    timeout: Duration,
) -> BuildReport {
    let mut report = BuildReport::default();
    for c in changes {
        let r = install::check_availability(nix, &c.attr, timeout);
        let ok = matches!(r.outcome, install::Outcome::ConfirmedAvailable { .. });
        let note = match r.outcome {
            install::Outcome::ConfirmedAvailable { note } => note,
            install::Outcome::UnfreeEvalBlocked => {
                "Restricted licence — allow it in Settings -> Software".to_string()
            }
            install::Outcome::Unavailable => "not available for this system".to_string(),
            install::Outcome::CheckFailed { reason } => reason,
        };
        report.items.push(BuildItem {
            attr: c.attr.clone(),
            ok,
            note,
        });
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(attr: &str) -> PackageIntent {
        PackageIntent {
            attr: attr.into(),
            scope: crate::desired_state::PackageScope::User,
        }
    }

    #[test]
    fn report_is_ok_only_when_all_buildable() {
        let mut r = BuildReport::default();
        assert!(r.all_ok()); // empty is vacuously ok
        r.items.push(BuildItem {
            attr: "a".into(),
            ok: true,
            note: String::new(),
        });
        r.items.push(BuildItem {
            attr: "b".into(),
            ok: false,
            note: "blocked".into(),
        });
        assert!(!r.all_ok());
        assert_eq!(r.items.len(), 2);
    }

    #[test]
    fn package_intent_constructs() {
        let c = p("firefox");
        assert_eq!(c.attr, "firefox");
        assert_eq!(c.scope, crate::desired_state::PackageScope::User);
    }

    #[test]
    fn happy_path_reaches_done() {
        use TxStep::*;
        let mut s = Ready;
        s = advance(s, TxEvent::Prepare).unwrap();
        s = advance(s, TxEvent::Prepared).unwrap();
        assert_eq!(s, BuildCheck);
        s = advance(s, TxEvent::Apply).unwrap();
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
        // A retry starts preparation again; nothing was applied.
        assert_eq!(advance(BuildFailed, TxEvent::Prepare), Some(Explaining));
    }

    #[test]
    fn apply_failure_rolls_back_and_recovers() {
        use TxStep::*;
        let s = advance(Applying, TxEvent::ApplyFailed);
        assert_eq!(s, Some(ApplyFailed));
        let s = advance(ApplyFailed, TxEvent::Rollback);
        assert_eq!(s, Some(RollingBack));
        assert_eq!(advance(RollingBack, TxEvent::RollbackOk), Some(RolledBack));
        // rollback is also available after a clean apply
        assert_eq!(advance(Done, TxEvent::Rollback), Some(RollingBack));
        assert_eq!(
            advance(RollingBack, TxEvent::RollbackFailed),
            Some(RollbackFailed)
        );
        assert_eq!(
            advance(RollbackFailed, TxEvent::Rollback),
            Some(RollingBack)
        );
        assert_eq!(advance(RolledBack, TxEvent::Prepare), Some(Explaining));
        // boot recovery returns to a usable state
        assert_eq!(advance(BootRecovery, TxEvent::Recovered), Some(Done));
    }

    #[test]
    fn prepare_retries_and_changed_inputs_follow_the_transition_table() {
        use TxStep::*;
        assert_eq!(advance(BuildFailed, TxEvent::Prepare), Some(Explaining));
        assert_eq!(advance(BuildCheck, TxEvent::Prepare), Some(Explaining));
        assert_eq!(advance(BuildCheck, TxEvent::InputsChanged), Some(Ready));
        assert_eq!(advance(Applying, TxEvent::InputsChanged), None);
    }

    #[test]
    fn repeated_apply_and_rollback_cycles_reuse_the_same_transitions() {
        use TxStep::*;
        let mut step = Ready;
        for _ in 0..2 {
            step = advance(step, TxEvent::Prepare).unwrap();
            assert_eq!(step, Explaining);
            step = advance(step, TxEvent::Prepared).unwrap();
            assert_eq!(step, BuildCheck);
            assert_eq!(view_for(step, true).primary_action, PrimaryAction::Apply);
            step = advance(step, TxEvent::Apply).unwrap();
            assert_eq!(step, Applying);
            step = advance(step, TxEvent::ApplyOk).unwrap();
            assert_eq!(step, Done);
            assert_eq!(view_for(step, true).primary_action, PrimaryAction::Rollback);
            step = advance(step, TxEvent::Rollback).unwrap();
            assert_eq!(step, RollingBack);
            step = advance(step, TxEvent::RollbackOk).unwrap();
            assert_eq!(step, RolledBack);
        }
    }

    #[test]
    fn view_mapping_gates_actions_by_state_and_activation_policy() {
        use PrimaryAction::*;
        use TxStep::*;

        let applying = view_for(Applying, true);
        assert!(!applying.prepare_enabled);
        assert_eq!(applying.primary_action, None);
        assert!(!applying.primary_enabled);

        let prepared_without_activation = view_for(BuildCheck, false);
        assert!(prepared_without_activation.prepare_enabled);
        assert_eq!(prepared_without_activation.primary_action, Apply);
        assert!(!prepared_without_activation.primary_enabled);
        assert!(prepared_without_activation.primary_tooltip.is_some());

        let done = view_for(Done, true);
        assert_eq!(done.primary_action, Rollback);
        assert!(done.primary_enabled);
        assert_eq!(done.primary_label, "Go back to the previous system");

        let failed_rollback = view_for(RollbackFailed, true);
        assert!(failed_rollback.prepare_enabled);
        assert_eq!(failed_rollback.primary_action, Rollback);
        assert!(failed_rollback.primary_enabled);
    }

    #[test]
    fn invalid_events_are_none() {
        use TxStep::*;
        assert_eq!(advance(Ready, TxEvent::ApplyOk), None);
        assert_eq!(advance(Ready, TxEvent::Rollback), None);
        assert_eq!(advance(Done, TxEvent::Prepare), None);
        assert_eq!(advance(Applying, TxEvent::Prepared), None);
        assert_eq!(advance(BuildFailed, TxEvent::Apply), None);
        assert_eq!(advance(RolledBack, TxEvent::Apply), None);
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
            RollingBack,
            BuildFailed,
            ApplyFailed,
            RollbackFailed,
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
