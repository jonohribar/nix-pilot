//! Settings page — the product-authored settings surface (settings-schema.md,
//! D14). First-release workflows (settings::SPECS): device name, network
//! connections, time zone, language/region, printing, power, remote access.
//! Plain-language only; a change stores the value in the desired state (the
//! app owns it) and every value renders as an app-wins `mkOverride` line into
//! the generated config. Out-of-scope desktop chrome stays a pointer.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use gtk4::prelude::*;

use crate::config_value::{read_config_value, ConfigValue, ConfigValueError};
use crate::desired_state::{AccountProfile, DesiredDoc, SettingValue};
use crate::settings::{self, ValueKind};

enum Control {
    Entry(gtk4::Entry),
    Switch(gtk4::Switch),
}

impl Clone for Control {
    fn clone(&self) -> Self {
        match self {
            Control::Entry(e) => Control::Entry(e.clone()),
            Control::Switch(s) => Control::Switch(s.clone()),
        }
    }
}

#[derive(Clone)]
struct RowData {
    key: &'static str,
    control: Control,
    current_label: gtk4::Label,
}

/// The app-level action behind the Settings home's "Updates" row
/// (navigates to the Updates surface; wired by the shell after startup).
type UpdatesCallback = Rc<dyn Fn()>;
type UpdatesCallbackCell = Rc<RefCell<Option<UpdatesCallback>>>;

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
    /// Action behind the home "Updates" row (the app-level stack; wired by
    /// the shell after startup).
    updates_cb: UpdatesCallbackCell,
}

