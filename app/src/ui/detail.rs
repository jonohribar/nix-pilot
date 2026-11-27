//! Store detail/install page. It checks package availability and lets the user
//! add or remove install intent; system changes are handled separately by Updates.
//! The "Try it" action runs `nix run nixpkgs#<attr>` as a detached child with
//! a Starting/Running/Ended lifecycle (try_run).
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use gtk4::prelude::*;

use crate::desired_state::{DesiredDoc, PackageScope};
use crate::install;
use crate::model::IndexEntry;
use crate::store;
use crate::try_run::{should_show_try_button, TrySession, TryState};
use crate::ui::widgets::{esc, meta_row};

/// Availability and queue state shown by the detail card.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum InstallState {
    /// No availability check has completed for the current entry.
    Idle,
    /// The package availability probe is running.
    Checking,
    /// The package can be added to the desired state.
    Available,
    /// The package is unavailable or could not be confirmed.
    Unavailable,
    /// The package is already selected in the currently shown scope.
    Added,
}

/// Detail/install page. GTK objects have interior mutability, so an Rc is
/// enough to share it across activation closures while populate() takes &self.
pub struct DetailUi {
    /// Selected package's display name.
    pub title: gtk4::Label,
    /// Secondary package identity and classification text.
    pub sub: gtk4::Label,
    /// Optional package description.
    pub desc: gtk4::Label,
    /// Chooses between per-account and system-wide install intent.
    pub scope: gtk4::Switch,
    /// The system-wide scope toggle row (hidden when install is unavailable).
    pub scope_row: gtk4::Box,
    /// The card holding the install button + scope toggle.
    pub install_card: gtk4::Box,
    /// Container for package metadata rows.
    pub meta: gtk4::Box,
    /// Opens the selected package's upstream project page.
    pub home: gtk4::Button,
    /// Root widget for the detail page.
    pub page: gtk4::Box,
    /// Adds or removes the selected package from desired state.
    pub btn_install: gtk4::Button,
    /// Runs the app without installing (nix run). Visible for desktop apps only.
    pub btn_try: gtk4::Button,
    /// Plain-language availability or selection status.
    pub status: gtk4::Label,
    /// Collapsible area for technical check output.
    pub expander: gtk4::Expander,
    /// Technical output from an unsuccessful availability check.
    pub tech: gtk4::Label,
    /// Upstream URL associated with the current entry, if any.
    pub home_url: Rc<RefCell<Option<String>>>,
    /// Availability and queue state for the currently selected entry.
    pub state: Rc<RefCell<InstallState>>,
    /// Attribute currently displayed; used to discard stale async check results.
    pub current_attr: Rc<RefCell<Option<String>>>,
    /// Current try session state (Starting/Running/Ended/Idle).
    pub try_state: Rc<RefCell<TryState>>,
    /// Active try session handle (kills child on drop).
    pub try_session: Rc<RefCell<Option<TrySession>>>,
    /// Receiver for try session terminal state from monitor thread.
    pub try_receiver: Rc<RefCell<Option<mpsc::Receiver<TryState>>>>,
}

