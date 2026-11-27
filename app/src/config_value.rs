//! Read-only Nix config value adapter: evaluate a NixOS option from the live
//! system configuration using `nix-instantiate --eval --strict` and parse the
//! result into a small typed enum. Never writes, never edits, never builds.

use std::process::{Command, Stdio};

use crate::config_source;

/// The result of evaluating a NixOS option from the live configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    /// A string value (unquoted, with Nix escapes resolved).
    Str(String),
    /// A boolean value.
    Bool(bool),
    /// Any other Nix value (attrsets, lists, functions, null, etc.) —
    /// represented as the raw printed output for display purposes.
    Unparsed(String),
    /// An open `nixpkgs.config.<subkey>` nobody set, so the system default
    /// applies. Distinct from a missing/removed typed option, which is a
    /// `NoSuchOption` error.
    Unset,
}

/// Error when reading a config value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValueError {
    /// NixOS configuration not found (not on NixOS, or flake-managed).
    NoLiveConfig,
    /// `nix-instantiate` not available on PATH.
    NoNixInstantiate,
    /// The nix-instantiate evaluation failed.
    EvalFailed(String),
    /// The evaluated option does not exist in this NixOS/nixpkgs — removed
    /// or renamed since the setting was authored (currency, settings-schema).
    NoSuchOption(String),
    /// The option path is invalid (empty or malformed).
    InvalidOptionPath(String),
}

/// Sentinel returned by the read expression for an unset open-config subkey.
const UNSET: &str = "__nix_pilot_config_unset__";

/// The read-only Nix expression for `option_path` against the live config,
/// plus whether the path is the open `nixpkgs.config.*` attrset.
///
/// Typed options are read as `.config.<path>`. The open `nixpkgs.config.*`
/// attrset has no declared sub-options, so an unset subkey is a missing
/// attribute: read it through `or <sentinel>` to distinguish "unset (system
/// default)" from the "option removed" error a typed option gives.
fn build_read_expr(
    option_path: &str,
    config_path: &std::path::Path,
    flake_root: Option<&std::path::Path>,
) -> (String, bool) {
    let under_nixpkgs_config = option_path.starts_with("nixpkgs.config.");
    let option_expr = if under_nixpkgs_config {
        format!("{option_path} or \"{UNSET}\"")
    } else {
        option_path.to_string()
    };
    let expr = match flake_root {
        // Mirror nixos-rebuild: with no `#name` the hostname selects the
        // configuration, so the live values come from the same system that
        // a rebuild would switch to.
        Some(root) => format!(
            r#"(builtins.getFlake "{}").nixosConfigurations."{}".config.{}"#,
            root.display(),
            machine_hostname(),
            option_expr
        ),
        None => format!(
            r#"(import <nixpkgs/nixos> {{ configuration = import {}; }}).config.{}"#,
            config_path.display(),
            option_expr
        ),
    };
    (expr, under_nixpkgs_config)
}

/// Hostname used as the flake's `nixosConfigurations` attribute.
fn machine_hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_string())
        .ok()
        .filter(|name| !name.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "default".to_string())
}

/// Parse `nix-instantiate --eval --strict` output (the Nix REPL printed form):
/// quoted strings, `true`/`false`, `null`, attrsets, lists, and `«lambda»`.
pub fn parse_nix_value(output: &str) -> ConfigValue {
    let trimmed = output.trim();

    // Boolean
    if trimmed == "true" {
        return ConfigValue::Bool(true);
    }
    if trimmed == "false" {
        return ConfigValue::Bool(false);
    }

    // Null
    if trimmed == "null" {
        return ConfigValue::Unparsed("null".to_string());
    }

    // Quoted string: "..." with Nix escapes
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        let inner = &trimmed[1..trimmed.len() - 1];
        return ConfigValue::Str(unescape_nix_string(inner));
    }

    // Everything else (attrsets, lists, functions, etc.)
    ConfigValue::Unparsed(trimmed.to_string())
}

