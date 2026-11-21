//! App shell: sidebar + stack of surfaces (Store list, detail, Updates). This
//! is the UI composition point; the detail/install flow is in `detail`, the
//! updates page in `updates_page`. Pure ranking lives in crate::search.
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use log::error;

use crate::desired_state::DesiredDoc;
use crate::model::{Class, IndexEntry, SearchText};
use crate::search;
use crate::store::ChangeQueue;
use crate::ui::{detail, settings_page, updates_page, widgets, your_system};

/// How many rows the list shows at once (empty query is a bounded browse).
const MAX_ROWS: usize = 60;

/// Group order for global search results (D16/D18 labelled groups).
const GROUP_ORDER: [Class; 4] = [Class::App, Class::Unverified, Class::TermApp, Class::Cli];

/// The browse set (B1/B2, requirements.md): applications only — ordinary
/// browsing starts from what a consumer recognises; CLI/terminal tools and
/// unverified entries stay reachable through search — de-duplicated by
/// package name + version so firmware-style variants don't repeat.
fn browse_entries(all: &[IndexEntry]) -> Vec<IndexEntry> {
    let mut seen = std::collections::HashSet::new();
    let mut apps: Vec<IndexEntry> = all
        .iter()
        .filter(|e| e.class == Class::App)
        .filter(|e| {
            let key = format!(
                "{}|{}",
                e.pname.clone().unwrap_or_else(|| e.attr.clone()),
                e.version.clone().unwrap_or_default()
            );
            seen.insert(key)
        })
        .cloned()
        .collect();
    // Alphabetical browse: an index-order slice reads as arbitrary noise.
    apps.sort_by_key(|e| e.display_name().to_lowercase());
    apps
}

/// Rank + render the list. Empty query: bounded browse over the applications
/// (B1). Search: ranked by search::rank, bucketed into the D16/D18 labelled
/// groups, capped. Writes the visible row order into `shown` so row-index
/// maps back to entries.
fn rebuild_list(
    listbox: &gtk4::ListBox,
    all: &[IndexEntry],
    texts: &[SearchText],
    shown: &RefCell<Vec<Option<IndexEntry>>>,
    query: &str,
) -> usize {
    while let Some(child) = listbox.first_child() {
        listbox.remove(&child);
    }
    let mut rows: Vec<Option<IndexEntry>> = Vec::new();
    if query.trim().is_empty() {
        for e in browse_entries(all).into_iter().take(MAX_ROWS) {
            listbox.append(&widgets::entry_row(&e));
            rows.push(Some(e));
        }
    } else {
        let picked: Vec<&IndexEntry> = search::rank(query, texts, MAX_ROWS)
            .into_iter()
            .map(|i| &all[i])
            .collect();
        // B2: search results de-duplicate the same name+version rows too.
        let mut seen = std::collections::HashSet::new();
        let picked: Vec<&IndexEntry> = picked
            .into_iter()
            .filter(|e| {
                let key = format!(
                    "{}|{}",
                    e.pname.clone().unwrap_or_else(|| e.attr.clone()),
                    e.version.clone().unwrap_or_default()
                );
                seen.insert(key)
            })
            .collect();
        let mut groups: Vec<(Class, Vec<&IndexEntry>)> = Vec::new();
        for c in GROUP_ORDER {
            let v: Vec<&IndexEntry> = picked.iter().filter(|e| e.class == c).cloned().collect();
            if !v.is_empty() {
                groups.push((c, v));
            }
        }
        let others: Vec<&IndexEntry> = picked
            .iter()
            .filter(|e| !GROUP_ORDER.contains(&e.class))
            .cloned()
            .collect();
        if !others.is_empty() {
            groups.push((Class::Other, others));
        }
        for (c, v) in groups {
            listbox.append(&widgets::group_header_row(c.group_label(), v.len()));
            rows.push(None);
            for e in v {
                listbox.append(&widgets::entry_row(e));
                rows.push(Some(e.clone()));
            }
        }
    }
    let n = rows.iter().filter(|r| r.is_some()).count();
    *shown.borrow_mut() = rows;
    n
}

