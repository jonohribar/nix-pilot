//! Desired-state model (7wo.2): the app owns *what the user wants*, and the
//! NixOS configuration is generated output (architecture / Product model,
//! decision D9). This module is the persisted source of truth plus a pure
//! config renderer. Nothing here touches Nix; the build/apply backend that
//! consumes the rendered module stays gated (7wo.2) until it lands.
//!
//! Format: JSON, one file per running account (the account's per-user scope
//! plus the machine's system scope). It is app-rewritten state — users never
//! hand-edit it (C2) — so JSON's strong typing and deterministic output beat
//! comment-capable formats. Deterministic: packages sort, keys sort. Versioned
//! (`schema_version`) so the file can grow additively (e.g. settings values
//! per settings-schema.md) without breaking older app installs, and a file
//! written by a *newer* version is refused with a clear message rather than
//! misparsed. Writes are atomic (temp file + rename) so a crash never
//! corrupts the source of truth.
//!
//! Multi-account note: the schema already carries a per-account `users` map.
//! Today the running account writes its own file (system scope + its own user
//! scope). A future machine-wide/merged layout (elevation, multiple accounts)
//! is a `schema_version` migration on top of this shape, not a rewrite.
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Current persisted schema version. Bump on any additive/non-additive change
/// and add a step to `migrate` so older files upgrade before use.
pub const CURRENT_SCHEMA: u8 = 1;

/// The NixOS account that owns a per-user package selection.
pub type Account = String;

/// A set of package attributes (nixpkgs attribute names, e.g. `firefox`).
/// Held as a sorted, deduped Vec for clean JSON and stable diffs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packages {
    #[serde(default)]
    pub packages: Vec<String>,
}

impl Packages {
    /// Build a sorted, deduped package set from any input order.
    pub fn new(mut packages: Vec<String>) -> Packages {
        packages.sort();
        packages.dedup();
        Packages { packages }
    }
}

/// The persisted desired-state document: the app-owned source of truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesiredDoc {
    pub schema_version: u8,
    #[serde(default)]
    pub system: Packages,
    #[serde(default)]
    pub users: BTreeMap<Account, Packages>,
}

impl Default for DesiredDoc {
    fn default() -> Self {
        DesiredDoc {
            schema_version: CURRENT_SCHEMA,
            system: Packages::default(),
            users: BTreeMap::new(),
        }
    }
}

impl DesiredDoc {
    /// The current-version empty document.
    pub fn current() -> DesiredDoc {
        DesiredDoc::default()
    }

    /// System-wide package attributes (always sorted).
    pub fn system_packages(&self) -> &[String] {
        &self.system.packages
    }

    /// A given account's per-user package attributes (empty if absent).
    pub fn packages_for(&self, account: &str) -> &[String] {
        self.users
            .get(account)
            .map(|p| p.packages.as_slice())
            .unwrap_or(&[])
    }

    /// Replace the system-wide package set.
    pub fn set_system(&mut self, packages: Vec<String>) {
        self.system = Packages::new(packages);
    }

    /// Replace one account's per-user package set; an empty set removes the
    /// account entry, keeping the file free of empty sections.
    pub fn set_user(&mut self, account: &str, packages: Vec<String>) {
        if packages.is_empty() {
            self.users.remove(account);
        } else {
            self.users
                .insert(account.to_string(), Packages::new(packages));
        }
    }

    /// Render the full generated NixOS module (system scope + every account).
    /// This is what the backend would feed `nixos-rebuild`; byte-deterministic.
    /// Note: per-user entries assume the account already exists (defined by
    /// the preserved/system config) — the app adds *packages* for existing
    /// accounts, it never creates accounts.
    pub fn render_module(&self) -> String {
        let mut body = String::new();
        if !self.system.packages.is_empty() {
            body.push_str(&format!(
                "  environment.systemPackages = with pkgs; [ {} ];\n",
                self.system.packages.join(" ")
            ));
        }
        for (account, pkgs) in &self.users {
            if pkgs.packages.is_empty() {
                continue;
            }
            body.push_str(&format!(
                "  users.users.{} = {{\n    packages = with pkgs; [ {} ];\n  }};\n",
                account,
                pkgs.packages.join(" ")
            ));
        }
        if body.is_empty() {
            body.push_str("  # no packages selected\n");
        }
        format!("{{ config, pkgs, ... }}:\n\n{{\n{body}}}\n")
    }
}

/// Errors loading the persisted desired state.
#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    /// The file was written by a newer app than the one reading it.
    NewerVersion {
        found: u8,
        supported: u8,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "could not read desired-state file: {e}"),
            LoadError::Parse(e) => write!(f, "desired-state file is not valid: {e}"),
            LoadError::NewerVersion { found, supported } => write!(
                f,
                "desired-state file was written by a newer version of this app (schema {found}, this app supports up to {supported})"
            ),
        }
    }
}

/// Loose top-level parse that tolerates unknown fields, so a newer file is
/// detected by its own version field before strict field validation.
#[derive(Deserialize)]
struct FileHeader {
    schema_version: u8,
}

/// Load a desired-state file. Exists as `Option`, errors through `LoadError`.
pub fn load(path: &Path) -> Result<DesiredDoc, LoadError> {
    let text = std::fs::read_to_string(path).map_err(LoadError::Io)?;
    let header: FileHeader = serde_json::from_str(&text).map_err(LoadError::Parse)?;
    if header.schema_version > CURRENT_SCHEMA {
        return Err(LoadError::NewerVersion {
            found: header.schema_version,
            supported: CURRENT_SCHEMA,
        });
    }
    let doc: DesiredDoc = serde_json::from_str(&text).map_err(LoadError::Parse)?;
    Ok(migrate(doc))
}