/// Unescape a Nix string literal (content without the surrounding quotes).
/// Handles: \\, \", \n, \t, \r, ${...} -> literal ${
fn unescape_nix_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('$') => result.push('$'), // escaped interpolation marker
                Some('{') => result.push('{'), // escaped brace
                Some(other) => {
                    // Unknown escape: keep both chars (conservative)
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'), // trailing backslash
            }
        } else if c == '$' && chars.peek() == Some(&'{') {
            // In Nix string interpolation, ${...} is the marker.
            // Since we're parsing *output* of nix-instantiate --eval,
            // the string value should already have interpolations resolved.
            // But if we see a bare ${
            result.push('$');
            chars.next(); // consume the {
        } else {
            result.push(c);
        }
    }

    result
}

/// Read one option from the user's live configuration.
///
/// Read-only: it evaluates an expression, and never writes, edits, or builds.
/// The root is the one apply uses, so a flake system reads its own flake.
///
/// # Errors
/// A malformed option path, no live configuration, no evaluator, an absent
/// option, or a failed evaluation.
pub fn read_config_value(option_path: &str) -> Result<ConfigValue, ConfigValueError> {
    if option_path.is_empty() {
        return Err(ConfigValueError::InvalidOptionPath(
            "option path cannot be empty".to_string(),
        ));
    }

    // Validate option path: dotted identifiers only
    if option_path
        .split('.')
        .any(|part| part.is_empty() || !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
    {
        return Err(ConfigValueError::InvalidOptionPath(
            "option path must be dotted identifiers (e.g. time.timeZone)".to_string(),
        ));
    }

    // Read from the same root that apply and setup use, so a flake system
    // never reports values from a stale /etc/nixos copy.
    let root = config_source::resolve_config_root().ok_or(ConfigValueError::NoLiveConfig)?;
    let flake_root = match root.kind {
        crate::config_source::ConfigKind::Flake => Some(root.path.clone()),
        crate::config_source::ConfigKind::Classic => None,
    };
    let config_path = root.path.join("configuration.nix");

    // Find nix-instantiate
    let nix_instantiate = which::which("nix-instantiate")
        .ok()
        .filter(|p| p.exists())
        .ok_or(ConfigValueError::NoNixInstantiate)?;

    // Build the read-only Nix expression for the requested option.
    let (expr, under_nixpkgs_config) =
        build_read_expr(option_path, &config_path, flake_root.as_deref());

    // Run nix-instantiate --eval --strict
    let output = Command::new(&nix_instantiate)
        .args(["--eval", "--strict", "--expr", &expr])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| ConfigValueError::EvalFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(classify_eval_failure(&stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if under_nixpkgs_config && stdout.trim() == format!("\"{UNSET}\"") {
        return Ok(ConfigValue::Unset);
    }
    Ok(parse_nix_value(&stdout))
}

/// Check if NixOS config reading is available (non-blocking heuristic).
/// Returns true if we're on a NixOS system with nix-instantiate available.
pub fn is_available() -> bool {
    config_source::resolve_config_root().is_some() && which::which("nix-instantiate").is_ok()
}

/// Decide whether an eval failure means the option no longer exists (removed
/// or renamed) versus any other evaluation problem. Removed options surface
/// as "attribute '...' missing" / "does not exist" in the eval error.
fn classify_eval_failure(stderr: &str) -> ConfigValueError {
    let lower = stderr.to_lowercase();
    if lower.contains("missing")
        || lower.contains("does not exist")
        || lower.contains("attribute name '")
    {
        ConfigValueError::NoSuchOption(stderr.to_owned())
    } else {
        ConfigValueError::EvalFailed(stderr.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_failure_classifies_removed_option() {
        // a removed/renamed option surfaces as an "attribute ... missing" error
        assert_eq!(
            classify_eval_failure("error: attribute 'timeZone' missing"),
            ConfigValueError::NoSuchOption("error: attribute 'timeZone' missing".into())
        );
        assert_eq!(
            classify_eval_failure("error: The option 'networking.nameservers' does not exist"),
            ConfigValueError::NoSuchOption(
                "error: The option 'networking.nameservers' does not exist".into()
            )
        );
        assert_eq!(
            classify_eval_failure("error: infinite recursion encountered"),
            ConfigValueError::EvalFailed("error: infinite recursion encountered".into())
        );
    }

    #[test]
    fn read_expr_adds_or_fallback_only_for_open_config_attrs() {
        let cfg = std::path::Path::new("/etc/nixos/configuration.nix");
        // Typed options keep a plain read and still surface a missing
        // attribute as "option removed", never as a default.
        let (typed, is_open) = build_read_expr("networking.hostName", cfg, None);
        assert!(!is_open);
        assert!(typed.contains(".config.networking.hostName"), "{typed}");
        assert!(!typed.contains(" or "), "{typed}");
        // The open nixpkgs.config.* attrset reads through `or <sentinel>` so
        // an unset subkey reports as "unset" rather than "removed".
        let (open, is_open) = build_read_expr("nixpkgs.config.allowUnfree", cfg, None);
        assert!(is_open);
        assert!(
            open.contains(r#".config.nixpkgs.config.allowUnfree or "__nix_pilot_config_unset__"#),
            "{open}"
        );
    }

    #[test]
    fn read_expr_evaluates_a_flake_root_as_a_flake() {
        let root = std::path::Path::new("/home/u/nixos-config");
        let (expr, _) = build_read_expr("time.timeZone", root, Some(root));
        assert!(expr.contains("builtins.getFlake"), "{expr}");
        assert!(expr.contains("nixosConfigurations"), "{expr}");
        assert!(expr.contains(".config.time.timeZone"), "{expr}");
        assert!(!expr.contains("<nixpkgs/nixos>"), "{expr}");
    }

    #[test]
    fn parse_boolean_true() {
        assert_eq!(parse_nix_value("true"), ConfigValue::Bool(true));
        assert_eq!(parse_nix_value("  true  "), ConfigValue::Bool(true));
    }

    #[test]
    fn parse_boolean_false() {
        assert_eq!(parse_nix_value("false"), ConfigValue::Bool(false));
        assert_eq!(parse_nix_value("  false  "), ConfigValue::Bool(false));
    }

    #[test]
    fn parse_null() {
        assert_eq!(
            parse_nix_value("null"),
            ConfigValue::Unparsed("null".to_string())
        );
    }

    #[test]
    fn parse_simple_string() {
        assert_eq!(
            parse_nix_value(r#""Europe/Berlin""#),
            ConfigValue::Str("Europe/Berlin".to_string())
        );
    }

    #[test]
    fn parse_string_with_escapes() {
        assert_eq!(
            parse_nix_value(r#""hello\nworld""#),
            ConfigValue::Str("hello\nworld".to_string())
        );
        assert_eq!(
            parse_nix_value(r#""she said \"hello\"""#),
            ConfigValue::Str("she said \"hello\"".to_string())
        );
        assert_eq!(
            parse_nix_value(r#""path\\to\\file""#),
            ConfigValue::Str("path\\to\\file".to_string())
        );
        assert_eq!(
            parse_nix_value(r#""tab\there""#),
            ConfigValue::Str("tab\there".to_string())
        );
    }

    #[test]
    fn parse_string_with_interpolation_marker() {
        // The output of nix-instantiate --eval should have interpolations
        // already resolved. If we see a bare ${ in the parsed string content,
        // our unescape treats it as an escaped $ (consumes the {).
        assert_eq!(
            parse_nix_value(r#""literal ${ not interpolated }""#),
            ConfigValue::Str("literal $ not interpolated }".to_string())
        );
    }

    #[test]
    fn parse_attrset_becomes_unparsed() {
        assert_eq!(
            parse_nix_value("{ foo = 1; bar = 2; }"),
            ConfigValue::Unparsed("{ foo = 1; bar = 2; }".to_string())
        );
    }

    #[test]
    fn parse_list_becomes_unparsed() {
        assert_eq!(
            parse_nix_value("[ 1 2 3 ]"),
            ConfigValue::Unparsed("[ 1 2 3 ]".to_string())
        );
    }

    #[test]
    fn parse_function_becomes_unparsed() {
        assert_eq!(
            parse_nix_value("«lambda»"),
            ConfigValue::Unparsed("«lambda»".to_string())
        );
    }

    #[test]
    fn invalid_option_path_empty() {
        let err = read_config_value("").unwrap_err();
        assert!(matches!(err, ConfigValueError::InvalidOptionPath(_)));
    }

    #[test]
    fn invalid_option_path_malformed() {
        let err = read_config_value("time..timeZone").unwrap_err();
        assert!(matches!(err, ConfigValueError::InvalidOptionPath(_)));
        let err = read_config_value("time/timeZone").unwrap_err();
        assert!(matches!(err, ConfigValueError::InvalidOptionPath(_)));
    }

    #[test]
    fn valid_option_paths() {
        // On a non-NixOS system, these should fail with NoLiveConfig or NoNixInstantiate.
        // On a NixOS system with nix-instantiate, they may succeed or fail with EvalFailed.
        // We just verify they don't fail with InvalidOptionPath.
        for key in [
            "time.timeZone",
            "networking.hostName",
            "services_openssh_enable",
        ] {
            match read_config_value(key) {
                Err(ConfigValueError::InvalidOptionPath(_)) => {
                    panic!("valid option path {key} rejected as invalid")
                }
                Err(ConfigValueError::NoLiveConfig)
                | Err(ConfigValueError::NoNixInstantiate)
                | Err(ConfigValueError::EvalFailed(_))
                | Err(ConfigValueError::NoSuchOption(_))
                | Ok(_) => {} // all acceptable
            }
        }
    }

    /// Live test: read time.timeZone from this host's configuration.
    /// Requires a NixOS system with nix-instantiate available.
    #[test]
    #[ignore = "requires nixos with nix-instantiate"]
    fn live_read_time_timezone() {
        let result = read_config_value("time.timeZone");
        assert!(result.is_ok(), "failed to read time.timeZone: {:?}", result);
        let value = result.unwrap();
        match value {
            ConfigValue::Str(s) => {
                // Should be a valid IANA timezone like "Europe/Berlin" or "UTC"
                assert!(!s.is_empty());
                assert!(!s.contains(' '));
                println!("time.timeZone = {}", s);
            }
            ConfigValue::Bool(_) => panic!("time.timeZone should be a string, not boolean"),
            ConfigValue::Unparsed(u) => {
                panic!("time.timeZone should parse as string, got unparsed: {}", u)
            }
            ConfigValue::Unset => {
                panic!("time.timeZone is a typed option and cannot be 'unset'")
            }
        }
    }

    /// Live test: read all seven owned settings and report their parsed types.
    #[test]
    #[ignore = "requires nixos with nix-instantiate"]
    fn live_read_all_seven_owned_settings() {
        let keys = [
            "time.timeZone",
            "networking.hostName",
            "networking.networkmanager.enable",
            "i18n.defaultLocale",
            "services.printing.enable",
            "powerManagement.enable",
            "services.openssh.enable",
        ];
        for key in keys {
            match read_config_value(key) {
                Ok(ConfigValue::Str(s)) => println!("{}: Str({})", key, s),
                Ok(ConfigValue::Bool(b)) => println!("{}: Bool({})", key, b),
                Ok(ConfigValue::Unparsed(u)) => println!("{}: Unparsed({})", key, u),
                Ok(ConfigValue::Unset) => println!("{}: Unset (system default)", key),
                Err(e) => println!("{}: Error({:?})", key, e),
            }
        }
    }
}
