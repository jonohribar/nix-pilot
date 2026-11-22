//! Desired-state model: the app owns *what the user wants*, and the
//! NixOS configuration is generated output (see architecture.md; decision D9). This
//! module is the persisted source of truth plus a pure
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

pub use crate::setting_value::SettingValue;

/// Current persisted schema version. Bump on any additive/non-additive change
/// and add a step to `migrate` so older files upgrade before use.
pub const CURRENT_SCHEMA: u8 = 1;

/// The NixOS account that owns a per-user package selection.
pub type Account = String;

/// Name of the per-app source overlay written next to the generated module.
/// The generated module imports it via `nixpkgs.overlays` when per-app
/// sources exist; `build.rs` writes the file with this name.
pub const PER_APP_OVERLAY_NAME: &str = "per-app-overlays.nix";

/// Which input a named app is sourced from (version-selection, D27).
/// Absence means the app uses the system's channel (the default); presence
/// sources that one app from a pinned unstable nixpkgs revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerAppSource {
    /// Pinned nixpkgs revision the app is sourced from (the "newest" channel).
    pub rev: String,
    /// Fixed-output hash for the pinned revision, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nar_hash: Option<String>,
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

/// Scope for one selected package attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageScope {
    /// Available to one account through its configured per-user package backend.
    User,
    /// Installed for the machine through the system package set.
    System,
}

/// A package selection projected from the desired state for an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageIntent {
    /// nixpkgs attribute path selected by the user.
    pub attr: String,
    /// Whether this selection belongs to the machine or one account.
    pub scope: PackageScope,
}

/// A set of package attributes (nixpkgs attribute names, e.g. `firefox`).
/// Held as a sorted, deduped Vec for clean JSON and stable diffs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packages {
    /// Sorted, duplicate-free nixpkgs attribute paths.
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

/// An account profile the app owns (user-accounts workflow). The app owns
/// only the accounts it creates; accounts defined by the live config are
/// shown read-only, never here (settings-schema.md, user-accounts design).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountProfile {
    /// Full/display name (`description`), empty when unset.
    #[serde(default)]
    pub full_name: String,
    /// Wheel membership (admin rights).
    #[serde(default)]
    pub admin: bool,
}

impl AccountProfile {
    /// Create an app-owned account profile with its display name and admin role.
    pub fn new(full_name: String, admin: bool) -> AccountProfile {
        AccountProfile { full_name, admin }
    }
}

/// The persisted desired-state document: the app-owned source of truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesiredDoc {
    /// Serialization format version used to reject incompatible future files.
    pub schema_version: u8,
    /// Packages installed for the whole machine.
    #[serde(default)]
    pub system: Packages,
    /// Per-account package selections; keys are Unix account names.
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
    /// Accounts the app owns (user-accounts workflow). Additive and optional;
    /// pre-existing v1 files parse unchanged.
    #[serde(default)]
    pub accounts: BTreeMap<Account, AccountProfile>,
    /// Per-app source inputs (version-selection): which apps come from a
    /// pinned second nixpkgs revision instead of the system channel. Additive
    /// and optional; pre-existing v1 files parse unchanged.
    #[serde(default)]
    pub sources: BTreeMap<String, PerAppSource>,
}