/// Upgrade an older-version document to CURRENT_SCHEMA. Add a step here for
/// each version the format outgrows; `load` runs it before the app uses the
/// file. For v1 there are no older formats, so this normalises the version and
/// is otherwise the identity.
fn migrate(mut doc: DesiredDoc) -> DesiredDoc {
    doc.schema_version = CURRENT_SCHEMA;
    doc
}

/// Write the desired state atomically (temp file + rename in the same
/// directory), so a crash mid-write never leaves a truncated source of truth.
pub fn save(doc: &DesiredDoc, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(doc).expect("DesiredDoc serializes");
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, format!("{text}\n"))?;
    std::fs::rename(&tmp, path)
}

/// Default location for this account's desired-state file:
/// $XDG_DATA_HOME/nix-pilot/desired-state.json (fallback ~/.local/share).
pub fn default_desired_path() -> std::path::PathBuf {
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => match std::env::var_os("HOME") {
            Some(home) => {
                let mut p = std::path::PathBuf::from(home);
                p.push(".local/share");
                p
            }
            None => std::path::PathBuf::from("."),
        },
    };
    desired_path_in(&base)
}

fn desired_path_in(base: &Path) -> std::path::PathBuf {
    base.join("nix-pilot").join("desired-state.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_with(system: &[&str], users: &[(&str, &[&str])]) -> DesiredDoc {
        let mut d = DesiredDoc::current();
        d.set_system(system.iter().map(|s| s.to_string()).collect());
        for (acct, pkgs) in users {
            d.set_user(acct, pkgs.iter().map(|s| s.to_string()).collect());
        }
        d
    }

    #[test]
    fn packages_sort_and_dedup() {
        assert_eq!(
            Packages::new(vec!["gimp".into(), "firefox".into(), "gimp".into()]).packages,
            vec!["firefox", "gimp"]
        );
    }

    #[test]
    fn set_user_removes_empty() {
        let mut d = DesiredDoc::current();
        d.set_user("alice", vec!["firefox".into()]);
        assert!(d.users.contains_key("alice"));
        d.set_user("alice", vec![]);
        assert!(!d.users.contains_key("alice"));
    }

    #[test]
    fn render_system_and_each_account() {
        let d = doc_with(&["gimp"], &[("alice", &["firefox"]), ("bob", &["vscode"])]);
        assert_eq!(
            d.render_module(),
            "{ config, pkgs, ... }:\n\n{\n  environment.systemPackages = with pkgs; [ gimp ];\n  users.users.alice = {\n    packages = with pkgs; [ firefox ];\n  };\n  users.users.bob = {\n    packages = with pkgs; [ vscode ];\n  };\n}\n"
        );
    }

    #[test]
    fn render_empty_is_comment() {
        assert_eq!(
            DesiredDoc::current().render_module(),
            "{ config, pkgs, ... }:\n\n{\n  # no packages selected\n}\n"
        );
    }

    #[test]
    fn round_trip_is_deterministic_and_stable() {
        let d = doc_with(&["gimp", "htop"], &[("alice", &["vscode", "firefox"])]);
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "nix-pilot-ds-roundtrip-{}.json",
            std::process::id()
        ));
        save(&d, &dir).unwrap();
        let bytes1 = std::fs::read(&dir).unwrap();
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded, d);
        // saving an identical state produces identical bytes
        save(&loaded, &dir).unwrap();
        let bytes2 = std::fs::read(&dir).unwrap();
        assert_eq!(bytes1, bytes2);
        let _ = std::fs::remove_file(&dir);

        let text = String::from_utf8(bytes1).unwrap();
        assert!(text.contains("\"schema_version\": 1"));
        // sorted output: firefox before vscode
        let f = text.find("firefox").unwrap();
        let v = text.find("vscode").unwrap();
        assert!(f < v);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("nix-pilot-ds-unknown-{}.json", std::process::id()));
        std::fs::write(&dir, "{\"schema_version\":1,\"bogus\":true}").unwrap();
        assert!(matches!(load(&dir), Err(LoadError::Parse(_))));
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn newer_version_is_refused() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("nix-pilot-ds-newer-{}.json", std::process::id()));
        std::fs::write(&dir, "{\"schema_version\":2}").unwrap();
        assert!(matches!(
            load(&dir),
            Err(LoadError::NewerVersion {
                found: 2,
                supported: 1
            })
        ));
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn older_version_normalises_to_current() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("nix-pilot-ds-older-{}.json", std::process::id()));
        // hypothetical legacy file with the same shape but version 0
        std::fs::write(&dir, "{\"schema_version\":0}").unwrap();
        assert_eq!(load(&dir).unwrap().schema_version, CURRENT_SCHEMA);
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn path_under_is_stable() {
        assert_eq!(
            desired_path_in(Path::new("/base")),
            Path::new("/base/nix-pilot/desired-state.json")
        );
    }

    /// Live proof (opt-in): the rendered module is *valid Nix*. Runs
    /// `nix-instantiate --parse` on it. Safe: parse only, never evaluates or
    /// builds. Needs nix-instantiate on PATH.
    #[test]
    #[ignore = "requires nix-instantiate on PATH"]
    fn rendered_module_parses_with_nix() {
        let d = doc_with(&["gimp"], &[("tester", &["firefox", "vscode"])]);
        let nix = d.render_module();
        let mut path = std::env::temp_dir();
        path.push(format!("nix-pilot-desired-{}.nix", std::process::id()));
        std::fs::write(&path, &nix).unwrap();
        let out = std::process::Command::new("nix-instantiate")
            .arg("--parse")
            .arg(&path)
            .output();
        let _ = std::fs::remove_file(&path);
        let out = out.expect("run nix-instantiate");
        assert!(
            out.status.success(),
            "generated config did not parse:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
