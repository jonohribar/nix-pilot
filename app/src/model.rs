//! Catalogue domain model (no GTK, no I/O).
//!
//! These types deserialize the generated catalogue and provide the shared
//! meanings and user-facing labels used by search and UI code. Catalogue
//! metadata may be absent or newer than this client; serde defaults and
//! fallbacks keep those cases representable without inventing evidence.
use std::collections::HashMap;

use serde::Deserialize;

/// Package class from the catalogue index (D16/D18 groups).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// An entry classified as a graphical application.
    App,
    /// A possible application whose application status is not verified.
    Unverified,
    /// An application intended to run in a terminal.
    TermApp,
    /// A command-line utility rather than a user-facing application.
    Cli,
    /// An unknown or uncategorized value, including unrecognized future values.
    #[serde(other)]
    Other,
}

#[allow(clippy::derivable_impls)] // default is Other (serde fallback), not the first variant
impl Default for Class {
    fn default() -> Self {
        Class::Other
    }
}

impl Class {
    /// Per-row label ("Application", "Command-line tool", ...).
    pub fn label(self) -> &'static str {
        match self {
            Class::App => "Application",
            Class::Unverified => "Unverified application",
            Class::TermApp => "Terminal app",
            Class::Cli => "Command-line tool",
            Class::Other => "Other",
        }
    }

    /// Plural label for group headers ("Applications", ...).
    pub fn group_label(self) -> &'static str {
        match self {
            Class::App => "Applications",
            Class::Unverified => "Unverified applications",
            Class::TermApp => "Terminal apps",
            Class::Cli => "Command-line tools",
            Class::Other => "Other",
        }
    }
}

/// Evidence tier (how the entry's app-ness was verified).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Rich catalogue evidence supports presenting the entry as downloadable.
    Rich,
    /// Basic catalogue evidence supports presenting the entry as an app.
    Basic,
    /// Listed, but package evaluation is deferred because its licence is restricted.
    UnfreeDeferred,
    /// The entry has not passed the app verification process.
    Unverified,
    /// Unknown or heuristic evidence; presented as not yet verified.
    #[serde(other)]
    Other,
}

#[allow(clippy::derivable_impls)]
impl Default for Tier {
    fn default() -> Self {
        Tier::Other
    }
}

