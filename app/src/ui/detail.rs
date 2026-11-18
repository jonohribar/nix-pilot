//! Store detail/install page. Builds the page and owns the install-intent
//! state machine (check -> review -> queued); the real apply is the Updates
//! transaction (a later surface) and nothing here mutates the system.
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::install;
use crate::model::IndexEntry;
use crate::store;
use crate::store::{ChangeQueue, PendingChange};
use crate::ui::widgets::{esc, meta_row};

/// Install intent state for the detail card (driven by wire_install).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum InstallState {
    Idle,
    Checking,
    Available,
    Unavailable,
    Added,
}

/// Detail/install page. GTK objects have interior mutability, so an Rc is
/// enough to share it across activation closures while populate() takes &self.
pub struct DetailUi {
    pub title: gtk4::Label,
    pub sub: gtk4::Label,
    pub desc: gtk4::Label,
    pub scope: gtk4::Switch,
    pub meta: gtk4::Box,
    pub home: gtk4::Button,
    pub page: gtk4::Box,
    pub btn_install: gtk4::Button,
    pub status: gtk4::Label,
    pub expander: gtk4::Expander,
    pub tech: gtk4::Label,
    pub home_url: Rc<RefCell<Option<String>>>,
    pub state: Rc<RefCell<InstallState>>,
    pub current_attr: Rc<RefCell<Option<String>>>,
}

impl DetailUi {
    pub fn new(install_hint: &str) -> DetailUi {
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

    /// Fill the fields for the selected entry and reset the install card.
    pub fn populate(&self, e: &IndexEntry) {
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

        // install card reset (the queue re-syncs this via sync_added after)
        *self.state.borrow_mut() = InstallState::Idle;
        *self.current_attr.borrow_mut() = Some(e.attr.clone());
        self.btn_install.set_sensitive(true);
        self.btn_install.set_label("Install for me");
        self.status.set_text("");
        self.expander.set_visible(false);
        self.expander.set_expanded(false);
    }

    /// Reflect that the current entry is already queued, if it is.
    pub fn sync_added(&self, queued: bool) {
        if queued {
            *self.state.borrow_mut() = InstallState::Added;
            self.btn_install.set_label("Remove from changes");
            self.status.set_text(
                "Added to your ready-to-change list — installs apply through Updates (next surface)",
            );
        }
    }
}

const ADDED_TEXT: &str =
    "Added to your ready-to-change list — installs apply through Updates (next surface)";

/// Wire the install button (async availability check + queue) and the home-page
/// button. `on_change` is called whenever the queue changes so the shell can
/// refresh its header count. Nothing here mutates the system.
pub fn wire(
    d: &Rc<DetailUi>,
    queue: &Rc<RefCell<ChangeQueue>>,
    nix: &Option<std::path::PathBuf>,
    home_win: &gtk4::ApplicationWindow,
    on_change: Rc<dyn Fn()>,
) {
    let d = d.clone();
    let d_home = d.clone();
    let btn_install = d.btn_install.clone();
    let queue_click = queue.clone();
    let nix_click = nix.clone();
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
                on_change();
            }
            InstallState::Available => {
                let system_wide = d.scope.is_active();
                let name = d.title.text().to_string();
                queue_click.borrow_mut().add(PendingChange {
                    attr: attr.clone(),
                    name,
                    system_wide,
                });
                *d.state.borrow_mut() = InstallState::Added;
                btn.set_label("Remove from changes");
                d.status.set_text(ADDED_TEXT);
                on_change();
            }
            InstallState::Idle | InstallState::Unavailable => {
                let Some(nixp) = nix_click.clone() else {
                    d.status
                        .set_text("The package checker is not available on this system");
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
                // guard against a stale result landing after the user switched
                // to a different entry while the check ran
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
                glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                    match rx.try_recv() {
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
                        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(_) => {
                            *state.borrow_mut() = InstallState::Unavailable;
                            b.set_sensitive(true);
                            b.set_label("Check again");
                            status.set_text("Could not be checked right now");
                            glib::ControlFlow::Break
                        }
                    }
                });
            }
        }
    });

    // home page button actually opens the URL
    let home_btn = d_home.home.clone();
    let home_url = d_home.home_url.clone();
    let win = home_win.clone();
    home_btn.connect_clicked(move |_| {
        if let Some(u) = home_url.borrow().as_deref() {
            gtk4::show_uri(Some(&win), u, gtk4::gdk::CURRENT_TIME);
        }
    });
}
