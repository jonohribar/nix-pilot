//! Store-local catalogue search and bounded package browsing.
//!
//! This component owns package rows, grouping, empty-query browsing, and its
//! local filter controls. It delegates ranking and cached fields to the pure
//! search corpus and reports selected package entries to the shell.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::model::{Class, IndexEntry};
use crate::search::SearchCorpus;
use crate::ui::widgets;

/// Maximum number of package entries shown for browse or search.
const MAX_ROWS: usize = 60;

/// Class order for store-local search groups.
const GROUP_ORDER: [Class; 4] = [Class::App, Class::Unverified, Class::TermApp, Class::Cli];

/// A package selection or a non-selectable group-header row.
#[derive(Debug, Clone)]
enum StoreTarget {
    Package(IndexEntry),
    GroupHeader,
}

impl StoreTarget {
    fn selection(self) -> Option<IndexEntry> {
        match self {
            Self::Package(entry) => Some(entry),
            Self::GroupHeader => None,
        }
    }
}

/// Store-local search controls and rendered catalogue list.
pub struct StoreSearch {
    /// Root page for the Packages surface.
    pub page: gtk4::Box,
    /// Store-scoped query entry.
    pub search: gtk4::SearchEntry,
    /// Optional filter that hides restricted-licence entries.
    pub unfree_filter: gtk4::CheckButton,
    /// Persistence warning displayed above the results.
    pub state_notice: gtk4::Label,
    listbox: gtk4::ListBox,
    summary: gtk4::Label,
    shown: Rc<RefCell<Vec<StoreTarget>>>,
}