fn wire_row_activation(
    listbox: &gtk4::ListBox,
    shown: &Rc<RefCell<Vec<Option<IndexEntry>>>>,
    detail: &Rc<detail::DetailUi>,
    queue: &Rc<RefCell<ChangeQueue>>,
    stack: &gtk4::Stack,
) {
    let detail = detail.clone();
    let stack = stack.clone();
    let shown = shown.clone();
    let queue = queue.clone();
    listbox.connect_row_activated(move |_, row| {
        let index = row.index();
        if index < 0 {
            return;
        }
        // Group headers have no entry in the row-to-entry map.
        let Some(Some(entry)) = shown.borrow().get(index as usize).cloned() else {
            return;
        };
        detail.populate(&entry);
        detail.sync_added(queue.borrow().contains(&entry.attr));
        stack.set_visible_child_name("detail");
    });
}

fn wire_search(
    search: &gtk4::SearchEntry,
    listbox: &gtk4::ListBox,
    stack: &gtk4::Stack,
    summary: &gtk4::Label,
    all: &Rc<RefCell<Vec<IndexEntry>>>,
    texts: &Rc<RefCell<Vec<SearchText>>>,
    shown: &Rc<RefCell<Vec<Option<IndexEntry>>>>,
) {
    let stack_for_changes = stack.clone();
    let summary_for_changes = summary.clone();
    let listbox_for_changes = listbox.clone();
    let all_for_changes = all.clone();
    let texts_for_changes = texts.clone();
    let shown_for_changes = shown.clone();
    search.connect_search_changed(move |search| {
        let query = search.text();
        let count = rebuild_list(
            &listbox_for_changes,
            &all_for_changes.borrow(),
            &texts_for_changes.borrow(),
            &shown_for_changes,
            &query,
        );
        // Distinguish an empty search result from the bounded browse count.
        if count == 0 && !query.trim().is_empty() {
            summary_for_changes.set_text(&format!(
                "No results for \"{}\" — try a different name",
                query.trim()
            ));
        } else {
            summary_for_changes.set_text(&format!("{count} shown"));
        }
        stack_for_changes.set_visible_child_name("list");
    });

    let listbox_for_stop = listbox.clone();
    let all_for_stop = all.clone();
    let texts_for_stop = texts.clone();
    let shown_for_stop = shown.clone();
    let summary_for_stop = summary.clone();
    search.connect_stop_search(move |search| {
        // Escape first clears the query; another stop restores the browse count.
        if !search.text().is_empty() {
            search.set_text("");
            return;
        }
        let count = rebuild_list(
            &listbox_for_stop,
            &all_for_stop.borrow(),
            &texts_for_stop.borrow(),
            &shown_for_stop,
            "",
        );
        summary_for_stop.set_text(&format!("{count} shown"));
    });
}

/// The app-wide search surface (fyq): a search entry atop the sidebar whose
/// results span apps + settings (S1/C4), grouped per surface (S2). Selecting
/// an app opens its detail page; selecting a setting opens its Settings
/// category.
struct GlobalSearch {
    entry: gtk4::SearchEntry,
    page: gtk4::Box,
    listbox: gtk4::ListBox,
    summary: gtk4::Label,
    /// Row payloads of the current render (activation maps rows to targets).
    shown: Rc<RefCell<Vec<Option<GlobalRow>>>>,
    /// Groups the user expanded past GROUP_CAP (cleared per query change? no:
    /// persists while typing so an expanded group stays open).
    expanded: Rc<RefCell<std::collections::HashSet<&'static str>>>,
}

fn build_global_search() -> GlobalSearch {
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
        expanded: Rc::new(RefCell::new(std::collections::HashSet::new())),
    }
}