impl Tier {
    /// Return the consumer-facing evidence label, not an internal generator term.
    pub fn label(self) -> &'static str {
        match self {
            // Consumer-language tier labels (C2/C8, sje): what the user can
            // rely on, not the generator's internal classification.
            Tier::Rich => "Verified download",
            Tier::Basic => "Verified app",
            Tier::UnfreeDeferred => "Listed — licence restricted",
            Tier::Unverified => "Not yet verified",
            // the generator's heuristic tier (and any future unknown) lands
            Tier::Other => "Not yet verified",
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// Optional installation hints attached to a catalogue entry.
pub struct InstallPlan {
    /// Preferred initial installation scope, when the catalogue provides one.
    #[serde(default)]
    pub scope_default: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// Top-level catalogue document loaded by the application.
pub struct CatalogueIndex {
    /// Package entries available to the store UI.
    pub entries: Vec<IndexEntry>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// Catalogue metadata for one installable nixpkgs attribute.
///
/// Most descriptive fields are optional because catalogue generation can
/// lack metadata. `attr` remains the stable package identifier used for
/// installation and as a display-name fallback.
pub struct IndexEntry {
    /// nixpkgs attribute path used to identify and resolve the package.
    pub attr: String,
    /// Human-readable package name, when supplied by the catalogue.
    #[serde(default)]
    pub pname: Option<String>,
    /// Package version reported by the catalogue, when available.
    #[serde(default)]
    pub version: Option<String>,
    /// Short package description, when available.
    #[serde(default)]
    pub description: Option<String>,
    /// Upstream project URL, when available.
    #[serde(default)]
    pub home_page: Option<String>,
    /// Executable name used when it differs from the package name.
    #[serde(default)]
    pub main_program: Option<String>,
    /// Whether nixpkgs marks the package as requiring an unfree allowance.
    #[serde(default)]
    pub unfree: bool,
    /// Whether the package supports the Linux target represented by the index.
    #[serde(default)]
    pub linux_ok: bool,
    /// Product-facing category for grouping the entry in the store.
    #[serde(default)]
    pub class: Class,
    /// Strength of the available app evidence, translated for consumers by [`Tier::label`].
    #[serde(default)]
    pub tier: Tier,
    /// Optional catalogue-provided hints for the install workflow.
    #[serde(default)]
    pub install: Option<InstallPlan>,
}

impl IndexEntry {
    /// Return the package name when present, otherwise its nixpkgs attribute.
    pub fn display_name(&self) -> &str {
        self.pname.as_deref().unwrap_or(self.attr.as_str())
    }
}

/// Catalogue-derived labels for package attributes, with an attribute-name
/// fallback for packages missing from the catalogue.
#[derive(Debug, Clone, Default)]
pub struct PackageNames(HashMap<String, String>);

impl PackageNames {
    /// Build the lookup from the complete catalogue, independent of filters.
    pub fn from_entries(entries: &[IndexEntry]) -> Self {
        Self(
            entries
                .iter()
                .map(|entry| (entry.attr.clone(), entry.display_name().to_string()))
                .collect(),
        )
    }

    /// Return the catalogue display name, or the package attribute if unknown.
    pub fn display_name<'a>(&'a self, attr: &'a str) -> &'a str {
        self.0.get(attr).map(String::as_str).unwrap_or(attr)
    }
}

/// Lowercased fields prepared once for repeated catalogue searches.
///
/// Keeping this representation separate from display metadata avoids
/// lowercasing and allocating strings again for each keystroke.
#[derive(Debug, Clone)]
pub struct SearchText {
    /// Lowercased nixpkgs attribute path.
    pub name: String,
    /// Lowercased package name, or an empty string when absent.
    pub pn: String,
    /// Lowercased description, or an empty string when absent.
    pub desc: String,
}

impl SearchText {
    /// Build the search representation from an entry, defaulting missing text to empty.
    pub fn build(e: &IndexEntry) -> SearchText {
        SearchText {
            name: e.attr.to_lowercase(),
            pn: e.pname.clone().unwrap_or_default().to_lowercase(),
            desc: e.description.clone().unwrap_or_default().to_lowercase(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_parses_and_labels() {
        assert_eq!(Class::App.label(), "Application");
        assert_eq!(Class::Cli.group_label(), "Command-line tools");
    }

    #[test]
    fn class_deserializes_from_index_values() {
        let c: Class = serde_json::from_str("\"term-app\"").unwrap();
        assert_eq!(c, Class::TermApp);
        let c: Class = serde_json::from_str("\"mystery\"").unwrap();
        assert_eq!(c, Class::Other);
    }

    #[test]
    fn tier_deserializes_and_labels() {
        let t: Tier = serde_json::from_str("\"unfree-deferred\"").unwrap();
        assert_eq!(t, Tier::UnfreeDeferred);
        assert!(t.label().contains("licence restricted"));
    }

    #[test]
    fn heuristic_tier_falls_back_to_other_label() {
        // the index's generator tier 'heuristic' is not a cache-evidence tier;
        // it must deserialize (fallback) and label honestly — in consumer
        // language (sje): not "verified", never generator jargon
        let t: Tier = serde_json::from_str("\"heuristic\"").unwrap();
        assert_eq!(t, Tier::Other);
        assert_eq!(t.label(), "Not yet verified");
    }

    #[test]
    fn package_names_use_catalogue_name_and_fall_back_to_attribute() {
        let entry = IndexEntry {
            attr: "firefox".into(),
            pname: Some("Firefox".into()),
            version: None,
            description: None,
            home_page: None,
            main_program: None,
            unfree: false,
            linux_ok: true,
            class: Class::App,
            tier: Tier::Basic,
            install: None,
        };
        let names = PackageNames::from_entries(&[entry]);

        assert_eq!(names.display_name("firefox"), "Firefox");
        assert_eq!(names.display_name("unknown-package"), "unknown-package");
    }
}
