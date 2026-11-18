// nix-pilot desktop client — Software Store (D19: Rust + GTK4 + libadwaita).
//
// This file is the UI surface + startup composition. Pure logic lives in the
// sibling modules: model (catalogue domain), search (ranking), store (desired
// change queue + result rule), install (nix adapter). UI calls those; they do
// not depend on GTK and the adapter owns all external I/O.
//
// Global search is v1 catalogue-only (spec: specifications/global-search.md):
// the header box is whole-app, the Store list shows ranked D16/D18 groups, and
// clicking a result opens a detail/install page. The install button is an
// intent flow: check availability, queue the change; applying is the Updates
// transaction (a later surface) and nothing here mutates the system.
use std::cell::RefCell;
use std::rc::Rc;

use glib::ExitCode;
use gtk4::prelude::*;

mod install;
mod model;
mod search;
mod store;

use model::{Class, IndexEntry, SearchText};
use store::ChangeQueue;

/// How many rows the list shows at once (empty query is a bounded browse).
const MAX_ROWS: usize = 60;

/// Group order for global search results (D16/D18 labelled groups).
const GROUP_ORDER: [Class; 4] = [Class::App, Class::Unverified, Class::TermApp, Class::Cli];

// ------- index loading ----------------------------------------------------------
fn resolve_index_path() -> Option<String> {
    if let Some(p) = std::env::args().nth(1) {
        return Some(p);
    }
    for cand in [
        "experiments/catalogue/results/catalogue-index.json", // repo root cwd
        "results/catalogue-index.json",                       // catalogue cwd
        "../experiments/catalogue/results/catalogue-index.json",
    ] {
        if std::path::Path::new(cand).exists() {
            return Some(cand.to_string());
        }
    }
    None
}

fn load_index(path: &str) -> Result<Vec<IndexEntry>, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?;
    let idx: model::CatalogueIndex =
        serde_json::from_str(&raw).map_err(|e| format!("cannot parse index: {}", e))?;
    Ok(idx.entries)
}

// ------- widget helpers --------------------------------------------------------
fn esc(s: &str) -> String {
    glib::markup_escape_text(s).to_string()
}

fn entry_row(e: &IndexEntry) -> gtk4::ListBoxRow {
    let boxv = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    boxv.set_margin_top(8);
    boxv.set_margin_bottom(8);
    boxv.set_margin_start(10);
    boxv.set_margin_end(10);

    let title = gtk4::Label::new(None);
    title.set_xalign(0.0);
    let version = e.version.clone().unwrap_or_default();
    title.set_markup(&format!(
        "<b>{}</b>  <span size='small'>{} · v{}</span>",
        esc(e.display_name()),
        esc(e.class.label()),
        esc(&version)
    ));
    boxv.append(&title);

    if let Some(d) = &e.description {
        let desc = gtk4::Label::new(Some(d));
        desc.set_xalign(0.0);
        desc.set_wrap(false);
        desc.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        desc.add_css_class("dim-label");
        boxv.append(&desc);
    }

    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&boxv));
    row
}

/// Non-clickable group header row ("Applications · 12 results").
fn group_header_row(label: &str, count: usize) -> gtk4::ListBoxRow {
    let s = if count == 1 { "" } else { "s" };
    let l = gtk4::Label::new(None);
    l.set_xalign(0.0);
    l.set_margin_top(12);
    l.set_margin_start(10);
    l.set_margin_end(10);
    l.set_markup(&format!(
        "<b>{}</b>  <span size='small' color='gray'>{} result{s}</span>",
        esc(label),
        count
    ));
    let row = gtk4::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.set_child(Some(&l));
    row
}

fn meta_row(key: &str, value: &str) -> gtk4::Box {
    let h = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
    let k = gtk4::Label::new(Some(key));
    k.set_xalign(0.0);
    k.set_width_chars(14);
    k.add_css_class("dim-label");
    let v = gtk4::Label::new(Some(value));
    v.set_xalign(0.0);
    v.set_wrap(true);
    v.set_hexpand(true);
    h.append(&k);
    h.append(&v);
    h
}

/// Install intent state for the detail card (see the click handler).
#[derive(Clone, Copy, PartialEq, Debug)]
enum InstallState {
    Idle,
    Checking,
    Available,
    Unavailable,
    Added,
}

