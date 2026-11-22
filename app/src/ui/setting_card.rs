//! The unified per-setting card control, shared by the Settings page and the
//! Updates upkeep surface so the same setting renders identically everywhere.
//! One card is a title row (name + info icon carrying `details`), the short
//! summary, and a horizontal control row: the control (switch, dropdown or
//! text entry), an inline capitalised `On`/`Off` for booleans, and an
//! always-visible `Reset to default` button.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;

use gtk4::prelude::*;

use crate::config_value::{read_config_value, ConfigValue};
use crate::desired_state::{DesiredDoc, SettingValue};
use crate::settings::{self, Spec, ValueKind};

/// The control a setting renders as.
enum Control {
    Entry(gtk4::Entry),
    Switch(gtk4::Switch),
    /// Product-authored dropdown (settings::ValueKind::Choice).
    Dropdown(gtk4::ComboBoxText),
}

/// A live-value read result, applied to a card once the read lands.
enum LiveValue {
    Bool(bool),
    Str(String),
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

/// One setting card rendered with the unified controls (see module doc).
#[derive(Clone)]
pub struct SettingCard {
    card: gtk4::Box,
    /// The settings map key (== NixOS option path).
    pub key: &'static str,
    control: Control,
    /// Inline status: capitalised On/Off beside boolean switches.
    status: Option<gtk4::Label>,
    /// Live system value for boolean rows (mirror-reality).
    live_bool: Cell<Option<bool>>,
    /// Live system value for entry/dropdown rows.
    live_str: RefCell<Option<String>>,
    /// Suppresses a widget handler during a programmatic prefill, so
    /// reflecting the live value never records a choice.
    syncing: Rc<Cell<bool>>,
    /// 'Reset to default': always visible, greyed when no change is stored.
    reset: gtk4::Button,
}

impl SettingCard {
    /// Build the card widget for `spec`. Call [`wire`] to connect handlers and
    /// [`refresh_live_values`] to preselect the mirror-reality live value.
    pub fn new(spec: &Spec) -> SettingCard {
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

        // One horizontal row: the control, inline status (capitalised On/Off
        // for booleans), and the always-visible Reset button.
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

        SettingCard {
            card,
            key: spec.key,
            control,
            status,
            live_bool: Cell::new(None),
            live_str: RefCell::new(None),
            syncing: Rc::new(Cell::new(false)),
            reset,
        }
    }

    /// The built card widget, for appending into a page.
    pub fn card(&self) -> &gtk4::Box {
        &self.card
    }

    /// Whether this card's control is a boolean switch (drives how its live
    /// value is read).
    pub fn is_boolean(&self) -> bool {
        matches!(&self.control, Control::Switch(_))
    }

    /// Apply a freshly-read live system value: record it, preselect the
    /// effective value when nothing is stored, and refresh status/Reset.
    fn apply_live(&self, desired: &Rc<RefCell<DesiredDoc>>, live: LiveValue) {
        match live {
            LiveValue::Bool(b) => {
                self.live_bool.set(Some(b));
                let choice = match desired.borrow().settings.get(self.key) {
                    Some(SettingValue::Bool(v)) => Some(*v),
                    _ => None,
                };
                if let Control::Switch(sw) = &self.control {
                    self.syncing.set(true);
                    sw.set_active(settings::bool_effective(b, choice));
                    self.syncing.set(false);
                }
                self.reset.set_sensitive(choice.is_some());
                if let Some(st) = &self.status {
                    st.set_text(settings::bool_word(choice.unwrap_or(b)));
                }
            }
            LiveValue::Str(raw) => {
                *self.live_str.borrow_mut() = Some(raw.clone());
                // Mirror-reality: preselect the live value when nothing is
                // stored, under the syncing guard so it is never a choice.
                if !desired.borrow().settings.contains_key(self.key) && !raw.is_empty() {
                    self.syncing.set(true);
                    match &self.control {
                        Control::Entry(entry) => entry.set_text(&raw),
                        Control::Dropdown(combo) => {
                            let _ = combo.set_active_id(Some(raw.as_str()));
                        }
                        Control::Switch(_) => {}
                    }
                    self.syncing.set(false);
                }
            }
        }
    }

