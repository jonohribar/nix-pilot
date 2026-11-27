//! Candidate builds of the user's own NixOS configuration (D33).
//!
//! This adapter runs `nixos-rebuild build`/`dry-build` against the user's
//! real configuration root (flake or classic) after the app has written its
//! `nix-pilot/state.nix` module there. It never activates a system; apply is
//! the privileged helper's job.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::config_source::{ConfigKind, ConfigRoot};
use crate::process;

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

/// Build the user's configuration root (pure; nothing activated).
///
/// `dry` evaluates without building anything into the store; otherwise the
/// candidate system closure is built. The command runs as the current user —
/// build needs no privileges.
pub fn build_user_config(root: &ConfigRoot, dry: bool, timeout: Duration) -> CandidateBuild {
    let Some(rebuild) = discover_rebuild() else {
        return CandidateBuild {
            ok: false,
            note: "nixos-rebuild is not available on this system".into(),
            log: String::new(),
        };
    };
    let mut command = if let Some(timeout_command) = which_timeout() {
        let mut command = Command::new(timeout_command);
        command
            .arg(timeout.as_secs().max(1).to_string())
            .arg(&rebuild);
        command
    } else {
        Command::new(&rebuild)
    };
    command.arg(if dry { "dry-build" } else { "build" });
    match root.kind {
        ConfigKind::Flake => {
            // Without a `#name`, nixos-rebuild selects the machine hostname.
            command.arg("--flake").arg(&root.path);
        }
        ConfigKind::Classic => {
            let config = root.path.join("configuration.nix");
            command
                .arg("-I")
                .arg(format!("nixos-config={}", config.display()))
                .env("NIXOS_CONFIG", config.as_os_str());
        }
    }

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
    let log = process::tail(&combined, 12);
    let dry_prefix = if dry { "dry-" } else { "" };
    CandidateBuild {
        ok: output.status.success(),
        note: if output.status.success() {
            format!("{dry_prefix}build of your configuration succeeded — nothing was activated")
        } else {
            "your configuration did not build".into()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("nix-pilot-build-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A fake `nixos-rebuild` that prints its arguments; the build adapter is
    /// exercised through `NIXOS_REBUILD_BIN` so no real Nix is involved.
    fn fake_rebuild(root: &Path) -> PathBuf {
        let script = root.join("fake-rebuild");
        std::fs::write(&script, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();
        script
    }

    #[test]
    fn flake_root_builds_with_dash_dash_flake() {
        let fixture = Fixture::new("flake-build");
        let rebuild = fake_rebuild(&fixture.0);
        std::env::set_var("NIXOS_REBUILD_BIN", &rebuild);

        let root = ConfigRoot {
            path: fixture.0.clone(),
            kind: ConfigKind::Flake,
        };
        let result = build_user_config(&root, false, Duration::from_secs(2));
        std::env::remove_var("NIXOS_REBUILD_BIN");

        assert!(result.ok, "fake build failed: {}", result.log);
        assert!(result.log.contains("build"));
        assert!(result.log.contains("--flake"));
        assert!(result.log.contains(&fixture.0.display().to_string()));
        assert!(!result.log.contains("nixos-config"));
        assert!(result.note.contains("nothing was activated"));
    }

    #[test]
    fn classic_root_builds_with_pinned_config() {
        let fixture = Fixture::new("classic-build");
        let rebuild = fake_rebuild(&fixture.0);
        std::fs::write(fixture.0.join("configuration.nix"), "{ ... }: { }").unwrap();
        std::env::set_var("NIXOS_REBUILD_BIN", &rebuild);

        let root = ConfigRoot {
            path: fixture.0.clone(),
            kind: ConfigKind::Classic,
        };
        let result = build_user_config(&root, true, Duration::from_secs(2));
        std::env::remove_var("NIXOS_REBUILD_BIN");

        assert!(result.ok, "fake build failed: {}", result.log);
        assert!(result.log.contains("dry-build"));
        assert!(result.log.contains("-I"));
        assert!(result.log.contains(&format!(
            "nixos-config={}",
            fixture.0.join("configuration.nix").display()
        )));
        assert!(!result.log.contains("--flake"));
        assert!(result.note.contains("nothing was activated"));
    }

    #[test]
    #[ignore = "requires nixos-rebuild; dry-builds a scratch NixOS config importing the rendered state"]
    fn scratch_dry_build_with_rendered_state_succeeds() {
        use crate::desired_state::{PackageScope, SettingValue};

        let fixture = Fixture::new("scratch");
        let mut doc = crate::desired_state::DesiredDoc::current();
        doc.add_package("alice", "hello".into(), PackageScope::System);
        for (key, value) in [
            (
                "networking.hostName",
                SettingValue::Str("nix-pilot-scratch".into()),
            ),
            ("nix.gc.automatic", SettingValue::Bool(true)),
            ("services.pipewire.enable", SettingValue::Bool(true)),
        ] {
            doc.settings.insert(key.into(), value);
        }
        let state = crate::system::state_file::write_state_file(&fixture.0, &doc).unwrap();
        std::fs::write(
            fixture.0.join("config.nix"),
            format!(
                "{{ config, pkgs, lib, ... }}:\n{{\n  imports = [ <nixpkgs/nixos> {} ];\n  system.stateVersion = \"26.05\";\n}}\n",
                state.display()
            ),
        )
        .unwrap();
        let root = ConfigRoot {
            path: fixture.0.clone(),
            kind: ConfigKind::Classic,
        };
        let build = build_user_config(&root, true, Duration::from_secs(900));
        assert!(build.ok, "dry-build failed:\n{}", build.log);
    }
}