/// One selectable global-search row: an app (opens detail) or a setting
/// (opens its Settings category). Group headers have no row payload.
#[derive(Clone)]
enum GlobalRow {
    App(IndexEntry),
    Setting(&'static crate::settings::Spec),
    /// A capped group header; activating it expands that group in place.
    GroupHeader(&'static str),
}

/// Rows shown per result group before "show all" (D28 review rule: settings
/// and app hits must not be buried under ~55 command-line rows).
const GROUP_CAP: usize = 5;

/// Rank + render the global results into `g`: groups in user-value order —
/// Applications, Unverified, Settings, Terminal, Command-line, other — each
/// capped at GROUP_CAP rows until its header is clicked. Returns the result
/// row count (headers excluded).
fn rebuild_global(
    g: &GlobalSearch,
    all: &[IndexEntry],
    texts: &[SearchText],
    query: &str,
) -> usize {
    while let Some(child) = g.listbox.first_child() {
        g.listbox.remove(&child);
    }
    let listbox = &g.listbox;
    let expanded = g.expanded.borrow();
    let mut rows: Vec<Option<GlobalRow>> = Vec::new();

    // One named result group: header + capped rows. Capped headers
    // re-render the whole list with that group expanded when clicked.
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
                listbox.append(&header);
                rows.push(Some(GlobalRow::GroupHeader(key)));
                let take = if capped { GROUP_CAP } else { items.len() };
                for item in items.into_iter().take(take) {
                    let row = ($mk_widget)(&item);
                    listbox.append(&row);
                    rows.push(Some(($mk_row)(&item)));
                }
            }
        }};
    }

    let apps: Vec<&IndexEntry> = search::rank(query, texts, MAX_ROWS)
        .into_iter()
        .map(|i| &all[i])
        .collect();
    let by_class =
        |c: Class| -> Vec<&IndexEntry> { apps.iter().filter(|e| e.class == c).cloned().collect() };
    // Applications and unverified first (consumer value), then the Settings
    // group, then terminal/CLI noise (global-search.md group order rule).
    push_group!(
        "app",
        Class::App.group_label(),
        by_class(Class::App),
        |e: &IndexEntry| widgets::entry_row(e),
        |e: &IndexEntry| GlobalRow::App(e.clone())
    );
    push_group!(
        "unverified",
        Class::Unverified.group_label(),
        by_class(Class::Unverified),
        |e: &IndexEntry| widgets::entry_row(e),
        |e: &IndexEntry| GlobalRow::App(e.clone())
    );
    let settings = search::rank_settings(query, crate::settings::SPECS, 20);
    push_group!(
        "settings",
        "Settings",
        settings,
        |i: &usize| widgets::setting_row(&crate::settings::SPECS[*i]),
        |i: &usize| GlobalRow::Setting(&crate::settings::SPECS[*i])
    );
    push_group!(
        "termapp",
        Class::TermApp.group_label(),
        by_class(Class::TermApp),
        |e: &IndexEntry| widgets::entry_row(e),
        |e: &IndexEntry| GlobalRow::App(e.clone())
    );
    push_group!(
        "cli",
        Class::Cli.group_label(),
        by_class(Class::Cli),
        |e: &IndexEntry| widgets::entry_row(e),
        |e: &IndexEntry| GlobalRow::App(e.clone())
    );
    let others: Vec<&IndexEntry> = apps
        .iter()
        .filter(|e| {
            !matches!(
                e.class,
                Class::App | Class::Unverified | Class::TermApp | Class::Cli
            )
        })
        .cloned()
        .collect();
    push_group!(
        "other",
        Class::Other.group_label(),
        others,
        |e: &IndexEntry| widgets::entry_row(e),
        |e: &IndexEntry| GlobalRow::App(e.clone())
    );

    let n = rows.iter().filter(|r| r.is_some()).count();
    *g.shown.borrow_mut() = rows;
    n
}

impl GlobalSearch {
    /// Widget-handle clone: shared Rc state, fresh widget references (gtk4
    /// widget clones are cheap handles to the same object).
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

