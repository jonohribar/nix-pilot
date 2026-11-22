//! Settings page — the product-authored settings surface (settings-schema.md, D14).
//! First-release workflows (settings::SPECS): device name, network
//! connections, time zone, language/region, printing, power, remote access.
//! Plain-language only; a change stores the value in the desired state (the
//! app owns it) and every value renders as an app-wins `mkOverride` line into
//! the generated config. Out-of-scope desktop chrome stays a pointer.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use gtk4::prelude::*;

use crate::account::{self, AccountPresence};
use crate::config_value::{read_config_value, ConfigValue};
use crate::desired_state::{AccountProfile, DesiredDoc, SettingValue};
use crate::settings::{self, ValueKind};

enum Control {
    Entry(gtk4::Entry),
    Switch(gtk4::Switch),
    /// Product-authored dropdown (settings::ValueKind::Choice).
    Dropdown(gtk4::ComboBoxText),
}

impl Clone for Control {
    fn clone(&self) -> Self {
        match self {
            Control::Entry(e) => Control::Entry(e.clone()),
            Control::Switch(s) => Control::Switch(s.clone()),
            Control::Dropdown(c) => Control::Dropdown(c.clone()),
        }
    }
}

/// A live-value read result: a boolean (switch rows) or a raw text value
/// (entry/dropdown rows), filled as the read lands.
enum RowLive {
    Bool(bool),
    Str(String),
}

#[derive(Clone)]
struct RowData {
    key: &'static str,
    control: Control,
    /// Inline status: capitalised On/Off beside boolean switches.
    status: Option<gtk4::Label>,
    /// Live system value for boolean rows (mirror-reality).
    live_bool: std::cell::Cell<Option<bool>>,
    /// Live system value for entry/dropdown rows.
    live_str: std::cell::RefCell<Option<String>>,
    /// Suppresses a widget handler during a programmatic prefill, so
    /// reflecting the live value never re-persists a choice.
    syncing: std::rc::Rc<std::cell::Cell<bool>>,
    /// Per-setting 'Reset to default': always visible, greyed when no change
    /// is stored.
    reset: gtk4::Button,
}

/// Restore a row's control to its "no stored change" state (mirror-reality's
/// reset-to-default): the stored choice has already been removed; this points
/// the widget at the effective value without emitting a change that would
/// re-persist a choice.
fn restore_live(control: &Control, live_bool: bool, live_str: Option<&str>) {
    match control {
        Control::Switch(sw) => sw.set_active(live_bool),
        // The dropdown's first seat is the explicit "use the system default".
        Control::Dropdown(combo) => {
            let _ = live_str;
            combo.set_active(Some(0));
        }
        Control::Entry(entry) => {
            let _ = live_bool;
            entry.set_text(live_str.unwrap_or(""));
        }
    }
}

/// Wire the per-setting 'Reset to default' button: point the setting back at
/// the system value (removing the stored choice), then keep it greyed until a
/// new choice is stored. Always visible so the control never pops in and out.
fn wire_reset_to_default(row: &RowData, desired: &Rc<RefCell<DesiredDoc>>, save: Rc<dyn Fn()>) {
    let key = row.key;
    let control = row.control.clone();
    let live_bool = row.live_bool.clone();
    let live_str = row.live_str.clone();
    let syncing = row.syncing.clone();
    let status = row.status.clone();
    let reset = row.reset.clone();
    let desired = desired.clone();
    let button = reset.clone();
    button.connect_clicked(move |_| {
        if syncing.get() {
            return;
        }
        desired.borrow_mut().settings.remove(key);
        syncing.set(true);
        restore_live(
            &control,
            live_bool.get().unwrap_or(false),
            live_str.borrow().as_deref(),
        );
        syncing.set(false);
        if let Some(st) = &status {
            let on = match desired.borrow().settings.get(key) {
                Some(SettingValue::Bool(b)) => *b,
                _ => live_bool.get().unwrap_or(false),
            };
            st.set_text(settings::bool_word(on));
        }
        reset.set_sensitive(false);
        save();
    });
}

