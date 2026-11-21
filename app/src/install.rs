//! Install backend adapter (v1 — intent flow).
//!
//! A Store install is a desired *system change*, applied as one transaction
//! through the Updates surface (architecture "Install and system-state model",
//! D9; failure-and-recovery.md). Nothing in this module mutates the system.
//! Its job is the front half: check a catalogue attribute against the system's
//! configured nixpkgs and return a structured outcome, with "unavailable on
//! your system" as a first-class state (revision drift, risk R07) and the raw
//! technical detail one click away. The dry-run below never downloads or
//! builds; verdicts reflect what the system's own configuration will actually
//! allow (an unfree refusal is "not allowed", not "installable").
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::process;

/// Typed verdict for an availability check. The UI maps these to wording.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Dry-run succeeded; optionally carries a "Downloads about X" note.
    ConfirmedAvailable { note: String },
    /// The dry-run was refused because the licence is unfree on this config.
    UnfreeEvalBlocked,
    /// The attribute does not exist on this system's nixpkgs (missing/alias).
    Unavailable,
    /// Could not be determined (spawn error, timeout, or unknown failure).
    CheckFailed { reason: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckResult {
    /// Classified verdict used by the UI and transaction preparation.
    pub outcome: Outcome,
    /// Raw nix output (technical detail, one click away).
    pub detail: String,
}

fn which_timeout() -> Option<String> {
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .map(|d| d.join("timeout"))
            .find(|p| p.exists())
            .map(|p| p.to_string_lossy().into_owned())
    })
}

/// Resolve the nix client: $NIX_BIN, then PATH, then well-known NixOS paths.
pub fn discover_nix() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("NIX_BIN") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    if Command::new("nix")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .status()
        .is_ok()
    {
        return Some(PathBuf::from("nix"));
    }
    let mut cands = vec![
        PathBuf::from("/run/current-system/sw/bin/nix"),
        PathBuf::from("/nix/var/nix/profiles/default/bin/nix"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        cands.push(PathBuf::from(home).join(".nix-profile/bin/nix"));
    }
    cands.into_iter().find(|p| p.exists())
}

/// Check whether `attr` is available under the system's configured nixpkgs.
///
/// Runs a dry-run availability evaluation with a deadline; it does not fetch,
/// build, or write a system configuration. Evaluation output is retained in
/// `detail` while `outcome` provides a stable classification for the UI.
pub fn check_availability(nix: &std::path::Path, attr: &str, timeout: Duration) -> CheckResult {
    // Wrap in timeout(1) when available so an orphaned check (app closed
    // mid-run) still terminates instead of lingering on a stalled download.
    let mut parts = vec![nix.to_string_lossy().into_owned()];
    parts.extend(["build", "--dry-run", "--no-link"].map(String::from));
    parts.push(format!("nixpkgs#{attr}"));
    if let Some(t) = which_timeout() {
        parts.insert(0, timeout.as_secs().max(1).to_string());
        parts.insert(0, t);
    }
    let mut cmd = Command::new(&parts[0]);
    cmd.args(&parts[1..]);
    let output = match process::run(&mut cmd, timeout) {
        Ok(output) => output,
        Err(error) => {
            let (reason, detail) = match error {
                process::Error::Timeout => (
                    "the availability check took too long".into(),
                    "the package check exceeded its time limit".into(),
                ),
                other => ("could not be checked right now".into(), other.to_string()),
            };
            return CheckResult {
                outcome: Outcome::CheckFailed { reason },
                detail,
            };
        }
    };
    classify(
        output.status.success(),
        &format!("{}\n{}", output.stdout, output.stderr),
    )
}

