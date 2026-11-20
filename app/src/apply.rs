//! Candidate-build adapter: turn the desired-state document into a built
//! candidate generation. It writes only into an explicit generation directory
//! and invokes `nixos-rebuild build` / `dry-build`; it never activates a system.
//! Activation and rollback are separate operations and remain outside this
//! adapter until their transaction and recovery guarantees are implemented.
//!
//! Generated-config shape (v1, revised by D21): the app owns the outer
//! generated `configuration.nix`, which *imports the live pre-existing
//! `configuration.nix` by absolute path* (never a copy — hand-edits are
//! picked up on every build) plus the desired-state module verbatim.
//! `system.stateVersion` is emitted only when there is no pre-existing
//! config to own it. Baseline options can be added to the settings surface as
//! supported workflows are designed.
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::desired_state::DesiredDoc;
use crate::process;

/// Names of the files the generated configuration owns inside `dir`.
pub const DESIRED_MODULE_NAME: &str = "generated-desired.nix";
pub const OUTER_CONFIG_NAME: &str = "configuration.nix";

/// Result of a candidate build.
#[derive(Debug, Clone)]
pub struct CandidateBuild {
    pub ok: bool,
    /// Short plain-language outcome.
    pub note: String,
    /// Tail of the nixos-rebuild output (technical detail, one click away).
    pub log: String,
}

/// Resolve the nixos-rebuild client: $NIXOS_REBUILD_BIN, PATH, then the
/// NixOS well-known path. Mirrors `install::discover_nix`.
pub fn discover_rebuild() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("NIXOS_REBUILD_BIN") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    if std::process::Command::new("nixos-rebuild")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
    {
        return Some(PathBuf::from("nixos-rebuild"));
    }
    let p = PathBuf::from("/run/current-system/sw/bin/nixos-rebuild");
    p.exists().then_some(p)
}

/// Parse the first two version fields of `nixos-version` output into a NixOS
/// stateVersion ("26.05.20261113.f1e5c35 (Yarara)" -> "26.05").
pub fn parse_state_version(v: &str) -> Option<String> {
    let first = v.split([' ', '(', '\t']).next().unwrap_or("").trim();
    if first.split('.').count() >= 2 && !first.is_empty() {
        let mut parts = first.split('.');
        Some(format!("{}.{}", parts.next()?, parts.next()?))
    } else {
        None
    }
}