impl Default for DesiredDoc {
    fn default() -> Self {
        DesiredDoc {
            schema_version: CURRENT_SCHEMA,
            system: Packages::default(),
            users: BTreeMap::new(),
            settings: BTreeMap::new(),
            authorized_keys: BTreeMap::new(),
            accounts: BTreeMap::new(),
            sources: BTreeMap::new(),
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

    /// Package selections for the system and one account, in stable order.
    /// The same attribute may appear once in each scope; scope is part of the
    /// identity of a package intent.
    pub fn package_intents_for(&self, account: &str) -> Vec<PackageIntent> {
        self.system_packages()
            .iter()
            .map(|attr| PackageIntent {
                attr: attr.clone(),
                scope: PackageScope::System,
            })
            .chain(self.packages_for(account).iter().map(|attr| PackageIntent {
                attr: attr.clone(),
                scope: PackageScope::User,
            }))
            .collect()
    }

    /// Whether this attribute is selected in either scope used by one
    /// running account: system-wide or that account's per-user set.
    pub fn contains_package_for(&self, account: &str, attr: &str) -> bool {
        self.system_packages().iter().any(|value| value == attr)
            || self.packages_for(account).iter().any(|value| value == attr)
    }

    /// Add one package selection in a specific scope; returns false when it
    /// was already selected there. The other scope remains independent.
    pub fn add_package(&mut self, account: &str, attr: String, scope: PackageScope) -> bool {
        let packages = match scope {
            PackageScope::System => &mut self.system.packages,
            PackageScope::User => &mut self.users.entry(account.to_string()).or_default().packages,
        };
        if packages.contains(&attr) {
            return false;
        }
        packages.push(attr);
        packages.sort();
        true
    }

    /// Remove one package selection from a specific scope.
    pub fn remove_package(&mut self, account: &str, attr: &str, scope: PackageScope) -> bool {
        match scope {
            PackageScope::System => {
                let before = self.system.packages.len();
                self.system.packages.retain(|value| value != attr);
                before != self.system.packages.len()
            }
            PackageScope::User => {
                let mut packages = self.packages_for(account).to_vec();
                let before = packages.len();
                packages.retain(|value| value != attr);
                if before == packages.len() {
                    return false;
                }
                self.set_user(account, packages);
                true
            }
        }
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

    /// App-owned account profiles, by name (empty if none).
    pub fn accounts(&self) -> impl Iterator<Item = (&str, &AccountProfile)> {
        self.accounts.iter().map(|(a, p)| (a.as_str(), p))
    }

    /// Create/update an app-owned account profile.
    pub fn set_account(&mut self, name: &str, profile: AccountProfile) {
        self.accounts.insert(name.to_string(), profile);
    }

    /// Remove an app-owned account (cleanly un-creates it in the generated
    /// config, since the app owns the definition — settings-schema.md).
    pub fn remove_account(&mut self, name: &str) {
        self.accounts.remove(name);
    }

    /// Validate values and names before they are used in generated Nix syntax.
    ///
    /// # Errors
    /// Returns the first invalid package attribute, account name, or setting.
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
        for (account, profile) in &self.accounts {
            if !is_nix_identifier(account) {
                return Err(StateValidationError::Account(account.clone()));
            }
            if profile.full_name.contains('\n') {
                return Err(StateValidationError::Setting {
                    key: format!("users.{account}.description"),
                    reason: "the full name must be a single line".into(),
                });
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
        for (attr, source) in &self.sources {
            if !is_nix_identifier(attr)
                || attr.contains('.')
                || attr.chars().next().is_some_and(|c| c.is_ascii_digit())
            {
                return Err(StateValidationError::SourceAttribute(attr.clone()));
            }
            if source.rev.is_empty() || source.rev.chars().any(char::is_whitespace) {
                return Err(StateValidationError::Setting {
                    key: format!("sources.{attr}"),
                    reason: "a pinned revision must be a non-empty, whitespace-free string".into(),
                });
            }
        }
        Ok(())
    }

    /// Render this desired state as a deterministic NixOS module.
    ///
    /// System packages, settings, and per-user packages are emitted for the
    /// selected backend. Authorized keys are emitted only for the NixOS-native
    /// backend. Per-user package and key entries refer to accounts that must
    /// already exist in the system configuration. Separately, profiles in
    /// [`DesiredDoc::accounts`] are app-owned account definitions and are
    /// emitted as NixOS `users.users` declarations.
    ///
    /// # Errors
    /// Returns an error if any value cannot safely or correctly be rendered.
    pub fn render_module(&self, backend: UserBackend) -> Result<String, StateValidationError> {
        self.validate()?;
        let mut body = String::new();
        body.push_str(&render_system_packages(&self.system.packages));
        // Per-account package selections use the chosen backend; authorized
        // keys are NixOS-native because Home Manager has no equivalent here.
        // Accounts come from the union of package users and key users, sorted.
        let mut accounts: Vec<&String> = self
            .users
            .keys()
            .chain(self.authorized_keys.keys())
            .collect();
        accounts.sort();
        accounts.dedup();
        for account in accounts {
            let packages = self.users.get(account).cloned().unwrap_or_default();
            let keys = self.authorized_keys_for(account);
            body.push_str(&render_account_packages(account, &packages, keys, backend));
        }
        // App-owned accounts (NixOS-native; no home-manager equivalent in this
        // scope). The app owns these definitions, so removing one cleanly
        // un-creates it.
        for (account, profile) in &self.accounts {
            body.push_str(&render_owned_account(account, profile));
        }
        body.push_str(&crate::settings::render_block(&self.settings));
        if !self.sources.is_empty() {
            body.insert_str(
                0,
                &format!("  nixpkgs.overlays = [ (import ./{PER_APP_OVERLAY_NAME}) ];\n"),
            );
        }
        if body.is_empty() {
            body.push_str("  # no packages selected\n");
        }
        Ok(format!("{{ config, pkgs, lib, ... }}:\n\n{{\n{body}}}\n"))
    }
}

/// The Nix lines installing the system-wide package list (empty when none).
fn render_system_packages(packages: &[String]) -> String {
    if packages.is_empty() {
        String::new()
    } else {
        format!(
            "  environment.systemPackages = [ {} ];\n",
            render_package_list(packages)
        )
    }
}

/// The Nix lines for one account's packages and authorized keys under the
/// chosen backend (empty when the account has neither). Authorized keys are
/// NixOS-native because Home Manager has no equivalent here.
fn render_account_packages(
    account: &str,
    packages: &Packages,
    keys: &[String],
    backend: UserBackend,
) -> String {
    if packages.packages.is_empty() && keys.is_empty() {
        return String::new();
    }
    match backend {
        UserBackend::NixUsers => {
            let mut lines = vec![format!("  users.users.\"{account}\" = {{")];
            if !packages.packages.is_empty() {
                lines.push(format!(
                    "    packages = [ {} ];",
                    render_package_list(&packages.packages)
                ));
            }
            if !keys.is_empty() {
                lines.push(format!(
                    "    openssh.authorizedKeys.keys = [ {} ];",
                    render_key_list(keys)
                ));
            }
            lines.push("  };".into());
            lines.join("\n") + "\n"
        }
        UserBackend::HomeManager => {
            if packages.packages.is_empty() {
                String::new()
            } else {
                format!(
                    "  home-manager.users.\"{account}\".home.packages = [ {} ];\n",
                    render_package_list(&packages.packages)
                )
            }
        }
    }
}

/// The Nix lines defining one app-owned user account.
fn render_owned_account(account: &str, profile: &AccountProfile) -> String {
    let mut lines = vec![format!("  users.users.\"{account}\" = {{")];
    lines.push("    isNormalUser = true;".into());
    if !profile.full_name.is_empty() {
        lines.push(format!(
            "    description = \"{}\";",
            escape_nix_str(&profile.full_name)
        ));
    }
    if profile.admin {
        lines.push("    extraGroups = [ \"wheel\" ];".into());
    }
    lines.push("  };".into());
    lines.join("\n") + "\n"
}
/// Render the per-app source overlay consumed by the generated module.
///
/// Each pinned revision is imported once (`builtins.fetchTree`); the named
/// apps are overridden from it. Deterministic over the sorted source map.
/// This is the shared per-package pin surface later reused by update recovery.
pub fn render_per_app_overlays(sources: &BTreeMap<String, PerAppSource>) -> String {
    let mut by_rev: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut hash_of: BTreeMap<String, Option<String>> = BTreeMap::new();
    for (attr, source) in sources {
        by_rev
            .entry(source.rev.clone())
            .or_default()
            .push(attr.clone());
        let slot = hash_of.entry(source.rev.clone()).or_insert(None);
        if slot.is_none() {
            *slot = source.nar_hash.clone();
        }
    }

    let mut out = String::from("# generated per-app source overlay\nfinal: prev: let\n");
    let mut import_of: BTreeMap<String, String> = BTreeMap::new();
    for rev in by_rev.keys() {
        let name = format!("p{}", import_of.len());
        let hash_line = match hash_of.get(rev.as_str()) {
            Some(Some(hash)) => format!("      narHash = \"{hash}\";\n"),
            _ => String::new(),
        };
        out.push_str(&format!(
            "  {name} = import (builtins.fetchTree {{
                type = \"github\";
                owner = \"NixOS\";
                repo = \"nixpkgs\";
                rev = \"{rev}\";
                {hash_line}
              }}) {{
                system = prev.stdenv.hostPlatform.system;
              }};
"
        ));
        import_of.insert(rev.clone(), name);
    }
    out.push_str("in {\n");
    for (rev, attrs) in &by_rev {
        let name = import_of.get(rev.as_str()).expect("rev imported above");
        for attr in attrs {
            out.push_str(&format!("  {attr} = {name}.{attr};\n"));
        }
    }
    out.push_str("}\n");
    out
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
    /// A package attribute is not a valid dot-separated Nix package path.
    PackageAttribute(String),
    /// An account name is not valid for use as a NixOS account identifier.
    Account(String),
    /// A setting value failed its setting-specific validation rule.
    Setting { key: String, reason: String },
    /// A per-app source key is not a valid top-level nixpkgs attribute name.
    SourceAttribute(String),
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
            Self::SourceAttribute(attr) => {
                write!(
                    f,
                    "per-app source key is not a valid top-level attribute: {attr:?}"
                )
            }
        }
    }
}