    /// Connect the control and Reset handlers for a desired-state + save pair.
    pub fn wire(&self, desired: &Rc<RefCell<DesiredDoc>>, save: Rc<dyn Fn()>) {
        let key = self.key;
        match &self.control {
            Control::Entry(entry) => {
                if let Some(SettingValue::Str(s)) = desired.borrow().settings.get(key).cloned() {
                    entry.set_text(&s);
                    self.reset.set_sensitive(true);
                }
                let syncing = self.syncing.clone();
                let reset = self.reset.clone();
                let desired = desired.clone();
                let save = save.clone();
                let entry = entry.clone();
                entry.connect_changed(move |e| {
                    // A programmatic prefill must not be a stored choice.
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
                // Initial position: a stored choice if any, else the neutral
                // placeholder the live read corrects a moment later.
                let choice_now = match desired.borrow().settings.get(key) {
                    Some(SettingValue::Bool(b)) => Some(*b),
                    _ => None,
                };
                sw.set_active(choice_now.unwrap_or(false));
                self.reset.set_sensitive(choice_now.is_some());
                if let Some(st) = &self.status {
                    st.set_text(settings::bool_word(choice_now.unwrap_or(false)));
                }
                let live_cell = self.live_bool.clone();
                let syncing = self.syncing.clone();
                let reset = self.reset.clone();
                let status = self.status.clone();
                let desired = desired.clone();
                let save = save.clone();
                let sw = sw.clone();
                sw.connect_state_set(move |_, active| {
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
                if let Some(SettingValue::Str(s)) = desired.borrow().settings.get(key).cloned() {
                    combo.set_active_id(Some(&s));
                    self.reset.set_sensitive(true);
                } else {
                    combo.set_active(Some(0));
                }
                let syncing = self.syncing.clone();
                let reset = self.reset.clone();
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
        self.wire_reset_to_default(desired, save);
    }

    /// Wire the always-visible 'Reset to default' button: point the setting
    /// back at the system value (removing the stored choice), then keep it
    /// greyed until a new choice is stored.
    fn wire_reset_to_default(&self, desired: &Rc<RefCell<DesiredDoc>>, save: Rc<dyn Fn()>) {
        let key = self.key;
        let control = self.control.clone();
        let live_bool = self.live_bool.clone();
        let live_str = self.live_str.clone();
        let syncing = self.syncing.clone();
        let status = self.status.clone();
        let reset = self.reset.clone();
        let desired = desired.clone();
        let save = save.clone();
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
}

/// Point a card's control at its "no stored change" state (mirror-reality's
/// reset-to-default): the stored choice has already been removed.
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

/// Read the live config values for a set of cards on background threads and
/// apply them as they land (mirror-reality preselection). No-op when config
/// values cannot be read; non-blocking on the main context.
pub fn refresh_live_values(cards: &[SettingCard], desired: &Rc<RefCell<DesiredDoc>>) {
    if !crate::config_value::is_available() {
        return;
    }
    let (tx, rx) = mpsc::channel::<(String, LiveValue)>();
    for card in cards {
        let key: &'static str = card.key;
        let is_boolean = card.is_boolean();
        let tx = tx.clone();
        std::thread::spawn(move || {
            let result = read_config_value(key);
            let live = if is_boolean {
                LiveValue::Bool(match result {
                    Ok(ConfigValue::Bool(b)) => b,
                    // An unset nixpkgs.config.* subkey means the default (off).
                    _ => false,
                })
            } else {
                LiveValue::Str(match result {
                    Ok(ConfigValue::Str(s)) => s,
                    Ok(ConfigValue::Unparsed(u)) => u,
                    _ => String::new(),
                })
            };
            let _ = tx.send((key.to_string(), live));
        });
    }
    drop(tx);

    let cards = cards.to_vec();
    let desired = desired.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        loop {
            match rx.try_recv() {
                Ok((key, live)) => {
                    if let Some(card) = cards.iter().find(|c| c.key == key) {
                        card.apply_live(&desired, live);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return glib::ControlFlow::Break,
            }
        }
        glib::ControlFlow::Continue
    });
}
