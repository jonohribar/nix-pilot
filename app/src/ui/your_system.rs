//! "Your system" surface (9f3.14): a plain view of what the app is asked to
//! install and set — the desired-state recipe — with remove/clear actions.
//! Read-only in the sense that it never invents content: every row comes from
//! the persisted DesiredDoc (system apps, this account's apps, owned
//! settings). Removing something = editing the desired state and persisting,
//! exactly like the Store's remove; the queue is kept in sync so the Updates
//! transaction matches what the user sees here.
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::desired_state::DesiredDoc;
use crate::settings;

pub struct YourSystemPage {
    pub page: gtk4::Box,
    list: gtk4::ListBox,
    account: String,
}

impl YourSystemPage {
    pub fn new(account: &str) -> YourSystemPage {
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        page.set_margin_top(18);
        page.set_margin_start(28);
        page.set_margin_end(28);

        let heading = gtk4::Label::new(Some("Your system"));
        heading.add_css_class("title-2");
        heading.set_xalign(0.0);
        page.append(&heading);

        let blurb = gtk4::Label::new(Some(
            "The apps to install and the settings to apply, in plain words. \
             Nothing here changes this computer until you prepare and apply \
             your changes on the Updates page.",
        ));
        blurb.set_xalign(0.0);
        blurb.set_wrap(true);
        blurb.add_css_class("dim-label");
        page.append(&blurb);

        let list = gtk4::ListBox::new();
        page.append(&list);

        YourSystemPage {
            page,
            list,
            account: account.to_string(),
        }
    }

    /// Rebuild the list from the current desired document. The queue stays
    /// in sync via `on_change` (the same hook the Store uses).
    pub fn refresh(
        &self,
        desired: &Rc<RefCell<DesiredDoc>>,
        queue: &Rc<RefCell<crate::store::ChangeQueue>>,
        on_change: &Rc<dyn Fn()>,
    ) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let doc = desired.borrow().clone();
        let mut rows: Vec<(String, String, String)> = Vec::new(); // (attr, label, scope)

        // System-wide apps.
        for attr in doc.system_packages() {
            rows.push((attr.clone(), attr.clone(), "system".to_string()));
        }
        // This account's apps.
        for attr in doc.packages_for(&self.account) {
            rows.push((
                attr.clone(),
                attr.clone(),
                format!("{} (this account)", self.account),
            ));
        }
        // Owned settings (SPECS order, labelled plainly; unknown keys shown raw).
        for spec in settings::SPECS {
            if doc.settings.contains_key(spec.key) {
                rows.push((
                    spec.key.to_string(),
                    spec.label.to_string(),
                    "setting".to_string(),
                ));
            }
        }
        if rows.is_empty() {
            let l = gtk4::Label::new(Some(
                "Nothing chosen yet — add apps from the store, or change a \
                 setting in Settings.",
            ));
            l.set_xalign(0.0);
            l.add_css_class("dim-label");
            self.list.append(&l);
            return;
        }
        for (attr, label, scope) in rows {
            let row = gtk4::ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let boxed = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            let label_w = gtk4::Label::new(Some(&label));
            label_w.set_xalign(0.0);
            label_w.set_hexpand(true);
            label_w.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            let scope_w = gtk4::Label::new(Some(&scope));
            scope_w.add_css_class("dim-label");
            let remove = gtk4::Button::with_label("Remove");
            remove.add_css_class("flat");
            remove.add_css_class("destructive-action");
            remove.set_tooltip_text(Some(&format!(
                "Stop asking for “{label}”. It stays on this computer until a \
                 future clean-up; nothing is removed until you apply a change."
            )));
            boxed.append(&label_w);
            boxed.append(&scope_w);
            boxed.append(&remove);
            row.set_child(Some(&boxed));
            self.list.append(&row);

            let desired = desired.clone();
            let queue = queue.clone();
            let on_change = on_change.clone();
            let account = self.account.clone();
            let attr = attr.clone();
            let row_widget = row.clone();
            let list = self.list.clone();
            remove.connect_clicked(move |_| {
                // Clear from the desired state: an app reverts to not-asked,
                // a setting reverts to the system default (the option is
                // simply no longer emitted). Re-borrow fresh so the check and
                // the edit can never disagree.
                let is_system = desired.borrow().system_packages().contains(&attr);
                let is_account = desired.borrow().packages_for(&account).contains(&attr);
                if is_system {
                    let pkgs: Vec<String> = desired
                        .borrow()
                        .system_packages()
                        .iter()
                        .filter(|a| *a != &attr)
                        .cloned()
                        .collect();
                    desired.borrow_mut().set_system(pkgs);
                } else if is_account {
                    let pkgs: Vec<String> = desired
                        .borrow()
                        .packages_for(&account)
                        .iter()
                        .filter(|a| *a != &attr)
                        .cloned()
                        .collect();
                    desired.borrow_mut().set_user(&account, pkgs);
                } else {
                    desired.borrow_mut().settings.remove(&attr);
                }
                queue.borrow_mut().remove(&attr);
                on_change();
                list.remove(&row_widget);
            });
        }
    }
}