    /// Re-render for the entry's current query and refresh the summary line.
    fn render(&self, all: &[IndexEntry], texts: &[SearchText]) {
        let query = self.entry.text();
        let count = rebuild_global(self, all, texts, &query);
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
}

fn wire_global_activation(
    g: &GlobalSearch,
    detail: &Rc<detail::DetailUi>,
    queue: &Rc<RefCell<ChangeQueue>>,
    stack: &gtk4::Stack,
    settings: &Rc<settings_page::SettingsPage>,
    all: &Rc<RefCell<Vec<IndexEntry>>>,
    texts: &Rc<RefCell<Vec<SearchText>>>,
) {
    let detail = detail.clone();
    let stack = stack.clone();
    let shown = g.shown.clone();
    let queue = queue.clone();
    let settings = settings.clone();
    let expanded = g.expanded.clone();
    let rerender = {
        let g = g.clone_handles();
        let all = all.clone();
        let texts = texts.clone();
        Rc::new(move || g.render(&all.borrow(), &texts.borrow()))
    };
    g.listbox.connect_row_activated(move |_, row| {
        let index = row.index();
        if index < 0 {
            return;
        }
        let Some(Some(row)) = shown.borrow().get(index as usize).cloned() else {
            return;
        };
        match row {
            GlobalRow::App(entry) => {
                detail.populate(&entry);
                detail.sync_added(queue.borrow().contains(&entry.attr));
                stack.set_visible_child_name("detail");
            }
            GlobalRow::Setting(spec) => {
                settings.show_category(spec.category);
                stack.set_visible_child_name("settings");
            }
            GlobalRow::GroupHeader(key) => {
                // Capped group header: toggle that group between the 5-row
                // cap and the full result set, then re-render in place.
                if !expanded.borrow_mut().insert(key) {
                    expanded.borrow_mut().remove(key);
                }
                rerender();
            }
        }
    });
}

/// Wire the sidebar-top global search: typing navigates to the global-search
/// page and shows grouped app + setting results; Esc clears the query and a
/// further stop returns to the Apps list.
fn wire_global_search(
    g: &GlobalSearch,
    stack: &gtk4::Stack,
    all: &Rc<RefCell<Vec<IndexEntry>>>,
    texts: &Rc<RefCell<Vec<SearchText>>>,
) {
    let stack_for_changes = stack.clone();
    let g_changes = g.clone_handles();
    let all_for_changes = all.clone();
    let texts_for_changes = texts.clone();
    g.entry.connect_search_changed(move |_| {
        g_changes.render(&all_for_changes.borrow(), &texts_for_changes.borrow());
        stack_for_changes.set_visible_child_name("global-search");
    });

    let stack_for_stop = stack.clone();
    g.entry.connect_stop_search(move |search| {
        // Escape first clears the query; another stop returns to the Apps list.
        if !search.text().is_empty() {
            search.set_text("");
            return;
        }
        stack_for_stop.set_visible_child_name("list");
    });
}

fn wire_search_shortcut(search: &gtk4::SearchEntry, window: &gtk4::ApplicationWindow) {
    let search = search.clone();
    let keys = gtk4::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if key == gtk4::gdk::Key::k && modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
            search.grab_focus();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);
}

/// Wrap a page root in a vertical scrolled window so content taller than the
/// window (Settings home, Updates with its upkeep cards, the recipe list) is
/// reachable by scrolling rather than clipped off-screen.
fn scrolled_page(child: &impl IsA<gtk4::Widget>) -> gtk4::ScrolledWindow {
    gtk4::ScrolledWindow::builder()
        .child(child)
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build()
}

/// A sidebar nav button that switches the stack and (for Updates) refreshes it.
fn nav_button(
    label: &str,
    page: &'static str,
    stack: &gtk4::Stack,
    on_show: Option<Rc<dyn Fn()>>,
) -> gtk4::Button {
    let b = gtk4::Button::with_label(label);
    b.add_css_class("flat");
    b.add_css_class("nav-item");
    let stack = stack.clone();
    b.connect_clicked(move |_| {
        if let Some(f) = on_show.as_ref() {
            f();
        }
        stack.set_visible_child_name(page);
    });
    b
}

struct HeaderWidgets {
    back: gtk4::Button,
    pending: gtk4::Button,
}

fn build_header(window: &gtk4::ApplicationWindow) -> HeaderWidgets {
    let header = gtk4::HeaderBar::new();
    let back = gtk4::Button::with_label("‹ Back to results");
    back.set_visible(false);
    header.pack_start(&back);

    let pending = gtk4::Button::new();
    pending.add_css_class("flat");
    pending.add_css_class("dim-label");
    pending.set_visible(false);
    header.pack_end(&pending);
    window.set_titlebar(Some(&header));

    HeaderWidgets { back, pending }
}

struct CatalogueList {
    page: gtk4::Box,
    listbox: gtk4::ListBox,
    summary: gtk4::Label,
    search: gtk4::SearchEntry,
    unfree_filter: gtk4::CheckButton,
    state_notice: gtk4::Label,
}

