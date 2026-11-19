//! Desired-state model (7wo.2): the app owns *what the user wants*, and the
//! NixOS configuration is generated output (architecture / Product model,
//! decision D9). This module is the persisted source of truth plus a pure
//! config renderer. This module performs no external I/O; the candidate-build
//! adapter consumes the rendered module, while activation is handled
//! separately. Per-user scope targets `users.users.<name>.packages` or, when
//! selected, `home-manager.users.<name>.home.packages`.
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
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Current persisted schema version. Bump on any additive/non-additive change
/// and add a step to `migrate` so older files upgrade before use.
pub const CURRENT_SCHEMA: u8 = 1;

/// The NixOS account that owns a per-user package selection.
pub type Account = String;

/// A user-set setting value (settings surface v1: strings and booleans).
/// Stored only for settings the user actively changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SettingValue {
    Str(String),
    Bool(bool),
}

/// Which per-user package mechanism the render targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserBackend {
    /// NixOS-native: `users.users.<name>.packages` (default, decision D9).
    NixUsers,
    /// home-manager: `home-manager.users.<name>.home.packages` — used when
    /// the system is home-manager managed (decision D22).
    HomeManager,
}

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
    /// User-set settings (key = NixOS option path; settings surface, D14).
    /// Additive and optional, so pre-existing v1 files parse unchanged.
    #[serde(default)]
    pub settings: BTreeMap<String, SettingValue>,
    /// Per-account SSH authorized public keys (remote-access workflow in the
    /// Settings surface, D14 scoped): renders as
    /// `users.users.<name>.openssh.authorizedKeys.keys`. Additive and
    /// optional; pre-existing v1 files parse unchanged.
    #[serde(default)]
    pub authorized_keys: BTreeMap<Account, Vec<String>>,
}

impl Default for DesiredDoc {
    fn default() -> Self {
        DesiredDoc {
            schema_version: CURRENT_SCHEMA,
            system: Packages::default(),
            users: BTreeMap::new(),
            settings: BTreeMap::new(),
            authorized_keys: BTreeMap::new(),
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

    /// The given account's authorized SSH public keys (empty if absent).
    pub fn authorized_keys_for(&self, account: &str) -> &[String] {
        self.authorized_keys
            .get(account)
            .map(|k| k.as_slice())
            .unwrap_or(&[])
    }

    /// Replace one account's authorized SSH public keys; an empty set removes
    /// the entry so the file stays free of empty sections.
    pub fn set_authorized_keys(&mut self, account: &str, keys: Vec<String>) {
        let mut keys: Vec<String> = keys
            .into_iter()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        keys.sort();
        keys.dedup();
        if keys.is_empty() {
            self.authorized_keys.remove(account);
        } else {
            self.authorized_keys.insert(account.to_string(), keys);
        }
    }

    /// Validate every value that will be inserted into generated Nix syntax.
    pub fn validate(&self) -> Result<(), StateValidationError> {
        for attr in &self.system.packages {
            validate_package_attr(attr)?;
        }
        for (account, packages) in &self.users {
            if !is_nix_identifier(account) {
                return Err(StateValidationError::Account(account.clone()));
            }
            for attr in &packages.packages {
                validate_package_attr(attr)?;
            }
        }
        for (account, keys) in &self.authorized_keys {
            if !is_nix_identifier(account) {
                return Err(StateValidationError::Account(account.clone()));
            }
            for key in keys {
                if key.contains('\n') {
                    return Err(StateValidationError::Setting {
                        key: format!("users.{account}.openssh.authorizedKeys.keys"),
                        reason: "a key must be a single line".into(),
                    });
                }
            }
        }
        for (key, value) in &self.settings {
            crate::settings::validate(key, value).map_err(|reason| {
                StateValidationError::Setting {
                    key: key.clone(),
                    reason,
                }
            })?;
        }
        Ok(())
    }

    /// Render the full generated NixOS module (system scope + every account's
    /// per-user packages through the chosen backend). This is what the backend
    /// would feed `nixos-rebuild`; byte-deterministic. Note: per-user entries
    /// assume the account already exists (defined by the preserved/system
    /// config) — the app adds *packages* for existing accounts, it never
    /// creates accounts.
    pub fn render_module(&self, backend: UserBackend) -> Result<String, StateValidationError> {
        self.validate()?;
        let mut body = String::new();
        if !self.system.packages.is_empty() {
            body.push_str(&format!(
                "  environment.systemPackages = [ {} ];\n",
                render_package_list(&self.system.packages)
            ));
        }
        // Per-account installs; authorized SSH keys are NixOS-native only
        // (home-manager has no equivalent in this scope).
        // Per-account installs and authorized SSH keys (NixOS-native only;
        // home-manager has no authorized-key equivalent in this scope).
        // Accounts come from the union of package users and key users, sorted.
        let mut accounts: Vec<&String> = self
            .users
            .keys()
            .chain(self.authorized_keys.keys())
            .collect();
        accounts.sort();
        accounts.dedup();
        for account in accounts {
            let pkgs = self.users.get(account).cloned().unwrap_or_default();
            let keys = self.authorized_keys_for(account);
            if pkgs.packages.is_empty() && keys.is_empty() {
                continue;
            }
            match backend {
                UserBackend::NixUsers => {
                    let mut lines = vec![format!("  users.users.\"{account}\" = {{")];
                    if !pkgs.packages.is_empty() {
                        lines.push(format!(
                            "    packages = [ {} ];",
                            render_package_list(&pkgs.packages)
                        ));
                    }
                    if !keys.is_empty() {
                        lines.push(format!(
                            "    openssh.authorizedKeys.keys = [ {} ];",
                            render_key_list(keys)
                        ));
                    }
                    lines.push("  };".into());
                    body.push_str(&lines.join("\n"));
                    body.push('\n');
                }
                UserBackend::HomeManager => {
                    if !pkgs.packages.is_empty() {
                        body.push_str(&format!(
                            "  home-manager.users.\"{account}\".home.packages = [ {} ];\n",
                            render_package_list(&pkgs.packages)
                        ));
                    }
                }
            }
        }
        body.push_str(&crate::settings::render_block(&self.settings));
        if body.is_empty() {
            body.push_str("  # no packages selected\n");
        }
        Ok(format!("{{ config, pkgs, lib, ... }}:\n\n{{\n{body}}}\n"))
    }
}

fn is_nix_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '\'' | '-'))
}

