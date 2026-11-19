//! Apply backend (7wo.2): turn the desired-state document into a *built
//! candidate generation*. Working rule (failure-and-recovery.md + the
//! project's safety commitment): this module writes only into an explicit
//! generation directory and only ever *builds* — `nixos-rebuild build` /
//! `dry-build` — it never switches or activates a system. Activation and
//! rollback stay behind the transaction machine (`updates::TxStep`) and are
//! the remaining 7wo.2 work; nothing here can change the running system.
//!
//! Generated-config shape (v1, revised by D21): the app owns the outer
//! generated `configuration.nix`, which *imports the live pre-existing
//! `configuration.nix` by absolute path* (never a copy — hand-edits are
//! picked up on every build) plus the desired-state module verbatim.
//! `system.stateVersion` is emitted only when there is no pre-existing
//! config to own it. The settings surface will own baseline options (boot,
//! network, …) later; the shape is recorded on the 7wo.3 beads task.
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::desired_state::DesiredDoc;

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
    std::fs::write(
        dir.join(DESIRED_MODULE_NAME),
        doc.render_module(detect_user_backend()),
    )?;

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
        parts.insert(0, format!("{}", timeout.as_secs()));
        parts.insert(0, t);
    }
    let timeout = timeout.max(Duration::from_secs(5));

    let mut cmd = std::process::Command::new(&parts[0]);
    // deterministic: always evaluate exactly this config file, no matter what
    // the ambient environment says (a no-op when NIXOS_CONFIG is unset; a
    // guarantee we can never accidentally build a different file)
    cmd.env("NIXOS_CONFIG", config.as_os_str())
        .args(&parts[1..])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CandidateBuild {
                ok: false,
                note: "could not start nixos-rebuild".into(),
                log: e.to_string(),
            }
        }
    };
    // drain pipes on reader threads so a big build log can never deadlock us
    let out_reader = child.stdout.take().map(|mut o| {
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut o, &mut s);
            s
        })
    });
    let err_reader = child.stderr.take().map(|mut e| {
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut e, &mut s);
            s
        })
    });
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = out_reader.map(|t| t.join());
                    let _ = err_reader.map(|t| t.join());
                    return CandidateBuild {
                        ok: false,
                        note: "the build took too long".into(),
                        log: "the build did not finish in time".into(),
                    };
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return CandidateBuild {
                    ok: false,
                    note: "nixos-rebuild failed to run".into(),
                    log: e.to_string(),
                }
            }
        }
    };
    let stdout = out_reader
        .map(|t| t.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = err_reader
        .map(|t| t.join().unwrap_or_default())
        .unwrap_or_default();
    let out = format!("{stdout}\n{stderr}");
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
        ok: status.success(),
        note: if status.success() {
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
            doc.render_module(detect_user_backend())
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
        // a user-set setting must render as a valid app-wins override
        doc.settings.insert(
            "networking.hostName".into(),
            crate::desired_state::SettingValue::Str("nix-pilot-scratch".into()),
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
}