impl DetailUi {
    /// Build an empty detail page using `install_hint` as guidance beside the action.
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
        let try_btn = gtk4::Button::with_label("Try it");
        try_btn.set_valign(gtk4::Align::Center);
        try_btn.set_tooltip_text(Some("Run this app without installing it"));
        try_btn.set_visible(false); // shown only for desktop apps (Class::App)
        let hint = gtk4::Label::new(Some(install_hint));
        hint.set_wrap(true);
        hint.add_css_class("dim-label");
        hint.set_hexpand(true);
        inst_h.append(&install);
        inst_h.append(&try_btn);
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
            scope_row: scope_h,
            install_card: card,
            meta,
            home,
            page,
            btn_install: install,
            btn_try: try_btn,
            status,
            expander,
            tech,
            home_url: Rc::new(RefCell::new(None)),
            state: Rc::new(RefCell::new(InstallState::Idle)),
            current_attr: Rc::new(RefCell::new(None)),
            try_state: Rc::new(RefCell::new(TryState::Idle)),
            try_session: Rc::new(RefCell::new(None)),
            try_receiver: Rc::new(RefCell::new(None)),
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
        // R07 surface: an entry with no Linux availability in the index is
        // first-class "not available on this system", not a blank field. linux_ok is a
        // yes/no from the catalogue; the index carries no per-architecture
        // data, so the label must not claim specific arches. Real per-arch
        // availability surfaces later via the install check.
        let platforms = if e.linux_ok {
            "Linux"
        } else {
            "Not available on Linux"
        };
        let main = e.main_program.as_deref().unwrap_or("—");
        if !e.linux_ok {
            let l = gtk4::Label::new(Some(
                "This entry is not available on your system — it cannot be installed.",
            ));
            l.set_xalign(0.0);
            l.set_wrap(true);
            l.add_css_class("error");
            self.meta.append(&l);
        }
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

        // Try button: only for desktop apps (Class::App), not CLI/terminal apps.
        let show_try = should_show_try_button(e);
        self.btn_try.set_visible(show_try);