fn render_package_list(packages: &[String]) -> String {
    packages
        .iter()
        .map(|attr| {
            let path = attr
                .split('.')
                .map(|component| format!("\"{component}\""))
                .collect::<Vec<_>>()
                .join(".");
            format!("pkgs.{path}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Escape a value for embedding inside a Nix double-quoted string (so no
/// accepted value can ever become Nix code: quotes, backslashes and the `$`
/// interpolation marker are all neutralised).
fn escape_nix_str(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', r#"${"$"}"#)
}

/// Render an SSH authorized-keys list as quoted, escaped Nix strings.
fn render_key_list(keys: &[String]) -> String {
    keys.iter()
        .map(|k| format!("\"{}\"", escape_nix_str(k)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn validate_package_attr(attr: &str) -> Result<(), StateValidationError> {
    if attr.split('.').all(|component| {
        !component.is_empty()
            && component
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '\'' | '-'))
    }) {
        Ok(())
    } else {
        Err(StateValidationError::PackageAttribute(attr.to_string()))
    }
}

/// A desired-state value cannot safely or correctly be represented in Nix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateValidationError {
    PackageAttribute(String),
    Account(String),
    Setting { key: String, reason: String },
}

impl fmt::Display for StateValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PackageAttribute(attr) => {
                write!(f, "package attribute is not a valid Nix path: {attr:?}")
            }
            Self::Account(account) => {
                write!(f, "account name is not a valid Nix identifier: {account:?}")
            }
            Self::Setting { key, reason } => {
                write!(f, "setting {key} is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for StateValidationError {}

/// Errors loading the persisted desired state.
#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    InvalidState(StateValidationError),
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
            LoadError::InvalidState(e) => write!(f, "desired-state file contains invalid data: {e}"),
            LoadError::NewerVersion { found, supported } => write!(
                f,
                "desired-state file was written by a newer version of this app (schema {found}, this app supports up to {supported})"
            ),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::Io(source) => Some(source),
            LoadError::Parse(source) => Some(source),
            LoadError::InvalidState(source) => Some(source),
            LoadError::NewerVersion { .. } => None,
        }
    }
}

impl LoadError {
    pub fn is_missing(&self) -> bool {
        matches!(self, LoadError::Io(error) if error.kind() == io::ErrorKind::NotFound)
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
    doc.validate().map_err(LoadError::InvalidState)?;
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
#[derive(Debug)]
pub enum SaveError {
    Io(io::Error),
    InvalidState(StateValidationError),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Io(error) => write!(f, "could not save desired-state file: {error}"),
            SaveError::InvalidState(error) => {
                write!(f, "refusing to save invalid desired state: {error}")
            }
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SaveError::Io(source) => Some(source),
            SaveError::InvalidState(source) => Some(source),
        }
    }
}

impl From<io::Error> for SaveError {
    fn from(error: io::Error) -> Self {
        SaveError::Io(error)
    }
}

pub fn save(doc: &DesiredDoc, path: &Path) -> Result<(), SaveError> {
    doc.validate().map_err(SaveError::InvalidState)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(doc).expect("DesiredDoc serializes");
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, format!("{text}\n"))?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Copy an unreadable or invalid state file to a unique, private backup path.
/// The source is never modified, so recovery remains possible if the copy fails.
pub fn preserve_unreadable_file(path: &Path) -> io::Result<std::path::PathBuf> {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    for attempt in 0..100 {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let backup = path.with_file_name(format!("{name}.unreadable-{timestamp}-{attempt}.bak"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut destination = match options.open(&backup) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let result = (|| {
            let mut source = File::open(path)?;
            io::copy(&mut source, &mut destination)?;
            destination.sync_all()
        })();
        if let Err(error) = result {
            drop(destination);
            let _ = std::fs::remove_file(&backup);
            return Err(error);
        }
        return Ok(backup);
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique desired-state backup name",
    ))
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
            d.render_module(UserBackend::NixUsers).unwrap(),
            "{ config, pkgs, lib, ... }:\n\n{\n  environment.systemPackages = [ pkgs.\"gimp\" ];\n  users.users.\"alice\" = {\n    packages = [ pkgs.\"firefox\" ];\n  };\n  users.users.\"bob\" = {\n    packages = [ pkgs.\"vscode\" ];\n  };\n}\n"
        );
    }

    #[test]
    fn render_authorized_keys_alongside_packages_and_alone() {
        // a user with packages + keys emits both inside one users.users block
        let mut d = DesiredDoc::current();
        d.set_user("alice", vec!["firefox".into()]);
        d.set_authorized_keys("alice", vec!["ssh-rsa AAAA...".into()]);
        assert_eq!(
            d.render_module(UserBackend::NixUsers).unwrap(),
            "{ config, pkgs, lib, ... }:\n\n{\n  users.users.\"alice\" = {\n    packages = [ pkgs.\"firefox\" ];\n    openssh.authorizedKeys.keys = [ \"ssh-rsa AAAA...\" ];\n  };\n}\n"
        );
        // a user with keys but no packages still renders (not skipped)
        let mut d = DesiredDoc::current();
        d.set_authorized_keys("bob", vec!["k1".into(), "k1".into(), "k0".into()]);
        assert_eq!(
            d.render_module(UserBackend::NixUsers).unwrap(),
            "{ config, pkgs, lib, ... }:\n\n{\n  users.users.\"bob\" = {\n    openssh.authorizedKeys.keys = [ \"k0\" \"k1\" ];\n  };\n}\n"
        );
    }

    #[test]
    fn authorized_keys_sort_dedup_and_empty_removes() {
        let mut d = DesiredDoc::current();
        d.set_authorized_keys(
            "alice",
            vec!["c".into(), "a".into(), "a".into(), "b".into()],
        );
        assert_eq!(
            d.authorized_keys_for("alice"),
            &["a".to_string(), "b".to_string(), "c".to_string()]
        );
        d.set_authorized_keys("alice", vec![]);
        assert!(d.authorized_keys.is_empty());
    }

    #[test]
    fn authorized_keys_reject_embedded_newline() {
        let mut d = DesiredDoc::current();
        d.set_authorized_keys("alice", vec!["a\nb".into()]);
        assert!(d.validate().is_err());
    }

    #[test]
    fn authorized_keys_roundtrip_json() {
        let mut d = DesiredDoc::current();
        d.set_authorized_keys("alice", vec!["pk".into()]);
        let json = serde_json::to_string(&d).unwrap();
        let back: DesiredDoc = serde_json::from_str(&json).unwrap();
        assert_eq!(back.authorized_keys_for("alice"), &["pk".to_string()]);
    }

    #[test]
    fn render_home_manager_backend_emits_home_packages() {
        let d = doc_with(&["gimp"], &[("alice", &["firefox"])]);
        assert_eq!(
            d.render_module(UserBackend::HomeManager).unwrap(),
            "{ config, pkgs, lib, ... }:\n\n{\n  environment.systemPackages = [ pkgs.\"gimp\" ];\n  home-manager.users.\"alice\".home.packages = [ pkgs.\"firefox\" ];\n}\n"
        );
    }

    #[test]
    fn render_empty_is_comment() {
        assert_eq!(
            DesiredDoc::current()
                .render_module(UserBackend::NixUsers)
                .unwrap(),
            "{ config, pkgs, lib, ... }:\n\n{\n  # no packages selected\n}\n"
        );
    }

    #[test]
    fn renderer_rejects_invalid_package_and_account_identifiers() {
        let mut package_doc = DesiredDoc::current();
        package_doc.set_system(vec!["hello; builtins.abort \"injected\"".into()]);
        assert!(matches!(
            package_doc.render_module(UserBackend::NixUsers),
            Err(StateValidationError::PackageAttribute(_))
        ));

        let mut account_doc = DesiredDoc::current();
        account_doc.set_user("alice}.builtins.abort", vec!["firefox".into()]);
        assert!(matches!(
            account_doc.render_module(UserBackend::NixUsers),
            Err(StateValidationError::Account(_))
        ));

        let nested_attr = doc_with(&["python3Packages.requests"], &[]);
        assert!(nested_attr.render_module(UserBackend::NixUsers).is_ok());

        let unusual_attr = doc_with(&["1password", "true"], &[])
            .render_module(UserBackend::NixUsers)
            .unwrap();
        assert!(unusual_attr.contains("pkgs.\"1password\" pkgs.\"true\""));
    }

    #[test]
    fn renderer_rejects_setting_type_mismatches() {
        let mut doc = DesiredDoc::current();
        doc.settings.insert(
            "services.openssh.enable".into(),
            SettingValue::Str("true".into()),
        );
        assert!(matches!(
            doc.render_module(UserBackend::NixUsers),
            Err(StateValidationError::Setting { .. })
        ));
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
    fn unreadable_state_backup_preserves_original_bytes() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("nix-pilot-ds-backup-{}.json", std::process::id()));
        let original = b"not valid json";
        std::fs::write(&dir, original).unwrap();

        let backup = preserve_unreadable_file(&dir).unwrap();
        assert_eq!(std::fs::read(&dir).unwrap(), original);
        assert_eq!(std::fs::read(&backup).unwrap(), original);

        let _ = std::fs::remove_file(&dir);
        let _ = std::fs::remove_file(backup);
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
    fn settings_round_trip_and_render() {
        let mut d = DesiredDoc::current();
        d.settings.insert(
            "networking.hostName".into(),
            SettingValue::Str("myhost".into()),
        );
        d.settings
            .insert("services.openssh.enable".into(), SettingValue::Bool(true));

        let mut dir = std::env::temp_dir();
        dir.push(format!("nix-pilot-settings-{}.json", std::process::id()));
        save(&d, &dir).unwrap();
        let loaded = load(&dir).unwrap();
        assert_eq!(loaded.settings, d.settings);
        let _ = std::fs::remove_file(&dir);

        // rendered into the generated module with app-wins priority
        let nix = d
            .render_module(crate::desired_state::UserBackend::NixUsers)
            .unwrap();
        assert!(nix.contains(&"networking.hostName = lib.mkOverride 10 \"myhost\";".to_string()));
        assert!(nix.contains("services.openssh.enable = lib.mkOverride 10 true;"));
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
        let d = doc_with(
            &["gimp", "1password", "python3Packages.requests"],
            &[("tester", &["firefox", "vscode"])],
        );
        let nix = d.render_module(UserBackend::NixUsers).unwrap();
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