/// Settings surface containing the profile, category navigation, and controls.
pub struct SettingsPage {
    /// The settings-internal stack: home + one subpage per category.
    pub page: gtk4::Stack,
    /// Row data in SPECS order.
    rows: Vec<RowData>,
    /// The system account the per-user workflows operate on.
    account: String,
    /// Buffer backing the SSH authorized-keys editor.
    keys_buffer: gtk4::TextBuffer,
    /// Accounts list container (app-owned user accounts).
    accounts_box: gtk4::Box,
    add_name: gtk4::Entry,
    add_full: gtk4::Entry,
    add_admin: gtk4::Switch,
    add_btn: gtk4::Button,
    accounts_status: gtk4::Label,
}

impl SettingsPage {
    /// Build the settings surface for the account whose user-scoped data it edits.
    pub fn new(account: &str) -> SettingsPage {
        // Settings is a stack of one page per category, selected from the
        // sidebar's two-column router (the category list lives there, not in
        // a drill-in home page).
        let stack = gtk4::Stack::new();
        stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        stack.set_margin_top(18);
        stack.set_margin_start(28);
        stack.set_margin_end(28);
        stack.set_vexpand(true);

        let mut cat_pages: std::collections::BTreeMap<&'static str, gtk4::Box> =
            std::collections::BTreeMap::new();
        // Categories are (label, summary, child-id) triples: the page title
        // is the label and the stack/map key is the id, so the sidebar buttons
        // (which set the visible child by id) and the card appends (which
        // key by spec category id) agree.
        for (title, _summary, child) in settings::SETTINGS_CATEGORIES {
            let page = category_page(title);
            stack.add_named(&page, Some(child));
            cat_pages.insert(child, page);
        }

        let mut rows = Vec::new();
        // The upkeep settings (category "updates") render on the Updates page,
        // not here; this surface builds only its own categories.
        for spec in settings::SPECS.iter().filter(|s| s.category != "updates") {
            let card = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
            card.add_css_class("card");
            card.set_margin_top(6);
            card.set_margin_start(4);
            card.set_margin_end(4);

            // Title row: heading + info icon carrying the detailed description.
            let title_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
            let label = gtk4::Label::new(Some(spec.label));
            label.add_css_class("heading");
            label.set_halign(gtk4::Align::Start);
            title_row.append(&label);
            let info = gtk4::Label::new(Some("ⓘ"));
            info.add_css_class("dim-label");
            info.set_halign(gtk4::Align::Start);
            info.set_valign(gtk4::Align::Start);
            info.set_tooltip_text(Some(spec.details));
            title_row.append(&info);
            card.append(&title_row);

            // Short summary, above the control.
            let blurb = gtk4::Label::new(Some(spec.blurb));
            blurb.set_xalign(0.0);
            blurb.set_wrap(true);
            blurb.add_css_class("dim-label");
            card.append(&blurb);

            // One horizontal row: the control, inline status (capitalised
            // On/Off for booleans), and the always-visible Reset button.
            let control_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
            control_row.set_margin_top(6);

            let mut status: Option<gtk4::Label> = None;
            let reset = gtk4::Button::with_label("Reset to default");
            reset.set_valign(gtk4::Align::Center);
            reset.set_sensitive(false);

            let control = match spec.kind {
                ValueKind::Boolean => {
                    let sw = gtk4::Switch::new();
                    sw.set_valign(gtk4::Align::Center);
                    control_row.append(&sw);
                    let st = gtk4::Label::new(Some("Off"));
                    st.set_valign(gtk4::Align::Center);
                    st.add_css_class("dim-label");
                    st.set_tooltip_text(Some("the value that will apply"));
                    control_row.append(&st);
                    status = Some(st);
                    Control::Switch(sw)
                }
                ValueKind::Choice(options) => {
                    let combo = gtk4::ComboBoxText::new();
                    combo.append(None, "(use the system default)");
                    for (value, plain) in options {
                        combo.append(Some(value), plain);
                    }
                    combo.set_active(Some(0));
                    combo.set_hexpand(true);
                    control_row.append(&combo);
                    Control::Dropdown(combo)
                }
                ValueKind::String => {
                    let entry = gtk4::Entry::new();
                    entry.set_placeholder_text(Some("(empty = use the system default)"));
                    entry.set_hexpand(true);
                    control_row.append(&entry);
                    Control::Entry(entry)
                }
            };
            control_row.append(&reset);
            card.append(&control_row);

            rows.push(RowData {
                key: spec.key,
                control,
                status,
                live_bool: std::cell::Cell::new(None),
                live_str: std::cell::RefCell::new(None),
                syncing: std::rc::Rc::new(std::cell::Cell::new(false)),
                reset,
            });
            cat_pages[spec.category].append(&card);
        }

        let keys_card = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        keys_card.add_css_class("card");
        keys_card.set_margin_top(6);
        let keys_label = gtk4::Label::new(Some("Remote access keys (SSH)"));
        keys_label.add_css_class("heading");
        keys_label.set_xalign(0.0);
        let keys_blurb = gtk4::Label::new(Some(
            "The public keys allowed to log into this computer remotely, one per \
             line. Removing a key stops that computer from connecting.",
        ));
        keys_blurb.set_xalign(0.0);
        keys_blurb.set_wrap(true);
        keys_blurb.add_css_class("dim-label");
        let buffer = gtk4::TextBuffer::new(None);
        let view = gtk4::TextView::with_buffer(&buffer);
        view.set_wrap_mode(gtk4::WrapMode::WordChar);
        view.set_height_request(96);
        view.set_margin_top(4);
        view.add_css_class("card");
        keys_card.append(&keys_label);
        keys_card.append(&keys_blurb);
        keys_card.append(&view);
        cat_pages["accounts"].append(&keys_card);

        // App-owned user accounts (user-accounts workflow). The app owns only
        // the accounts it creates; system accounts are never edited here.
        let accounts_card = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        accounts_card.add_css_class("card");
        accounts_card.set_margin_top(6);
        let accounts_label = gtk4::Label::new(Some("User accounts"));
        accounts_label.add_css_class("heading");
        accounts_label.set_xalign(0.0);
        let accounts_blurb = gtk4::Label::new(Some(
            "Accounts you create here. Accounts set up by the system stay as they are — \
             this app never changes those.",
        ));
        accounts_blurb.set_xalign(0.0);
        accounts_blurb.set_wrap(true);
        accounts_blurb.add_css_class("dim-label");
        let accounts_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        accounts_box.set_margin_top(6);

        let add_name = gtk4::Entry::new();
        add_name.set_placeholder_text(Some("account name"));
        add_name.set_hexpand(true);
        let add_full = gtk4::Entry::new();
        add_full.set_placeholder_text(Some("full name"));
        add_full.set_hexpand(true);
        let add_admin = gtk4::Switch::new();
        add_admin.set_halign(gtk4::Align::Start);
        add_admin.set_tooltip_text(Some("administrator rights"));
        let add_btn = gtk4::Button::with_label("Add account");
        let add_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        add_row.append(&add_name);
        add_row.append(&add_full);
        add_row.append(&add_admin);
        add_row.append(&add_btn);
        let accounts_status = gtk4::Label::new(None);
        accounts_status.set_xalign(0.0);
        accounts_status.set_wrap(true);
        accounts_status.add_css_class("dim-label");
        accounts_status.set_visible(false);
        accounts_card.append(&accounts_label);
        accounts_card.append(&accounts_blurb);
        accounts_card.append(&accounts_box);
        accounts_card.append(&add_row);
        accounts_card.append(&accounts_status);
        cat_pages["accounts"].append(&accounts_card);

        // D14 pointer: desktop chrome is owned by the desktop, never the app
        let pointer = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        pointer.add_css_class("card");
        pointer.set_margin_top(10);
        let p_head = gtk4::Label::new(Some("Appearance"));
        p_head.add_css_class("heading");
        p_head.set_xalign(0.0);
        let p_body = gtk4::Label::new(Some(
            "Wallpaper, themes and how the desktop looks live in your desktop's \
             own settings app — this app does not manage them.",
        ));
        p_body.set_xalign(0.0);
        p_body.set_wrap(true);
        p_body.add_css_class("dim-label");
        pointer.append(&p_head);
        pointer.append(&p_body);
        cat_pages["appearance"].append(&pointer);

        SettingsPage {
            page: stack,
            rows,
            account: account.to_string(),
            keys_buffer: buffer,
            accounts_box,
            add_name,
            add_full,
            add_admin,
            add_btn,
            accounts_status,
        }
    }

