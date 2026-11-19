//! Settings page — the product-authored settings surface (settings-schema.md,
//! D14). First-release workflows (settings::SPECS): device name, network
//! connections, time zone, language/region, printing, power, remote access.
//! Plain-language only; a change stores the value in the desired state (the
//! app owns it) and every value renders as an app-wins `mkOverride` line into
//! the generated config. Out-of-scope desktop chrome stays a pointer.
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::desired_state::{DesiredDoc, SettingValue};
use crate::settings;

enum Control {
    Entry(gtk4::Entry),
    Switch(gtk4::Switch),
}

pub struct SettingsPage {
    pub page: gtk4::Box,
    /// (spec key, widget) in SPECS order.
    rows: Vec<(&'static str, Control)>,
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
            let control = if matches!(
                spec.key,
                "services.openssh.enable"
                    | "services.printing.enable"
                    | "powerManagement.enable"
                    | "networking.networkmanager.enable"
            ) {
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
            rows.push((spec.key, control));
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
        for (key, control) in &self.rows {
            // copy the &'static str so the move-closures aren't borrowing self
            let key: &'static str = key;
            match control {
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
                        // a switch represents an explicit enable; off removes
                        // the app's override so the system default returns
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
}