fn build_catalogue_list() -> CatalogueList {
    // Store-local search (fyq): the bar that scopes to the catalogue lives
    // inside the Packages tab; the app-wide search sits atop the sidebar.
    let search = gtk4::SearchEntry::new();
    // dxy: the two search boxes must read differently at a glance — the
    // sidebar one says what it spans; this one is store-scoped (D28).
    search.set_placeholder_text(Some("Search packages…"));
    search.set_tooltip_text(Some("Searches the package catalogue only"));
    search.set_hexpand(true);
    // optional non-free filter (decision D1): off by default
    // x6r/D1: consumer wording, not packaging jargon.
    let unfree_filter = gtk4::CheckButton::with_label("Hide packages with licence restrictions");
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

    CatalogueList {
        page,
        listbox,
        summary,
        search,
        unfree_filter,
        state_notice,
    }
}

fn load_desired_state(notice: &gtk4::Label) -> (DesiredDoc, bool) {
    let path = crate::desired_state::default_desired_path();
    let (document, warning, persistence_enabled) = match crate::desired_state::load(&path) {
        Ok(document) => (document, None, true),
        Err(error) if error.is_missing() => (DesiredDoc::current(), None, true),
        Err(error @ crate::desired_state::LoadError::NewerVersion { .. }) => (
            DesiredDoc::current(),
            Some(format!(
                "Saved changes were written by a newer version ({error}). This session starts empty and will not overwrite the saved file."
            )),
            false,
        ),
        Err(error) => match crate::desired_state::preserve_unreadable_file(&path) {
            Ok(backup) => (
                DesiredDoc::current(),
                Some(format!(
                    "Saved changes could not be loaded ({error}). The original was preserved at {}. This session starts with an empty desired state.",
                    backup.display()
                )),
                true,
            ),
            Err(backup_error) => (
                DesiredDoc::current(),
                Some(format!(
                    "Saved changes could not be loaded ({error}), and a backup could not be created ({backup_error}). This session's changes will not be saved, and the original file was left untouched."
                )),
                false,
            ),
        },
    };

    if let Some(warning) = warning {
        notice.set_text(&warning);
        notice.set_visible(true);
    }
    (document, persistence_enabled)
}

fn seed_queue(queue: &mut ChangeQueue, document: &DesiredDoc, account: &str) {
    for attr in document.system_packages() {
        queue.add(crate::store::PendingChange {
            attr: attr.clone(),
            name: attr.clone(),
            system_wide: true,
        });
    }
    for attr in document.packages_for(account) {
        queue.add(crate::store::PendingChange {
            attr: attr.clone(),
            name: attr.clone(),
            system_wide: false,
        });
    }
}

fn wire_desired_state_persistence(
    queue: &Rc<RefCell<ChangeQueue>>,
    desired: &Rc<RefCell<DesiredDoc>>,
    account: &str,
    persistence_enabled: bool,
    notice: &gtk4::Label,
) -> Rc<dyn Fn()> {
    let queue = queue.clone();
    let desired = desired.clone();
    let account = account.to_string();
    let notice = notice.clone();
    let path = crate::desired_state::default_desired_path();

    Rc::new(move || {
        if !persistence_enabled {
            return;
        }
        let mut system_packages = Vec::new();
        let mut user_packages = Vec::new();
        for change in queue.borrow().items() {
            if change.system_wide {
                system_packages.push(change.attr.clone());
            } else {
                user_packages.push(change.attr.clone());
            }
        }
        {
            let mut document = desired.borrow_mut();
            document.set_system(system_packages);
            document.set_user(&account, user_packages);
        }
        if let Err(error) = crate::desired_state::save(&desired.borrow(), &path) {
            error!("could not save desired state: {error}");
            notice.set_text(&format!(
                "Your changes could not be saved and will be lost on restart ({error})."
            ));
            notice.set_visible(true);
        }
    })
}

fn update_pending_button(queue: &ChangeQueue, button: &gtk4::Button) {
    if queue.is_empty() {
        button.set_visible(false);
        button.set_label("");
        return;
    }

    let suffix = if queue.len() == 1 { "" } else { "s" };
    button.set_label(&format!("{} change{suffix} ready to apply", queue.len()));
    let items = queue
        .summary()
        .iter()
        .map(|(name, system_wide)| {
            format!(
                "- {} ({})",
                name,
                if *system_wide { "system" } else { "user" }
            )
        })
        .collect::<Vec<_>>();
    button.set_tooltip_text(Some(&items.join("\n")));
    button.set_visible(true);
}