    /// Show a category subpage, for example after selecting a global-search result.
    pub fn show_category(&self, child_id: &str) {
        self.page.set_visible_child_name(child_id);
    }

    /// Load controls from desired state and persist valid user edits through `save`.
    ///
    /// Invalid setting values remain out of desired state and are marked on
    /// their controls. Account and SSH-key edits are wired here as well.
    pub fn wire(&self, desired: &Rc<RefCell<DesiredDoc>>, save: Rc<dyn Fn()>) {
        for row in &self.rows {
            let key: &'static str = row.key;
            match &row.control {
                Control::Entry(entry) => {
                    if let Some(SettingValue::Str(s)) = desired.borrow().settings.get(key).cloned()
                    {
                        entry.set_text(&s);
                        row.reset.set_sensitive(true);
                    }
                    let syncing = row.syncing.clone();
                    let reset = row.reset.clone();
                    let desired = desired.clone();
                    let save = save.clone();
                    let entry = entry.clone();
                    entry.connect_changed(move |e| {
                        // A programmatic prefill (restoring the live value)
                        // must not be recorded as the user's choice.
                        if syncing.get() {
                            return;
                        }
                        let text = e.text().to_string();
                        if text.is_empty() {
                            e.remove_css_class("error");
                            e.set_tooltip_text(None);
                            desired.borrow_mut().settings.remove(key);
                            reset.set_sensitive(false);
                            save();
                            return;
                        }
                        match settings::validate(key, &SettingValue::Str(text.clone())) {
                            Ok(()) => {
                                e.remove_css_class("error");
                                e.set_tooltip_text(None);
                                desired
                                    .borrow_mut()
                                    .settings
                                    .insert(key.to_string(), SettingValue::Str(text));
                                reset.set_sensitive(true);
                                save();
                            }
                            Err(reason) => {
                                e.add_css_class("error");
                                e.set_tooltip_text(Some(&reason));
                            }
                        }
                    });
                }
                Control::Switch(sw) => {
                    // Initial position: a stored choice if any, else a neutral
                    // placeholder the live read corrects a moment later
                    // (mirror-reality: the switch shows what the value is).
                    let choice_now = match desired.borrow().settings.get(key) {
                        Some(SettingValue::Bool(b)) => Some(*b),
                        _ => None,
                    };
                    sw.set_active(choice_now.unwrap_or(false));
                    row.reset.set_sensitive(choice_now.is_some());
                    if let Some(st) = &row.status {
                        st.set_text(settings::bool_word(choice_now.unwrap_or(false)));
                    }
                    let live_cell = row.live_bool.clone();
                    let syncing = row.syncing.clone();
                    let reset = row.reset.clone();
                    let status = row.status.clone();
                    let desired = desired.clone();
                    let save = save.clone();
                    let sw = sw.clone();
                    sw.connect_state_set(move |_, active| {
                        // A programmatic set_active (reflecting the live value)
                        // must not persist a choice.
                        if syncing.get() {
                            return glib::Propagation::Proceed;
                        }
                        let live = live_cell.get().unwrap_or(active);
                        let choice = settings::apply_bool_toggle(live, active);
                        match choice {
                            Some(b) => {
                                desired
                                    .borrow_mut()
                                    .settings
                                    .insert(key.to_string(), SettingValue::Bool(b));
                            }
                            None => {
                                desired.borrow_mut().settings.remove(key);
                            }
                        }
                        reset.set_sensitive(choice.is_some());
                        if let Some(st) = &status {
                            st.set_text(settings::bool_word(choice.unwrap_or(live)));
                        }
                        save();
                        glib::Propagation::Proceed
                    });
                }
                Control::Dropdown(combo) => {
                    // Load current choice; empty/absent = system default.
                    let current = desired.borrow().settings.get(key).cloned();
                    if let Some(SettingValue::Str(s)) = current {
                        combo.set_active_id(Some(&s));
                        row.reset.set_sensitive(true);
                    } else {
                        combo.set_active(Some(0));
                    }
                    let syncing = row.syncing.clone();
                    let reset = row.reset.clone();
                    let desired = desired.clone();
                    let save = save.clone();
                    combo.connect_changed(move |c| {
                        if syncing.get() {
                            return;
                        }
                        let chosen = c.active_id().map(|g| g.to_string());
                        match chosen {
                            Some(v) if !v.is_empty() => {
                                desired
                                    .borrow_mut()
                                    .settings
                                    .insert(key.to_string(), SettingValue::Str(v));
                                reset.set_sensitive(true);
                            }
                            _ => {
                                desired.borrow_mut().settings.remove(key);
                                reset.set_sensitive(false);
                            }
                        }
                        save();
                    });
                }
            }
            // Always-visible Reset-to-default, greyed until a change is stored.
            wire_reset_to_default(row, desired, save.clone());
        }

        // SSH authorized keys: debounced save of the multi-line editor.
        let account = self.account.clone();
        let buffer = self.keys_buffer.clone();
        let current = desired.borrow().authorized_keys_for(&account).join("\n");
        buffer.set_text(&current);
        let desired = desired.clone();
        let save = save.clone();
        let accounts_desired = desired.clone();
        let accounts_save = save.clone();
        let debounce = std::rc::Rc::new(std::cell::Cell::new(None::<glib::SourceId>));
        buffer.connect_changed(move |buffer| {
            // Re-arm the debounce: a fresh timeout replaces the pending one.
            if let Some(id) = debounce.take() {
                id.remove();
            }
            let buffer = buffer.clone();
            let desired = desired.clone();
            let save = save.clone();
            let account = account.clone();
            let pending = debounce.clone();
            pending.set(Some(glib::timeout_add_local(
                std::time::Duration::from_millis(800),
                move || {
                    let (start, end) = buffer.bounds();
                    let keys: Vec<String> = buffer
                        .text(&start, &end, true)
                        .to_string()
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect();
                    desired.borrow_mut().set_authorized_keys(&account, keys);
                    save();
                    glib::ControlFlow::Break
                },
            )));
        });

        // App-owned accounts: list + add (collision-refusing) + confirmed remove.
        let accounts_box = self.accounts_box.clone();
        let accounts_status = self.accounts_status.clone();
        let add_name = self.add_name.clone();
        let add_full = self.add_full.clone();
        let add_admin = self.add_admin.clone();
        let add_btn = self.add_btn.clone();
        refresh_accounts(&accounts_box, &accounts_desired, accounts_save.clone());
        let finish_add: Rc<dyn Fn(String, String, bool)> = {
            let accounts_box = accounts_box.clone();
            let accounts_desired = accounts_desired.clone();
            let accounts_save = accounts_save.clone();
            let accounts_status = accounts_status.clone();
            let add_name = add_name.clone();
            let add_full = add_full.clone();
            let add_admin = add_admin.clone();
            Rc::new(move |name, full, admin| {
                accounts_desired
                    .borrow_mut()
                    .set_account(&name, AccountProfile::new(full, admin));
                accounts_save();
                add_name.set_text("");
                add_full.set_text("");
                add_admin.set_active(false);
                accounts_status.set_visible(false);
                refresh_accounts(&accounts_box, &accounts_desired, accounts_save.clone());
            })
        };
        let add_btn_for_click = add_btn.clone();
        add_btn.connect_clicked(move |_| {
            let name = add_name.text().to_string().trim().to_string();
            if name.is_empty() {
                accounts_status.set_text("Enter an account name first");
                accounts_status.set_visible(true);
                return;
            }
            let full = add_full.text().to_string();
            let admin = add_admin.is_active();
            // Existing app-owned accounts may be edited without a live lookup.
            let already_owned = accounts_desired.borrow().accounts.contains_key(&name);
            let finish_add = finish_add.clone();
            if already_owned {
                finish_add(name, full, admin);
                return;
            }

            // Account-directory queries run off the GTK thread. A failed
            // lookup blocks creation rather than assuming the name is free.
            add_btn_for_click.set_sensitive(false);
            accounts_status.set_text("Checking whether this account name is available…");
            accounts_status.set_visible(true);
            let checked_name = name.clone();
            let (sender, receiver) = mpsc::channel();
            std::thread::spawn(move || {
                let result = account::check_presence(&checked_name);
                let _ = sender.send((checked_name, result));
            });

            let add_btn = add_btn_for_click.clone();
            let add_name = add_name.clone();
            let add_full = add_full.clone();
            let add_admin = add_admin.clone();
            let accounts_status = accounts_status.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                match receiver.try_recv() {
                    Ok((checked_name, presence)) => {
                        add_btn.set_sensitive(true);
                        if add_name.text().trim() != checked_name {
                            accounts_status.set_text("The account name changed; check it again.");
                            return glib::ControlFlow::Break;
                        }
                        match presence {
                            AccountPresence::Exists => accounts_status.set_text(&format!(
                                "\u{201c}{checked_name}\u{201d} already exists on this system — the app cannot own it."
                            )),
                            AccountPresence::NotFound => {
                                finish_add(
                                    checked_name,
                                    add_full.text().to_string(),
                                    add_admin.is_active(),
                                )
                            }
                            AccountPresence::QueryFailed => accounts_status.set_text(
                                "Could not check this account name. Try again before adding it.",
                            ),
                        }
                        glib::ControlFlow::Break
                    }
                    Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        add_btn.set_sensitive(true);
                        accounts_status.set_text(
                            "Could not check this account name. Try again before adding it.",
                        );
                        glib::ControlFlow::Break
                    }
                }
            });
        });
    }

    /// Load and display current config values from the live NixOS configuration.
    /// Non-blocking: runs on background threads and updates the UI via the
    /// main context. Only attempts to read if NixOS config is available.
    pub fn load_current_values(&self, desired: &Rc<RefCell<DesiredDoc>>) {
        // Check availability heuristic (cheap, non-blocking); rows stay at
        // their placeholders when config values cannot be read.
        if !crate::config_value::is_available() {
            return;
        }

        // Create a channel to receive results from background threads
        let (tx, rx) = mpsc::channel::<(String, RowLive)>();

        // Spawn a thread per option: boolean rows carry the live bool, text
        // and choice rows carry the raw live value for prefill.
        for row in &self.rows {
            let key = row.key;
            let is_switch = matches!(&row.control, Control::Switch(_));
            let tx = tx.clone();
            std::thread::spawn(move || {
                let result = read_config_value(key);
                let message = if is_switch {
                    match result {
                        Ok(ConfigValue::Bool(b)) => RowLive::Bool(b),
                        // An unset nixpkgs.config.* subkey means the boolean
                        // default (off) applies.
                        _ => RowLive::Bool(false),
                    }
                } else {
                    RowLive::Str(match result {
                        Ok(ConfigValue::Str(s)) => s,
                        Ok(ConfigValue::Unparsed(u)) => u,
                        _ => String::new(),
                    })
                };
                let _ = tx.send((key.to_string(), message));
            });
        }

        // Drop the original sender so the receiver knows when all senders are done
        drop(tx);

        // Poll the receiver on the main context instead of blocking on it: an
        // async `for (_,_) in rx` runs on the GTK thread and freezes the UI
        // until every nix-instantiate thread finishes. This fill-in poll is
        // non-blocking and shows each value as its read lands.
        let rows = self.rows.clone();
        let desired = desired.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            loop {
                match rx.try_recv() {
                    Ok((key, message)) => {
                        if let Some(row) = rows.iter().find(|r| r.key == key) {
                            match message {
                                RowLive::Str(raw) => {
                                    *row.live_str.borrow_mut() = Some(raw.clone());
                                    // Mirror-reality: an unset row shows the
                                    // live value, prefils under the syncing
                                    // guard so it is never recorded as a choice.
                                    if !desired.borrow().settings.contains_key(row.key)
                                        && !raw.is_empty()
                                    {
                                        row.syncing.set(true);
                                        match &row.control {
                                            Control::Entry(entry) => entry.set_text(&raw),
                                            Control::Dropdown(combo) => {
                                                let _ = combo.set_active_id(Some(raw.as_str()));
                                            }
                                            Control::Switch(_) => {}
                                        }
                                        row.syncing.set(false);
                                    }
                                }
                                RowLive::Bool(live) => render_bool_row(row, &desired, live),
                            }
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    // The channel closes only when every producer thread has
                    // finished (all sender clones dropped), so stop polling —
                    // every read result has arrived by now.
                    Err(mpsc::TryRecvError::Disconnected) => return glib::ControlFlow::Break,
                }
            }
            glib::ControlFlow::Continue
        });
    }
}

