//! Settings surface v1 (settings-schema.md, decision D14): the app's
//! set of *owned* settings. Pure product data, validation, and Nix-text
//! rendering — no GTK, Nix evaluation, or external I/O.
//!
//! Boundary: a setting is owned only when it maps to a stable NixOS option,
//! means the same thing on every desktop, and the app owns its versioning (D14).
//! Only settings the user actively changed are stored (in
//! `DesiredDoc.settings`); everything else is untouched and falls back to the
//! system config. Rendered settings use `lib.mkOverride 10`, a priority a
//! step above `mkForce`, so "what you change in the app always wins".
use std::collections::BTreeMap;

use crate::setting_value::SettingValue;

/// Value type accepted by a setting and used to choose its UI control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// Free-form text validated by the setting's specific validator.
    String,
    /// A true/false setting represented by a switch.
    Boolean,
    /// A small product-authored value set rendered as a dropdown (S3/D14
    /// authored control; C2: raw option values never shown as free text).
    /// Each item is (stored value, plain-language label).
    Choice(&'static [(&'static str, &'static str)]),
}

/// Product-authored definition of one setting the app is responsible for.
///
/// The metadata drives its plain-language UI, search aliases, input control,
/// validation, and NixOS option rendering. Only values present in desired
/// state are rendered; absence leaves the live system configuration in charge.
pub struct Spec {
    /// The settings map key (== the NixOS option path, dotted).
    pub key: &'static str,
    /// Plain-language name.
    pub label: &'static str,
    /// One-line plain-language behaviour (REQ-6).
    pub blurb: &'static str,
    /// Longer plain-language detail shown behind the info icon.
    pub details: &'static str,
    /// Value type, shared by validation and UI construction.
    pub kind: ValueKind,
    /// Category page that owns this setting ("system", "network", "time",
    /// "privacy" in Settings, "updates" on the Updates surface).
    pub category: &'static str,
    /// Consumer search aliases for the global search: words a user might
    /// type instead of the label ("wifi" for Network connections).
    pub tags: &'static [&'static str],
    validate: fn(&str) -> Result<(), String>,
}

impl Spec {
    /// Attach consumer search aliases (see [`Spec::tags`]); const so it works
    /// in the static SPECS table.
    pub const fn with_tags(mut self, tags: &'static [&'static str]) -> Spec {
        self.tags = tags;
        self
    }
}

#[allow(clippy::too_many_arguments)]
const fn str_spec(
    category: &'static str,
    key: &'static str,
    label: &'static str,
    blurb: &'static str,
    details: &'static str,
    validate: fn(&str) -> Result<(), String>,
) -> Spec {
    Spec {
        key,
        label,
        blurb,
        details,
        kind: ValueKind::String,
        category,
        tags: &[],
        validate,
    }
}

