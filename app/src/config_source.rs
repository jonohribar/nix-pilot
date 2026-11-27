//! Discovery of the user's NixOS configuration root.
//!
//! Read-only: it locates the configuration (flake or classic) and never
//! builds, evaluates, or modifies it.

use std::path::{Path, PathBuf};

/// The user's NixOS configuration root and how to rebuild it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigRoot {
    /// Directory holding the configuration (flake or classic layout).
    pub path: PathBuf,
    /// Whether the root is a flake or a classic `configuration.nix` layout.
    pub kind: ConfigKind,
}

/// How a configuration root is rebuilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigKind {
    /// A `flake.nix` layout; rebuilt with `--flake`.
    Flake,
    /// A classic `configuration.nix` layout; rebuilt with `-I nixos-config`.
    Classic,
}

/// Environment override naming the user's configuration root (dev/test hook).
pub const CONFIG_ROOT_ENV: &str = "NIX_PILOT_CONFIG_DIR";

/// Resolve the user's configuration root: the env override first (when it
/// names a real config), then the conventional locations.
pub fn resolve_config_root() -> Option<ConfigRoot> {
    if let Some(dir) = std::env::var_os(CONFIG_ROOT_ENV) {
        if let Some(root) = classify_root(&PathBuf::from(dir)) {
            return Some(root);
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    resolve_in(conventional_candidates(home.as_deref()))
}

/// Conventional configuration locations: well-known home flake directories,
/// then the system path. A flake is an actively-maintained config, while a
/// classic `/etc/nixos` on a flake-driven machine is usually a stale copy a
/// rebuild ignores — so a flake anywhere wins over a classic `/etc/nixos`.
fn conventional_candidates(home: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = home {
        for relative in ["nixos-config", "Projects/nixos-config", "dotfiles", "nixos"] {
            candidates.push(home.join(relative));
        }
    }
    candidates.push(PathBuf::from("/etc/nixos"));
    candidates
}

/// First flake wins (home dirs are scanned first), else the first classic.
fn resolve_in(candidates: Vec<PathBuf>) -> Option<ConfigRoot> {
    let mut classic = None;
    for candidate in candidates {
        if let Some(root) = classify_root(&candidate) {
            match root.kind {
                ConfigKind::Flake => return Some(root),
                ConfigKind::Classic if classic.is_none() => classic = Some(root),
                ConfigKind::Classic => {}
            }
        }
    }
    classic
}

/// Classify a directory as a configuration root, or `None` when it is not one.
fn classify_root(path: &Path) -> Option<ConfigRoot> {
    if !path.is_dir() {
        return None;
    }
    if path.join("flake.nix").is_file() {
        return Some(ConfigRoot {
            path: path.to_path_buf(),
            kind: ConfigKind::Flake,
        });
    }
    path.join("configuration.nix")
        .is_file()
        .then(|| ConfigRoot {
            path: path.to_path_buf(),
            kind: ConfigKind::Classic,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "nix-pilot-config-source-{}-{name}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, name: &str) {
            std::fs::write(self.0.join(name), "{}").unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn classifies_flake_and_classic_roots() {
        let flake = Fixture::new("root-flake");
        flake.write("flake.nix");
        assert_eq!(
            classify_root(&flake.0),
            Some(ConfigRoot {
                path: flake.0.clone(),
                kind: ConfigKind::Flake,
            })
        );

        let classic = Fixture::new("root-classic");
        classic.write("configuration.nix");
        assert_eq!(
            classify_root(&classic.0),
            Some(ConfigRoot {
                path: classic.0.clone(),
                kind: ConfigKind::Classic,
            })
        );

        let empty = Fixture::new("root-empty");
        assert_eq!(classify_root(&empty.0), None);
        assert_eq!(classify_root(Path::new("/definitely/not/here")), None);
    }

    #[test]
    fn conventional_candidates_scan_home_before_the_system_path() {
        let home =
            std::env::temp_dir().join(format!("nix-pilot-candidates-{}", std::process::id()));
        let candidates = conventional_candidates(Some(&home));
        assert_eq!(candidates.last(), Some(&PathBuf::from("/etc/nixos")));
        assert_eq!(candidates[0], home.join("nixos-config"));
    }

    #[test]
    fn resolve_in_prefers_a_flake_over_an_earlier_classic() {
        let classic = Fixture::new("classic-first");
        classic.write("configuration.nix");
        let flake = Fixture::new("flake-second");
        flake.write("flake.nix");
        assert_eq!(
            resolve_in(vec![classic.0.clone(), flake.0.clone()]),
            Some(ConfigRoot {
                path: flake.0.clone(),
                kind: ConfigKind::Flake,
            })
        );
    }

    #[test]
    fn resolve_in_takes_the_first_classic_without_a_flake() {
        let first = Fixture::new("classic-a");
        first.write("configuration.nix");
        let second = Fixture::new("classic-b");
        second.write("configuration.nix");
        assert_eq!(
            resolve_in(vec![first.0.clone(), second.0.clone()]),
            Some(ConfigRoot {
                path: first.0.clone(),
                kind: ConfigKind::Classic,
            })
        );
    }

    #[test]
    fn resolve_config_root_honours_the_override() {
        let flake = Fixture::new("resolve-flake");
        flake.write("flake.nix");
        std::env::set_var(CONFIG_ROOT_ENV, &flake.0);
        assert_eq!(
            resolve_config_root(),
            Some(ConfigRoot {
                path: flake.0.clone(),
                kind: ConfigKind::Flake,
            })
        );

        let classic = Fixture::new("resolve-classic");
        classic.write("configuration.nix");
        std::env::set_var(CONFIG_ROOT_ENV, &classic.0);
        assert_eq!(
            resolve_config_root(),
            Some(ConfigRoot {
                path: classic.0.clone(),
                kind: ConfigKind::Classic,
            })
        );
        std::env::remove_var(CONFIG_ROOT_ENV);
    }
}
