//! Catalogue domain model (no GTK, no I/O). One source of truth for the
//! package classes and evidence tiers, so labels are never duplicated.
use serde::Deserialize;

/// Package class from the catalogue index (D16/D18 groups).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    App,
    Unverified,
    TermApp,
    Cli,
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
    Rich,
    Basic,
    UnfreeDeferred,
    Unverified,
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
    pub fn label(self) -> &'static str {
        match self {
            Tier::Rich => "Verified — rich metadata",
            Tier::Basic => "Verified — desktop entry",
            Tier::UnfreeDeferred => "Unfree — listed, no cache evidence",
            Tier::Unverified => "Unverified — no desktop evidence",
            Tier::Other => "Unknown",
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    #[serde(default)]
    pub scope_default: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueIndex {
    pub entries: Vec<IndexEntry>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    pub attr: String,
    #[serde(default)]
    pub pname: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub home_page: Option<String>,
    #[serde(default)]
    pub main_program: Option<String>,
    #[serde(default)]
    pub unfree: bool,
    #[serde(default)]
    pub linux_ok: bool,
    #[serde(default)]
    pub class: Class,
    #[serde(default)]
    pub tier: Tier,
    #[serde(default)]
    pub install: Option<InstallPlan>,
}

impl IndexEntry {
    pub fn display_name(&self) -> &str {
        self.pname.as_deref().unwrap_or(self.attr.as_str())
    }
}

/// In-memory dictionary of pre-lowered search fields, built once at load so a
/// keystroke never relowercases or reallocates the whole catalogue.
pub struct SearchText {
    pub name: String,
    pub pn: String,
    pub desc: String,
}

impl SearchText {
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
        assert!(t.label().contains("Unfree"));
    }
}