/// Apply a freshly-read live system value to a boolean setting row: record it,
/// point the switch at the effective value (stored choice when present, else
/// the system value), and update the capitalised status label and Reset.
fn render_bool_row(row: &RowData, desired: &Rc<RefCell<DesiredDoc>>, live: bool) {
    row.live_bool.set(Some(live));
    let choice = match desired.borrow().settings.get(row.key) {
        Some(SettingValue::Bool(b)) => Some(*b),
        _ => None,
    };
    if let Control::Switch(sw) = &row.control {
        row.syncing.set(true);
        sw.set_active(settings::bool_effective(live, choice));
        row.syncing.set(false);
    }
    row.reset.set_sensitive(choice.is_some());
    if let Some(st) = &row.status {
        st.set_text(settings::bool_word(choice.unwrap_or(live)));
    }
}

/// A settings category subpage: back button + title; returned as the Box the
/// category's cards are appended to.
fn category_page(title: &str) -> gtk4::Box {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    page.set_vexpand(true);
    let heading = gtk4::Label::new(Some(title));
    heading.add_css_class("title-2");
    heading.set_xalign(0.0);
    page.append(&heading);
    page
}

/// Re-render the app-owned accounts list (name · full name · administrator),
/// each row with a two-step confirmed Remove.
fn refresh_accounts(container: &gtk4::Box, desired: &Rc<RefCell<DesiredDoc>>, save: Rc<dyn Fn()>) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    for (name, profile) in desired.borrow().accounts() {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        let mut text = name.to_string();
        if !profile.full_name.is_empty() {
            text.push_str(&format!(" — {}", profile.full_name));
        }
        if profile.admin {
            text.push_str(" (administrator)");
        }
        let label = gtk4::Label::new(Some(&text));
        label.set_hexpand(true);
        label.set_xalign(0.0);
        let remove = gtk4::Button::with_label("Remove");
        remove.add_css_class("destructive-action");
        let confirmed = Rc::new(std::cell::Cell::new(false));
        let confirmed2 = confirmed.clone();
        let name2 = name.to_string();
        let cap_container = container.clone();
        let cap_desired = desired.clone();
        let cap_save = save.clone();
        remove.connect_clicked(move |btn| {
            if !confirmed2.replace(true) {
                // First click: ask in plain words (REQ-3). Resets after 5 s.
                btn.set_label("Confirm removal");
                btn.set_tooltip_text(Some(
                    "Removing stops this account from logging in on the next update; \
                     this app cannot undo a removal.",
                ));
                let btn = btn.clone();
                let confirmed = confirmed2.clone();
                glib::timeout_add_local(std::time::Duration::from_secs(5), move || {
                    btn.set_label("Remove");
                    confirmed.set(false);
                    glib::ControlFlow::Break
                });
                return;
            }
            cap_desired.borrow_mut().remove_account(&name2);
            cap_save();
            confirmed.set(false);
            refresh_accounts(&cap_container, &cap_desired, cap_save.clone());
        });
        row.append(&label);
        row.append(&remove);
        container.append(&row);
    }
}