/// Detail/install page. GTK objects have interior mutability, so an Rc is
/// enough to share it across activation closures while populate() takes &self.
struct DetailUi {
    title: gtk4::Label,
    sub: gtk4::Label,
    desc: gtk4::Label,
    scope: gtk4::Switch,
    meta: gtk4::Box,
    home: gtk4::Button,
    page: gtk4::Box,
    btn_install: gtk4::Button,
    status: gtk4::Label,
    expander: gtk4::Expander,
    tech: gtk4::Label,
    home_url: Rc<RefCell<Option<String>>>,
    state: Rc<RefCell<InstallState>>,
    current_attr: Rc<RefCell<Option<String>>>,
}

impl DetailUi {
    fn new(install_hint: &str) -> DetailUi {
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        page.set_margin_start(42);
        page.set_margin_end(42);
        page.set_margin_top(20);

        let head = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        let title = gtk4::Label::new(None);
        title.add_css_class("title-1");
        title.set_xalign(0.0);
        let sub = gtk4::Label::new(None);
        sub.set_xalign(0.0);
        sub.add_css_class("dim-label");
        let desc = gtk4::Label::new(None);
        desc.set_xalign(0.0);
        desc.set_wrap(true);
        desc.set_visible(false);
        head.append(&title);
        head.append(&sub);
        head.append(&desc);
        page.append(&head);

        let sep = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        sep.set_margin_top(18);
        sep.set_margin_bottom(14);
        page.append(&sep);

        // install card (per-user scope, D9)
        let card = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        card.add_css_class("card");
        let inst_h = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        let install = gtk4::Button::with_label("Install for me");
        install.add_css_class("suggested-action");
        install.set_valign(gtk4::Align::Center);
        install.set_tooltip_text(Some("Adds this app to your ready-to-change list."));
        let hint = gtk4::Label::new(Some(install_hint));
        hint.set_wrap(true);
        hint.add_css_class("dim-label");
        hint.set_hexpand(true);
        inst_h.append(&install);
        inst_h.append(&hint);
        card.append(&inst_h);

        let status = gtk4::Label::new(None);
        status.set_xalign(0.0);
        status.set_wrap(true);
        status.add_css_class("dim-label");
        card.append(&status);

        let tech = gtk4::Label::new(None);
        tech.set_xalign(0.0);
        tech.set_wrap(true);
        let expander = gtk4::Expander::new(Some("Technical detail"));
        expander.set_visible(false);
        expander.set_child(Some(&tech));
        card.append(&expander);

        let scope_h = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        let scope_labelv = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        let scope_label = gtk4::Label::new(Some("Install for everyone on this computer"));
        scope_label.set_xalign(0.0);
        let scope_sub = gtk4::Label::new(Some(
            "Choose this only if you administer the system (system-wide installation)",
        ));
        scope_sub.set_xalign(0.0);
        scope_sub.add_css_class("dim-label");
        scope_labelv.append(&scope_label);
        scope_labelv.append(&scope_sub);
        scope_labelv.set_hexpand(true);
        let scope = gtk4::Switch::new();
        scope_h.append(&scope_labelv);
        scope_h.append(&scope);
        card.append(&scope_h);
        page.append(&card);

        let more = gtk4::Label::new(Some("More information"));
        more.add_css_class("heading");
        more.set_xalign(0.0);
        more.set_margin_top(14);
        page.append(&more);

        let meta_card = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        meta_card.add_css_class("card");
        let meta = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        meta_card.append(&meta);
        page.append(&meta_card);

        let home = gtk4::Button::with_label("Open project home page");
        home.set_visible(false);
        home.set_margin_top(8);
        page.append(&home);

        DetailUi {
            title,
            sub,
            desc,
            scope,
            meta,
            home,
            page,
            btn_install: install,
            status,
            expander,
            tech,
            home_url: Rc::new(RefCell::new(None)),
            state: Rc::new(RefCell::new(InstallState::Idle)),
            current_attr: Rc::new(RefCell::new(None)),
        }
    }

