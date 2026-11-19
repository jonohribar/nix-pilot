//! Settings surface v1 (9f3.8, settings-schema.md + decision D14): the app's
//! set of *owned* settings. Pure product data + rendering — no GTK, no Nix.
//!
//! Boundary: a setting is owned only when it maps to a stable NixOS option,
//! means the same thing on every desktop, and the app owns its versioning
//! (D14). Only settings the user actively changed are stored (in
//! `DesiredDoc.settings`); everything else is untouched and falls back to the
//! system config. Rendered settings use `lib.mkOverride 10`, a priority a
//! step above `mkForce`, so "what you change in the app always wins".
use std::collections::BTreeMap;

use crate::desired_state::SettingValue;

/// One owned setting workflow: how it presents and how its value validates.
pub struct Spec {
    /// The settings map key (== the NixOS option path, dotted).
    pub key: &'static str,
    /// Plain-language name.
    pub label: &'static str,
    /// One-line plain-language behaviour (REQ-6).
    pub blurb: &'static str,
    validate: fn(&str) -> Result<(), String>,
    allow_empty: bool,
}

const fn str_spec(
    key: &'static str,
    label: &'static str,
    blurb: &'static str,
    validate: fn(&str) -> Result<(), String>,
) -> Spec {
    Spec {
        key,
        label,
        blurb,
        validate,
        allow_empty: false,
    }
}

/// The owned surface, in display order (first-release, verified on the
/// pinned nixpkgs: networking.hostName, networking.networkmanager.enable,
/// time.timeZone, i18n.defaultLocale, services.printing.enable,
/// powerManagement.enable, services.openssh.enable).
pub static SPECS: &[Spec] = &[
    str_spec(
        "networking.hostName",
        "Device name",
        "The machine's name on the network (a simple name, no domain or spaces).",
        |v| {
            if v.len() > 63 {
                Err("too long (max 63 characters)".into())
            } else if !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                Err("use only letters, numbers and hyphens".into())
            } else {
                Ok(())
            }
        },
    ),
    bool_spec(
        "networking.networkmanager.enable",
        "Network connections",
        "Manage Wi-Fi and wired connections (NetworkManager) instead of static settings.",
    ),
    str_spec(
        "time.timeZone",
        "Time zone",
        "The time zone used for times and dates (like Europe/Berlin).",
        |v| {
            if v.chars().any(char::is_whitespace) {
                Err("no spaces — use an IANA name like Europe/Berlin".into())
            } else {
                Ok(())
            }
        },
    ),
    str_spec(
        "i18n.defaultLocale",
        "Language and region",
        "The language for messages, dates and numbers (like en_US.UTF-8).",
        |v| {
            if v.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '@' | '-'))
                && !v.is_empty()
            {
                Ok(())
            } else {
                Err("use a locale name like en_US.UTF-8".into())
            }
        },
    ),
    bool_spec(
        "services.printing.enable",
        "Printing",
        "Start the printing service so the machine can use and share printers.",
    ),
    bool_spec(
        "powerManagement.enable",
        "Power management",
        "Enable suspend and power-saving features (mainly for laptops).",
    ),
    bool_spec(
        "services.openssh.enable",
        "Remote access (SSH)",
        "Allow connecting to this computer from elsewhere (remote shell).",
    ),
];

const fn bool_spec(key: &'static str, label: &'static str, blurb: &'static str) -> Spec {
    Spec {
        key,
        label,
        blurb,
        validate: |_| Ok(()),
        allow_empty: true,
    }
}

/// Validate a value for the given key (returns a plain-language reason).
pub fn validate(key: &str, value: &SettingValue) -> Result<(), String> {
    let Some(spec) = SPECS.iter().find(|s| s.key == key) else {
        return Err("unknown setting".into());
    };
    match value {
        SettingValue::Bool(_) => Ok(()),
        SettingValue::Str(s) => {
            if s.is_empty() && spec.allow_empty {
                return Ok(());
            }
            (spec.validate)(s)
        }
    }
}