fn wire_pending_button(
    button: &gtk4::Button,
    stack: &gtk4::Stack,
    updates: &Rc<updates_page::UpdatesPage>,
    queue: &Rc<RefCell<ChangeQueue>>,
) {
    let stack = stack.clone();
    let updates = updates.clone();
    let queue = queue.clone();
    button.connect_clicked(move |_| {
        updates.refresh(&queue);
        stack.set_visible_child_name("updates");
    });
}

fn build_sidebar(
    stack: &gtk4::Stack,
    updates: &Rc<updates_page::UpdatesPage>,
    queue: &Rc<RefCell<ChangeQueue>>,
    your_system: &Rc<your_system::YourSystemPage>,
    desired: &Rc<RefCell<DesiredDoc>>,
    on_queue_change: &Rc<dyn Fn()>,
    global_search: &gtk4::SearchEntry,
) -> gtk4::Box {
    let sidebar = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    sidebar.set_width_request(220);
    // The app-wide search sits atop the sidebar, above the surface nav
    // (fyq): it spans apps + settings, so it cannot live inside one surface.
    let search_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    search_box.set_margin_top(8);
    search_box.set_margin_bottom(4);
    search_box.set_margin_start(8);
    search_box.set_margin_end(8);
    global_search.set_placeholder_text(Some("Search packages and settings…"));
    global_search.set_tooltip_text(Some("Searches packages and settings everywhere"));
    search_box.append(global_search);
    sidebar.append(&search_box);
    sidebar.append(&nav_button("Packages", "list", stack, None));
    sidebar.append(&nav_button("Settings", "settings", stack, None));

    let updates = updates.clone();
    let updates_queue = queue.clone();
    let refresh_updates = Rc::new(move || updates.refresh(&updates_queue));
    sidebar.append(&nav_button(
        "Updates",
        "updates",
        stack,
        Some(refresh_updates),
    ));

    // Your system: refresh the recipe list each time it is shown so removals
    // made elsewhere (Store, Settings) show up here.
    let your_system = your_system.clone();
    let desired = desired.clone();
    let your_queue = queue.clone();
    let on_change = on_queue_change.clone();
    let refresh_your_system =
        Rc::new(move || your_system.refresh(&desired, &your_queue, &on_change));
    sidebar.append(&nav_button(
        "Your system",
        "your-system",
        stack,
        Some(refresh_your_system),
    ));
    sidebar
}

fn set_catalogue_summary(summary: &gtk4::Label, entries: &[IndexEntry], error: Option<&str>) {
    if let Some(error) = error {
        summary.set_text(error);
        summary.add_css_class("error");
        return;
    }

    // Plain-language browse summary (sqi): what the list shows, not a class
    // breakdown of internal tiers.
    let apps = browse_entries(entries).len();
    let showing = apps.min(MAX_ROWS);
    summary.set_text(&format!(
        "Showing {showing} of {apps} packages — search or scroll for more"
    ));
}