impl StoreSearch {
    /// Build the store-local search surface.
    pub fn new() -> StoreSearch {
        let search = gtk4::SearchEntry::new();
        search.set_placeholder_text(Some("Search packages…"));
        search.set_tooltip_text(Some("Searches the package catalogue only"));
        search.set_hexpand(true);
        let unfree_filter =
            gtk4::CheckButton::with_label("Hide packages with licence restrictions");
        unfree_filter.set_tooltip_text(Some(
            "Some packages carry licences your system may refuse; this hides them",
        ));
        let filter_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        filter_row.append(&search);
        filter_row.append(&unfree_filter);

        let summary = gtk4::Label::new(None);
        summary.set_xalign(0.0);
        summary.add_css_class("dim-label");

        let state_notice = gtk4::Label::new(None);
        state_notice.set_xalign(0.0);
        state_notice.set_wrap(true);
        state_notice.add_css_class("error");
        state_notice.set_visible(false);

        let listbox = gtk4::ListBox::new();
        let scrolled = gtk4::ScrolledWindow::builder()
            .child(&listbox)
            .vexpand(true)
            .build();
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        page.set_margin_top(6);
        page.set_margin_bottom(6);
        page.append(&filter_row);
        page.append(&state_notice);
        page.append(&summary);
        page.append(&scrolled);

        StoreSearch {
            page,
            search,
            unfree_filter,
            state_notice,
            listbox,
            summary,
            shown: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Set the initial browse summary or display a catalogue-load error.
    pub fn set_initial_summary(&self, corpus: &SearchCorpus, error: Option<&str>) {
        if let Some(error) = error {
            self.summary.set_text(error);
            self.summary.add_css_class("error");
            return;
        }
        let apps = browse_entries(corpus).len();
        let showing = apps.min(MAX_ROWS);
        self.summary.set_text(&format!(
            "Showing {showing} of {apps} packages — search or scroll for more"
        ));
    }

    /// Render browse or ranked rows and replace the row-selection map atomically.
    pub fn render(&self, corpus: &SearchCorpus, query: &str) -> usize {
        render_into(&self.listbox, &self.shown, corpus, query)
    }

    /// Re-render after a corpus replacement and show the current row count.
    pub fn refresh(&self, corpus: &SearchCorpus, query: &str) {
        let count = self.render(corpus, query);
        self.summary.set_text(&format!("{count} shown"));
    }

    /// Connect query, filter, and package-selection events to shell callbacks.
    pub fn wire(
        &self,
        corpus: &Rc<RefCell<SearchCorpus>>,
        on_query: Rc<dyn Fn()>,
        on_filter: Rc<dyn Fn(bool)>,
        on_package: Rc<dyn Fn(IndexEntry)>,
    ) {
        let listbox = self.listbox.clone();
        let summary = self.summary.clone();
        let shown = self.shown.clone();
        let corpus_for_query = corpus.clone();
        let on_query_for_change = on_query.clone();
        self.search.connect_search_changed(move |entry| {
            let query = entry.text();
            let count = render_into(&listbox, &shown, &corpus_for_query.borrow(), &query);
            set_query_summary(&summary, count, &query);
            on_query_for_change();
        });

        let listbox = self.listbox.clone();
        let summary = self.summary.clone();
        let shown = self.shown.clone();
        let corpus_for_stop = corpus.clone();
        self.search.connect_stop_search(move |entry| {
            if !entry.text().is_empty() {
                entry.set_text("");
                return;
            }
            let count = render_into(&listbox, &shown, &corpus_for_stop.borrow(), "");
            summary.set_text(&format!("{count} shown"));
        });

        let on_filter = on_filter.clone();
        self.unfree_filter.connect_toggled(move |button| {
            on_filter(button.is_active());
        });

        let shown = self.shown.clone();
        self.listbox.connect_row_activated(move |_, row| {
            let index = row.index();
            if index < 0 {
                return;
            }
            if let Some(target) = shown.borrow().get(index as usize).cloned() {
                if let Some(entry) = target.selection() {
                    on_package(entry);
                }
            }
        });
    }

    pub(super) fn clone_handles(&self) -> StoreSearch {
        StoreSearch {
            page: self.page.clone(),
            search: self.search.clone(),
            unfree_filter: self.unfree_filter.clone(),
            state_notice: self.state_notice.clone(),
            listbox: self.listbox.clone(),
            summary: self.summary.clone(),
            shown: self.shown.clone(),
        }
    }
}

fn render_into(
    listbox: &gtk4::ListBox,
    shown: &RefCell<Vec<StoreTarget>>,
    corpus: &SearchCorpus,
    query: &str,
) -> usize {
    while let Some(child) = listbox.first_child() {
        listbox.remove(&child);
    }
    let mut rows = Vec::new();
    if query.trim().is_empty() {
        for entry in browse_entries(corpus).into_iter().take(MAX_ROWS) {
            listbox.append(&widgets::entry_row(&entry));
            rows.push(StoreTarget::Package(entry));
        }
    } else {
        let picked = deduplicate(corpus.rank(query, MAX_ROWS));
        let mut groups = Vec::new();
        for class in GROUP_ORDER {
            let entries: Vec<&IndexEntry> = picked
                .iter()
                .filter(|entry| entry.class == class)
                .copied()
                .collect();
            if !entries.is_empty() {
                groups.push((class, entries));
            }
        }
        let others: Vec<&IndexEntry> = picked
            .iter()
            .filter(|entry| !GROUP_ORDER.contains(&entry.class))
            .copied()
            .collect();
        if !others.is_empty() {
            groups.push((Class::Other, others));
        }
        for (class, entries) in groups {
            listbox.append(&widgets::group_header_row(
                class.group_label(),
                entries.len(),
            ));
            rows.push(StoreTarget::GroupHeader);
            for entry in entries {
                listbox.append(&widgets::entry_row(entry));
                rows.push(StoreTarget::Package(entry.clone()));
            }
        }
    }
    let count = rows
        .iter()
        .filter(|row| matches!(row, StoreTarget::Package(_)))
        .count();
    *shown.borrow_mut() = rows;
    count
}

fn browse_entries(corpus: &SearchCorpus) -> Vec<IndexEntry> {
    let mut seen = std::collections::HashSet::new();
    let mut apps: Vec<IndexEntry> = corpus
        .entries()
        .filter(|entry| entry.class == Class::App)
        .filter(|entry| seen.insert(identity(entry)))
        .cloned()
        .collect();
    apps.sort_by_key(|entry| entry.display_name().to_lowercase());
    apps
}

fn deduplicate(entries: Vec<&IndexEntry>) -> Vec<&IndexEntry> {
    let mut seen = std::collections::HashSet::new();
    entries
        .into_iter()
        .filter(|entry| seen.insert(identity(entry)))
        .collect()
}

fn identity(entry: &IndexEntry) -> String {
    format!(
        "{}|{}",
        entry.pname.as_deref().unwrap_or(&entry.attr),
        entry.version.as_deref().unwrap_or_default()
    )
}

fn set_query_summary(summary: &gtk4::Label, count: usize, query: &str) {
    if count == 0 && !query.trim().is_empty() {
        summary.set_text(&format!(
            "No results for \"{}\" — try a different name",
            query.trim()
        ));
    } else {
        summary.set_text(&format!("{count} shown"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_payloads_select_packages_but_not_group_headers() {
        let package = IndexEntry {
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
        };
        assert_eq!(
            StoreTarget::Package(package.clone())
                .selection()
                .map(|entry| entry.attr),
            Some("firefox".into())
        );
        assert!(StoreTarget::GroupHeader.selection().is_none());
    }
}