impl std::error::Error for StateValidationError {}

/// Errors loading the persisted desired state.
#[derive(Debug)]
pub enum LoadError {
    /// The file could not be read from storage.
    Io(std::io::Error),
    /// The file was not valid JSON or did not match the stored schema.
    Parse(serde_json::Error),
    /// Parsed state contains a value rejected by desired-state validation.
    InvalidState(StateValidationError),
    /// The file was written by a newer app than the one reading it.
    NewerVersion { found: u8, supported: u8 },
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
    /// Whether loading failed because the file does not exist.
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

/// Read, validate, and migrate a persisted desired-state document.
///
/// A file from a newer schema is rejected without interpreting it as the
/// current format. Supported older documents are migrated before return.
///
/// # Errors
/// Returns [`LoadError`] for I/O, JSON/schema, validation, or newer-version
/// failures. A missing file is reported as [`LoadError::Io`] and can be tested
/// with [`LoadError::is_missing`].
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

/// Failure to validate or persist a desired-state document.
#[derive(Debug)]
pub enum SaveError {
    /// The destination could not be created or written.
    Io(io::Error),
    /// State failed validation and was not written.
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

/// Validate and atomically replace the persisted desired-state file.
///
/// The temporary file is created beside the destination and renamed into
/// place, so an interrupted write does not leave a partially written state
/// document.
///
/// # Errors
/// Returns [`SaveError::InvalidState`] before writing invalid state, or
/// [`SaveError::Io`] if directory creation, writing, or replacement fails.
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
        for attr in system {
            d.add_package("test", (*attr).into(), PackageScope::System);
        }
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
    fn render_app_owned_account_profiles() {
        let mut d = DesiredDoc::current();
        d.set_account("alice", AccountProfile::new("Alice A".into(), true));
        d.set_account("bob", AccountProfile::new(String::new(), false));
        assert_eq!(
            d.render_module(UserBackend::NixUsers).unwrap(),
            "{ config, pkgs, lib, ... }:\n\n{\n  users.users.\"alice\" = {\n    isNormalUser = true;\n    description = \"Alice A\";\n    extraGroups = [ \"wheel\" ];\n  };\n  users.users.\"bob\" = {\n    isNormalUser = true;\n  };\n}\n"
        );
    }

