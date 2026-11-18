//! Updates page — review of the queued changes with a safe "Prepare and check"
//! step (dry-run only) and an Apply button disabled until the app owns a
//! desired-state/generated config (see updates.rs). Never switches a system.
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::store::ChangeQueue;
use crate::updates;

pub struct UpdatesPage {
    pub page: gtk4::Box,
    step_label: gtk4::Label,
    queue_list: gtk4::ListBox,
    results_label: gtk4::Label,
    results_list: gtk4::ListBox,
    build_btn: gtk4::Button,
}

impl UpdatesPage {
    pub fn new() -> UpdatesPage {
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        page.set_margin_top(18);
        page.set_margin_start(28);
        page.set_margin_end(28);

        let heading = gtk4::Label::new(Some("Updates"));
        heading.add_css_class("title-2");
        heading.set_xalign(0.0);
        page.append(&heading);

        let step_label = gtk4::Label::new(Some("Changes are ready — nothing has been applied yet"));
        step_label.set_xalign(0.0);
        step_label.add_css_class("dim-label");
        page.append(&step_label);

        let queue_list = gtk4::ListBox::new();
        page.append(&queue_list);

        let results_label = gtk4::Label::new(None);
        results_label.set_xalign(0.0);
        results_label.add_css_class("dim-label");
        page.append(&results_label);

        let results_list = gtk4::ListBox::new();
        page.append(&results_list);

        let build_btn = gtk4::Button::with_label("Prepare and check");
        let apply_btn = gtk4::Button::with_label("Apply changes");
        apply_btn.add_css_class("suggested-action");
        apply_btn.set_sensitive(false);
        apply_btn.set_tooltip_text(Some(
            "Apply needs the system-change layer, which lands with the desired-state backend (7wo.2).",
        ));
        let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        buttons.set_halign(gtk4::Align::End);
        buttons.append(&build_btn);
        buttons.append(&apply_btn);
        page.append(&buttons);

        UpdatesPage {
            page,
            step_label,
            queue_list,
            results_label,
            results_list,
            build_btn,
        }
    }

    /// Reset then repopulate the queued-change list (call when shown).
    pub fn refresh(&self, queue: &Rc<RefCell<ChangeQueue>>) {
        self.reset();
        while let Some(child) = self.queue_list.first_child() {
            self.queue_list.remove(&child);
        }
        let q = queue.borrow();
        if q.is_empty() {
            let l = gtk4::Label::new(Some(
                "Nothing ready to change yet — add apps from the store.",
            ));
            l.set_xalign(0.0);
            l.add_css_class("dim-label");
            self.queue_list.append(&l);
            return;
        }
        for (name, system_wide) in q.summary() {
            let l = gtk4::Label::new(Some(&format!(
                "{} · {}",
                name,
                if system_wide { "system" } else { "user" }
            )));
            l.set_xalign(0.0);
            let row = gtk4::ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            row.set_child(Some(&l));
            self.queue_list.append(&row);
        }
    }

    /// Reset the results/step lines (run before a fresh Prepare or when shown).
    fn reset(&self) {
        self.step_label
            .set_text("Changes are ready — nothing has been applied yet");
        self.results_label.set_text("");
        while let Some(child) = self.results_list.first_child() {
            self.results_list.remove(&child);
        }
    }

    /// Wire Prepare (safe dry-run build-verify on a thread) + the gated Apply.
    /// The button reads the queue at click time, so it reflects the latest adds.
    pub fn wire(&self, queue: &Rc<RefCell<ChangeQueue>>, nix: &Option<std::path::PathBuf>) {
        let queue = queue.clone();
        let nix = nix.clone();
        let step_label = self.step_label.clone();
        let results_label = self.results_label.clone();
        let results_list = self.results_list.clone();
        let build_btn = self.build_btn.clone();
        build_btn.connect_clicked(move |b| {
            b.set_sensitive(false);
            let Some(nixp) = nix.clone() else {
                results_label.set_text("The package checker is not available on this system");
                b.set_sensitive(true);
                return;
            };
            // clone the widget handles so the nested 'move' closures own their
            // own copies (a Fn handler cannot move its captures into them)
            let step_label = step_label.clone();
            let results_label = results_label.clone();
            let results_list = results_list.clone();
            let b = b.clone();
            let changes: Vec<updates::PendingChange> = queue
                .borrow()
                .items()
                .iter()
                .map(|p| updates::PendingChange {
                    attr: p.attr.clone(),
                    name: p.name.clone(),
                    system_wide: p.system_wide,
                })
                .collect();
            let (tx, rx) = std::sync::mpsc::channel::<updates::BuildReport>();
            std::thread::spawn(move || {
                let rep = updates::verify_queued(&nixp, &changes, std::time::Duration::from_secs(60));
                let _ = tx.send(rep);
            });
            glib::timeout_add_local(
                std::time::Duration::from_millis(100),
                move || match rx.try_recv() {
                    Ok(rep) => {
                        while let Some(ch) = results_list.first_child() {
                            results_list.remove(&ch);
                        }
                        for it in &rep.items {
                            let line = if it.ok {
                                format!("{} — buildable", it.name)
                            } else {
                                format!("{} — {}", it.name, it.note)
                            };
                            let l = gtk4::Label::new(Some(&line));
                            l.set_xalign(0.0);
                            let row = gtk4::ListBoxRow::new();
                            row.set_selectable(false);
                            row.set_activatable(false);
                            row.set_child(Some(&l));
                            results_list.append(&row);
                        }
                        if rep.all_ok() {
                            step_label.set_text("Prepared — the changes are built and checked");
                            results_label.set_text(if rep.items.is_empty() {
                                "Nothing queued to prepare"
                            } else {
                                "All changes prepare cleanly — apply stays off until the system-change layer lands"
                            });
                        } else {
                            step_label.set_text("Nothing was changed — the prepare step failed");
                            results_label.set_text("Some changes cannot be built for this system");
                        }
                        b.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => {
                        b.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                },
            );
        });
    }
}
