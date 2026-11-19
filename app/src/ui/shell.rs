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
use crate::ui::{detail, settings_page, updates_page, widgets};

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

    // header: back (detail only), pending-changes, global search
    let header = gtk4::HeaderBar::new();
    let back = gtk4::Button::new();
    back.set_label("‹ Back to results");
    back.set_visible(false);
    header.pack_start(&back);
    let pending_btn = gtk4::Button::new();
    pending_btn.add_css_class("flat");
    pending_btn.add_css_class("dim-label");
    pending_btn.set_visible(false);
    header.pack_end(&pending_btn);
    let search = gtk4::SearchEntry::new();
    search.set_placeholder_text(Some("Search apps…"));
    search.set_hexpand(true);
    header.pack_end(&search);
    window.set_titlebar(Some(&header));

    // shared app state
    let all = Rc::new(RefCell::new(entries));
    let texts = Rc::new(RefCell::new(
        all.borrow()
            .iter()
            .map(SearchText::build)
            .collect::<Vec<_>>(),
    ));
    let shown = Rc::new(RefCell::new(Vec::<Option<IndexEntry>>::new()));
    let queue = Rc::new(RefCell::new(ChangeQueue::new()));

    // list page
    let summary = gtk4::Label::new(None);
    summary.set_xalign(0.0);
    summary.add_css_class("dim-label");
    let listbox = gtk4::ListBox::new();
    let scrolled = gtk4::ScrolledWindow::builder()
        .child(&listbox)
        .vexpand(true)
        .build();
    let list_page = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    list_page.set_margin_top(6);
    list_page.set_margin_bottom(6);
    list_page.append(&summary);
    list_page.append(&scrolled);

    // detail/install page
    let detail = Rc::new(detail::DetailUi::new(
        "Installs into your user account — no administrator rights needed",
    ));

    // desired-state persistence (7wo.2): the app's owned model, stored one
    // file per running account. The OS user stands in for the NixOS account;
    // the backend resolves the real one at apply time.
    let account: String = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let wanted_path = crate::desired_state::default_desired_path();
    let desired = Rc::new(RefCell::new(
        crate::desired_state::load(&wanted_path).unwrap_or_else(|e| {
            eprintln!("nix-pilot: {e}, starting with a clean desired state");
            DesiredDoc::current()
        }),
    ));

    // seed the in-app queue from the persisted state so selections survive a
    // restart (attrs double as display names until re-selected)
    {
        let mut q = queue.borrow_mut();
        let d = desired.borrow();
        for a in d.system_packages() {
            q.add(crate::store::PendingChange {
                attr: a.clone(),
                name: a.clone(),
                system_wide: true,
            });
        }
        for a in d.packages_for(&account) {
            q.add(crate::store::PendingChange {
                attr: a.clone(),
                name: a.clone(),
                system_wide: false,
            });
        }
    }

    // updates page (builds/reviews from the persisted desired doc)
    let updates = Rc::new(updates_page::UpdatesPage::new());
    updates.wire(&queue, &desired, &nix);

    // settings page (editable workflows)
    let settings = Rc::new(settings_page::SettingsPage::new());

    // pending-changes header button -> Updates page (refresh on show)
    let update_pending_label: Rc<dyn Fn()> = {
        let queue = queue.clone();
        let label = pending_btn.clone();
        Rc::new(move || {
            let q = queue.borrow();
            if q.is_empty() {
                label.set_visible(false);
                label.set_label("");
            } else {
                let pl = if q.len() == 1 { "" } else { "s" };
                label.set_label(&format!("{} change{} ready to apply", q.len(), pl));
                let items: Vec<String> = q
                    .summary()
                    .iter()
                    .map(|(name, system_wide)| {
                        format!(
                            "- {} ({})",
                            name,
                            if *system_wide { "system" } else { "user" }
                        )
                    })
                    .collect();
                label.set_tooltip_text(Some(&items.join("\n")));
                label.set_visible(true);
            }
        })
    };
    // persist after every queue change: rebuild this account's user scope and
    // the system scope from the queue, keeping other sections already present
    let persist: Rc<dyn Fn()> = {
        let queue = queue.clone();
        let desired = desired.clone();
        let account = account.clone();
        let wanted_path = wanted_path.clone();
        Rc::new(move || {
            let mut sys = Vec::new();
            let mut user = Vec::new();
            for p in queue.borrow().items() {
                if p.system_wide {
                    sys.push(p.attr.clone());
                } else {
                    user.push(p.attr.clone());
                }
            }
            {
                let mut d = desired.borrow_mut();
                d.set_system(sys);
                d.set_user(&account, user);
            }
            let d = desired.borrow();
            if let Err(e) = crate::desired_state::save(&d, &wanted_path) {
                eprintln!("nix-pilot: could not save desired state: {e}");
            }
        })
    };
    // persist the freshly-seeded queue before any user action
    persist();

    // settings edits write back through the same persist (they touch only the
    // settings map; the system/users rebuild from the queue stays untouched)
    settings.wire(&desired, persist.clone());

    let on_change: Rc<dyn Fn()> = {
        let label = update_pending_label.clone();
        let persist = persist.clone();
        Rc::new(move || {
            label();
            persist();
        })
    };

    // install flow + home button (owns the state machine)
    detail::wire(&detail, &queue, &nix, &window, on_change);
    update_pending_label();

    // stack: list <-> detail <-> updates
    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    stack.add_named(&list_page, Some("list"));
    stack.add_named(&detail.page, Some("detail"));
    stack.add_named(&updates.page, Some("updates"));
    stack.add_named(&settings.page, Some("settings"));
    stack.set_visible_child_name("list");

    // sidebar nav
    let sidebar = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    sidebar.set_width_request(220);
    sidebar.append(&nav_button("Home", "list", &stack, None));
    sidebar.append(&nav_button("Settings", "settings", &stack, None));
    {
        let updates = updates.clone();
        let queue = queue.clone();
        let show = Rc::new(move || updates.refresh(&queue));
        sidebar.append(&nav_button("Updates", "updates", &stack, Some(show)));
    }

    let body = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    body.append(&sidebar);
    body.append(&stack);
    window.set_child(Some(&body));

    // pending button -> Updates
    {
        let stack = stack.clone();
        let updates = updates.clone();
        let queue = queue.clone();
        pending_btn.connect_clicked(move |_| {
            updates.refresh(&queue);
            stack.set_visible_child_name("updates");
        });
    }

    let back2 = back.clone();
    let stack_note = stack.clone();
    stack.connect_visible_child_name_notify(move |s| {
        back2.set_visible(s.visible_child_name().as_deref() == Some("detail"));
        let _ = stack_note;
    });
    let stack_back = stack.clone();
    back.connect_clicked(move |_| stack_back.set_visible_child_name("list"));

    // list header summary
    let by_class = {
        let es = all.borrow();
        es.iter().fold([0i64; 4], |mut a, e| {
            match e.class {
                Class::App => a[0] += 1,
                Class::Unverified => a[1] += 1,
                Class::TermApp => a[2] += 1,
                _ => a[3] += 1,
            }
            a
        })
    };
    if let Some(e) = load_error {
        summary.set_text(&e);
    } else {
        summary.set_text(&format!(
            "{} entries · {} application · {} unverified · {} terminal · {} command-line — click a result for details",
            all.borrow().len(),
            by_class[0],
            by_class[1],
            by_class[2],
            by_class[3]
        ));
    }

    // detail on row activation
    let detail_for_row = detail.clone();
    let stack_for_row = stack.clone();
    let shown_for_row = shown.clone();
    let queue_for_row = queue.clone();
    listbox.connect_row_activated(move |_, row| {
        let i = row.index();
        if i >= 0 {
            if let Some(Some(e)) = shown_for_row.borrow().get(i as usize).cloned() {
                detail_for_row.populate(&e);
                detail_for_row.sync_added(queue_for_row.borrow().contains(&e.attr));
                stack_for_row.set_visible_child_name("detail");
            }
        }
    });

    // live global search
    let stack_live = stack.clone();
    let summary_live = summary.clone();
    let listbox_live = listbox.clone();
    let all_live = all.clone();
    let texts_live = texts.clone();
    let shown_live = shown.clone();
    search.connect_search_changed(move |s| {
        let n = rebuild_list(
            &listbox_live,
            &all_live.borrow(),
            &texts_live.borrow(),
            &shown_live,
            &s.text(),
        );
        summary_live.set_text(&format!("{} shown", n));
        stack_live.set_visible_child_name("list");
    });
    // Esc clears the query and shows the browse list again
    let listbox_esc = listbox.clone();
    let all_esc = all.clone();
    let texts_esc = texts.clone();
    let shown_esc = shown.clone();
    let summary_esc = summary.clone();
    search.connect_stop_search(move |s| {
        if !s.text().is_empty() {
            s.set_text("");
        } else {
            let n = rebuild_list(
                &listbox_esc,
                &all_esc.borrow(),
                &texts_esc.borrow(),
                &shown_esc,
                "",
            );
            summary_esc.set_text(&format!("{} shown", n));
        }
    });
    // Ctrl+K focuses the global search
    let kc = gtk4::EventControllerKey::new();
    let search_k = search.clone();
    kc.connect_key_pressed(move |_, key, _, mods| {
        if key == gtk4::gdk::Key::k && mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
            search_k.grab_focus();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(kc);

    rebuild_list(
        &listbox,
        &all.borrow(),
        &texts.borrow(),
        &shown,
        &search.text(),
    );
    window.present();
}