/// Adapter: the running system's stateVersion (None on non-NixOS).
pub fn system_state_version() -> Option<String> {
    let out = std::process::Command::new("nixos-version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_state_version(&String::from_utf8_lossy(&out.stdout))
}

/// The pre-existing NixOS config files; the outer generated config imports
/// the live configuration.nix by pointer, which itself imports the machine's
/// hardware-configuration.nix. Opinionated: this looks only at the standard
/// `/etc/nixos` layout — we deliberately do not chase `NIXOS_CONFIG` env (the
/// app targets the conventional install, not Nix-hobbyist layouts). Returns
/// (configuration.nix, hardware-configuration.nix); both None if not NixOS
/// (or a flake-managed dir, which this app does not touch).
pub fn discover_nixos_sources() -> (Option<PathBuf>, Option<PathBuf>) {
    let base = PathBuf::from("/etc/nixos");
    if !base.is_dir() {
        return (None, None);
    }
    let cfg = base.join("configuration.nix");
    if !cfg.exists() {
        // flake-managed (flake.nix present, no configuration.nix): refuse
        // rather than guess — never build a bare config over one
        return (None, None);
    }
    let hw = base.join("hardware-configuration.nix");
    (Some(cfg), hw.exists().then_some(hw))
}

/// Write the generated configuration into `dir` (an explicit generation
/// directory — scratch-only). The outer file imports the LIVE pre-existing
/// configuration.nix by absolute path (D21: pointer, never a copy) plus the
/// desired-state module; `state_version` is emitted only when there is no
/// pre-existing config to own it. Returns the path to the outer
/// configuration.nix.
/// Which per-user backend this machine's installs target (decision D22).
///
/// Explicit, defaulting to NixOS-native `users.users` (D9). home-manager
/// mode is valid ONLY when the config tree being built enables the
/// home-manager NixOS module (e.g. the app module added inside a flake that
/// already imports home-manager) — it must never be auto-detected from the
/// OS user's state, because a user can use home-manager standalone while the
/// /etc/nixos config we build against does not enable its module (that makes
/// `home-manager.users.*` an undefined option and the build fail). The
/// override is an env var for now (same pattern as $NIX_BIN); the settings
/// surface owns this choice later.
pub fn detect_user_backend() -> crate::desired_state::UserBackend {
    match std::env::var("NIX_PILOT_USER_BACKEND").as_deref() {
        Ok("home-manager") | Ok("hm") => crate::desired_state::UserBackend::HomeManager,
        _ => crate::desired_state::UserBackend::NixUsers,
    }
}

pub fn write_generation(
    dir: &Path,
    doc: &DesiredDoc,
    state_version: &str,
    preserved_config: Option<&Path>,
    hardware_config: Option<&Path>,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let desired_module = doc
        .render_module(detect_user_backend())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    std::fs::write(dir.join(DESIRED_MODULE_NAME), desired_module)?;

    let mut imports = Vec::new();
    if let Some(user_cfg) = preserved_config {
        imports.push(format!("{}", user_cfg.display()));
    } else if let Some(hw) = hardware_config {
        imports.push(format!("{}", hw.display()));
    }
    imports.push(format!("./{DESIRED_MODULE_NAME}"));
    let list = imports
        .iter()
        .map(|i| format!("    {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    // with a live user config present it owns stateVersion; emit it only
    // when there is nothing to inherit it from (fresh / non-NixOS case)
    let version = if preserved_config.is_none() {
        format!("  system.stateVersion = \"{state_version}\";\n")
    } else {
        String::new()
    };
    let body = format!("  imports = [\n{list}\n  ];\n{version}");

    let outer = format!("{{ config, pkgs, lib, ... }}:\n\n{{\n{body}}}\n");
    let outer_path = dir.join(OUTER_CONFIG_NAME);
    std::fs::write(&outer_path, outer)?;
    Ok(outer_path)
}

/// Generate + *build* the desired state's candidate generation in a fresh
/// scratch dir under the system temp dir. Build-only; never activates. Uses
/// the system's own nixos-rebuild, config sources and state version; on a
/// non-NixOS host it reports that the build is unavailable.
pub fn build_from_desired(
    doc: &DesiredDoc,
    state_version: &str,
    preserved: Option<&Path>,
    hardware: Option<&Path>,
    timeout: Duration,
) -> CandidateBuild {
    let Some(rebuild) = discover_rebuild() else {
        return CandidateBuild {
            ok: false,
            note: "nixos-rebuild is not available on this system".into(),
            log: String::new(),
        };
    };
    let dir = std::env::temp_dir().join(format!("nix-pilot-generation-{}", std::process::id()));
    let outer = match write_generation(&dir, doc, state_version, preserved, hardware) {
        Ok(p) => p,
        Err(e) => {
            return CandidateBuild {
                ok: false,
                note: "could not write the candidate generation".into(),
                log: e.to_string(),
            }
        }
    };
    let b = build_candidate(&rebuild, &outer, false, timeout);
    let _ = std::fs::remove_dir_all(&dir);
    b
}

/// Build a candidate generation from a written config directory.
///
/// Safety: `nixos-rebuild build` (or `dry-build` with `dry = true`) only
/// *builds* — it never switches, activates or touches the bootloader. The
/// config path is whatever `write_generation` produced; callers never point
/// this at a live config outside an explicit generation dir.
pub fn build_candidate(
    rebuild: &Path,
    config: &Path,
    dry: bool,
    timeout: Duration,
) -> CandidateBuild {
    let mut parts = vec![rebuild.to_string_lossy().into_owned()];
    parts.push(if dry { "dry-build" } else { "build" }.to_string());
    // nixpkgs comes from the ambient NIX_PATH; only the config is overridden,
    // keeping the build in the exact generation dir we wrote.
    parts.push("-I".to_string());
    parts.push(format!("nixos-config={}", config.display()));

    // timeout(1) wrapper like install.rs, so an orphaned build (app closed
    // mid-run) still terminates instead of lingering.
    if let Some(t) = which_timeout() {
        parts.insert(0, timeout.as_secs().max(1).to_string());
        parts.insert(0, t);
    }
    let timeout = timeout.max(Duration::from_secs(5));

    let mut cmd = std::process::Command::new(&parts[0]);
    // deterministic: always evaluate exactly this config file, no matter what
    // the ambient environment says (a no-op when NIXOS_CONFIG is unset; a
    // guarantee we can never accidentally build a different file)
    cmd.env("NIXOS_CONFIG", config.as_os_str())
        .args(&parts[1..]);
    let output = match process::run(&mut cmd, timeout) {
        Ok(output) => output,
        Err(error) => {
            let note = match &error {
                process::Error::Timeout => "the build took too long".into(),
                _ => "nixos-rebuild failed to run".into(),
            };
            return CandidateBuild {
                ok: false,
                note,
                log: error.to_string(),
            };
        }
    };
    let out = format!("{}\n{}", output.stdout, output.stderr);
    let tail: String = out
        .lines()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    let dry_note = if dry { "dry-" } else { "" };
    CandidateBuild {
        ok: output.status.success(),
        note: if output.status.success() {
            format!("{dry_note}build of the candidate generation succeeded — nothing was activated")
        } else {
            "the candidate generation did not build".into()
        },
        log: tail,
    }
}

fn which_timeout() -> Option<String> {
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .map(|d| d.join("timeout"))
            .find(|p| p.exists())
            .map(|p| p.to_string_lossy().into_owned())
    })
}

// --- Activation surface (D24-D26) -----------------------------------------

/// Whether activation is allowed in this environment.
/// True only when NIX_PILOT_ALLOW_SWITCH is set AND the nix-pilot-apply
/// helper is available on PATH.
pub fn can_activate() -> bool {
    std::env::var_os("NIX_PILOT_ALLOW_SWITCH").is_some() && which_apply_helper().is_some()
}

/// Locate the nix-pilot-apply helper on PATH (or well-known NixOS path).
/// Probes by path existence, not by running the helper: it has no `--version`
/// subcommand. Returns an absolute path so pkexec can resolve it.
fn which_apply_helper() -> Option<std::path::PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|d| d.join("nix-pilot-apply"))
        .find(|p| p.is_file())
        .or_else(|| {
            let p = std::path::PathBuf::from("/run/current-system/sw/bin/nix-pilot-apply");
            p.is_file().then_some(p)
        })
}

