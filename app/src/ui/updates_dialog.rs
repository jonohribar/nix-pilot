//! Updates review dialog (safe build-first). A modal review of the queued
//! changes with a safe "Prepare and check" step (dry-run only) and an Apply
//! button that stays disabled until the app owns a desired-state/generated
//! config (see updates.rs). Never switches a system.
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::store::ChangeQueue;
use crate::updates;

pub fn open_updates_dialog(
    win_parent: &gtk4::ApplicationWindow,
    queue: &Rc<RefCell<ChangeQueue>>,
    nix: &Option<std::path::PathBuf>,
    retained: &Rc<RefCell<Vec<gtk4::Window>>>,
) {
    let win = gtk4::Window::builder()
        .transient_for(win_parent)
        .modal(true)
        .title("Updates — review")
        .default_width(520)
        .default_height(420)
        .build();
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    body.set_margin_top(16);
    body.set_margin_bottom(16);
    body.set_margin_start(20);
    body.set_margin_end(20);

    let summary = gtk4::Label::new(Some("Changes are ready — nothing has been applied yet"));
    summary.set_xalign(0.0);
    summary.add_css_class("dim-label");
    body.append(&summary);

    let queue_list = gtk4::ListBox::new();
    for (name, system_wide) in queue.borrow().summary() {
        let row_label = gtk4::Label::new(Some(&format!(
            "{} · {}",
            name,
            if system_wide { "system" } else { "user" }
        )));
        row_label.set_xalign(0.0);
        let row = gtk4::ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(false);
        row.set_child(Some(&row_label));
        queue_list.append(&row);
    }
    body.append(&queue_list);

    let results_label = gtk4::Label::new(None);
    results_label.set_xalign(0.0);
    results_label.add_css_class("dim-label");
    body.append(&results_label);
    let results_list = gtk4::ListBox::new();
    body.append(&results_list);

    let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    buttons.set_halign(gtk4::Align::End);
    let build_btn = gtk4::Button::with_label("Prepare and check");
    let apply_btn = gtk4::Button::with_label("Apply changes");
    apply_btn.add_css_class("suggested-action");
    apply_btn.set_sensitive(false);
    apply_btn.set_tooltip_text(Some(
        "Apply needs the system-change layer, which lands with the desired-state backend (next).",
    ));
    let close_btn = gtk4::Button::with_label("Close");
    buttons.append(&build_btn);
    buttons.append(&apply_btn);
    buttons.append(&close_btn);
    body.append(&buttons);
    win.set_child(Some(&body));
    win.present();
    retained.borrow_mut().push(win.clone());

    let win_close = win;
    close_btn.connect_clicked(move |_| win_close.close());

    // Prepare: run the safe dry-run build-verify on a thread, report per item.
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
    let nix_owned = nix.clone();
    build_btn.connect_clicked(move |b| {
        b.set_sensitive(false);
        let Some(nixp) = nix_owned.clone() else {
            results_label.set_text("The package checker is not available on this system");
            b.set_sensitive(true);
            return;
        };
        let results_label = results_label.clone();
        let results_list = results_list.clone();
        let b = b.clone();
        let changes = changes.clone();
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
                    results_label.set_text(if rep.all_ok() {
                        "All changes prepare cleanly — apply stays off until the system-change layer lands"
                    } else {
                        "Some changes cannot be built for this system"
                    });
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
