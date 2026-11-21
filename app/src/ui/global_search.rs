//! App-wide search across catalogue entries and authored settings.
//!
//! This component owns global query rendering, result groups and expansion,
//! and converts activated rows into typed package or setting selections. The
//! shell remains responsible for navigating to the selected surface.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::model::{Class, IndexEntry};
use crate::search::SearchCorpus;
use crate::settings::{self, Spec};
use crate::ui::widgets;

/// Maximum number of package matches considered before group rendering.
const MAX_ROWS: usize = 60;
/// Maximum number of items shown per collapsed group.
const GROUP_CAP: usize = 5;

/// A package or setting selected from global search.
#[derive(Clone)]
pub enum GlobalSelection {
    /// A catalogue package; the shell opens its detail surface.
    Package(IndexEntry),
    /// An authored setting; the shell opens its category.
    Setting(&'static Spec),
}

#[derive(Clone)]
enum GlobalRow {
    Package(IndexEntry),
    Setting(&'static Spec),
    GroupHeader(&'static str),
}

impl GlobalRow {
    fn selection(self) -> Option<GlobalSelection> {
        match self {
            Self::Package(entry) => Some(GlobalSelection::Package(entry)),
            Self::Setting(spec) => Some(GlobalSelection::Setting(spec)),
            Self::GroupHeader(_) => None,
        }
    }
}

/// Sidebar search entry, result page, and group-expansion state.
pub struct GlobalSearch {
    /// Search entry placed at the top of the application sidebar.
    pub entry: gtk4::SearchEntry,
    /// Search-results page added to the application stack.
    pub page: gtk4::Box,
    listbox: gtk4::ListBox,
    summary: gtk4::Label,
    shown: Rc<RefCell<Vec<GlobalRow>>>,
    /// Groups expanded by the user; retained while the query changes.
    expanded: Rc<RefCell<HashSet<&'static str>>>,
}

impl GlobalSearch {
    /// Build the app-wide search controls and result surface.
    pub fn new() -> GlobalSearch {
        let entry = gtk4::SearchEntry::new();
        let summary = gtk4::Label::new(None);
        summary.set_xalign(0.0);
        summary.add_css_class("dim-label");
        let listbox = gtk4::ListBox::new();
        let scrolled = gtk4::ScrolledWindow::builder()
            .child(&listbox)
            .vexpand(true)
            .build();
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        page.set_margin_top(6);
        page.set_margin_bottom(6);
        page.append(&summary);
        page.append(&scrolled);

        GlobalSearch {
            entry,
            page,
            listbox,
            summary,
            shown: Rc::new(RefCell::new(Vec::new())),
            expanded: Rc::new(RefCell::new(HashSet::new())),
        }
    }

    /// Render results and update the summary for the current query.
    pub fn render(&self, corpus: &SearchCorpus) {
        let query = self.entry.text();
        let count = render_into(self, corpus, &query);
        if count == 0 && !query.trim().is_empty() {
            self.summary.set_text(&format!(
                "No results for \"{}\" — try a different name",
                query.trim()
            ));
        } else if query.trim().is_empty() {
            self.summary.set_text("Search packages and settings");
        } else {
            self.summary.set_text(&format!("{count} shown"));
        }
    }

    /// Connect query and row activation to shell navigation callbacks.
    pub fn wire(
        &self,
        corpus: &Rc<RefCell<SearchCorpus>>,
        on_query: Rc<dyn Fn()>,
        on_return_to_store: Rc<dyn Fn()>,
        on_selection: Rc<dyn Fn(GlobalSelection)>,
    ) {
        let component = self.clone_handles();
        let corpus_for_change = corpus.clone();
        let on_query_for_change = on_query.clone();
        self.entry.connect_search_changed(move |_| {
            component.render(&corpus_for_change.borrow());
            on_query_for_change();
        });

        let shown = self.shown.clone();
        let expanded = self.expanded.clone();
        let corpus_for_activation = corpus.clone();
        let component = self.clone_handles();
        self.listbox.connect_row_activated(move |_, row| {
            let index = row.index();
            if index < 0 {
                return;
            }
            let Some(target) = shown.borrow().get(index as usize).cloned() else {
                return;
            };
            match target {
                GlobalRow::GroupHeader(key) => {
                    let mut expanded = expanded.borrow_mut();
                    if !expanded.insert(key) {
                        expanded.remove(key);
                    }
                    drop(expanded);
                    component.render(&corpus_for_activation.borrow());
                }
                target => {
                    if let Some(selection) = target.selection() {
                        on_selection(selection);
                    }
                }
            }
        });

        self.entry.connect_stop_search(move |entry| {
            if entry.text().is_empty() {
                on_return_to_store();
            } else {
                entry.set_text("");
            }
        });
    }

    fn clone_handles(&self) -> GlobalSearch {
        GlobalSearch {
            entry: self.entry.clone(),
            page: self.page.clone(),
            listbox: self.listbox.clone(),
            summary: self.summary.clone(),
            shown: self.shown.clone(),
            expanded: self.expanded.clone(),
        }
    }
}

fn render_into(component: &GlobalSearch, corpus: &SearchCorpus, query: &str) -> usize {
    while let Some(child) = component.listbox.first_child() {
        component.listbox.remove(&child);
    }
    let expanded = component.expanded.borrow();
    let mut rows = Vec::new();

    macro_rules! push_group {
        ($key:expr, $label:expr, $items:expr, $mk_widget:expr, $mk_row:expr) => {{
            let items: Vec<_> = $items;
            if !items.is_empty() {
                let key: &'static str = $key;
                let capped = items.len() > GROUP_CAP && !expanded.contains(key);
                let header = widgets::group_header_row($label, items.len());
                if items.len() > GROUP_CAP {
                    header.set_activatable(true);
                    header.set_tooltip_text(Some("Click to show every result in this group"));
                }
                component.listbox.append(&header);
                rows.push(GlobalRow::GroupHeader(key));
                let take = if capped { GROUP_CAP } else { items.len() };
                for item in items.into_iter().take(take) {
                    component.listbox.append(&($mk_widget)(&item));
                    rows.push(($mk_row)(&item));
                }
            }
        }};
    }

    let packages = corpus.rank(query, MAX_ROWS);
    let by_class = |class: Class| -> Vec<&IndexEntry> {
        packages
            .iter()
            .filter(|entry| entry.class == class)
            .copied()
            .collect()
    };
    // Consumer-facing groups first; settings are prioritized ahead of
    // terminal and command-line matches.
    push_group!(
        "app",
        Class::App.group_label(),
        by_class(Class::App),
        |entry: &&IndexEntry| widgets::entry_row(entry),
        |entry: &&IndexEntry| GlobalRow::Package((*entry).clone())
    );
    push_group!(
        "unverified",
        Class::Unverified.group_label(),
        by_class(Class::Unverified),
        |entry: &&IndexEntry| widgets::entry_row(entry),
        |entry: &&IndexEntry| GlobalRow::Package((*entry).clone())
    );
    let settings = crate::search::rank_settings(query, settings::SPECS, 20);
    push_group!(
        "settings",
        "Settings",
        settings,
        |index: &usize| widgets::setting_row(&settings::SPECS[*index]),
        |index: &usize| GlobalRow::Setting(&settings::SPECS[*index])
    );
    push_group!(
        "termapp",
        Class::TermApp.group_label(),
        by_class(Class::TermApp),
        |entry: &&IndexEntry| widgets::entry_row(entry),
        |entry: &&IndexEntry| GlobalRow::Package((*entry).clone())
    );
    push_group!(
        "cli",
        Class::Cli.group_label(),
        by_class(Class::Cli),
        |entry: &&IndexEntry| widgets::entry_row(entry),
        |entry: &&IndexEntry| GlobalRow::Package((*entry).clone())
    );
    let others: Vec<&IndexEntry> = packages
        .iter()
        .filter(|entry| {
            !matches!(
                entry.class,
                Class::App | Class::Unverified | Class::TermApp | Class::Cli
            )
        })
        .copied()
        .collect();
    push_group!(
        "other",
        Class::Other.group_label(),
        others,
        |entry: &&IndexEntry| widgets::entry_row(entry),
        |entry: &&IndexEntry| GlobalRow::Package((*entry).clone())
    );

    let count = rows.len().saturating_sub(
        rows.iter()
            .filter(|row| matches!(row, GlobalRow::GroupHeader(_)))
            .count(),
    );
    *component.shown.borrow_mut() = rows;
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_payloads_distinguish_navigation_targets_from_expand_headers() {
        let package = GlobalRow::Package(IndexEntry {
            attr: "firefox".into(),
            pname: None,
            version: None,
            description: None,
            home_page: None,
            main_program: None,
            unfree: false,
            linux_ok: true,
            class: Class::App,
            tier: crate::model::Tier::Basic,
            install: None,
        });
        assert!(matches!(
            package.selection(),
            Some(GlobalSelection::Package(_))
        ));
        assert!(matches!(
            GlobalRow::Setting(&settings::SPECS[0]).selection(),
            Some(GlobalSelection::Setting(_))
        ));
        assert!(GlobalRow::GroupHeader("app").selection().is_none());
    }
}