/// Activation action requested by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivateAction {
    Switch,
    Rollback,
}

/// Result of an activation attempt.
#[derive(Debug, Clone)]
pub struct ActivateResult {
    pub ok: bool,
    /// Short plain-language outcome.
    pub note: String,
    /// Technical detail (one click away).
    pub log: String,
}

/// Build a tar of exactly [configuration.nix, generated-desired.nix] from the
/// persisted DesiredDoc and pipe it to `pkexec nix-pilot-apply <switch|rollback>`
/// on stdin. Drains streams concurrently (reuses process.rs), applies a deadline,
/// returns ok/note/log.
///
/// The subcommand comes ONLY from the fixed ActivateAction enum.
pub fn activate(
    action: ActivateAction,
    doc: &crate::desired_state::DesiredDoc,
    timeout: std::time::Duration,
) -> ActivateResult {
    // Defense-in-depth: never reach the elevated helper unless activation is
    // genuinely enabled (module installed + flag set), regardless of caller.
    if !can_activate() {
        return ActivateResult {
            ok: false,
            note: "activation is not enabled on this system".into(),
            log: "set NIX_PILOT_ALLOW_SWITCH only with the nix-pilot system module installed."
                .into(),
        };
    }
    // 1. Build the generated configuration in a scratch dir
    let (preserved, hardware) = discover_nixos_sources();
    let state_version = system_state_version().unwrap_or_else(|| "26.05".to_string());

    let dir = std::env::temp_dir().join(format!("nix-pilot-activation-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let _outer = match write_generation(
        &dir,
        doc,
        &state_version,
        preserved.as_deref(),
        hardware.as_deref(),
    ) {
        Ok(p) => p,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            return ActivateResult {
                ok: false,
                note: "could not write the activation configuration".into(),
                log: e.to_string(),
            };
        }
    };

    // 2. Create tar.gz of the two files on stdout
    let tar_output = match create_tar_gz(&dir) {
        Ok(bytes) => bytes,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            return ActivateResult {
                ok: false,
                note: "could not package the activation configuration".into(),
                log: e.to_string(),
            };
        }
    };

    let _ = std::fs::remove_dir_all(&dir);

    // 3. Run pkexec nix-pilot-apply <subcommand> with tar on stdin
    let helper = match which_apply_helper() {
        Some(p) => p,
        None => {
            return ActivateResult {
                ok: false,
                note: "nix-pilot-apply helper not found".into(),
                log: String::new(),
            };
        }
    };

    let subcommand = match action {
        ActivateAction::Switch => "switch",
        ActivateAction::Rollback => "rollback",
    };

    let mut cmd = std::process::Command::new("pkexec");
    cmd.arg(&helper).arg(subcommand);

    // Write tar to stdin, capture stdout/stderr with deadline
    match run_with_stdin(&mut cmd, &tar_output, timeout) {
        Ok(output) => {
            let tail: String = format!("{}\n{}", output.stdout, output.stderr)
                .lines()
                .rev()
                .take(12)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            ActivateResult {
                ok: output.status.success(),
                note: if output.status.success() {
                    match action {
                        ActivateAction::Switch => "the change was applied".into(),
                        ActivateAction::Rollback => "the previous system was restored".into(),
                    }
                } else {
                    match action {
                        ActivateAction::Switch => "applying the change failed".into(),
                        ActivateAction::Rollback => "rollback failed".into(),
                    }
                },
                log: tail,
            }
        }
        Err(e) => ActivateResult {
            ok: false,
            note: match action {
                ActivateAction::Switch => "applying the change failed".into(),
                ActivateAction::Rollback => "rollback failed".into(),
            },
            log: e.to_string(),
        },
    }
}

