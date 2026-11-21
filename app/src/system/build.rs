//! Desired-state configuration generation and candidate builds.
//!
//! This adapter writes only into explicit scratch directories and invokes
//! `nixos-rebuild build` or `dry-build`; it never activates a system. The outer
//! generated configuration imports the live configuration by absolute path so
//! user edits remain authoritative inputs to each candidate build.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::desired_state::DesiredDoc;
use crate::process;

/// Name of the generated desired-state module inside a generation directory.
pub const DESIRED_MODULE_NAME: &str = "generated-desired.nix";
/// Name of the generated outer configuration inside a generation directory.
pub const OUTER_CONFIG_NAME: &str = "configuration.nix";

/// Result of building a candidate configuration.
#[derive(Debug, Clone)]
pub struct CandidateBuild {
    /// Whether the candidate configuration built successfully.
    pub ok: bool,
    /// Short plain-language outcome.
    pub note: String,
    /// Tail of the `nixos-rebuild` output for technical detail.
    pub log: String,
}

/// Resolve `nixos-rebuild` from its override, PATH, or the standard NixOS path.
pub fn discover_rebuild() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("NIXOS_REBUILD_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    if process::run(
        Command::new("nixos-rebuild").arg("--version"),
        Duration::from_secs(5),
    )
    .is_ok()
    {
        return Some(PathBuf::from("nixos-rebuild"));
    }
    let path = PathBuf::from("/run/current-system/sw/bin/nixos-rebuild");
    path.exists().then_some(path)
}

/// Parse a NixOS version string into the `system.stateVersion` value.
pub fn parse_state_version(version: &str) -> Option<String> {
    let first = version.split([' ', '(', '\t']).next().unwrap_or("").trim();
    if first.split('.').count() >= 2 && !first.is_empty() {
        let mut parts = first.split('.');
        Some(format!("{}.{}", parts.next()?, parts.next()?))
    } else {
        None
    }
}

/// Read the running system's state version, or return `None` when unavailable.
pub fn system_state_version() -> Option<String> {
    let output = process::run(&mut Command::new("nixos-version"), Duration::from_secs(5)).ok()?;
    if !output.status.success() {
        return None;
    }
    parse_state_version(&output.stdout)
}

/// Select the per-user package backend for generated configuration.
///
/// Defaults to NixOS-native `users.users`. Home Manager mode is selected only
/// by `NIX_PILOT_USER_BACKEND=home-manager` (or `hm`); it is not inferred from
/// the logged-in user's environment.
pub fn detect_user_backend() -> crate::desired_state::UserBackend {
    match std::env::var("NIX_PILOT_USER_BACKEND").as_deref() {
        Ok("home-manager") | Ok("hm") => crate::desired_state::UserBackend::HomeManager,
        _ => crate::desired_state::UserBackend::NixUsers,
    }
}

/// Write the desired-state module and outer configuration into `dir`.
///
/// The outer file imports `preserved_config` when supplied; otherwise it
/// imports `hardware_config` when available. It always imports the generated
/// desired-state module and emits `system.stateVersion` only when there is no
/// preserved configuration. Returns the outer configuration path.
///
/// # Errors
/// Returns an I/O error for directory or file operations, or an
/// `InvalidData` I/O error if the desired state cannot be rendered.
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
    if let Some(user_config) = preserved_config {
        imports.push(format!("{}", user_config.display()));
    } else if let Some(hardware) = hardware_config {
        imports.push(format!("{}", hardware.display()));
    }
    imports.push(format!("./{DESIRED_MODULE_NAME}"));
    let list = imports
        .iter()
        .map(|import| format!("    {import}"))
        .collect::<Vec<_>>()
        .join("\n");
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