    fn populate(&self, e: &IndexEntry) {
        let scope_default = e
            .install
            .as_ref()
            .and_then(|i| i.scope_default.as_deref())
            .unwrap_or("user");
        self.title.set_label(e.display_name());
        let version = e.version.clone().unwrap_or_default();
        self.sub.set_markup(&format!(
            "{} · version {}",
            esc(e.class.label()),
            esc(&version)
        ));
        if let Some(d) = &e.description {
            self.desc.set_label(d);
            self.desc.set_visible(true);
        } else {
            self.desc.set_label("");
            self.desc.set_visible(false);
        }
        self.scope.set_active(scope_default == "system");
        *self.home_url.borrow_mut() = e.home_page.clone();

        while let Some(child) = self.meta.first_child() {
            self.meta.remove(&child);
        }
        let licence = if e.unfree {
            "Unfree licence"
        } else {
            "Free licence"
        };
        let platforms = if e.linux_ok {
            "Linux (x86_64 / aarch64)"
        } else {
            "Unknown"
        };
        let main = e.main_program.as_deref().unwrap_or("—");
        self.meta.append(&meta_row("Licence", licence));
        self.meta.append(&meta_row("Evidence", e.tier.label()));
        self.meta.append(&meta_row("Available for", platforms));
        self.meta.append(&meta_row("Command", main));
        self.meta.append(&meta_row(
            "Scope",
            if scope_default == "system" {
                "System-wide"
            } else {
                "Per user"
            },
        ));

        if let Some(hp) = &e.home_page {
            self.home.set_visible(true);
            self.home.set_tooltip_text(Some(hp));
        } else {
            self.home.set_visible(false);
            self.home.set_tooltip_text(None);
        }

        // install card reset (the queue re-syncs this after populate)
        *self.state.borrow_mut() = InstallState::Idle;
        *self.current_attr.borrow_mut() = Some(e.attr.clone());
        self.btn_install.set_sensitive(true);
        self.btn_install.set_label("Install for me");
        self.status.set_text("");
        self.expander.set_visible(false);
        self.expander.set_expanded(false);
    }
}

// ------- list rebuild (ranks + renders, writes row order into `shown`) ---------
/// Maps the row order back to entries (None marks a group header) and renders.
/// Empty query: bounded browse in index order. Search: ranked by search::rank,
/// bucketed into the D16/D18 labelled groups, cap MAX_ROWS in total.
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
            listbox.append(&entry_row(e));
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
            listbox.append(&group_header_row(c.group_label(), v.len()));
            rows.push(None);
            for e in v {
                listbox.append(&entry_row(e));
                rows.push(Some(e.clone()));
            }
        }
    }
    let n = rows.iter().filter(|r| r.is_some()).count();
    *shown.borrow_mut() = rows;
    n
}

