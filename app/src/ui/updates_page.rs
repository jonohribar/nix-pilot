//! Updates page — review of the queued changes with a safe "Prepare and check"
//! step (dry-run only) and an Apply button disabled until the app owns a
//! desired-state/generated config (see updates.rs). Never switches a system.
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::desired_state::DesiredDoc;
use crate::store::ChangeQueue;
use crate::updates::{self, TxEvent, TxStep};

pub struct UpdatesPage {
    pub page: gtk4::Box,
    step_label: gtk4::Label,
    step: Rc<RefCell<TxStep>>,
    queue_list: gtk4::ListBox,
    results_label: gtk4::Label,
    results_list: gtk4::ListBox,
    config_view: gtk4::Label,
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

        let step = Rc::new(RefCell::new(TxStep::Ready));
        let step_label = gtk4::Label::new(Some(TxStep::Ready.label()));
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

        // developer diagnostic (constraint C8): the config the desired-state
        // renderer would hand to nixos-rebuild. Hidden until prepared.
        let config_expander = gtk4::Expander::new(Some("Generated configuration (developer)"));
        config_expander.set_vexpand(false);
        let config_view = gtk4::Label::new(None);
        config_view.set_xalign(0.0);
        config_view.set_wrap(false);
        config_view.add_css_class("monospace");
        config_view.add_css_class("dim-label");
        let config_scroll = gtk4::ScrolledWindow::builder()
            .child(&config_view)
            .min_content_height(120)
            .propagate_natural_height(true)
            .build();
        config_expander.set_child(Some(&config_scroll));
        page.append(&config_expander);

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

        // boot-recovery help (failure-and-recovery.md): ships before any failure
        let recover_card = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        recover_card.add_css_class("card");
        recover_card.set_margin_top(10);
        let rec_heading = gtk4::Label::new(Some("If your computer will not start after a change"));
        rec_heading.add_css_class("heading");
        rec_heading.set_xalign(0.0);
        let rec_body = gtk4::Label::new(Some(
            "Every change keeps a copy of the way things were. If the computer will not start, the start menu offers earlier systems in plain words — pick \u{201c}Start with yesterday's system\u{201d}. If no earlier system appears, ask for help.",
        ));
        rec_body.set_xalign(0.0);
        rec_body.set_wrap(true);
        rec_body.add_css_class("dim-label");
        recover_card.append(&rec_heading);
        recover_card.append(&rec_body);
        page.append(&recover_card);

        UpdatesPage {
            page,
            step_label,
            step,
            queue_list,
            results_label,
            results_list,
            config_view,
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
        *self.step.borrow_mut() = TxStep::Ready;
        self.step_label.set_text(TxStep::Ready.label());
        self.results_label.set_text("");
        while let Some(child) = self.results_list.first_child() {
            self.results_list.remove(&child);
        }
    }

    /// Wire Prepare (safe dry-run build-verify on a thread) + the gated Apply.
    /// The button reads the queue at click time, so it reflects the latest adds.
    /// `account` names the NixOS user whose per-user packages the generated
    /// config targets (developer diagnostic only; the real account resolution
    /// belongs to the 7wo.2 backend).
    pub fn wire(
        &self,
        queue: &Rc<RefCell<ChangeQueue>>,
        nix: &Option<std::path::PathBuf>,
        account: &str,
    ) {
        let queue = queue.clone();
        let nix = nix.clone();
        let account = account.to_string();
        let step = self.step.clone();
        let step_label = self.step_label.clone();
        let results_label = self.results_label.clone();
        let results_list = self.results_list.clone();
        let config_view = self.config_view.clone();
        let build_btn = self.build_btn.clone();
        build_btn.connect_clicked(move |b| {
            b.set_sensitive(false);
            // the transaction advances: Ready --Prepare--> Explaining
            *step.borrow_mut() = TxStep::Explaining;
            step_label.set_text(TxStep::Explaining.label());
            let Some(nixp) = nix.clone() else {
                results_label.set_text("The package checker is not available on this system");
                b.set_sensitive(true);
                return;
            };
            // clone the handles so the nested 'move' closure owns its own
            // copies (a Fn handler cannot move its captures into them)
            let step_label = step_label.clone();
            let step = step.clone();
            let results_label = results_label.clone();
            let results_list = results_list.clone();
            let config_view = config_view.clone();
            let b = b.clone();
            // render the config the desired-state model would generate
            let mut sys = Vec::new();
            let mut user = Vec::new();
            for p in queue.borrow().items() {
                if p.system_wide {
                    sys.push(p.attr.clone());
                } else {
                    user.push(p.attr.clone());
                }
            }
            let mut desired = DesiredDoc::current();
            desired.set_system(sys);
            desired.set_user(&account, user);
            config_view.set_text(&desired.render_module());
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
                let rep =
                    updates::verify_queued(&nixp, &changes, std::time::Duration::from_secs(60));
                let _ = tx.send(rep);
            });
            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                match rx.try_recv() {
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
                        // Explaining --Prepared--> BuildCheck (or --BuildFailed-->
                        // BuildFailed): the prepare result advances the transaction
                        // Explaining --Prepared--> BuildCheck (or --BuildFailed-->
                        // BuildFailed); the prepare result advances the machine
                        let (ev, fallback) = if rep.all_ok() {
                            (TxEvent::Prepared, TxStep::BuildCheck)
                        } else {
                            (TxEvent::BuildFailed, TxStep::BuildFailed)
                        };
                        let target = updates::advance(*step.borrow(), ev).unwrap_or(fallback);
                        *step.borrow_mut() = target;
                        step_label.set_text(target.label());
                        if rep.all_ok() {
                            results_label.set_text(if rep.items.is_empty() {
                                "Nothing queued to prepare"
                            } else {
                                "All changes prepare cleanly"
                            });
                        } else {
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
                }
            });
        });
    }
}
