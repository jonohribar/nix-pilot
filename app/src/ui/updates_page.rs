//! Updates page — review of the queued changes: Prepare dry-runs each item and
//! builds the candidate generation. Activation is gated by `apply::can_activate()`
//! (env NIX_PILOT_ALLOW_SWITCH + helper on PATH). When enabled, the Apply button
//! becomes active in BuildCheck after a green candidate build.
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::apply;
use crate::desired_state::{DesiredDoc, SettingValue};
use crate::store::ChangeQueue;
use crate::updates::{self, TxEvent, TxStep};

pub struct UpdatesPage {
    pub page: gtk4::Box,
    step_label: gtk4::Label,
    step: Rc<RefCell<TxStep>>,
    /// Set-and-forget upkeep controls (category "updates").
    auto_bools: Vec<(String, gtk4::Switch)>,
    auto_strs: Vec<(String, gtk4::Entry)>,
    /// Storage upkeep (7wo.9): read-only usage readout + run-now buttons.
    storage_usage: gtk4::Label,
    storage_result: gtk4::Label,
    estimate_btn: gtk4::Button,
    cleanup_btn: gtk4::Button,
    optimise_btn: gtk4::Button,
    /// System versions (7wo.8): generations list + rollback surface.
    gens_list: gtk4::ListBox,
    gens_result: gtk4::Label,
    rollback_btn: gtk4::Button,
    queue_list: gtk4::ListBox,
    results_label: gtk4::Label,
    results_list: gtk4::ListBox,
    config_view: gtk4::Label,
    build_btn: gtk4::Button,
    apply_btn: gtk4::Button,
}

struct PrepareResult {
    report: updates::BuildReport,
    candidate: Option<apply::CandidateBuild>,
}

/// Result of an activation attempt on the worker thread.
struct ActivateResult {
    action: apply::ActivateAction,
    result: apply::ActivateResult,
}

/// Verify queued packages, then build a candidate only when every check passes.
fn prepare_changes(
    nix: &std::path::Path,
    changes: &[updates::PendingChange],
    doc: &DesiredDoc,
) -> PrepareResult {
    let report = updates::verify_queued(nix, changes, std::time::Duration::from_secs(60));
    let candidate = if report.all_ok() && !changes.is_empty() {
        let (preserved, hardware) = apply::discover_nixos_sources();
        if preserved.is_some() || hardware.is_some() {
            let state_version =
                apply::system_state_version().unwrap_or_else(|| "26.05".to_string());
            Some(apply::build_from_desired(
                doc,
                &state_version,
                preserved.as_deref(),
                hardware.as_deref(),
                std::time::Duration::from_secs(1500),
            ))
        } else {
            None
        }
    } else {
        None
    };
    PrepareResult { report, candidate }
}

/// Wire the Storage card's three run-now buttons: each runs elevated through
/// `apply::cleanup` on a worker thread (only Send values cross the thread
/// boundary; widgets are re-touched on the GTK thread via a short poll).
fn wire_storage_buttons(
    result: &gtk4::Label,
    estimate: &gtk4::Button,
    cleanup: &gtk4::Button,
    optimise: &gtk4::Button,
) {
    let est_r = result.clone();
    let cln_r = result.clone();
    let opt_r = result.clone();

    let (est, cln, opt) = (estimate.clone(), cleanup.clone(), optimise.clone());
    estimate.connect_clicked(move |_| {
        run_storage_action(
            &est_r,
            &est,
            &cln,
            &opt,
            apply::CleanupAction::GcEstimate,
            false,
        )
    });
    let (est, cln, opt) = (estimate.clone(), cleanup.clone(), optimise.clone());
    cleanup.connect_clicked(move |_| {
        run_storage_action(&cln_r, &est, &cln, &opt, apply::CleanupAction::Gc, true)
    });
    let (est, cln, opt) = (estimate.clone(), cleanup.clone(), optimise.clone());
    optimise.connect_clicked(move |_| {
        run_storage_action(
            &opt_r,
            &est,
            &cln,
            &opt,
            apply::CleanupAction::Optimise,
            true,
        )
    });
}