/// Settings owned by the app, in user-interface display order. The NixOS
/// options were verified against the pinned nixpkgs snapshot:
/// `networking.hostName`, `networking.networkmanager.enable`,
/// hardware.bluetooth.enable, hardware.bluetooth.powerOnBoot, time.timeZone,
/// i18n.defaultLocale, services.printing.enable, powerManagement.enable,
/// services.pipewire.enable, services.openssh.enable).
pub static SPECS: &[Spec] = &[
    str_spec(
        "system",
        "networking.hostName",
        "Device name",
        "The machine's name on the network.",
        "Shown to other computers on your network so they can tell machines apart. Use a simple name without spaces or punctuation.",
        |v| {
            if v.len() > 63 {
                Err("too long (max 63 characters)".into())
            } else if !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                Err("use only letters, numbers and hyphens".into())
            } else {
                Ok(())
            }
        },
    )
    .with_tags(&["hostname", "rename", "computer name", "machine name"]),
    bool_spec(
        "network",
        "networking.networkmanager.enable",
        "Network connections",
        "Manage Wi-Fi and wired connections.",
        "Lets the desktop manage Wi-Fi, wired and VPN connections for you. Turn it off only if you set networking up manually in the system configuration.",
    )
    .with_tags(&[
        "wifi",
        "wireless",
        "ethernet",
        "internet",
        "network manager",
    ]),
    bool_spec(
        "network",
        "hardware.bluetooth.enable",
        "Bluetooth",
        "Use Bluetooth devices.",
        "Lets this computer use Bluetooth headphones, mice and keyboards. Turning it off disables Bluetooth entirely.",
    )
    .with_tags(&["wireless", "headphones", "mice", "keyboards"]),
    bool_spec(
        "network",
        "hardware.bluetooth.powerOnBoot",
        "Bluetooth at startup",
        "Turn Bluetooth on when the computer starts.",
        "Whether Bluetooth turns on by itself at startup. It only matters when Bluetooth is enabled.",
    )
    .with_tags(&["boot", "start up", "wireless"]),
    str_spec(
        "time",
        "time.timeZone",
        "Time zone",
        "The time zone used for times and dates.",
        "Pick your region so times and dates are correct. Uses standard IANA names like Europe/Berlin.",
        |v| {
            if v.chars().any(char::is_whitespace) {
                Err("no spaces — use an IANA name like Europe/Berlin".into())
            } else {
                Ok(())
            }
        },
    )
    .with_tags(&["clock", "timezone", "time"]),
    str_spec(
        "time",
        "i18n.defaultLocale",
        "Language and region",
        "The language for messages, dates and numbers.",
        "Sets the language for messages and how dates and numbers are written. Example: en_US.UTF-8.",
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
    )
    .with_tags(&["locale", "language", "region", "numbers", "dates"]),
    bool_spec(
        "system",
        "services.printing.enable",
        "Printing",
        "Start the printing service.",
        "Starts the printing service so this computer can use and share printers.",
    )
    .with_tags(&["printer", "print"]),
    bool_spec(
        "system",
        "powerManagement.enable",
        "Power management",
        "Enable suspend and power-saving.",
        "Turns on suspend and power-saving so the machine uses less energy; mainly useful on laptops.",
    )
    .with_tags(&["battery", "suspend", "sleep", "power saving"]),
    bool_spec(
        "system",
        "services.pipewire.enable",
        "Sound",
        "Play and record sound.",
        "Provides sound playback and recording through the desktop using PipeWire.",
    )
    .with_tags(&["audio", "speakers", "microphone", "volume"]),
    bool_spec(
        "privacy",
        "services.openssh.enable",
        "Remote access (SSH)",
        "Allow connecting to this computer remotely.",
        "Lets you connect to this computer from another machine using a secure shell. Before enabling it, set up remote access keys.",
    )
    .with_tags(&["ssh", "remote", "remote shell", "secure shell"]),
    bool_spec(
        "software",
        "nixpkgs.config.allowUnfree",
        "Restricted-licence software",
        "Allow apps whose licence restricts use.",
        "Lets you install apps whose licence restricts copying or changes, including some common proprietary apps. Leave it off to install only free and open-source apps.",
    )
    .with_tags(&[
        "unfree",
        "proprietary",
        "closed source",
        "licence restrictions",
        "allow restricted",
    ]),
    // Automatic upkeep (category "updates", rendered on the Updates page).
    // All options verified to exist in the pinned nixpkgs snapshot.
    bool_spec(
        "updates",
        "nix.gc.automatic",
        "Automatic cleanup",
        "Regularly remove files nothing needs.",
        "Periodically removes old versions and unused files so the system does not fill up over time.",
    )
    .with_tags(&["garbage", "disk space", "free space", "delete files"]),
    choice_spec(
        "updates",
        "nix.gc.options",
        "What cleanup keeps",
        "How old a version must be before cleanup removes it.",
        "How many days of history cleanup keeps. Shorter protects less disk; longer keeps more versions to fall back to.",
        &[
            ("--delete-older-than 7d", "Keep about 7 days"),
            ("--delete-older-than 30d", "Keep about 30 days"),
        ],
    )
    .with_tags(&["retention", "older versions", "delete older", "disk space"]),
    bool_spec(
        "updates",
        "nix.optimise.automatic",
        "Reclaim storage",
        "Join identical files together.",
        "Periodically links identical files together to save disk space.",
    )
    .with_tags(&["disk space", "deduplicate", "same files", "compact"]),
    bool_spec(
        "updates",
        "system.autoUpgrade.enable",
        "Automatic system updates",
        "Check for and switch to new versions.",
        "Checks for a new system version on the schedule and switches to it automatically.",
    )
    .with_tags(&["upgrade", "security", "patches", "new version"]),
    str_spec(
        "updates",
        "system.autoUpgrade.dates",
        "Update schedule",
        "When to check for updates.",
        "The time of day to check and apply updates, in 24-hour time like 02:00. Leave empty for the system's default schedule.",
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
    )
    .with_tags(&["when", "daily", "frequency", "time of day"]),
    choice_spec(
        "updates",
        "system.autoUpgrade.randomizedDelaySec",
        "Update delay",
        "Spread updates out within a window.",
        "Spreads scheduled updates across a window so not all machines on a network update in the same instant.",
        &[
            ("300", "Within about 5 minutes"),
            ("1800", "Within about 30 minutes"),
        ],
    )
    .with_tags(&["random", "spread out", "stagger"]),
    choice_spec(
        "updates",
        "system.autoUpgrade.operation",
        "Update action",
        "What happens after a new version is built.",
        "What to do once a new version is ready: switch now, or prepare it and apply at the next restart.",
        &[
            ("switch", "Apply right away"),
            ("boot", "Apply at the next restart"),
        ],
    )
    .with_tags(&["switch", "restart", "apply"]),
    str_spec(
        "updates",
        "system.autoUpgrade.flake",
        "System configuration",
        "Which setup automatic updates follow.",
        "The source the automatic updater follows. Leave empty to use this computer's standard setup.",
        |v| {
            if v.is_empty() || !v.chars().any(char::is_whitespace) {
                Ok(())
            } else {
                Err("use a location without spaces, or leave empty".into())
            }
        },
    )
    .with_tags(&["flake", "configuration", "description"]),
];