    #[test]
    fn accounts_roundtrip_json_and_removal() {
        let mut d = DesiredDoc::current();
        d.set_account("alice", AccountProfile::new("A".into(), false));
        let back: DesiredDoc = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back.accounts().count(), 1);
        d.remove_account("alice");
        assert!(d.accounts.is_empty());
        assert!(d.validate().is_ok());
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
        package_doc.add_package(
            "alice",
            "hello; builtins.abort \"injected\"".into(),
            PackageScope::System,
        );
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
        let mut d = doc_with(&["gimp", "htop"], &[("alice", &["vscode", "firefox"])]);
        // The serialized model permits one attribute in both scopes, and each
        // selection must survive independently.
        d.add_package("alice", "firefox".into(), PackageScope::System);
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
    fn package_intents_preserve_independent_scopes() {
        let mut document = DesiredDoc::current();
        assert!(document.add_package("alice", "firefox".into(), PackageScope::System));
        assert!(document.add_package("alice", "firefox".into(), PackageScope::User));
        assert!(!document.add_package("alice", "firefox".into(), PackageScope::User));

        assert!(document.contains_package_for("alice", "firefox"));
        assert_eq!(
            document.package_intents_for("alice"),
            vec![
                PackageIntent {
                    attr: "firefox".into(),
                    scope: PackageScope::System,
                },
                PackageIntent {
                    attr: "firefox".into(),
                    scope: PackageScope::User,
                },
            ]
        );

        assert!(document.remove_package("alice", "firefox", PackageScope::User));
        assert_eq!(document.system_packages(), &["firefox"]);
        assert!(document.packages_for("alice").is_empty());
        assert!(document.contains_package_for("alice", "firefox"));
        assert!(!document.remove_package("alice", "firefox", PackageScope::User));
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

    #[test]
    fn render_module_emits_overlays_only_when_sources_exist() {
        let plain = DesiredDoc::default();
        let without = plain.render_module(UserBackend::NixUsers).unwrap();
        assert!(!without.contains("nixpkgs.overlays"));

        let mut doc = DesiredDoc::default();
        doc.sources.insert(
            "firefox".into(),
            PerAppSource {
                rev: "a".repeat(40),
                nar_hash: Some("sha256-abc".into()),
            },
        );
        let with = doc.render_module(UserBackend::NixUsers).unwrap();
        assert!(with.contains("nixpkgs.overlays = [ (import ./per-app-overlays.nix) ];"));
    }

    #[test]
    fn module_renders_licence_setting_and_overlays_together() {
        let mut doc = DesiredDoc::current();
        use crate::setting_value::SettingValue;
        doc.settings.insert(
            "nixpkgs.config.allowUnfree".into(),
            SettingValue::Bool(true),
        );
        doc.sources.insert(
            "firefox".into(),
            PerAppSource {
                rev: "a".repeat(40),
                nar_hash: None,
            },
        );
        let nix = doc.render_module(UserBackend::NixUsers).unwrap();
        assert!(nix.contains("nixpkgs.overlays = [ (import ./per-app-overlays.nix) ];"));
        assert!(nix.contains("nixpkgs.config.allowUnfree = lib.mkOverride 10 true;"));
        assert!(doc.validate().is_ok());
    }

    #[test]
    fn per_app_overlays_group_by_rev_and_are_deterministic() {
        let mut sources = BTreeMap::new();
        sources.insert(
            "firefox".into(),
            PerAppSource {
                rev: "a".repeat(40),
                nar_hash: Some("sha256-abc".into()),
            },
        );
        sources.insert(
            "gimp".into(),
            PerAppSource {
                rev: "a".repeat(40),
                nar_hash: None,
            },
        );
        sources.insert(
            "vscode".into(),
            PerAppSource {
                rev: "b".repeat(40),
                nar_hash: None,
            },
        );
        let overlay = render_per_app_overlays(&sources);
        assert!(overlay.starts_with("# generated per-app source overlay"));
        assert!(overlay.contains("builtins.fetchTree"));
        // Same rev imports once; two distinct revs -> two imports.
        assert_eq!(
            overlay.lines().filter(|l| l.contains("fetchTree")).count(),
            2
        );
        // Shorter (lexicographically earlier) rev imports first (p0) and keeps
        // the hash; firefox and gimp share p0, vscode comes from p1.
        let rev_a = "a".repeat(40);
        let rev_b = "b".repeat(40);
        assert!(overlay.contains(&format!("rev = \"{rev_a}\";")));
        assert!(overlay.contains(&format!("rev = \"{rev_b}\";")));
        assert!(overlay.contains("narHash = \"sha256-abc\";"));
        assert!(overlay.contains("firefox = p0.firefox;"));
        assert!(overlay.contains("gimp = p0.gimp;"));
        assert!(overlay.contains("vscode = p1.vscode;"));
        assert_eq!(overlay, render_per_app_overlays(&sources));
    }

    #[test]
    fn validate_rejects_unsafe_source_keys() {
        let mut doc = DesiredDoc::default();
        doc.sources.insert(
            "python3Packages.requests".into(),
            PerAppSource {
                rev: "a".repeat(40),
                nar_hash: None,
            },
        );
        assert!(matches!(
            doc.validate(),
            Err(StateValidationError::SourceAttribute(_))
        ));
    }

    /// Live proof (opt-in): the rendered per-app overlay is *valid Nix*
    /// syntax. `--parse` never evaluates `fetchTree`, so no network is used.
    #[test]
    #[ignore = "requires nix-instantiate on PATH"]
    fn per_app_overlay_parses_with_nix() {
        let mut sources = BTreeMap::new();
        sources.insert(
            "firefox".into(),
            PerAppSource {
                rev: "a".repeat(40),
                nar_hash: Some("sha256-abc".into()),
            },
        );
        let overlay = render_per_app_overlays(&sources);
        let mut path = std::env::temp_dir();
        path.push(format!("nix-pilot-overlay-{}.nix", std::process::id()));
        std::fs::write(&path, &overlay).unwrap();
        let out = std::process::Command::new("nix-instantiate")
            .arg("--parse")
            .arg(&path)
            .output();
        let _ = std::fs::remove_file(&path);
        let out = out.expect("run nix-instantiate");
        assert!(
            out.status.success(),
            "per-app overlay did not parse:\n{}",
            String::from_utf8_lossy(&out.stderr)
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
