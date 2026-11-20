//! App shell: sidebar + stack of surfaces (Store list, detail, Updates). This
//! is the UI composition point; the detail/install flow is in `detail`, the
//! updates page in `updates_page`. Pure ranking lives in crate::search.
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::desired_state::DesiredDoc;
use crate::model::{Class, IndexEntry, SearchText};
use crate::search;
use crate::store::ChangeQueue;
use crate::ui::{detail, settings_page, updates_page, widgets, your_system};

/// How many rows the list shows at once (empty query is a bounded browse).
const MAX_ROWS: usize = 60;

/// Group order for global search results (D16/D18 labelled groups).
const GROUP_ORDER: [Class; 4] = [Class::App, Class::Unverified, Class::TermApp, Class::Cli];

/// Rank + render the list. Empty query: bounded browse in index order. Search:
/// ranked by search::rank, bucketed into the D16/D18 labelled groups, capped.
/// Writes the visible row order into `shown` so row-index maps back to entries.
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
        for e in all.iter().take(MAX_ROWS) {
            listbox.append(&widgets::entry_row(e));
            rows.push(Some(e.clone()));
        }
    } else {
        let picked: Vec<&IndexEntry> = search::rank(query, texts, MAX_ROWS)
            .into_iter()
            .map(|i| &all[i])
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
    search: gtk4::SearchEntry,
    unfree_filter: gtk4::CheckButton,
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

    let search = gtk4::SearchEntry::new();
    search.set_placeholder_text(Some("Search apps…"));
    search.set_hexpand(true);
    header.pack_end(&search);

    // optional non-free filter (decision D1): off by default
    let unfree_filter = gtk4::CheckButton::with_label("Hide non-free");
    header.pack_end(&unfree_filter);
    window.set_titlebar(Some(&header));

    HeaderWidgets {
        back,
        pending,
        search,
        unfree_filter,
    }
}

struct CatalogueList {
    page: gtk4::Box,
    listbox: gtk4::ListBox,
    summary: gtk4::Label,
    state_notice: gtk4::Label,
}

fn build_catalogue_list() -> CatalogueList {
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
    page.append(&state_notice);
    page.append(&summary);
    page.append(&scrolled);

    CatalogueList {
        page,
        listbox,
        summary,
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
            eprintln!("nix-pilot: could not save desired state: {error}");
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
) -> gtk4::Box {
    let sidebar = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    sidebar.set_width_request(220);
    sidebar.append(&nav_button("Apps", "list", stack, None));
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

    let mut counts = [0usize; 4];
    for entry in entries {
        let index = match entry.class {
            Class::App => 0,
            Class::Unverified => 1,
            Class::TermApp => 2,
            _ => 3,
        };
        counts[index] += 1;
    }
    summary.set_text(&format!(
        "{} entries · {} application · {} unverified · {} terminal · {} command-line — click a result for details",
        entries.len(), counts[0], counts[1], counts[2], counts[3]
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
    );
    // the Settings home's "Updates" row navigates to the Updates surface
    let settings_for_updates = settings.clone();
    let stack_for_updates = stack.clone();
    settings_for_updates.wire_updates(Rc::new(move || {
        stack_for_updates.set_visible_child_name("updates")
    }));
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
    wire_search(
        &header.search,
        &catalogue.listbox,
        &stack,
        &catalogue.summary,
        &all,
        &texts,
        &shown,
    );
    wire_search_shortcut(&header.search, &window);

    // Non-free filter (D1): flipping re-filters the full set and rebuilds.
    {
        let all = all.clone();
        let texts = texts.clone();
        let shown = shown.clone();
        let listbox = catalogue.listbox.clone();
        let summary = catalogue.summary.clone();
        let search = header.search.clone();
        let full_entries = full_entries.clone();
        let hide = hide_unfree.clone();
        header.unfree_filter.connect_toggled(move |btn| {
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
        &header.search.text(),
    );
    window.present();
}