/// Generate and build a candidate in a fresh system-temporary directory.
///
/// This operation never activates the candidate. The temporary directory is
/// removed after writing or building, including failure paths.
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
        Ok(path) => path,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&dir);
            return CandidateBuild {
                ok: false,
                note: "could not write the candidate generation".into(),
                log: error.to_string(),
            };
        }
    };
    let result = build_candidate(&rebuild, &outer, false, timeout);
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Build a candidate from a configuration file in an explicit generation dir.
///
/// `nixos-rebuild build` and `dry-build` only build; they do not activate or
/// update the bootloader. The Nix configuration path is pinned to `config`.
pub fn build_candidate(
    rebuild: &Path,
    config: &Path,
    dry: bool,
    timeout: Duration,
) -> CandidateBuild {
    let mut command = if let Some(timeout_command) = which_timeout() {
        let mut command = Command::new(timeout_command);
        command
            .arg(timeout.as_secs().max(1).to_string())
            .arg(rebuild);
        command
    } else {
        Command::new(rebuild)
    };
    command
        .arg(if dry { "dry-build" } else { "build" })
        .arg("-I")
        .arg(format!("nixos-config={}", config.display()))
        .env("NIXOS_CONFIG", config.as_os_str());

    let output = match process::run(&mut command, timeout.max(Duration::from_secs(5))) {
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
    let combined = format!("{}\n{}", output.stdout, output.stderr);
    let log = tail(&combined);
    let dry_prefix = if dry { "dry-" } else { "" };
    CandidateBuild {
        ok: output.status.success(),
        note: if output.status.success() {
            format!(
                "{dry_prefix}build of the candidate generation succeeded — nothing was activated"
            )
        } else {
            "the candidate generation did not build".into()
        },
        log,
    }
}

fn which_timeout() -> Option<String> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|directory| directory.join("timeout"))
            .find(|path| path.exists())
            .map(|path| path.to_string_lossy().into_owned())
    })
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
    use crate::config_source;

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
        doc.add_package(
            "alice",
            "gimp".into(),
            crate::desired_state::PackageScope::System,
        );
        doc.set_user("alice", vec!["firefox".into()]);
        let dir = std::env::temp_dir().join(format!("np-gen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let hardware = dir.join("hw.nix");
        std::fs::write(&hardware, "{ ... }: { }").unwrap();
        let config = dir.join("cfg.nix");
        std::fs::write(
            &config,
            "{ config, pkgs, ... }: { networking.hostName = \"x\"; }",
        )
        .unwrap();
        let generation_dir = dir.join("gen");
        let outer = write_generation(
            &generation_dir,
            &doc,
            "26.05",
            Some(&config),
            Some(&hardware),
        )
        .unwrap();
        assert_eq!(outer, generation_dir.join("configuration.nix"));

        let outer_text = std::fs::read_to_string(&outer).unwrap();
        assert!(outer_text.contains(&config.display().to_string()));
        assert!(!outer_text.contains("system.stateVersion"));
        assert!(!outer_text.contains("hardware-configuration.nix"));
        assert!(!outer_text.contains("user-configuration.nix"));
        assert!(outer_text.contains("./generated-desired.nix"));
        assert_eq!(
            std::fs::read_to_string(generation_dir.join(DESIRED_MODULE_NAME)).unwrap(),
            doc.render_module(detect_user_backend()).unwrap()
        );
        assert!(!generation_dir.join("user-configuration.nix").exists());
        assert!(!generation_dir.join("hardware-configuration.nix").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_generation_without_preserved_config() {
        let doc = DesiredDoc::current();
        let dir = std::env::temp_dir().join(format!("np-gen2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let outer = write_generation(&dir, &doc, "26.05", None, None).unwrap();
        let text = std::fs::read_to_string(&outer).unwrap();
        assert!(!text.contains("user-configuration.nix"));
        assert!(!text.contains("hardware-configuration.nix"));
        assert!(text.contains("./generated-desired.nix"));
        assert!(text.contains("system.stateVersion = \"26.05\";"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn candidate_command_is_explicitly_build_only_and_pins_its_config() {
        use std::os::unix::fs::PermissionsExt;

        let directory =
            std::env::temp_dir().join(format!("np-build-command-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let script = directory.join("fake-rebuild");
        std::fs::write(&script, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();
        let config = directory.join("configuration.nix");

        let result = build_candidate(&script, &config, false, Duration::from_secs(2));
        let _ = std::fs::remove_dir_all(&directory);
        assert!(result.ok, "fake build failed: {}", result.log);
        assert!(result.log.contains("build"));
        assert!(result.log.contains("-I"));
        assert!(result
            .log
            .contains(&format!("nixos-config={}", config.display())));
        assert!(result.note.contains("nothing was activated"));
    }

    #[test]
    #[ignore = "requires nixos-rebuild on a NixOS host"]
    fn scratch_dry_build_succeeds() {
        let Some(rebuild) = discover_rebuild() else {
            panic!("nixos-rebuild not found");
        };
        let (config, hardware) = config_source::discover()
            .map(|sources| (Some(sources.configuration), sources.hardware_configuration))
            .unwrap_or((None, None));
        let Some(state_version) = system_state_version() else {
            panic!("not a NixOS system");
        };
        let account = std::env::var("USER").unwrap_or_default();
        let mut doc = DesiredDoc::current();
        if account.is_empty() {
            doc.add_package(
                "alice",
                "hello".into(),
                crate::desired_state::PackageScope::System,
            );
        } else {
            doc.set_user(&account, vec!["hello".into()]);
        }
        use crate::desired_state::SettingValue;
        for (key, value) in [
            (
                "networking.hostName",
                SettingValue::Str("nix-pilot-scratch".into()),
            ),
            ("nix.gc.automatic", SettingValue::Bool(true)),
            ("system.autoUpgrade.enable", SettingValue::Bool(true)),
            ("hardware.bluetooth.enable", SettingValue::Bool(true)),
            ("hardware.bluetooth.powerOnBoot", SettingValue::Bool(false)),
            ("services.pipewire.enable", SettingValue::Bool(true)),
        ] {
            doc.settings.insert(key.into(), value);
        }
        let dir = std::env::temp_dir().join(format!("np-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let outer = write_generation(
            &dir,
            &doc,
            &state_version,
            config.as_deref(),
            hardware.as_deref(),
        )
        .unwrap();
        let build = build_candidate(&rebuild, &outer, true, Duration::from_secs(600));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(build.ok, "dry-build failed:\n{}", build.log);
    }

    #[test]
    #[ignore = "requires nixos-rebuild on a NixOS host; builds into the store"]
    fn scratch_build_succeeds() {
        let Some(rebuild) = discover_rebuild() else {
            panic!("nixos-rebuild not found");
        };
        let (config, hardware) = config_source::discover()
            .map(|sources| (Some(sources.configuration), sources.hardware_configuration))
            .unwrap_or((None, None));
        let Some(state_version) = system_state_version() else {
            panic!("not a NixOS system");
        };
        let account = std::env::var("USER").unwrap_or_default();
        let mut doc = DesiredDoc::current();
        if account.is_empty() {
            doc.add_package(
                "alice",
                "hello".into(),
                crate::desired_state::PackageScope::System,
            );
        } else {
            doc.set_user(&account, vec!["hello".into()]);
        }
        let dir = std::env::temp_dir().join(format!("np-live2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let outer = write_generation(
            &dir,
            &doc,
            &state_version,
            config.as_deref(),
            hardware.as_deref(),
        )
        .unwrap();
        let build = build_candidate(&rebuild, &outer, false, Duration::from_secs(1200));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(build.ok, "build failed:\n{}", build.log);
    }
}