/// Verdict rules, matching what the dry run actually prints (verified against
/// nix 2.34.9 and the pinned nixpkgs snapshot):
///   rc = 0          -> ConfirmedAvailable; optionally "will be fetched (…)".
///   unfree refusal  -> UnfreeEvalBlocked (this config refuses; see doc header).
///   attr missing    -> Unavailable; may carry "Did you mean …" (aliases, D10).
///   anything else   -> CheckFailed.
fn classify(ok: bool, output: &str) -> CheckResult {
    let detail = output.trim().to_string();
    if ok {
        let size = output
            .lines()
            .rev()
            .find(|l| l.contains("will be fetched") || l.contains("will be built"))
            .and_then(extract_size);
        CheckResult {
            outcome: Outcome::ConfirmedAvailable {
                note: size
                    .map(|s| format!("Downloads about {s}"))
                    .unwrap_or_default(),
            },
            detail,
        }
    } else if output.contains("has an unfree license") {
        CheckResult {
            outcome: Outcome::UnfreeEvalBlocked,
            detail,
        }
    } else if output.contains("does not provide attribute") {
        CheckResult {
            outcome: Outcome::Unavailable,
            detail,
        }
    } else {
        CheckResult {
            outcome: Outcome::CheckFailed {
                reason: "could not be checked right now".into(),
            },
            detail,
        }
    }
}

/// Pull the download size out of a line like
/// `this path will be fetched (73.6 KiB download, 273.1 KiB unpacked):`
fn extract_size(line: &str) -> Option<String> {
    let di = line.find(" download")?;
    let before = &line[..di];
    let start = before.rfind(['(', ',']).map(|i| i + 1).unwrap_or(0);
    let s = before[start..].trim();
    (!s.is_empty()).then(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_or_fetchable_pkg_confirms_available() {
        let out = "this path will be fetched (73.6 KiB download, 273.1 KiB unpacked):\n  /nix/store/fkv...-hello-2.12.3\n";
        let r = classify(true, out);
        assert_eq!(
            r.outcome,
            Outcome::ConfirmedAvailable {
                note: "Downloads about 73.6 KiB".into()
            }
        );
    }

    #[test]
    fn cached_pkg_without_size_line_confirms_available() {
        let r = classify(true, "");
        assert_eq!(
            r.outcome,
            Outcome::ConfirmedAvailable {
                note: String::new()
            }
        );
    }

    #[test]
    fn missing_attr_is_unavailable() {
        let out = "error: flake 'flake:nixpkgs' does not provide attribute 'packages.x86_64-linux.this-thing' or 'this-thing'\n";
        let r = classify(false, out);
        assert_eq!(r.outcome, Outcome::Unavailable);
        assert!(r.detail.contains("does not provide attribute"));
    }

    #[test]
    fn alias_hint_surfaces_in_detail() {
        let out = "error: flake 'flake:nixpkgs' does not provide attribute 'packages.x86_64-linux.mangatui' or 'mangatui'\n       Did you mean manga-tui?\n";
        let r = classify(false, out);
        assert_eq!(r.outcome, Outcome::Unavailable);
        assert!(r.detail.contains("Did you mean manga-tui"));
    }

    #[test]
    fn unfree_refusal_is_blocked_not_installable() {
        let out = "error: Refusing to evaluate package 'anydesk-8.1.0' in .../package.nix:173 because it has an unfree license ('unfree')\n";
        let r = classify(false, out);
        assert_eq!(r.outcome, Outcome::UnfreeEvalBlocked);
    }

    #[test]
    fn unknown_failure_is_check_failed() {
        let r = classify(false, "error: some other mystery failure");
        assert_eq!(
            r.outcome,
            Outcome::CheckFailed {
                reason: "could not be checked right now".into()
            }
        );
    }

    // Live integration checks (dry-run only; never mutates anything).
    // Cargo test -- --ignored on a machine with nix on PATH.
    #[test]
    #[ignore]
    fn real_nix_hello_is_available() {
        let nix = discover_nix().expect("nix client not found");
        let r = check_availability(&nix, "hello", Duration::from_secs(90));
        assert!(
            matches!(r.outcome, Outcome::ConfirmedAvailable { .. }),
            "detail: {}",
            r.detail
        );
    }

    #[test]
    #[ignore]
    fn real_nix_missing_attr_is_unavailable() {
        let nix = discover_nix().expect("nix client not found");
        let r = check_availability(
            &nix,
            "this-definitely-does-not-exist",
            Duration::from_secs(90),
        );
        assert_eq!(r.outcome, Outcome::Unavailable);
        assert!(r.detail.contains("does not provide attribute"));
    }
}