        // install card reset (the desired document re-syncs this via sync_added after).
        // Known-unavailable entries (linux_ok=false from the catalogue) are a
        // first-class state (architecture.md): the install action is not
        // offered, never an enabled CTA beside a "cannot be installed" note.
        *self.state.borrow_mut() = InstallState::Idle;
        *self.current_attr.borrow_mut() = Some(e.attr.clone());
        *self.try_state.borrow_mut() = TryState::Idle;
        *self.try_session.borrow_mut() = None;
        *self.try_receiver.borrow_mut() = None;
        if e.linux_ok {
            self.btn_install.set_sensitive(true);
            self.btn_install.set_label("Install for me");
            self.install_card.set_visible(true);
            self.scope_row.set_visible(true);
        } else {
            self.btn_install.set_sensitive(false);
            self.btn_install.set_label("Not available on this system");
            // The scope toggle is part of the install offer; with no install
            // it is noise, so the whole card steps aside.
            self.install_card.set_visible(false);
        }
        self.status.set_text("");
        self.expander.set_visible(false);
        self.expander.set_expanded(false);
        self.btn_try.set_label(TryState::Idle.button_label());
        self.btn_try
            .set_tooltip_text(Some(TryState::Idle.button_tooltip()));
    }

    /// Reflect selection in the currently chosen scope. If this package is
    /// selected only in the other scope, keep it ready to add here as well.
    /// Start the availability check when an entry opens, so the install button
    /// lands on one already-verified "Add to ready-to-change list" instead of
    /// a misleading manual "Install for me" check click.
    pub fn auto_check(&self, nix: &Option<std::path::PathBuf>, allow_unfree: bool) {
        if !matches!(*self.state.borrow(), InstallState::Idle) {
            return;
        }
        let Some(attr) = self.current_attr.borrow().clone() else {
            return;
        };
        let Some(nixp) = nix.clone() else {
            self.status
                .set_text("The package checker is not available on this system");
            return;
        };
        start_availability_check(self, &self.btn_install, nixp, attr, allow_unfree);
    }

    pub fn sync_added(&self, desired: &DesiredDoc, account: &str) {
        let Some(attr) = self.current_attr.borrow().clone() else {
            return;
        };
        let scope = if self.scope.is_active() {
            PackageScope::System
        } else {
            PackageScope::User
        };
        let selected_here = desired
            .package_intents_for(account)
            .iter()
            .any(|intent| intent.attr == attr && intent.scope == scope);
        let selected_elsewhere = desired.contains_package_for(account, &attr);

        if selected_here {
            *self.state.borrow_mut() = InstallState::Added;
            self.btn_install.set_label("Remove from changes");
            self.status.set_text(ADDED_TEXT);
        } else if selected_elsewhere || *self.state.borrow() == InstallState::Added {
            *self.state.borrow_mut() = InstallState::Available;
            self.btn_install.set_label("Add to ready-to-change list");
            self.status.set_text("Ready to add for the selected scope.");
        }
    }

    /// Handle the Try button click: start/stop/cancel a try session.
    /// Returns true if a try session is active (Starting/Running), false if idle/ended.
    pub fn handle_try_click(&self, allow_unfree: bool) -> bool {
        let current_state = *self.try_state.borrow();
        match current_state {
            TryState::Idle => {
                let Some(attr) = self.current_attr.borrow().clone() else {
                    return false;
                };
                match TrySession::spawn(attr, allow_unfree) {
                    Ok((session, receiver)) => {
                        *self.try_session.borrow_mut() = Some(session);
                        *self.try_receiver.borrow_mut() = Some(receiver);
                        self.update_try_ui(TryState::Starting);
                        self.poll_try_receiver();
                        // Move to Running once the app is likely up (the
                        // monitor reports only exit, so use a grace period).
                        self.schedule_running_transition();
                        true
                    }
                    Err(err) => {
                        self.status
                            .set_text(&format!("Could not start try session: {err}"));
                        false
                    }
                }
            }
            TryState::Starting => {
                // Cancel the build
                if let Some(mut session) = self.try_session.borrow_mut().take() {
                    let _ = session.terminate();
                }
                self.update_try_ui(TryState::Ended {
                    stopped_by_user: true,
                });
                false
            }
            TryState::Running => {
                // Stop the running app
                if let Some(mut session) = self.try_session.borrow_mut().take() {
                    let _ = session.terminate();
                }
                self.update_try_ui(TryState::Ended {
                    stopped_by_user: true,
                });
                false
            }
            TryState::Ended { .. } => {
                // Terminal state: reset to idle on next click
                self.reset_try_ui();
                false
            }
        }
    }

    /// Update UI for a new try state.
    fn update_try_ui(&self, state: TryState) {
        apply_try_state(
            state,
            &self.try_state,
            &self.btn_try,
            &self.status,
            &self.title,
        );
    }

    /// Reset try UI to idle state.
    fn reset_try_ui(&self) {
        *self.try_state.borrow_mut() = TryState::Idle;
        *self.try_session.borrow_mut() = None;
        *self.try_receiver.borrow_mut() = None;
        self.btn_try.set_label(TryState::Idle.button_label());
        self.btn_try
            .set_tooltip_text(Some(TryState::Idle.button_tooltip()));
        self.btn_try.set_sensitive(true);
        // Don't clear status here — let the install/availability status show
        // or keep the ended message until next action.
    }

    /// Poll the try receiver for terminal state updates.
    fn poll_try_receiver(&self) {
        // mpsc::Receiver is not Clone; take ownership once for the poll loop.
        let receiver = match self.try_receiver.borrow_mut().take() {
            Some(r) => r,
            None => return,
        };
        let try_state = self.try_state.clone();
        let btn_try = self.btn_try.clone();
        let status = self.status.clone();
        let title = self.title.clone();
        let try_session = self.try_session.clone();
        let try_receiver = self.try_receiver.clone();

        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            // A dropped sender means the monitor is gone: report a plain
            // end rather than leaving the card on "Starting".
            let terminal = match receiver.try_recv() {
                Ok(state) => state,
                Err(mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => TryState::Ended {
                    stopped_by_user: false,
                },
            };
            *try_session.borrow_mut() = None;
            *try_receiver.borrow_mut() = None;
            // A user stop already explained itself; the monitor's plain end
            // would overwrite "Stopped" with "Finished".
            let stopped_by_user = matches!(
                *try_state.borrow(),
                TryState::Ended {
                    stopped_by_user: true
                }
            );
            if !stopped_by_user {
                apply_try_state(terminal, &try_state, &btn_try, &status, &title);
            }
            glib::ControlFlow::Break
        });
    }

    /// After a grace period, move a live Starting session to Running so the
    /// card reads "Running X" once the app is up. The monitor reports only
    /// exit, so this is a defensible grace-period proxy; an actual exit keeps
    /// the session at Started/Ended via the monitor, which overrides.
    fn schedule_running_transition(&self) {
        let try_state = self.try_state.clone();
        let try_session = self.try_session.clone();
        let btn_try = self.btn_try.clone();
        let status = self.status.clone();
        let title = self.title.clone();
        glib::timeout_add_local(std::time::Duration::from_secs(15), move || {
            if *try_state.borrow() != TryState::Starting {
                return glib::ControlFlow::Break;
            }
            let alive = try_session
                .borrow()
                .as_ref()
                .map(|s| s.is_running())
                .unwrap_or(false);
            if !alive {
                return glib::ControlFlow::Break;
            }
            *try_state.borrow_mut() = TryState::Running;
            let app_name = title.label().to_string();
            btn_try.set_label(TryState::Running.button_label());
            btn_try.set_tooltip_text(Some(TryState::Running.button_tooltip()));
            btn_try.set_sensitive(true);
            status.set_text(&TryState::Running.status_text(&app_name));
            glib::ControlFlow::Break
        });
    }
}