/// Render the settings block for a desired-state settings map. Only keys the
/// user set appear (the desired state stores exactly what changed), each as
/// `option.path = lib.mkOverride 10 <value>;`. Prefixes the block so it is
/// emitted verbatim into the generated module.
pub fn render_block(settings: &BTreeMap<String, SettingValue>) -> String {
    let mut out = String::new();
    for spec in SPECS {
        let Some(v) = settings.get(spec.key) else {
            continue;
        };
        match v {
            SettingValue::Str(s) => {
                // Escape for a Nix double-quoted string: backslash, quote,
                // and every `$` as the interpolation `${...}` marker. A bare
                // `$` (or a `$` that opens no `{`) is not dangerous, but
                // replacing all of them with a literal is always valid, so no
                // accepted value can ever become Nix code.
                let esc = s
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('$', r#"${"$"}"#);
                out.push_str(&format!(
                    "  {} = lib.mkOverride 10 \"{}\";\n",
                    spec.key, esc
                ));
            }
            SettingValue::Bool(b) => {
                out.push_str(&format!(
                    "  {} = lib.mkOverride 10 {};\n",
                    spec.key,
                    if *b { "true" } else { "false" }
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_block_escapes_interpolation() {
        // `${...}` in an accepted value must never reach the generated Nix as
        // code: it is re-emitted as a literal-dollar interpolation.
        let mut m = BTreeMap::new();
        m.insert(
            "time.timeZone".into(),
            SettingValue::Str("evil${builtins.abort \"x\"}z".into()),
        );
        let out = render_block(&m);
        assert!(
            out.contains(r#"${"$"}{builtins"#),
            "dollar must be quoted: {out}"
        );
        assert!(
            !out.contains("${builtins.abort"),
            "interpolation must not survive: {out}"
        );
    }

    fn str_map() -> BTreeMap<String, SettingValue> {
        let mut m = BTreeMap::new();
        m.insert(
            "networking.hostName".into(),
            SettingValue::Str("myhost".into()),
        );
        m
    }

    #[test]
    fn render_emits_mkoverride_lines_only_for_set_keys() {
        let mut m = str_map();
        m.insert("services.openssh.enable".into(), SettingValue::Bool(true));
        let out = render_block(&m);
        assert_eq!(
            out,
            "  networking.hostName = lib.mkOverride 10 \"myhost\";\n  services.openssh.enable = lib.mkOverride 10 true;\n"
        );
        assert!(!out.contains("timeZone"));
        assert!(!out.contains("defaultLocale"));
    }

    #[test]
    fn unset_keys_render_nothing() {
        assert_eq!(render_block(&BTreeMap::new()), "");
    }

    #[test]
    fn values_escape_quotes() {
        let mut m = BTreeMap::new();
        m.insert(
            "networking.hostName".into(),
            SettingValue::Str("a\"b".into()),
        );
        assert!(render_block(&m).contains("a\\\"b"));
    }

    #[test]
    fn hostname_validates_like_a_dns_label() {
        assert!(validate("networking.hostName", &SettingValue::Str("my-host2".into())).is_ok());
        assert!(validate(
            "networking.hostName",
            &SettingValue::Str("has_underscore".into())
        )
        .is_err());
        assert!(validate("networking.hostName", &SettingValue::Str("x".repeat(64))).is_err());
    }

    #[test]
    fn locale_and_timezone_validate() {
        assert!(validate(
            "i18n.defaultLocale",
            &SettingValue::Str("en_US.UTF-8".into())
        )
        .is_ok());
        assert!(validate(
            "i18n.defaultLocale",
            &SettingValue::Str("bad locale".into())
        )
        .is_err());
        assert!(validate("time.timeZone", &SettingValue::Str("Europe/Berlin".into())).is_ok());
        assert!(validate("time.timeZone", &SettingValue::Str("Europe Berlin".into())).is_err());
    }

    #[test]
    fn bools_always_validate() {
        assert!(validate("services.openssh.enable", &SettingValue::Bool(true)).is_ok());
        assert!(validate("services.openssh.enable", &SettingValue::Bool(false)).is_ok());
        assert!(validate("unknown.key", &SettingValue::Bool(true)).is_err());
    }
}
