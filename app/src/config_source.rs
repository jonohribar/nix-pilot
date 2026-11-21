//! Read-only discovery of the conventional NixOS configuration layout.
//!
//! This adapter knows the `/etc/nixos` file convention but does not build,
//! evaluate, or modify configuration. Build and settings adapters may depend
//! on its named source paths and typed unavailable outcomes.

use std::path::{Path, PathBuf};

/// Live configuration files recognized by the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NixOSSources {
    /// Existing conventional `configuration.nix` file.
    pub configuration: PathBuf,
    /// Existing `hardware-configuration.nix`, when present.
    pub hardware_configuration: Option<PathBuf>,
}

/// Why the conventional NixOS configuration could not be discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryError {
    /// The expected NixOS configuration directory does not exist.
    NoNixOSDirectory,
    /// A flake is present, but this application does not edit flake layouts.
    FlakeManaged,
    /// The directory exists but has no conventional `configuration.nix`.
    MissingConfiguration,
}

/// Discover files under the supported `/etc/nixos` layout.
pub fn discover() -> Result<NixOSSources, DiscoveryError> {
    discover_in(Path::new("/etc/nixos"))
}

/// Discover the supported layout under `base`; separated for filesystem tests.
pub fn discover_in(base: &Path) -> Result<NixOSSources, DiscoveryError> {
    if !base.is_dir() {
        return Err(DiscoveryError::NoNixOSDirectory);
    }

    let configuration = base.join("configuration.nix");
    if !configuration.exists() {
        if base.join("flake.nix").exists() {
            return Err(DiscoveryError::FlakeManaged);
        }
        return Err(DiscoveryError::MissingConfiguration);
    }

    let hardware = base.join("hardware-configuration.nix");
    Ok(NixOSSources {
        configuration,
        hardware_configuration: hardware.exists().then_some(hardware),
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
    fn missing_directory_is_not_nixos() {
        let base = std::env::temp_dir().join(format!(
            "nix-pilot-config-source-{}-absent",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        assert_eq!(discover_in(&base), Err(DiscoveryError::NoNixOSDirectory));
    }

    #[test]
    fn empty_directory_has_no_conventional_configuration() {
        let fixture = Fixture::new("empty");
        assert_eq!(
            discover_in(&fixture.0),
            Err(DiscoveryError::MissingConfiguration)
        );
    }

    #[test]
    fn flake_only_layout_is_explicitly_unsupported() {
        let fixture = Fixture::new("flake");
        fixture.write("flake.nix");
        assert_eq!(discover_in(&fixture.0), Err(DiscoveryError::FlakeManaged));
    }

    #[test]
    fn conventional_configuration_allows_missing_hardware_file() {
        let fixture = Fixture::new("configuration-only");
        fixture.write("configuration.nix");
        assert_eq!(
            discover_in(&fixture.0),
            Ok(NixOSSources {
                configuration: fixture.0.join("configuration.nix"),
                hardware_configuration: None,
            })
        );
    }

    #[test]
    fn discovers_both_conventional_files() {
        let fixture = Fixture::new("with-hardware");
        fixture.write("configuration.nix");
        fixture.write("hardware-configuration.nix");
        assert_eq!(
            discover_in(&fixture.0),
            Ok(NixOSSources {
                configuration: fixture.0.join("configuration.nix"),
                hardware_configuration: Some(fixture.0.join("hardware-configuration.nix")),
            })
        );
    }
}