const ADDED_TEXT: &str = "Added to your ready-to-change list — review and prepare it in Updates.";

/// Apply a try state to the card widgets.
///
/// Shared by the click handlers and the background poll, so both paths render
/// a state identically.
fn apply_try_state(
    state: TryState,
    try_state: &Rc<RefCell<TryState>>,
    btn_try: &gtk4::Button,
    status: &gtk4::Label,
    title: &gtk4::Label,
) {
    *try_state.borrow_mut() = state;
    btn_try.set_label(state.button_label());
    btn_try.set_tooltip_text(Some(state.button_tooltip()));
    btn_try.set_sensitive(state.is_sensitive());
    let app_name = title.label().to_string();
    status.set_text(&state.status_text(&app_name));
}

fn start_availability_check(
    detail: &DetailUi,
    button: &gtk4::Button,
    nix_path: std::path::PathBuf,
    attr: String,
    allow_unfree: bool,
) {
    // Evaluate off the GTK thread; the main-loop poller updates widgets and
    // rejects the result if navigation has since selected another entry.
    *detail.state.borrow_mut() = InstallState::Checking;
    button.set_sensitive(false);
    button.set_label("Checking…");
    detail.status.set_text("Checking availability…");
    detail.expander.set_visible(false);
    detail.expander.set_expanded(false);

    let state = detail.state.clone();
    let current_attr = detail.current_attr.clone();
    let status = detail.status.clone();
    let technical_detail = detail.tech.clone();
    let expander = detail.expander.clone();
    let button = button.clone();
    let checked_attr = attr.clone();
    crate::ui::async_task::run_on_main(
        std::time::Duration::from_millis(100),
        move || {
            install::check_availability(
                &nix_path,
                &attr,
                allow_unfree,
                std::time::Duration::from_secs(60),
            )
        },
        move |r| {
            if let Ok(result) = r {
                // Ignore results for a card that has since been replaced.
                if !store::result_matches_selection(
                    current_attr.borrow().as_deref().unwrap_or(""),
                    &checked_attr,
                ) {
                    return;
                }
                match result.outcome {
                    install::Outcome::ConfirmedAvailable { note } => {
                        *state.borrow_mut() = InstallState::Available;
                        button.set_sensitive(true);
                        button.set_label("Add to ready-to-change list");
                        status.set_text(if note.is_empty() {
                            "Ready to install for you"
                        } else {
                            note.as_str()
                        });
                    }
                    outcome => {
                        *state.borrow_mut() = InstallState::Unavailable;
                        button.set_sensitive(true);
                        button.set_label("Check again");
                        let message = match outcome {
                            install::Outcome::UnfreeEvalBlocked => {
                                "This app is free to use but carries a licence your system \
                                 refuses, so nothing was installed. Turn on 'Restricted-licence \
                                 software' in Settings -> Software to allow it."
                                    .to_string()
                            }
                            install::Outcome::Unavailable => {
                                "Not available for this system".to_string()
                            }
                            install::Outcome::CheckFailed { reason } => reason,
                            install::Outcome::ConfirmedAvailable { .. } => unreachable!(),
                        };
                        status.set_text(&message);
                        // The raw nix trace stays one click away (collapsed): a
                        // normal user should not be handed stdenv internals.
                        technical_detail.set_text(&result.detail);
                        expander.set_visible(true);
                        expander.set_expanded(false);
                    }
                }
            } else {
                *state.borrow_mut() = InstallState::Unavailable;
                button.set_sensitive(true);
                button.set_label("Check again");
                status.set_text("Could not be checked right now");
            }
        },
    );
}