/// Create a tar.gz archive of the generation directory containing exactly
/// configuration.nix and generated-desired.nix.
fn create_tar_gz(dir: &std::path::Path) -> std::io::Result<Vec<u8>> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new("tar");
    cmd.arg("-czf")
        .arg("-")
        .arg("configuration.nix")
        .arg("generated-desired.nix")
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "tar failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output.stdout)
}

/// Run a command with the given stdin data, capturing stdout/stderr concurrently
/// with a deadline. Reuses the pattern from process.rs.
fn run_with_stdin(
    command: &mut std::process::Command,
    stdin_data: &[u8],
    timeout: std::time::Duration,
) -> Result<crate::process::Output, crate::process::Error> {
    use std::io::Write;
    use std::process::Stdio;
    use std::thread;
    use std::time::{Duration, Instant};

    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(crate::process::Error::Spawn)?;

    // Write stdin
    if let Some(mut stdin) = child.stdin.take() {
        let data = stdin_data.to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&data);
        });
    }

    let stdout = child.stdout.take().map(crate::process::spawn_reader);
    let stderr = child.stderr.take().map(crate::process::spawn_reader);
    let deadline = Instant::now() + timeout;

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => {
                crate::process::terminate(&mut child);
                crate::process::discard_reader(stdout);
                crate::process::discard_reader(stderr);
                return Err(crate::process::Error::Timeout);
            }
            Err(source) => {
                crate::process::terminate(&mut child);
                crate::process::discard_reader(stdout);
                crate::process::discard_reader(stderr);
                return Err(crate::process::Error::Wait(source));
            }
        }
    };

    Ok(crate::process::Output {
        status,
        stdout: crate::process::collect_reader(stdout, "stdout")?,
        stderr: crate::process::collect_reader(stderr, "stderr")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_version_parses_nixos_output() {
        assert_eq!(
            parse_state_version("26.05.20261113.f1e5c35 (Yarara)"),
            Some("26.05".into())
        );
        assert_eq!(parse_state_version("26.05"), Some("26.05".into()));
        assert_eq!(parse_state_version(""), None);
        assert_eq!(parse_state_version("not-a-version"), None);
    }

    #[test]
    fn write_generation_owns_the_shape() {
        let mut doc = DesiredDoc::current();
        doc.set_system(vec!["gimp".into()]);
        doc.set_user("alice", vec!["firefox".into()]);
        let dir = std::env::temp_dir().join(format!("np-gen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let hw = dir.join("hw.nix");
        std::fs::write(&hw, "{ ... }: { }").unwrap();
        let cfg = dir.join("cfg.nix");
        std::fs::write(
            &cfg,
            "{ config, pkgs, ... }: { networking.hostName = \"x\"; }",
        )
        .unwrap();
        let gen = dir.join("gen");
        let outer = write_generation(&gen, &doc, "26.05", Some(&cfg), Some(&hw)).unwrap();
        assert_eq!(outer, gen.join("configuration.nix"));

        let outer_text = std::fs::read_to_string(&outer).unwrap();
        // D21: the live config is imported by pointer (absolute path), and a
        // present user config owns stateVersion (so no duplicate line here)
        assert!(outer_text.contains(&cfg.display().to_string()));
        assert!(!outer_text.contains("system.stateVersion"));
        assert!(!outer_text.contains("hardware-configuration.nix"));
        assert!(!outer_text.contains("user-configuration.nix"));
        assert!(outer_text.contains("./generated-desired.nix"));
        // the desired module is written verbatim (byte for byte identical to
        // the renderer with the detected backend, so the build matches UI)
        assert_eq!(
            std::fs::read_to_string(gen.join("generated-desired.nix")).unwrap(),
            doc.render_module(detect_user_backend()).unwrap()
        );
        // nothing is copied into the generation dir
        assert!(!gen.join("user-configuration.nix").exists());
        assert!(!gen.join("hardware-configuration.nix").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_generation_without_preserved_config() {
        let doc = DesiredDoc::current();
        let dir = std::env::temp_dir().join(format!("np-gen2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let outer = write_generation(&dir, &doc, "26.05", None, None).unwrap();
        let text = std::fs::read_to_string(&outer).unwrap();
        // fresh / non-NixOS: only the desired module, and stateVersion is ours
        assert!(!text.contains("user-configuration.nix"));
        assert!(!text.contains("hardware-configuration.nix"));
        assert!(text.contains("./generated-desired.nix"));
        assert!(text.contains("system.stateVersion = \"26.05\";"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Live proof (opt-in): write + dry-build the candidate generation in a
    /// scratch dir against this machine's own nixpkgs. Build-only, ~seconds
    /// because the base system is already in the local store. Never activates.
    /// Uses the real OS user so the per-user entry merges into an account the
    /// preserved config already defines (the renderer adds packages only for
    /// existing accounts); falls back to system scope if USER is unset.
    #[test]
    #[ignore = "requires nixos-rebuild on a NixOS host"]
    fn scratch_dry_build_succeeds() {
        let Some(rebuild) = discover_rebuild() else {
            panic!("nixos-rebuild not found");
        };
        let (cfg, hw) = discover_nixos_sources();
        let Some(sv) = system_state_version() else {
            panic!("not a NixOS system");
        };
        let account = std::env::var("USER").unwrap_or_default();
        let mut doc = DesiredDoc::current();
        if account.is_empty() {
            doc.set_system(vec!["hello".into()]);
        } else {
            doc.set_user(&account, vec!["hello".into()]);
        }
        // user-set settings must render as valid app-wins overrides; include
        // an upkeep (category "updates") option to prove it evaluates too
        use crate::desired_state::SettingValue;
        doc.settings.insert(
            "networking.hostName".into(),
            SettingValue::Str("nix-pilot-scratch".into()),
        );
        doc.settings.insert("nix.gc.automatic".into(), SettingValue::Bool(true));
        doc.settings.insert(
            "system.autoUpgrade.enable".into(),
            SettingValue::Bool(true),
        );
        let dir = std::env::temp_dir().join(format!("np-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let outer = write_generation(&dir, &doc, &sv, cfg.as_deref(), hw.as_deref()).unwrap();
        let b = build_candidate(&rebuild, &outer, true, Duration::from_secs(600));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(b.ok, "dry-build failed:\n{}", b.log);
    }

    /// Live proof (opt-in): write + *build* the candidate generation in a
    /// scratch dir. Build-only (never switches or activates); base system is
    /// cached so this relinks the toplevel with the added package and stops.
    /// Same account handling as the dry-build proof.
    #[test]
    #[ignore = "requires nixos-rebuild on a NixOS host; builds into the store"]
    fn scratch_build_succeeds() {
        let Some(rebuild) = discover_rebuild() else {
            panic!("nixos-rebuild not found");
        };
        let (cfg, hw) = discover_nixos_sources();
        let Some(sv) = system_state_version() else {
            panic!("not a NixOS system");
        };
        let account = std::env::var("USER").unwrap_or_default();
        let mut doc = DesiredDoc::current();
        if account.is_empty() {
            doc.set_system(vec!["hello".into()]);
        } else {
            doc.set_user(&account, vec!["hello".into()]);
        }
        let dir = std::env::temp_dir().join(format!("np-live2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let outer = write_generation(&dir, &doc, &sv, cfg.as_deref(), hw.as_deref()).unwrap();
        let b = build_candidate(&rebuild, &outer, false, Duration::from_secs(1200));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(b.ok, "build failed:\n{}", b.log);
    }

    #[test]
    fn can_activate_false_when_flag_unset() {
        std::env::remove_var("NIX_PILOT_ALLOW_SWITCH");
        assert!(!can_activate());
    }

    #[test]
    fn activate_fails_closed_when_not_enabled() {
        // Regardless of caller, the elevated helper must be unreachable unless
        // activation is genuinely enabled (defense-in-depth, D24 gating).
        std::env::remove_var("NIX_PILOT_ALLOW_SWITCH");
        let doc = crate::desired_state::DesiredDoc::current();
        let r = activate(
            ActivateAction::Switch,
            &doc,
            std::time::Duration::from_secs(5),
        );
        assert!(!r.ok);
    }

    #[test]
    fn tar_contains_exactly_two_files_and_is_deterministic() {
        let mut doc = DesiredDoc::current();
        doc.set_system(vec!["gimp".into()]);
        doc.set_user("alice", vec!["firefox".into()]);
        let state_version = "26.05";
        let (preserved, hardware) = discover_nixos_sources();

        let dir1 = std::env::temp_dir().join(format!("np-tar1-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir1);
        std::fs::create_dir_all(&dir1).unwrap();
        let _outer1 = write_generation(
            &dir1,
            &doc,
            state_version,
            preserved.as_deref(),
            hardware.as_deref(),
        )
        .unwrap();
        let tar1 = create_tar_gz(&dir1).unwrap();
        let _ = std::fs::remove_dir_all(&dir1);

        let dir2 = std::env::temp_dir().join(format!("np-tar2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir2);
        std::fs::create_dir_all(&dir2).unwrap();
        let _outer2 = write_generation(
            &dir2,
            &doc,
            state_version,
            preserved.as_deref(),
            hardware.as_deref(),
        )
        .unwrap();
        let tar2 = create_tar_gz(&dir2).unwrap();
        let _ = std::fs::remove_dir_all(&dir2);

        assert_eq!(
            tar1, tar2,
            "tar output must be deterministic across temp dirs"
        );

        // Write tar archives to temp files for listing (stdin from Cursor not supported)
        let tar1_path = std::env::temp_dir().join(format!("np-tar1-list-{}", std::process::id()));
        let tar2_path = std::env::temp_dir().join(format!("np-tar2-list-{}", std::process::id()));
        std::fs::write(&tar1_path, &tar1).unwrap();
        std::fs::write(&tar2_path, &tar2).unwrap();

        use std::process::Command;
        let list1 = Command::new("tar")
            .args(["-tzvf", &tar1_path.to_string_lossy()])
            .output()
            .unwrap();
        let list2 = Command::new("tar")
            .args(["-tzvf", &tar2_path.to_string_lossy()])
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&tar1_path);
        let _ = std::fs::remove_file(&tar2_path);
        assert!(list1.status.success());
        assert!(list2.status.success());
        let files1: Vec<_> = String::from_utf8_lossy(&list1.stdout)
            .lines()
            .map(|l| l.split_whitespace().last().unwrap_or("").to_string())
            .collect();
        let files2: Vec<_> = String::from_utf8_lossy(&list2.stdout)
            .lines()
            .map(|l| l.split_whitespace().last().unwrap_or("").to_string())
            .collect();
        assert_eq!(
            files1.len(),
            2,
            "tar must contain exactly two files, got: {:?}",
            files1
        );
        assert_eq!(files2.len(), 2);
        assert!(files1.contains(&"configuration.nix".to_string()));
        assert!(files1.contains(&"generated-desired.nix".to_string()));
    }

    #[test]
    fn activate_action_enum_values() {
        let _switch = ActivateAction::Switch;
        let _rollback = ActivateAction::Rollback;
        assert_ne!(ActivateAction::Switch, ActivateAction::Rollback);
    }
}