/// The settings categories shown as the two-column router: (label, summary,
/// child-id). The sidebar renders this list; SettingsPage keys its card
/// pages by the same child-id so the two can never disagree.
pub static SETTINGS_CATEGORIES: &[(&str, &str, &str)] = &[
    ("System", "Device name, printing, power, sound", "system"),
    ("Software", "Installs and licence choices", "software"),
    ("Network & internet", "Wi-Fi, wired, Bluetooth", "network"),
    ("Accounts", "Accounts and remote access keys", "accounts"),
    ("Time & language", "Time zone, language and region", "time"),
    (
        "Personalisation",
        "Where your desktop settings live",
        "appearance",
    ),
    ("Privacy & security", "Remote access (SSH)", "privacy"),
];

const fn bool_spec(
    category: &'static str,
    key: &'static str,
    label: &'static str,
    blurb: &'static str,
    details: &'static str,
) -> Spec {
    Spec {
        key,
        label,
        blurb,
        details,
        kind: ValueKind::Boolean,
        category,
        tags: &[],
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
    details: &'static str,
    options: &'static [(&'static str, &'static str)],
) -> Spec {
    Spec {
        key,
        label,
        blurb,
        details,
        kind: ValueKind::Choice(options),
        category,
        tags: &[],
        // Choice values are validated centrally against the kind's options
        // (the fn-pointer field cannot capture `options`).
        validate: |_| Ok(()),
    }
}

/// Validate a value against the setting definition for `key`.
///
/// The returned error is suitable for a plain-language control hint. Unknown
/// keys and mismatches between the value type and control kind are rejected.
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
/// emitted verbatim into the generated module. Call [`validate`] before
/// rendering untrusted or persisted values; this function assumes the map is
/// valid and omits keys that are not in [`SPECS`].
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

/// The switch position to show for a boolean setting (mirror-reality): the
/// user's stored choice when one exists, otherwise the live system value.
/// The toggle therefore always shows what the setting effectively is.
pub fn bool_effective(live: bool, choice: Option<bool>) -> bool {
    choice.unwrap_or(live)
}

/// The stored choice after the user flips a boolean switch.
///
/// Flipping to whatever the system currently does means "no change, keep the
/// system default" and clears the stored choice; flipping away records the
/// user's choice. This maps a binary toggle onto the three real states
/// (your choice on / your choice off / unset = system decides) by making the
/// system value the "return to default" endpoint.
pub fn apply_bool_toggle(live: bool, toggled_to: bool) -> Option<bool> {
    if toggled_to == live {
        None
    } else {
        Some(toggled_to)
    }
}

/// The capitalised state word shown beside a boolean switch: the toggle's
/// effective position spelt out for users who do not read a switch's fill.
pub fn bool_word(on: bool) -> &'static str {
    if on {
        "On"
    } else {
        "Off"
    }
}

