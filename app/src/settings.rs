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

/// Value type accepted by a setting and used to choose its UI control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    String,
    Boolean,
    /// A small product-authored value set rendered as a dropdown (S3/D14
    /// authored control, C2: raw option values never shown as free text).
    /// Each item is (stored value, plain-language label).
    Choice(&'static [(&'static str, &'static str)]),
}

/// One owned setting workflow: how it presents and how its value validates.
pub struct Spec {
    /// The settings map key (== the NixOS option path, dotted).
    pub key: &'static str,
    /// Plain-language name.
    pub label: &'static str,
    /// One-line plain-language behaviour (REQ-6).
    pub blurb: &'static str,
    /// Value type, shared by validation and UI construction.
    pub kind: ValueKind,
    /// Category page that owns this setting ("system", "network", "time",
    /// "privacy" in Settings, "updates" on the Updates surface).
    pub category: &'static str,
    validate: fn(&str) -> Result<(), String>,
}

#[allow(clippy::too_many_arguments)]
const fn str_spec(
    category: &'static str,
    key: &'static str,
    label: &'static str,
    blurb: &'static str,
    validate: fn(&str) -> Result<(), String>,
) -> Spec {
    Spec {
        key,
        label,
        blurb,
        kind: ValueKind::String,
        category,
        validate,
    }
}

/// The owned surface, in display order (first-release, verified on the
/// pinned nixpkgs: networking.hostName, networking.networkmanager.enable,
/// hardware.bluetooth.enable, hardware.bluetooth.powerOnBoot, time.timeZone,
/// i18n.defaultLocale, services.printing.enable, powerManagement.enable,
/// services.pipewire.enable, services.openssh.enable).
pub static SPECS: &[Spec] = &[
    str_spec(
        "system",
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
        "network",
        "networking.networkmanager.enable",
        "Network connections",
        "Manage Wi-Fi and wired connections (NetworkManager) instead of static settings.",
    ),
    bool_spec(
        "network",
        "hardware.bluetooth.enable",
        "Bluetooth",
        "Use Bluetooth devices (headphones, mice, keyboards).",
    ),
    bool_spec(
        "network",
        "hardware.bluetooth.powerOnBoot",
        "Bluetooth at startup",
        "Turn Bluetooth on when the computer starts (needs Bluetooth to be \
         enabled).",
    ),
    str_spec(
        "time",
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
        "time",
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
        "system",
        "services.printing.enable",
        "Printing",
        "Start the printing service so the machine can use and share printers.",
    ),
    bool_spec(
        "system",
        "powerManagement.enable",
        "Power management",
        "Enable suspend and power-saving features (mainly for laptops).",
    ),
    bool_spec(
        "system",
        "services.pipewire.enable",
        "Sound",
        "Play and record sound through the desktop (PipeWire).",
    ),
    bool_spec(
        "privacy",
        "services.openssh.enable",
        "Remote access (SSH)",
        "Allow connecting to this computer from elsewhere (remote shell).",
    ),
    // Automatic upkeep (category "updates", rendered on the Updates page).
    // All options verified to exist in the pinned nixpkgs snapshot.
    bool_spec(
        "updates",
        "nix.gc.automatic",
        "Automatic cleanup",
        "Regularly remove packages and files nothing needs anymore, so the \
         system does not fill up over time.",
    ),
    choice_spec(
        "updates",
        "nix.gc.options",
        "What cleanup keeps",
        "How old a version must be before cleanup removes it.",
        &[
            ("--delete-older-than 7d", "Keep about 7 days"),
            ("--delete-older-than 30d", "Keep about 30 days"),
        ],
    ),
    bool_spec(
        "updates",
        "nix.optimise.automatic",
        "Reclaim storage",
        "Periodically link identical files together to save disk space.",
    ),
    bool_spec(
        "updates",
        "system.autoUpgrade.enable",
        "Automatic system updates",
        "Check for and switch to new system versions on a schedule.",
    ),
    str_spec(
        "updates",
        "system.autoUpgrade.dates",
        "Update schedule",
        "When to check for updates, like 02:00 (empty = the system's \
         default schedule).",
        |v| {
            fn valid(t: &str) -> bool {
                let mut parts = t.split(':');
                match (parts.next(), parts.next()) {
                    (Some(h), Some(m)) => {
                        h.parse::<u32>().is_ok_and(|x| x < 24)
                            && m.parse::<u32>().is_ok_and(|x| x < 60)
                    }
                    _ => false,
                }
            }
            if v.is_empty() || valid(v) {
                Ok(())
            } else {
                Err("use 24-hour time like 02:00".into())
            }
        },
    ),
    choice_spec(
        "updates",
        "system.autoUpgrade.randomizedDelaySec",
        "Update delay",
        "Spread updates out within a window so they do not all happen at once.",
        &[
            ("300", "Within about 5 minutes"),
            ("1800", "Within about 30 minutes"),
        ],
    ),
    choice_spec(
        "updates",
        "system.autoUpgrade.operation",
        "Update action",
        "What happens after a new version is checked and built.",
        &[
            ("switch", "Apply right away"),
            ("boot", "Apply at the next restart"),
        ],
    ),
    str_spec(
        "updates",
        "system.autoUpgrade.flake",
        "System configuration",
        "Which description of this computer automatic updates follow (empty = \
         this computer's standard setup).",
        |v| {
            if v.is_empty() || !v.chars().any(char::is_whitespace) {
                Ok(())
            } else {
                Err("use a location without spaces, or leave empty".into())
            }
        },
    ),
];