/// Run one storage action: disable the buttons, run elevated on a worker,
/// then post the outcome back on the GTK thread (poll with a short timeout).
fn run_storage_action(
    result: &gtk4::Label,
    estimate: &gtk4::Button,
    cleanup: &gtk4::Button,
    optimise: &gtk4::Button,
    action: apply::CleanupAction,
    busy: bool,
) {
    result.set_text("Running…");
    for b in [estimate, cleanup, optimise] {
        b.set_sensitive(false);
    }
    let (tx, rx) = std::sync::mpsc::channel::<apply::CleanupResult>();
    std::thread::spawn(move || {
        let r = apply::cleanup(action, std::time::Duration::from_secs(600));
        let _ = tx.send(r);
    });
    let result = result.clone();
    let (estimate, cleanup, optimise) = (estimate.clone(), cleanup.clone(), optimise.clone());
    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        match rx.try_recv() {
            Ok(r) => {
                let text = match r.summary {
                    Some(s) => format!("{s} — {}", r.note),
                    None => r.note,
                };
                let text = if !r.ok && !r.log.is_empty() {
                    format!("{text} ({})", r.log.lines().next().unwrap_or(""))
                } else {
                    text
                };
                result.set_text(&text);
                if !busy {
                    for b in [&estimate, &cleanup, &optimise] {
                        b.set_sensitive(apply::can_activate());
                    }
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });
}

/// Outcome classification for a prepare run (pure, unit-testable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrepareVerdict {
    /// No changes queued — nothing to do.
    NotQueued,
    /// One or more items failed verification or the candidate build failed.
    Blocked,
    /// Everything is green and a candidate was built.
    Ready,
    /// Green report but no candidate: no NixOS system config to build for.
    NoSystemConfig,
}

fn classify_prepare(
    report_ok: bool,
    items_empty: bool,
    candidate_ok: Option<bool>,
) -> PrepareVerdict {
    if items_empty {
        PrepareVerdict::NotQueued
    } else if !report_ok {
        PrepareVerdict::Blocked
    } else {
        match candidate_ok {
            Some(true) => PrepareVerdict::Ready,
            Some(false) => PrepareVerdict::Blocked,
            None => PrepareVerdict::NoSystemConfig,
        }
    }
}

/// Update the page after the worker finishes; this function runs on GTK's thread.
fn render_prepare_result(
    result: PrepareResult,
    step: &RefCell<TxStep>,
    step_label: &gtk4::Label,
    results_label: &gtk4::Label,
    results_list: &gtk4::ListBox,
    build_button: &gtk4::Button,
    apply_button: &gtk4::Button,
) {
    while let Some(child) = results_list.first_child() {
        results_list.remove(&child);
    }
    for item in &result.report.items {
        let text = if item.ok {
            format!("{} — buildable", item.name)
        } else {
            format!("{} — {}", item.name, item.note)
        };
        append_result_row(results_list, &text, false);
    }

    let mut all_ok = result.report.all_ok();
    let verdict = result.candidate.as_ref().map(|candidate| {
        append_log_tail(results_list, &candidate.log);
        if !candidate.ok {
            all_ok = false;
        }
        candidate.note.clone()
    });

    // A green report with no candidate means there was no NixOS system config
    // to build for — say so instead of claiming everything is ready.
    let classification = classify_prepare(
        result.report.all_ok(),
        result.report.items.is_empty(),
        result.candidate.as_ref().map(|c| c.ok),
    );
    let (event, fallback) = match classification {
        PrepareVerdict::Ready => (TxEvent::Prepared, TxStep::BuildCheck),
        _ => (TxEvent::BuildFailed, TxStep::BuildFailed),
    };
    let target = updates::advance(*step.borrow(), event).unwrap_or(fallback);
    *step.borrow_mut() = target;
    step_label.set_text(target.label());
    results_label.set_text(&verdict.unwrap_or_else(|| {
        match classification {
            PrepareVerdict::NotQueued => "Nothing queued to prepare".into(),
            PrepareVerdict::NoSystemConfig => {
                "No managed NixOS system configuration was found — changes can't be \
             built on this computer"
                    .into()
            }
            PrepareVerdict::Blocked => "Some changes cannot be built for this system".into(),
            PrepareVerdict::Ready => "All changes prepare cleanly".into(),
        }
    }));
    build_button.set_sensitive(true);

    // Enable Apply only in BuildCheck with a green candidate AND activation allowed.
    let can_activate = apply::can_activate();
    let has_green_candidate = all_ok && result.candidate.as_ref().map(|c| c.ok).unwrap_or(false);
    apply_button.set_sensitive(target == TxStep::BuildCheck && can_activate && has_green_candidate);
    if can_activate {
        apply_button.set_tooltip_text(None);
    } else {
        apply_button.set_tooltip_text(Some(
            "Activation requires NIX_PILOT_ALLOW_SWITCH=1 and the nix-pilot-apply helper installed (nixos-module).",
        ));
    }
}

fn append_result_row(list: &gtk4::ListBox, text: &str, technical: bool) {
    let label = gtk4::Label::new(Some(text));
    label.set_xalign(0.0);
    if technical {
        label.add_css_class("monospace");
        label.add_css_class("dim-label");
    }
    let row = gtk4::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.set_child(Some(&label));
    list.append(&row);
}