/// A setting's chosen value worded the way its control shows it: booleans as
/// capitalised On/Off, choices as their plain-language option, strings as
/// themselves. Unknown values fall back to their raw form. Lets read-only
/// surfaces (Your system) mirror the Settings editor's wording.
pub fn value_plain(spec: &Spec, value: &SettingValue) -> String {
    match (&spec.kind, value) {
        (ValueKind::Boolean, SettingValue::Bool(b)) => bool_word(*b).to_string(),
        (ValueKind::Choice(options), SettingValue::Str(s)) => options
            .iter()
            .find(|(v, _)| v == s)
            .map(|(_, p)| *p)
            .unwrap_or(s)
            .to_string(),
        (_, SettingValue::Str(s)) => s.clone(),
        (_, SettingValue::Bool(b)) => bool_word(*b).to_string(),
    }
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
    fn allow_unfree_is_an_owned_boolean_setting() {
        let mut m = BTreeMap::new();
        m.insert(
            "nixpkgs.config.allowUnfree".into(),
            SettingValue::Bool(true),
        );
        assert!(validate("nixpkgs.config.allowUnfree", &SettingValue::Bool(true)).is_ok());
        assert!(validate(
            "nixpkgs.config.allowUnfree",
            &SettingValue::Str("true".into())
        )
        .is_err());
        let out = render_block(&m);
        assert!(out.contains("  nixpkgs.config.allowUnfree = lib.mkOverride 10 true;\n"));
        // Search + category placement are wired through the spec table.
        let spec = SPECS
            .iter()
            .find(|s| s.key == "nixpkgs.config.allowUnfree")
            .unwrap();
        assert_eq!(spec.category, "software");
        assert!(spec.tags.contains(&"proprietary"));
        assert!(spec.tags.contains(&"licence restrictions"));
    }

    #[test]
    fn bool_toggle_mirrors_the_live_value_as_the_default_endpoint() {
        // Effective position: stored choice when present, else the system value.
        assert!(bool_effective(true, None));
        assert!(!bool_effective(true, Some(false)));
        assert!(bool_effective(false, Some(true)));
        // Flipping to whatever the system does clears the choice (system
        // default); flipping away stores the user's choice.
        assert_eq!(apply_bool_toggle(true, true), None);
        assert_eq!(apply_bool_toggle(true, false), Some(false));
        assert_eq!(apply_bool_toggle(false, true), Some(true));
        assert_eq!(apply_bool_toggle(false, false), None);
    }

    #[test]
    fn bool_word_is_capitalised_regardless_of_state() {
        assert_eq!(bool_word(true), "On");
        assert_eq!(bool_word(false), "Off");
    }

    #[test]
    fn value_plain_words_booleans_choices_and_strings() {
        let b = bool_spec("system", "x.b", "B", "b", "d");
        let c = choice_spec(
            "system",
            "x.c",
            "C",
            "c",
            "d",
            &[("--delete-older-than 7d", "Keep about 7 days")],
        );
        let s = str_spec("system", "x.s", "S", "s", "d", |_| Ok(()));
        assert_eq!(value_plain(&b, &SettingValue::Bool(true)), "On");
        assert_eq!(value_plain(&b, &SettingValue::Bool(false)), "Off");
        assert_eq!(
            value_plain(&c, &SettingValue::Str("--delete-older-than 7d".into())),
            "Keep about 7 days"
        );
        assert_eq!(
            value_plain(&s, &SettingValue::Str("Europe/Berlin".into())),
            "Europe/Berlin"
        );
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