/// Build and present the window for a loaded catalogue.
pub fn run(
    app: &gtk4::Application,
    entries: Vec<IndexEntry>,
    load_error: Option<String>,
    nix: Option<std::path::PathBuf>,
) {
    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("nix-pilot")
        .default_width(1180)
        .default_height(840)
        .build();
    let header = build_header(&window);
    let catalogue = build_catalogue_list();
    let global = build_global_search();
    let full_entries = entries;
    let hide_unfree = Rc::new(std::cell::Cell::new(false));
    let all = Rc::new(RefCell::new(Vec::<IndexEntry>::new()));
    let texts = Rc::new(RefCell::new(Vec::<SearchText>::new()));
    {
        let mut a = all.borrow_mut();
        let mut t = texts.borrow_mut();
        *a = search::filter_unfree(&full_entries, false);
        *t = a.iter().map(SearchText::build).collect();
    }
    let shown = Rc::new(RefCell::new(Vec::<Option<IndexEntry>>::new()));
    let queue = Rc::new(RefCell::new(ChangeQueue::new()));

    let account = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let (initial_document, persistence_enabled) = load_desired_state(&catalogue.state_notice);
    let desired = Rc::new(RefCell::new(initial_document));
    seed_queue(&mut queue.borrow_mut(), &desired.borrow(), &account);

    let detail = Rc::new(detail::DetailUi::new(
        "Installs into your user account — no administrator rights needed",
    ));
    let updates = Rc::new(updates_page::UpdatesPage::new());
    let settings = Rc::new(settings_page::SettingsPage::new(&account));
    let your_system = Rc::new(your_system::YourSystemPage::new(&account));

    let persist_desired_state = wire_desired_state_persistence(
        &queue,
        &desired,
        &account,
        persistence_enabled,
        &catalogue.state_notice,
    );
    persist_desired_state();
    updates.wire(&queue, &desired, &nix, persist_desired_state.clone());
    settings.wire(&desired, persist_desired_state.clone());

    let pending = header.pending.clone();
    let queue_for_label = queue.clone();
    let update_pending = Rc::new(move || {
        update_pending_button(&queue_for_label.borrow(), &pending);
    });
    let on_queue_change: Rc<dyn Fn()> = {
        let update_pending = update_pending.clone();
        let persist_desired_state = persist_desired_state.clone();
        Rc::new(move || {
            update_pending();
            persist_desired_state();
        })
    };
    let on_queue_change_for_sidebar = on_queue_change.clone();
    detail::wire(&detail, &queue, &nix, &window, on_queue_change);
    update_pending();

    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    stack.add_named(&catalogue.page, Some("list"));
    stack.add_named(&detail.page, Some("detail"));
    stack.add_named(&scrolled_page(&global.page), Some("global-search"));
    // Tall surfaces scroll; a plain Box would push content below the window
    // edge with no way to reach it (onz). Wrap them so everything is shown.
    stack.add_named(&scrolled_page(&updates.page), Some("updates"));
    stack.add_named(&scrolled_page(&settings.page), Some("settings"));
    stack.add_named(&scrolled_page(&your_system.page), Some("your-system"));
    stack.set_visible_child_name("list");

    // Load current config values when Settings page becomes visible (lazy, non-blocking)
    let settings_for_load = settings.clone();
    stack.connect_visible_child_name_notify(move |s| {
        if s.visible_child_name().as_deref() == Some("settings") {
            settings_for_load.load_current_values();
        }
    });

    let sidebar = build_sidebar(
        &stack,
        &updates,
        &queue,
        &your_system,
        &desired,
        &on_queue_change_for_sidebar,
        &global.entry,
    );
    let body = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    body.append(&sidebar);
    body.append(&stack);
    window.set_child(Some(&body));

    wire_pending_button(&header.pending, &stack, &updates, &queue);
    let back_button = header.back.clone();
    stack.connect_visible_child_name_notify(move |s| {
        back_button.set_visible(s.visible_child_name().as_deref() == Some("detail"));
    });
    let stack_for_back = stack.clone();
    header
        .back
        .connect_clicked(move |_| stack_for_back.set_visible_child_name("list"));

    set_catalogue_summary(&catalogue.summary, &all.borrow(), load_error.as_deref());
    wire_row_activation(&catalogue.listbox, &shown, &detail, &queue, &stack);
    // Store-local search (fyq): filters only the catalogue, lives in the
    // Apps tab; the global search below spans apps + settings.
    wire_search(
        &catalogue.search,
        &catalogue.listbox,
        &stack,
        &catalogue.summary,
        &all,
        &texts,
        &shown,
    );
    wire_search_shortcut(&global.entry, &window);
    wire_global_search(&global, &stack, &all, &texts);
    wire_global_activation(&global, &detail, &queue, &stack, &settings, &all, &texts);

    // Non-free filter (D1): flipping re-filters the full set and rebuilds.
    {
        let all = all.clone();
        let texts = texts.clone();
        let shown = shown.clone();
        let listbox = catalogue.listbox.clone();
        let summary = catalogue.summary.clone();
        let search = catalogue.search.clone();
        let full_entries = full_entries.clone();
        let hide = hide_unfree.clone();
        catalogue.unfree_filter.connect_toggled(move |btn| {
            hide.set(btn.is_active());
            {
                let mut a = all.borrow_mut();
                let mut t = texts.borrow_mut();
                *a = search::filter_unfree(&full_entries, hide.get());
                *t = a.iter().map(SearchText::build).collect();
            }
            let count = rebuild_list(
                &listbox,
                &all.borrow(),
                &texts.borrow(),
                &shown,
                &search.text(),
            );
            summary.set_text(&format!("{count} shown"));
        });
    }

    rebuild_list(
        &catalogue.listbox,
        &all.borrow(),
        &texts.borrow(),
        &shown,
        &catalogue.search.text(),
    );
    window.present();
}
