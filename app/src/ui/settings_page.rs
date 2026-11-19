//! Settings page — the product-authored settings surface (settings-schema.md,
//! D14). First-release workflows (settings::SPECS): device name, network
//! connections, time zone, language/region, printing, power, remote access.
//! Plain-language only; a change stores the value in the desired state (the
//! app owns it) and every value renders as an app-wins `mkOverride` line into
//! the generated config. Out-of-scope desktop chrome stays a pointer.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use glib::MainContext;
use gtk4::prelude::*;

use crate::config_value::{read_config_value, ConfigValue};
use crate::desired_state::{DesiredDoc, SettingValue};
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

pub struct SettingsPage {
    pub page: gtk4::Box,
    /// Row data in SPECS order.
    rows: Vec<RowData>,
}

impl SettingsPage {
    pub fn new() -> SettingsPage {
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        page.set_margin_top(18);
        page.set_margin_start(28);
        page.set_margin_end(28);

        let heading = gtk4::Label::new(Some("Settings"));
        heading.add_css_class("title-2");
        heading.set_xalign(0.0);
        page.append(&heading);

        let notice = gtk4::Label::new(Some(
            "Settings apply to this computer and always explain what changes \
             before they are applied. Changes are collected and applied through \
             Updates.",
        ));
        notice.set_xalign(0.0);
        notice.set_wrap(true);
        notice.add_css_class("dim-label");
        page.append(&notice);

        let mut rows = Vec::new();
        for spec in settings::SPECS {
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
            page.append(&card);
        }

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
        page.append(&pointer);

        page.set_vexpand(true);
        SettingsPage { page, rows }
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
                    Err(_) => "could not read value".to_string(),
                };
                let _ = tx.send((key.to_string(), text));
            });
        }

        // Drop the original sender so the receiver knows when all senders are done
        drop(tx);

        // Poll the receiver on the main context
        let ctx = MainContext::default();
        let rows = self.rows.clone();
        ctx.spawn_local(async move {
            for (key, text) in rx {
                // Find the row for this key and update its label
                if let Some(row) = rows.iter().find(|r| r.key == key) {
                    row.current_label.set_text(&text);
                    row.current_label.set_visible(true);
                }
            }
        });
    }
}