// ------- app -------------------------------------------------------------------
fn main() -> ExitCode {
    if libadwaita::init().is_err() {
        eprintln!("failed to initialise libadwaita");
        return ExitCode::FAILURE;
    }

    let app = gtk4::Application::builder()
        .application_id("dev.nixpilot.app")
        .build();

    app.connect_activate(|app| {
        let window = gtk4::ApplicationWindow::builder()
            .application(app)
            .title("nix-pilot — Software Store")
            .default_width(1020)
            .default_height(720)
            .build();

        // header: back (visible only on detail) + live search
        let header = gtk4::HeaderBar::new();
        let back = gtk4::Button::new();
        back.set_label("‹ Back to results");
        back.set_visible(false);
        header.pack_start(&back);
        let search = gtk4::SearchEntry::new();
        search.set_placeholder_text(Some("Search apps…"));
        search.set_hexpand(true);
        header.pack_end(&search);
        window.set_titlebar(Some(&header));

        // index loading (fail loudly in the window, not silently)
        let (all_entries, error) = match resolve_index_path() {
            Some(p) => match load_index(&p) {
                Ok(entries) => (entries, None),
                Err(e) => (vec![], Some(e)),
            },
            None => (
                vec![],
                Some("catalogue index not found — pass a path or run from the repo root".into()),
            ),
        };
        let all = Rc::new(RefCell::new(all_entries));
        let texts = Rc::new(RefCell::new(all.borrow().iter().map(SearchText::build).collect::<Vec<_>>()));
        let shown = Rc::new(RefCell::new(Vec::<Option<IndexEntry>>::new()));

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

        let detail = Rc::new(DetailUi::new(
            "Installs into your user account — no administrator rights needed",
        ));

        // desired changes (application actions) + the nix client for checks
        let queue = Rc::new(RefCell::new(ChangeQueue::new()));
        let nix = install::discover_nix();

        // header count of queued changes (reads the queue)
        let pending_label = gtk4::Label::new(None);
        pending_label.add_css_class("dim-label");
        pending_label.set_visible(false);
        header.pack_end(&pending_label);
        let update_pending_label = {
            let queue = queue.clone();
            let label = pending_label.clone();
            move || {
                let q = queue.borrow();
                if q.is_empty() {
                    label.set_visible(false);
                    label.set_text("");
                } else {
                    let pl = if q.len() == 1 { "" } else { "s" };
                    label.set_text(&format!("{} change{} ready to apply", q.len(), pl));
                    let items: Vec<String> = q
                        .summary()
                        .iter()
                        .map(|(name, system_wide)| {
                            format!("- {} ({})", name, if *system_wide { "system" } else { "user" })
                        })
                        .collect();
                    label.set_tooltip_text(Some(&items.join("\n")));
                    label.set_visible(true);
                }
            }
        };
        update_pending_label();

        // install button state machine: check -> review -> queued (apply is the
        // Updates transaction, next surface). Nothing here mutates the system.
        let d = detail.clone();
        let btn_install = d.btn_install.clone();
        let queue_click = queue.clone();
        let nix_click = nix.clone();
        let update_lbl = update_pending_label.clone();
        btn_install.connect_clicked(move |btn| {
            let attr = match d.current_attr.borrow().clone() {
                Some(a) => a,
                None => return,
            };
            match *d.state.borrow() {
                InstallState::Checking => {}
                InstallState::Added => {
                    queue_click.borrow_mut().remove(&attr);
                    *d.state.borrow_mut() = InstallState::Idle;
                    btn.set_label("Install for me");
                    d.status.set_text("");
                    d.expander.set_visible(false);
                    update_lbl();
                }
                InstallState::Available => {
                    let system_wide = d.scope.is_active();
                    let name = d.title.text().to_string();
                    queue_click.borrow_mut().add(store::PendingChange {
                        attr: attr.clone(),
                        name,
                        system_wide,
                    });
                    *d.state.borrow_mut() = InstallState::Added;
                    btn.set_label("Remove from changes");
                    d.status.set_text(
                        "Added to your ready-to-change list — installs apply through Updates (next surface)",
                    );
                    update_lbl();
                }
                InstallState::Idle | InstallState::Unavailable => {
                    let Some(nixp) = nix_click.clone() else {
                        d.status.set_text("The package checker is not available on this system");
                        return;
                    };
                    *d.state.borrow_mut() = InstallState::Checking;
                    btn.set_sensitive(false);
                    btn.set_label("Checking…");
                    d.status.set_text("Checking availability…");
                    d.expander.set_visible(false);
                    d.expander.set_expanded(false);
                    let state = d.state.clone();
                    let b = btn.clone();
                    let status = d.status.clone();
                    let tech = d.tech.clone();
                    let exp = d.expander.clone();
                    // guard against a stale result landing after the user
                    // switched to a different entry while the check ran
                    let attr_for_result = attr.clone();
                    let current_attr = d.current_attr.clone();
                    let (tx, rx) = std::sync::mpsc::channel::<install::CheckResult>();
                    std::thread::spawn(move || {
                        let r = install::check_availability(
                            &nixp,
                            &attr,
                            std::time::Duration::from_secs(60),
                        );
                        let _ = tx.send(r);
                    });
                    // main-loop poller: only the result crosses threads; the
                    // widget handles stay on the main thread (no Send bound).
                    glib::timeout_add_local(std::time::Duration::from_millis(100), move || match rx.try_recv() {
                            Ok(r) => {
                                // stale results (user switched entries mid-check)
                                // leave the card untouched
                                if !store::result_applies(
                                    current_attr.borrow().as_deref().unwrap_or(""),
                                    &attr_for_result,
                                ) {
                                    return glib::ControlFlow::Break;
                                }
                                match r.outcome {
                                    install::Outcome::ConfirmedAvailable { note } => {
                                        *state.borrow_mut() = InstallState::Available;
                                        b.set_sensitive(true);
                                        b.set_label("Add to ready-to-change list");
                                        status.set_text(if note.is_empty() {
                                            "Ready to install for you"
                                        } else {
                                            note.as_str()
                                        });
                                    }
                                    other => {
                                        *state.borrow_mut() = InstallState::Unavailable;
                                        b.set_sensitive(true);
                                        b.set_label("Check again");
                                        let wording = match other {
                                            install::Outcome::UnfreeEvalBlocked => {
                                                "Unfree — your system does not allow it".to_string()
                                            }
                                            install::Outcome::Unavailable => {
                                                "Not available for this system".to_string()
                                            }
                                            install::Outcome::CheckFailed { reason } => reason,
                                            _ => unreachable!(),
                                        };
                                        status.set_text(&wording);
                                        tech.set_text(&r.detail);
                                        exp.set_visible(true);
                                        exp.set_expanded(true);
                                    }
                                }
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                glib::ControlFlow::Continue
                            }
                            Err(_) => {
                                *state.borrow_mut() = InstallState::Unavailable;
                                b.set_sensitive(true);
                                b.set_label("Check again");
                                status.set_text("Could not be checked right now");
                                glib::ControlFlow::Break
                            }
                        },
                    );
                }
            }
        });

        // home page button actually opens the URL
        let home_btn = detail.home.clone();
        let home_url = detail.home_url.clone();
        let win = window.clone();
        home_btn.connect_clicked(move |_| {
            if let Some(u) = home_url.borrow().as_deref() {
                gtk4::show_uri(Some(&win), u, gtk4::gdk::CURRENT_TIME);
            }
        });

        let stack = gtk4::Stack::new();
        stack.add_named(&list_page, Some("list"));
        stack.add_named(&detail.page, Some("detail"));
        stack.set_visible_child_name("list");
        window.set_child(Some(&stack));

        // header back button toggles with the page
        let back2 = back.clone();
        stack.connect_visible_child_name_notify(move |s| {
            back2.set_visible(s.visible_child_name().as_deref() == Some("detail"));
        });
        let stack_back = stack.clone();
        back.connect_clicked(move |_| stack_back.set_visible_child_name("list"));

        // summary header across the top of the list page
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
        if let Some(e) = error {
            summary.set_text(&e);
        } else {
            summary.set_text(&format!(
                "{} entries · {} application · {} unverified · {} terminal · {} command-line — click a result for details",
                all.borrow().len(), by_class[0], by_class[1], by_class[2], by_class[3]
            ));
        }

        // row activation opens the detail page for the selected entry
        let detail_for_row = detail.clone();
        let stack_for_row = stack.clone();
        let shown_for_row = shown.clone();
        let queue_for_row = queue.clone();
        listbox.connect_row_activated(move |_, row| {
            let i = row.index();
            if i >= 0 {
                if let Some(Some(e)) = shown_for_row.borrow().get(i as usize).cloned() {
                    detail_for_row.populate(&e);
                    if queue_for_row.borrow().contains(&e.attr) {
                        *detail_for_row.state.borrow_mut() = InstallState::Added;
                        detail_for_row.btn_install.set_label("Remove from changes");
                        detail_for_row.status.set_text(
                            "Added to your ready-to-change list — installs apply through Updates (next surface)",
                        );
                    }
                    stack_for_row.set_visible_child_name("detail");
                }
            }
        });

        // live global search: rerank into groups, return to the list page
        let stack_live = stack.clone();
        let summary_live = summary.clone();
        let listbox_live = listbox.clone();
        let all_live = all.clone();
        let texts_live = texts.clone();
        let shown_live = shown.clone();
        search.connect_search_changed(move |s| {
            let n = rebuild_list(&listbox_live, &all_live.borrow(), &texts_live.borrow(), &shown_live, &s.text());
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
                let n = rebuild_list(&listbox_esc, &all_esc.borrow(), &texts_esc.borrow(), &shown_esc, "");
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

        // initial browse paint
        rebuild_list(&listbox, &all.borrow(), &texts.borrow(), &shown, &search.text());

        window.present();
    });

    app.run()
}