/// Connect the install button (availability check + desired state) and home-page button.
/// `on_change` runs whenever package intent changes so the shell can
/// refresh its header count. Nothing here mutates the system.
pub fn wire(
    d: &Rc<DetailUi>,
    desired: &Rc<RefCell<DesiredDoc>>,
    account: &str,
    nix: &Option<std::path::PathBuf>,
    home_win: &gtk4::ApplicationWindow,
    on_change: Rc<dyn Fn()>,
) {
    let home_btn = d.home.clone();
    let home_url = d.home_url.clone();
    let home_win = home_win.clone();
    let btn_install = d.btn_install.clone();
    let desired_click = desired.clone();
    let account = account.to_string();
    let scope_detail = d.clone();
    let scope_desired = desired.clone();
    let scope_account = account.clone();
    d.scope.connect_active_notify(move |_| {
        scope_detail.sync_added(&scope_desired.borrow(), &scope_account);
    });
    let nix_click = nix.clone();
    let d_click = d.clone();
    btn_install.connect_clicked(move |btn| {
        let attr = match d_click.current_attr.borrow().clone() {
            Some(a) => a,
            None => return,
        };
        // Snapshot the state so `d.state` is not held borrowed across the
        // match (its temporaries live until the end of the match), which
        // would otherwise re-borrow panics in arms that mutate it (e.g. the
        // Idle/Unavailable arm's start_availability_check).
        let state = *d_click.state.borrow();
        match state {
            InstallState::Checking => {}
            InstallState::Added => {
                let scope = if d_click.scope.is_active() {
                    PackageScope::System
                } else {
                    PackageScope::User
                };
                desired_click
                    .borrow_mut()
                    .remove_package(&account, &attr, scope);
                d_click.sync_added(&desired_click.borrow(), &account);
                on_change();
            }
            InstallState::Available => {
                let scope = if d_click.scope.is_active() {
                    PackageScope::System
                } else {
                    PackageScope::User
                };
                desired_click
                    .borrow_mut()
                    .add_package(&account, attr.clone(), scope);
                d_click.sync_added(&desired_click.borrow(), &account);
                on_change();
            }
            InstallState::Idle | InstallState::Unavailable => {
                let Some(nixp) = nix_click.clone() else {
                    d_click
                        .status
                        .set_text("The package checker is not available on this system");
                    return;
                };
                let allow_unfree = crate::settings::allow_unfree(&desired_click.borrow().settings);
                start_availability_check(&d_click, btn, nixp, attr, allow_unfree);
            }
        }
    });

    home_btn.connect_clicked(move |_| {
        if let Some(u) = home_url.borrow().as_deref() {
            gtk4::show_uri(Some(&home_win), u, gtk4::gdk::CURRENT_TIME);
        }
    });

    // Wire the Try button
    let btn_try = d.btn_try.clone();
    let desired_try = desired.clone();
    let d_try = d.clone();
    btn_try.connect_clicked(move |_| {
        let allow_unfree = crate::settings::allow_unfree(&desired_try.borrow().settings);
        d_try.handle_try_click(allow_unfree);
    });
}