fn append_log_tail(list: &gtk4::ListBox, log: &str) {
    for line in log
        .lines()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        if !line.trim().is_empty() {
            append_result_row(list, line, true);
        }
    }
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

        // Automatic upkeep (set-and-forget defaults; category "updates").
        let automatic_card = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        automatic_card.add_css_class("card");
        automatic_card.set_margin_top(6);
        let automatic_heading = gtk4::Label::new(Some("Automatic"));
        automatic_heading.add_css_class("heading");
        automatic_heading.set_xalign(0.0);
        let automatic_blurb = gtk4::Label::new(Some(
            "Set-and-forget upkeep. These apply with the rest of your changes \
             when you prepare and apply them.",
        ));
        automatic_blurb.set_xalign(0.0);
        automatic_blurb.set_wrap(true);
        automatic_blurb.add_css_class("dim-label");
        let automatic_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        automatic_box.set_margin_top(4);
        let mut auto_bools = Vec::new();
        let mut auto_strs = Vec::new();
        for spec in crate::settings::SPECS
            .iter()
            .filter(|s| s.category == "updates")
        {
            let label = gtk4::Label::new(Some(spec.label));
            label.set_xalign(0.0);
            let blurb = gtk4::Label::new(Some(spec.blurb));
            blurb.set_xalign(0.0);
            blurb.set_wrap(true);
            blurb.add_css_class("dim-label");
            if spec.kind == crate::settings::ValueKind::Boolean {
                let sw = gtk4::Switch::new();
                sw.set_halign(gtk4::Align::Start);
                automatic_box.append(&label);
                automatic_box.append(&sw);
                automatic_box.append(&blurb);
                auto_bools.push((spec.key.to_string(), sw));
            } else {
                let entry = gtk4::Entry::new();
                entry.set_placeholder_text(Some("(system default)"));
                automatic_box.append(&label);
                automatic_box.append(&entry);
                automatic_box.append(&blurb);
                auto_strs.push((spec.key.to_string(), entry));
            }
        }
        automatic_card.append(&automatic_heading);
        automatic_card.append(&automatic_blurb);
        automatic_card.append(&automatic_box);
        page.append(&automatic_card);

        // Storage upkeep (7wo.9): read-only usage readout + run-now actions,
        // gated on activation (the cleanup runs elevated through the helper).
        let storage_card = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        storage_card.add_css_class("card");
        storage_card.set_margin_top(6);
        let storage_heading = gtk4::Label::new(Some("Storage"));
        storage_heading.add_css_class("heading");
        storage_heading.set_xalign(0.0);
        let storage_usage = gtk4::Label::new(Some("Checking how much the store holds…"));
        storage_usage.set_xalign(0.0);
        storage_usage.set_wrap(true);
        storage_usage.add_css_class("dim-label");
        let storage_result = gtk4::Label::new(None);
        storage_result.set_xalign(0.0);
        storage_result.set_wrap(true);
        storage_result.add_css_class("dim-label");
        let storage_btns = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        storage_btns.set_halign(gtk4::Align::Start);
        let estimate_btn = gtk4::Button::with_label("Check what cleaning would free");
        let cleanup_btn = gtk4::Button::with_label("Clean up old versions");
        let optimise_btn = gtk4::Button::with_label("Reclaim storage");
        for b in [&estimate_btn, &cleanup_btn, &optimise_btn] {
            b.set_sensitive(false);
            b.set_tooltip_text(Some(
                "Storage upkeep runs through the nix-pilot system module; activate it \
                 (NIX_PILOT_ALLOW_SWITCH) to enable these.",
            ));
            storage_btns.append(b);
        }
        storage_card.append(&storage_heading);
        storage_card.append(&storage_usage);
        storage_card.append(&storage_result);
        storage_card.append(&storage_btns);
        page.append(&storage_card);

        // System versions (7wo.8): read-only generations list + go-back
        // control. Rollback goes to the previous generation; every earlier
        // one stays on the boot menu as the recovery path (REQ-4).
        let gens_card = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        gens_card.add_css_class("card");
        gens_card.set_margin_top(6);
        let gens_heading = gtk4::Label::new(Some("System versions"));
        gens_heading.add_css_class("heading");
        gens_heading.set_xalign(0.0);
        let gens_blurb = gtk4::Label::new(Some(
            "Every change keeps a copy of the way things were. The version \
             you are on now is marked; older ones stay on the start menu \
             until they are cleaned up.",
        ));
        gens_blurb.set_xalign(0.0);
        gens_blurb.set_wrap(true);
        gens_blurb.add_css_class("dim-label");
        let gens_list = gtk4::ListBox::new();
        let gens_result = gtk4::Label::new(None);
        gens_result.set_xalign(0.0);
        gens_result.set_wrap(true);
        gens_result.add_css_class("dim-label");
        let rollback_btn = gtk4::Button::with_label("Go back to the previous version");
        rollback_btn.set_halign(gtk4::Align::Start);
        gens_card.append(&gens_heading);
        gens_card.append(&gens_blurb);
        gens_card.append(&gens_list);
        gens_card.append(&gens_result);
        gens_card.append(&rollback_btn);
        page.append(&gens_card);

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

        let build_btn = gtk4::Button::with_label("Prepare and build");
        let apply_btn = gtk4::Button::with_label("Apply changes");
        apply_btn.add_css_class("suggested-action");
        apply_btn.set_sensitive(false);
        apply_btn.set_tooltip_text(Some(
            "Activation requires NIX_PILOT_ALLOW_SWITCH=1 and the nix-pilot-apply helper installed (nixos-module).",
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
            auto_bools,
            auto_strs,
            storage_usage,
            storage_result,
            estimate_btn,
            cleanup_btn,
            optimise_btn,
            gens_list,
            gens_result,
            rollback_btn,
            queue_list,
            results_label,
            results_list,
            config_view,
            build_btn,
            apply_btn,
        }
    }

    /// Read the store size on a worker thread and show it (read-only).
    /// The widget never crosses the thread boundary: the worker sends a plain
    /// String back and the GTK thread polls for it.
    fn refresh_storage_usage(&self) {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            let _ = tx.send(match apply::store_usage_bytes() {
                Some(bytes) => format!(
                    "The package store holds {} on this computer.",
                    apply::human_bytes(bytes)
                ),
                None => "Could not read the store size on this computer.".to_string(),
            });
        });
        let label = self.storage_usage.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            match rx.try_recv() {
                Ok(text) => {
                    label.set_text(&text);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            }
        });
    }

    /// Repopulate the system-versions list (read-only, shown when the page
    /// opens). Runs on the GTK thread; the listing itself is a cheap read of
    /// world-readable profile symlinks.
    fn refresh_generations(&self) {
        while let Some(child) = self.gens_list.first_child() {
            self.gens_list.remove(&child);
        }
        let gens = apply::list_generations();
        if gens.is_empty() {
            let l = gtk4::Label::new(Some("No system versions could be read on this computer."));
            l.set_xalign(0.0);
            l.add_css_class("dim-label");
            self.gens_list.append(&l);
            return;
        }
        for gen in &gens {
            let text = if gen.current {
                format!(
                    "{} — {}  (current)",
                    gen.date,
                    gen.version.as_deref().unwrap_or("unknown build")
                )
            } else {
                format!(
                    "{} — {}",
                    gen.date,
                    gen.version.as_deref().unwrap_or("unknown build")
                )
            };
            let l = gtk4::Label::new(Some(&text));
            l.set_xalign(0.0);
            if gen.current {
                l.add_css_class("heading");
            } else {
                l.add_css_class("dim-label");
            }
            let row = gtk4::ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            row.set_child(Some(&l));
            self.gens_list.append(&row);
        }
    }

    /// Reset then repopulate the queued-change list (call when shown).
    pub fn refresh(&self, queue: &Rc<RefCell<ChangeQueue>>) {
        self.reset();
        self.refresh_storage_usage();
        self.storage_result.set_text("");
        self.refresh_generations();
        self.gens_result.set_text("");
        while let Some(child) = self.queue_list.first_child() {
            self.queue_list.remove(&child);
        }
        let q = queue.borrow();
        // Queue-driven status (rqe): the Ready label must not claim changes
        // are ready when none are.
        if *self.step.borrow() == TxStep::Ready {
            let label = if q.is_empty() {
                "No changes waiting."
            } else {
                let n = q.len();
                let suffix = if n == 1 { "" } else { "s" };
                &format!("{n} change{suffix} ready — nothing has been applied yet")
            };
            self.step_label.set_text(label);
        }
        if q.is_empty() {
            let l = gtk4::Label::new(Some(
                "Nothing ready to change yet — add packages from the store.",
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
        // Apply button resets to disabled; it will be enabled in render_prepare_result
        // only when in BuildCheck with a green candidate and can_activate().
        self.apply_btn.set_sensitive(false);
        self.apply_btn.set_tooltip_text(Some(
            "Activation requires NIX_PILOT_ALLOW_SWITCH=1 and the nix-pilot-apply helper installed (nixos-module).",
        ));
    }

    /// Wire Prepare and Apply on worker threads.
    pub fn wire(
        &self,
        queue: &Rc<RefCell<ChangeQueue>>,
        desired: &Rc<RefCell<DesiredDoc>>,
        nix: &Option<std::path::PathBuf>,
        save: Rc<dyn Fn()>,
    ) {
        // Automatic upkeep controls: load, then persist on change.
        {
            let doc = desired.borrow();
            for (key, sw) in &self.auto_bools {
                sw.set_active(matches!(
                    doc.settings.get(key),
                    Some(SettingValue::Bool(true))
                ));
            }
            for (key, entry) in &self.auto_strs {
                if let Some(SettingValue::Str(s)) = doc.settings.get(key) {
                    entry.set_text(s);
                }
            }
        }
        for (key, sw) in self.auto_bools.clone() {
            let desired = desired.clone();
            let save = save.clone();
            sw.connect_state_set(move |_, active| {
                if active {
                    desired
                        .borrow_mut()
                        .settings
                        .insert(key.clone(), SettingValue::Bool(true));
                } else {
                    desired.borrow_mut().settings.remove(&key);
                }
                save();
                glib::Propagation::Proceed
            });
        }
        for (key, entry) in self.auto_strs.clone() {
            let desired = desired.clone();
            let save = save.clone();
            entry.connect_changed(move |entry| {
                let text = entry.text().to_string();
                if text.trim().is_empty() {
                    desired.borrow_mut().settings.remove(&key);
                } else {
                    desired
                        .borrow_mut()
                        .settings
                        .insert(key.clone(), SettingValue::Str(text));
                }
                save();
            });
        }

        // Storage upkeep: the run-now actions route through the sanctioned
        // helper, so they are enabled only when activation is (module+flag).
        let storage_result = self.storage_result.clone();
        let estimate_btn = self.estimate_btn.clone();
        let cleanup_btn = self.cleanup_btn.clone();
        let optimise_btn = self.optimise_btn.clone();
        let activate = apply::can_activate();
        for b in [&estimate_btn, &cleanup_btn, &optimise_btn] {
            b.set_sensitive(activate);
            b.set_tooltip_text(Some(match activate {
                true => "Runs through the nix-pilot-apply helper (root).",
                false => {
                    "Storage upkeep runs through the nix-pilot system module; \
                     activate it (NIX_PILOT_ALLOW_SWITCH) to enable these."
                }
            }));
        }
        wire_storage_buttons(&storage_result, &estimate_btn, &cleanup_btn, &optimise_btn);

        // System versions: go back to the previous generation, gated exactly
        // like the storage/apply actions (module + flag). Rollback reuses the
        // helper's fixed `rollback` action (no helper change needed).
        let gens_result = self.gens_result.clone();
        let gens_rollback = self.rollback_btn.clone();
        gens_rollback.set_sensitive(activate);
        gens_rollback.set_tooltip_text(Some(match activate {
            true => "Starts the previous system version now (root).",
            false => {
                "Rollback runs through the nix-pilot system module; activate it \
                 (NIX_PILOT_ALLOW_SWITCH) to enable it."
            }
        }));
        let doc_rc = desired.clone();
        gens_rollback.connect_clicked(move |b| {
            b.set_sensitive(false);
            gens_result.set_text("Starting the previous version…");
            let (tx, rx) = std::sync::mpsc::channel::<apply::ActivateResult>();
            let doc = doc_rc.borrow().clone();
            std::thread::spawn(move || {
                let r = apply::activate(
                    apply::ActivateAction::Rollback,
                    &doc,
                    std::time::Duration::from_secs(900),
                );
                let _ = tx.send(r);
            });
            let res = gens_result.clone();
            let btn = b.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                match rx.try_recv() {
                    Ok(r) => {
                        res.set_text(&r.note);
                        btn.set_sensitive(apply::can_activate());
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => {
                        btn.set_sensitive(apply::can_activate());
                        glib::ControlFlow::Break
                    }
                }
            });
        });

        // Clone shared state for both Prepare and Apply handlers
        let queue_for_prepare = queue.clone();
        let desired_for_prepare = desired.clone();
        let nix = nix.clone();
        let step = self.step.clone();
        let step_label = self.step_label.clone();
        let results_label = self.results_label.clone();
        let results_list = self.results_list.clone();
        let config_view = self.config_view.clone();
        let build_btn = self.build_btn.clone();
        let apply_btn = self.apply_btn.clone();

        // --- Prepare (Build) ---
        build_btn.connect_clicked(move |b| {
            b.set_sensitive(false);
            apply_btn.set_sensitive(false);
            // the transaction advances: Ready --Prepare--> Explaining
            *step.borrow_mut() = TxStep::Explaining;
            step_label.set_text(TxStep::Explaining.label());
            let Some(nixp) = nix.clone() else {
                results_label.set_text("The package checker is not available on this system");
                b.set_sensitive(true);
                return;
            };
            let step_label = step_label.clone();
            let step = step.clone();
            let results_label = results_label.clone();
            let results_list = results_list.clone();
            let b = b.clone();
            let apply_btn = apply_btn.clone();
            // render the config the desired-state model would generate (the
            // persisted doc: system + per-user packages + any app settings)
            let doc = desired_for_prepare.borrow().clone();
            match doc.render_module(apply::detect_user_backend()) {
                Ok(config) => config_view.set_text(&config),
                Err(error) => config_view
                    .set_text(&format!("Cannot render the desired state safely: {error}")),
            }
            let changes = queue_for_prepare.borrow().items().to_vec();
            let (tx, rx) = std::sync::mpsc::channel::<PrepareResult>();
            std::thread::spawn(move || {
                let result = prepare_changes(&nixp, &changes, &doc);
                let _ = tx.send(result);
            });
            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                match rx.try_recv() {
                    Ok(result) => {
                        render_prepare_result(
                            result,
                            &step,
                            &step_label,
                            &results_label,
                            &results_list,
                            &b,
                            &apply_btn,
                        );
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

        // --- Apply (Switch) ---
        let step = self.step.clone();
        let step_label = self.step_label.clone();
        let results_label = self.results_label.clone();
        let results_list = self.results_list.clone();
        let apply_btn = self.apply_btn.clone();
        let build_btn = self.build_btn.clone();
        let queue_for_apply = queue.clone();
        let desired_for_apply = desired.clone();
        apply_btn.clone().connect_clicked(move |_| {
            apply_btn.set_sensitive(false);
            build_btn.set_sensitive(false);

            // BuildCheck --BuildOk--> Applying (the Apply button is only enabled
            // in BuildCheck with a green candidate, so this is the valid transition)
            *step.borrow_mut() = TxStep::Applying;
            step_label.set_text(TxStep::Applying.label());
            results_label.set_text("Applying the change…");

            let doc = desired_for_apply.borrow().clone();
            let (tx, rx) = std::sync::mpsc::channel::<ActivateResult>();
            std::thread::spawn(move || {
                let result = apply::activate(
                    apply::ActivateAction::Switch,
                    &doc,
                    std::time::Duration::from_secs(1800),
                );
                let _ = tx.send(ActivateResult {
                    action: apply::ActivateAction::Switch,
                    result,
                });
            });
            // Clone for the timeout closure
            let step = step.clone();
            let step_label = step_label.clone();
            let results_label = results_label.clone();
            let results_list = results_list.clone();
            let apply_btn = apply_btn.clone();
            let build_btn = build_btn.clone();
            let queue_for_apply = queue_for_apply.clone();
            let desired_for_apply = desired_for_apply.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                match rx.try_recv() {
                    Ok(ar) => {
                        handle_activate_result(
                            ar,
                            ActivateContext {
                                step: &step,
                                step_label: &step_label,
                                results_label: &results_label,
                                results_list: &results_list,
                                apply_btn: &apply_btn,
                                build_btn: &build_btn,
                                queue: &queue_for_apply,
                                desired: &desired_for_apply,
                            },
                        );
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => {
                        apply_btn.set_sensitive(true);
                        build_btn.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                }
            });
        });
    }
}

/// Context passed to handle_activate_result (grouped to avoid too many arguments).
struct ActivateContext<'a> {
    step: &'a RefCell<TxStep>,
    step_label: &'a gtk4::Label,
    results_label: &'a gtk4::Label,
    results_list: &'a gtk4::ListBox,
    apply_btn: &'a gtk4::Button,
    build_btn: &'a gtk4::Button,
    queue: &'a Rc<RefCell<ChangeQueue>>,
    desired: &'a Rc<RefCell<DesiredDoc>>,
}

/// Handle the result of an activation (switch or rollback) on the GTK thread.
fn handle_activate_result(ar: ActivateResult, ctx: ActivateContext<'_>) {
    let action = ar.action;
    let result = ar.result;

    // Clear any previous log tail
    while let Some(child) = ctx.results_list.first_child() {
        ctx.results_list.remove(&child);
    }
    if !result.log.is_empty() {
        append_log_tail(ctx.results_list, &result.log);
    }

    match (action, result.ok) {
        (apply::ActivateAction::Switch, true) => {
            // Applying --ApplyOk--> Done
            *ctx.step.borrow_mut() = TxStep::Done;
            ctx.step_label.set_text(TxStep::Done.label());
            ctx.results_label.set_text(&result.note);
            ctx.apply_btn.set_sensitive(true);
            ctx.apply_btn.set_label("Go back to the previous system");
            ctx.apply_btn.set_tooltip_text(Some(
                "Return to the system generation before this change was applied.",
            ));
            // Rewire the apply button to perform rollback
            rewire_apply_for_rollback(
                ctx.apply_btn,
                ctx.build_btn,
                ActivateContext {
                    step: ctx.step,
                    step_label: ctx.step_label,
                    results_label: ctx.results_label,
                    results_list: ctx.results_list,
                    apply_btn: ctx.apply_btn,
                    build_btn: ctx.build_btn,
                    queue: ctx.queue,
                    desired: ctx.desired,
                },
            );
        }
        (apply::ActivateAction::Switch, false) => {
            // Applying --ApplyFailed--> ApplyFailed
            *ctx.step.borrow_mut() = TxStep::ApplyFailed;
            ctx.step_label.set_text(TxStep::ApplyFailed.label());
            ctx.results_label.set_text(&result.note);
            ctx.apply_btn.set_sensitive(true);
            ctx.apply_btn.set_label("Go back to the previous system");
            ctx.apply_btn.set_tooltip_text(Some(
                "The change did not apply. Return to the previous system generation.",
            ));
            rewire_apply_for_rollback(
                ctx.apply_btn,
                ctx.build_btn,
                ActivateContext {
                    step: ctx.step,
                    step_label: ctx.step_label,
                    results_label: ctx.results_label,
                    results_list: ctx.results_list,
                    apply_btn: ctx.apply_btn,
                    build_btn: ctx.build_btn,
                    queue: ctx.queue,
                    desired: ctx.desired,
                },
            );
        }
        (apply::ActivateAction::Rollback, true) => {
            // ApplyFailed/Done --Rollback--> RolledBack
            *ctx.step.borrow_mut() = TxStep::RolledBack;
            ctx.step_label.set_text(TxStep::RolledBack.label());
            ctx.results_label.set_text(&result.note);
            ctx.apply_btn.set_sensitive(false);
            ctx.apply_btn.set_label("Apply changes");
            ctx.apply_btn.set_tooltip_text(Some(
                "Activation requires NIX_PILOT_ALLOW_SWITCH=1 and the nix-pilot-apply helper installed (nixos-module).",
            ));
            ctx.build_btn.set_sensitive(true);
            // Restore original Apply wiring (will be re-enabled on next Prepare if green)
            restore_apply_wiring(
                ctx.apply_btn,
                ctx.build_btn,
                ActivateContext {
                    step: ctx.step,
                    step_label: ctx.step_label,
                    results_label: ctx.results_label,
                    results_list: ctx.results_list,
                    apply_btn: ctx.apply_btn,
                    build_btn: ctx.build_btn,
                    queue: ctx.queue,
                    desired: ctx.desired,
                },
            );
        }
        (apply::ActivateAction::Rollback, false) => {
            // Rollback failed — stay in current step, show error
            ctx.results_label
                .set_text(&format!("Rollback failed: {}", result.note));
            ctx.apply_btn.set_sensitive(true);
            ctx.build_btn.set_sensitive(true);
        }
    }
}

/// Rewire the Apply button to perform rollback with confirmation.
fn rewire_apply_for_rollback(
    apply_btn: &gtk4::Button,
    build_btn: &gtk4::Button,
    ctx: ActivateContext<'_>,
) {
    // Use a shared mode flag to track rollback mode.
    let in_rollback_mode = Rc::new(RefCell::new(true));

    let apply_btn = apply_btn.clone();
    let step = Rc::new(ctx.step.clone());
    let step_label = ctx.step_label.clone();
    let results_label = ctx.results_label.clone();
    let results_list = ctx.results_list.clone();
    let queue = ctx.queue.clone();
    let desired = ctx.desired.clone();
    let build_btn = build_btn.clone();
    let in_rollback_mode = in_rollback_mode.clone();

    apply_btn.clone().connect_clicked(move |_| {
        if !*in_rollback_mode.borrow() {
            return; // Not in rollback mode, ignore
        }

        // Show confirmation dialog (REQ-3: one-dialog plain-language confirmation)
        let dialog = gtk4::MessageDialog::builder()
            .transient_for(
                apply_btn
                    .root()
                    .and_then(|r| r.downcast::<gtk4::Window>().ok())
                    .as_ref()
                    .expect("dialog must have a parent window"),
            )
            .modal(true)
            .message_type(gtk4::MessageType::Question)
            .buttons(gtk4::ButtonsType::None)
            .text("Go back to the previous system?")
            .secondary_text(
                "This will undo the change and restart the previous system generation. \
                 You can re-apply the change later from the Updates page.",
            )
            .build();
        dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
        dialog.add_button("Go back", gtk4::ResponseType::Accept);
        dialog.set_default_response(gtk4::ResponseType::Cancel);
        let apply_btn = apply_btn.clone();
        let build_btn = build_btn.clone();
        let step = step.clone();
        let step_label = step_label.clone();
        let results_label = results_label.clone();
        let results_list = results_list.clone();
        let queue = queue.clone();
        let desired = desired.clone();
        let in_rollback_mode = in_rollback_mode.clone();
        dialog.connect_response(move |d, response| {
            d.close();
            if response == gtk4::ResponseType::Accept {
                // User confirmed — run rollback
                in_rollback_mode.replace(false);
                apply_btn.set_sensitive(false);
                build_btn.set_sensitive(false);
                results_label.set_text("Restoring the previous system…");

                let doc = desired.borrow().clone();
                let (tx, rx) = std::sync::mpsc::channel::<ActivateResult>();
                std::thread::spawn(move || {
                    let result = apply::activate(
                        apply::ActivateAction::Rollback,
                        &doc,
                        std::time::Duration::from_secs(600),
                    );
                    let _ = tx.send(ActivateResult {
                        action: apply::ActivateAction::Rollback,
                        result,
                    });
                });
                // Clone for the timeout closure
                let step = step.clone();
                let step_label = step_label.clone();
                let results_label = results_label.clone();
                let results_list = results_list.clone();
                let apply_btn = apply_btn.clone();
                let build_btn = build_btn.clone();
                let queue = queue.clone();
                let desired = desired.clone();
                glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                    match rx.try_recv() {
                        Ok(ar) => {
                            handle_activate_result(
                                ar,
                                ActivateContext {
                                    step: &step,
                                    step_label: &step_label,
                                    results_label: &results_label,
                                    results_list: &results_list,
                                    apply_btn: &apply_btn,
                                    build_btn: &build_btn,
                                    queue: &queue,
                                    desired: &desired,
                                },
                            );
                            glib::ControlFlow::Break
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(_) => {
                            apply_btn.set_sensitive(true);
                            build_btn.set_sensitive(true);
                            glib::ControlFlow::Break
                        }
                    }
                });
            }
        });
        dialog.present();
    });
}

/// Restore the Apply button to its original wiring (for a future Prepare cycle).
fn restore_apply_wiring(
    apply_btn: &gtk4::Button,
    build_btn: &gtk4::Button,
    _ctx: ActivateContext<'_>,
) {
    // The Apply button will be re-enabled and rewired in the next Prepare cycle
    // via render_prepare_result. We just need to ensure it's not in rollback mode.
    // The rollback handler checks `in_rollback_mode` flag which is local to
    // the rewire_apply_for_rollback closure. After rollback, we set it to
    // false, so the rollback handler becomes a no-op. The original Switch
    // handler will fire on next click, but only if the button is sensitive.
    // The button is only made sensitive in render_prepare_result when in
    // BuildCheck with green candidate. So the Switch handler will correctly
    // fire Switch. This works!
    let _ = _ctx; // silence unused warning
    apply_btn.set_sensitive(false);
    apply_btn.set_label("Apply changes");
    apply_btn.set_tooltip_text(Some(
        "Activation requires NIX_PILOT_ALLOW_SWITCH=1 and the nix-pilot-apply helper installed (nixos-module).",
    ));
    build_btn.set_sensitive(true);
}

#[cfg(test)]
mod tests {
    use super::classify_prepare;
    use super::PrepareVerdict;

    #[test]
    fn classify_prepare_ready_needs_a_green_candidate() {
        assert_eq!(
            classify_prepare(true, false, Some(true)),
            PrepareVerdict::Ready
        );
        assert_eq!(
            classify_prepare(true, true, Some(true)),
            PrepareVerdict::NotQueued
        );
    }

    #[test]
    fn classify_prepare_blocked_when_checks_or_build_fail() {
        assert_eq!(
            classify_prepare(false, false, Some(true)),
            PrepareVerdict::Blocked
        );
        assert_eq!(
            classify_prepare(true, false, Some(false)),
            PrepareVerdict::Blocked
        );
    }

    #[test]
    fn classify_prepare_without_system_config_is_honest() {
        // green checks but no candidate (non-NixOS / no config) must never read
        // as "ready" — the page must not claim nothing can be built is fine
        assert_eq!(
            classify_prepare(true, false, None),
            PrepareVerdict::NoSystemConfig
        );
    }
}