impl SettingsPage {
    pub fn new(account: &str) -> SettingsPage {
        // Settings-internal stack: a home (profile + category navigation) and
        // one subpage per category, like the common desktop settings shells.
        let stack = gtk4::Stack::new();
        stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        stack.set_margin_top(18);
        stack.set_margin_start(28);
        stack.set_margin_end(28);
        stack.set_vexpand(true);

        let updates_cb: UpdatesCallbackCell = Rc::new(RefCell::new(None));

        let home = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        let heading = gtk4::Label::new(Some("Settings"));
        heading.add_css_class("title-2");
        heading.set_xalign(0.0);
        home.append(&heading);
        let hint = gtk4::Label::new(Some(
            "Settings apply to this computer and always explain what changes \
             before they are applied.",
        ));
        hint.set_xalign(0.0);
        hint.set_wrap(true);
        hint.add_css_class("dim-label");
        home.append(&hint);

        // Profile header: avatar + name + account, click -> Accounts.
        let profile = gtk4::Button::new();
        profile.set_halign(gtk4::Align::Fill);
        profile.add_css_class("card");
        let avatar = gtk4::Label::new(Some(&initial_of(account)));
        avatar.add_css_class("avatar");
        avatar.set_margin_end(12);
        let name_col = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let name_label = gtk4::Label::new(Some(&full_name_of(account)));
        name_label.add_css_class("heading");
        name_label.set_xalign(0.0);
        let acc_label = gtk4::Label::new(Some(&format!("@{account}")));
        acc_label.add_css_class("dim-label");
        acc_label.set_xalign(0.0);
        name_col.append(&name_label);
        name_col.append(&acc_label);
        let profile_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        profile_row.append(&avatar);
        profile_row.append(&name_col);
        profile.set_child(Some(&profile_row));
        {
            let s = stack.clone();
            profile.connect_clicked(move |_| s.set_visible_child_name("accounts"));
        }
        home.append(&profile);

        // Category navigation (familiar names shared across desktop shells).
        let categories: [(&str, &str); 7] = [
            ("System", "system"),
            ("Network & internet", "network"),
            ("Accounts", "accounts"),
            ("Time & language", "time"),
            ("Personalisation", "appearance"),
            ("Privacy & security", "privacy"),
            ("Updates", "updates"),
        ];
        for (label, child) in categories {
            let btn = gtk4::Button::with_label(label);
            btn.set_halign(gtk4::Align::Fill);
            btn.add_css_class("nav-item");
            let s = stack.clone();
            let child = child.to_string();
            btn.connect_clicked(move |_| s.set_visible_child_name(&child));
            home.append(&btn);
        }
        stack.add_named(&home, Some("home"));

        let mut cat_pages: std::collections::BTreeMap<&'static str, gtk4::Box> =
            std::collections::BTreeMap::new();
        // Categories are (label, child-id) pairs: the page title is the label
        // and the stack/map key is the id, so the nav buttons (which set the
        // visible child by id) and the card appends (which key by spec
        // category id) agree. (Fixed 2026-11-20: the first release bound
        // these swapped and the map was keyed by labels, panicking on the
        // first non-updates spec.)
        for (title, child) in categories {
            let page = category_page(&stack, title);
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
            let label = gtk4::Label::new(Some(spec.label));
            label.add_css_class("heading");
            label.set_xalign(0.0);
            let blurb = gtk4::Label::new(Some(spec.blurb));
            blurb.set_xalign(0.0);
            blurb.set_wrap(true);
            blurb.add_css_class("dim-label");
            card.append(&label);
            card.append(&blurb);

            // bool settings render as a switch; string settings as an entry
            let control = if spec.kind == ValueKind::Boolean {
                let sw = gtk4::Switch::new();
                sw.set_halign(gtk4::Align::Start);
                sw.set_margin_top(4);
                card.append(&sw);
                Control::Switch(sw)
            } else {
                let entry = gtk4::Entry::new();
                entry.set_placeholder_text(Some("(not set — uses your system default)"));
                entry.set_margin_top(4);
                card.append(&entry);
                Control::Entry(entry)
            };

            // Current value label (dim, shown under the control)
            let current_label = gtk4::Label::new(None);
            current_label.set_xalign(0.0);
            current_label.add_css_class("dim-label");
            current_label.set_margin_top(4);
            current_label.set_visible(false);
            card.append(&current_label);

            rows.push(RowData {
                key: spec.key,
                control,
                current_label,
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

        // Updates category: maintenance lives on the Updates page; this row
        // opens it (wired by the shell via wire_updates).
        let updates_card = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        updates_card.add_css_class("card");
        updates_card.set_margin_top(6);
        let u_head = gtk4::Label::new(Some("Automatic updates & maintenance"));
        u_head.add_css_class("heading");
        u_head.set_xalign(0.0);
        let u_body = gtk4::Label::new(Some(
            "Set-and-forget upkeep — automatic updates, garbage collection and \
             store optimisation — lives on the Updates page.",
        ));
        u_body.set_xalign(0.0);
        u_body.set_wrap(true);
        u_body.add_css_class("dim-label");
        let u_go = gtk4::Button::with_label("Open Updates");
        {
            let updates_cb = updates_cb.clone();
            u_go.connect_clicked(move |_| {
                if let Some(open) = updates_cb.borrow().as_ref() {
                    open();
                }
            });
        }
        updates_card.append(&u_head);
        updates_card.append(&u_body);
        updates_card.append(&u_go);
        cat_pages["updates"].append(&updates_card);

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
            updates_cb,
        }
    }

    /// Provide the action behind the home/updates "Open Updates" control
    /// (navigates the app-level stack; called by the shell after startup).
    pub fn wire_updates(&self, open_updates: UpdatesCallback) {
        *self.updates_cb.borrow_mut() = Some(open_updates);
    }

    /// Populate from the desired state and wire edits back through `save`.
    /// A valid change stores/removes the value; an invalid one is flagged on
    /// the control and left out of the desired state.
    pub fn wire(&self, desired: &Rc<RefCell<DesiredDoc>>, save: Rc<dyn Fn()>) {
        for row in &self.rows {
            let key: &'static str = row.key;
            match &row.control {
                Control::Entry(entry) => {
                    let current = desired.borrow().settings.get(key).cloned();
                    if let Some(SettingValue::Str(s)) = current {
                        entry.set_text(&s);
                    }
                    let desired = desired.clone();
                    let save = save.clone();
                    let entry = entry.clone();
                    entry.connect_changed(move |e| {
                        let text = e.text().to_string();
                        if text.is_empty() {
                            e.remove_css_class("error");
                            e.set_tooltip_text(None);
                            desired.borrow_mut().settings.remove(key);
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
                    if let Some(SettingValue::Bool(b)) = desired.borrow().settings.get(key) {
                        sw.set_active(*b);
                    }
                    let desired = desired.clone();
                    let save = save.clone();
                    let sw = sw.clone();
                    sw.connect_state_set(move |_, active| {
                        if active {
                            desired
                                .borrow_mut()
                                .settings
                                .insert(key.to_string(), SettingValue::Bool(true));
                        } else {
                            desired.borrow_mut().settings.remove(key);
                        }
                        save();
                        glib::Propagation::Proceed
                    });
                }
            }
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
        add_btn.connect_clicked(move |_| {
            let name = add_name.text().to_string().trim().to_string();
            let full = add_full.text().to_string();
            if name.is_empty() {
                accounts_status.set_text("Enter an account name first");
                accounts_status.set_visible(true);
                return;
            }
            // The app can own only accounts it creates: refuse names the live
            // system already defines (read-only collision check).
            let already_owned = accounts_desired.borrow().accounts.contains_key(&name);
            if !already_owned && account_exists(&name) {
                accounts_status.set_text(&format!(
                    "\u{201c}{name}\u{201d} already exists on this system — the app cannot own it."
                ));
                accounts_status.set_visible(true);
                return;
            }
            accounts_desired
                .borrow_mut()
                .set_account(&name, AccountProfile::new(full, add_admin.is_active()));
            accounts_save();
            add_name.set_text("");
            add_full.set_text("");
            add_admin.set_active(false);
            accounts_status.set_visible(false);
            refresh_accounts(&accounts_box, &accounts_desired, accounts_save.clone());
        });
    }

    /// Load and display current config values from the live NixOS configuration.
    /// Non-blocking: runs on background threads and updates the UI via the
    /// main context. Only attempts to read if NixOS config is available.
    pub fn load_current_values(&self) {
        // Check availability heuristic (cheap, non-blocking)
        if !crate::config_value::is_available() {
            for row in &self.rows {
                row.current_label
                    .set_text("could not read value (not on NixOS or nix unavailable)");
                row.current_label.set_visible(true);
            }
            return;
        }

        // Create a channel to receive results from background threads
        let (tx, rx) = mpsc::channel::<(String, String)>();

        // Spawn a thread for each option
        for row in &self.rows {
            let key = row.key;
            let tx = tx.clone();
            std::thread::spawn(move || {
                let result = read_config_value(key);
                let text = match result {
                    Ok(ConfigValue::Str(s)) => format!("currently: {}", s),
                    Ok(ConfigValue::Bool(b)) => {
                        format!("currently: {}", if b { "on" } else { "off" })
                    }
                    Ok(ConfigValue::Unparsed(u)) => format!("currently: {}", u),
                    Err(ConfigValueError::NoSuchOption(_)) => {
                        "this setting no longer applies — the option was removed or \
                         renamed in a newer NixOS"
                            .to_string()
                    }
                    Err(_) => "could not read value".to_string(),
                };
                let _ = tx.send((key.to_string(), text));
            });
        }

        // Drop the original sender so the receiver knows when all senders are done
        drop(tx);

        // Poll the receiver on the main context instead of blocking on it: an
        // async `for (_,_) in rx` runs on the GTK thread and freezes the UI
        // until every nix-instantiate thread finishes. This fill-in poll is
        // non-blocking and shows each value as its read lands.
        let rows = self.rows.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            loop {
                match rx.try_recv() {
                    Ok((key, text)) => {
                        if let Some(row) = rows.iter().find(|r| r.key == key) {
                            row.current_label.set_text(&text);
                            row.current_label.set_visible(true);
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

/// A settings category subpage: back button + title; returned as the Box the
/// category's cards are appended to.
fn category_page(stack: &gtk4::Stack, title: &str) -> gtk4::Box {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    page.set_vexpand(true);
    let back = gtk4::Button::with_label("‹ Settings");
    let s = stack.clone();
    back.connect_clicked(move |_| s.set_visible_child_name("home"));
    page.append(&back);
    let heading = gtk4::Label::new(Some(title));
    heading.add_css_class("title-2");
    heading.set_xalign(0.0);
    page.append(&heading);
    page
}

/// Best-effort display name of the OS account: getent passwd field 5, else
/// the username. Read-only display; never used for decisions.
fn full_name_of(account: &str) -> String {
    if let Ok(out) = std::process::Command::new("getent")
        .args(["passwd", account])
        .output()
    {
        if out.status.success() {
            let line = String::from_utf8_lossy(&out.stdout);
            if let Some(gecos) = line.split(':').nth(4) {
                let gecos = gecos.trim();
                if !gecos.is_empty() {
                    return gecos.to_string();
                }
            }
        }
    }
    account.to_string()
}

/// One-letter avatar: first character of the account name, uppercased.
fn initial_of(account: &str) -> String {
    account
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// Whether an account name already exists on this system (read-only collision
/// check). The app can own only accounts it creates: a name the live system
/// already defines cannot be cleanly owned (settings-schema.md design).
fn account_exists(name: &str) -> bool {
    std::process::Command::new("id")
        .arg("-u")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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