const fn bool_spec(
    category: &'static str,
    key: &'static str,
    label: &'static str,
    blurb: &'static str,
) -> Spec {
    Spec {
        key,
        label,
        blurb,
        kind: ValueKind::Boolean,
        category,
        validate: |_| Ok(()),
    }
}

/// A choice setting: plain-language dropdown of authored values. Empty always
/// means "leave to the system default" and is prepended automatically.
const fn choice_spec(
    category: &'static str,
    key: &'static str,
    label: &'static str,
    blurb: &'static str,
    options: &'static [(&'static str, &'static str)],
) -> Spec {
    Spec {
        key,
        label,
        blurb,
        kind: ValueKind::Choice(options),
        category,
        // Choice values are validated centrally against the kind's options
        // (the fn-pointer field cannot capture `options`).
        validate: |_| Ok(()),
    }
}

/// Validate a value for the given key (returns a plain-language reason).
pub fn validate(key: &str, value: &SettingValue) -> Result<(), String> {
    let Some(spec) = SPECS.iter().find(|s| s.key == key) else {
        return Err("unknown setting".into());
    };
    match (spec.kind, value) {
        (ValueKind::Boolean, SettingValue::Bool(_)) => Ok(()),
        (ValueKind::String, SettingValue::Str(s)) => (spec.validate)(s),
        (ValueKind::Choice(options), SettingValue::Str(s)) => {
            if s.is_empty() || options.iter().any(|(v, _)| *v == s) {
                Ok(())
            } else {
                Err("choose one of the listed options (or leave empty)".into())
            }
        }
        (ValueKind::Boolean, SettingValue::Str(_)) => Err("expected a true/false value".into()),
        (ValueKind::String | ValueKind::Choice(_), SettingValue::Bool(_)) => {
            Err("expected a text value".into())
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
    fn upkeep_validators_accept_sensible_and_reject_junk() {
        // automatic-upkeep workflows (category "updates")
        let ok = |key: &str, value: &str| validate(key, &SettingValue::Str(value.to_string()));
        assert!(ok("system.autoUpgrade.dates", "02:00").is_ok());
        assert!(ok("system.autoUpgrade.dates", "25:00").is_err());
        assert!(ok("system.autoUpgrade.operation", "boot").is_ok());
        assert!(ok("system.autoUpgrade.operation", "explode").is_err());
        assert!(ok("nix.gc.options", "--delete-older-than 7d").is_ok());
        assert!(ok("nix.gc.options", "rm -rf /").is_err());
        // Authored choices: only the listed values pass.
        assert!(ok("system.autoUpgrade.randomizedDelaySec", "1800").is_ok());
        assert!(ok("system.autoUpgrade.randomizedDelaySec", "45min").is_err());
        assert!(ok("system.autoUpgrade.flake", "github:user/nixos-config").is_ok());
        assert!(ok("system.autoUpgrade.flake", "a b").is_err());
    }

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
    fn values_must_match_the_setting_type() {
        assert!(validate("services.openssh.enable", &SettingValue::Bool(true)).is_ok());
        assert!(validate("services.openssh.enable", &SettingValue::Bool(false)).is_ok());
        assert!(validate("services.openssh.enable", &SettingValue::Str("true".into())).is_err());
        assert!(validate("networking.hostName", &SettingValue::Bool(true)).is_err());
        assert!(validate("unknown.key", &SettingValue::Bool(true)).is_err());
    }
}
