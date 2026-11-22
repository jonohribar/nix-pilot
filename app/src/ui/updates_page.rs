//! Updates page — review of selected changes: Prepare dry-runs each item and
//! builds the candidate generation. Activation is gated by `activation::can_activate()`
//! (env NIX_PILOT_ALLOW_SWITCH + helper on PATH). When enabled, the Apply button
//! becomes active in BuildCheck after a green candidate build.
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::desired_state::{DesiredDoc, PackageIntent, PackageScope};
use crate::model::PackageNames;
use crate::system::{activation, build, generation, storage};
use crate::ui::setting_card;
use crate::updates::{self, TxEvent, TxStep};

/// How many recent generations the System-versions card shows by default
/// (the rest sit behind the "Show older" expander).
const RECENT_GENERATIONS: usize = 10;

/// One System-versions row: a human day label with the current generation
/// marked, and the nixpkgs revision demoted to a tooltip (never the main
/// content).
fn generation_row(gen: &generation::GenerationInfo, now: u64) -> gtk4::ListBoxRow {
    let day = generation::humanize_day(&gen.date, now);
    let text = if gen.current {
        format!("{day} — this system now")
    } else {
        day
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
    if let Some(version) = gen.version.as_deref() {
        row.set_tooltip_text(Some(version));
    }
    row
}

/// The "Show older" expander that reveals every remaining generation, so the
/// card stays a bounded, browsable history instead of an unbounded wall.
fn show_older_rows(older: &[generation::GenerationInfo], now: u64) -> gtk4::ListBoxRow {
    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    for gen in older {
        inner.append(&generation_row(gen, now));
    }
    let label = format!("Show older — {} more", older.len());
    let expander = gtk4::Expander::new(Some(label.as_str()));
    expander.set_child(Some(&inner));
    let row = gtk4::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.set_child(Some(&expander));
    row
}

/// Review, prepare, and (when enabled) apply desired system changes.
pub struct UpdatesPage {
    /// Root widget for the Updates surface.
    pub page: gtk4::Box,
    step_label: gtk4::Label,
    step: Rc<RefCell<TxStep>>,
    prepared_doc: Rc<RefCell<Option<DesiredDoc>>>,
    /// Set-and-forget upkeep cards (category "updates"), rendered with the
    /// same unified controls as the Settings page.
    auto_cards: Vec<setting_card::SettingCard>,
    /// Storage upkeep: read-only usage readout + run-now buttons.
    storage_usage: gtk4::Label,
    storage_result: gtk4::Label,
    estimate_btn: gtk4::Button,
    cleanup_btn: gtk4::Button,
    optimise_btn: gtk4::Button,
    /// System versions: generations list + rollback surface.
    gens_list: gtk4::ListBox,
    gens_result: gtk4::Label,
    rollback_btn: gtk4::Button,
    queue_list: gtk4::ListBox,
    results_label: gtk4::Label,
    results_list: gtk4::ListBox,
    config_view: gtk4::Label,
    build_btn: gtk4::Button,
    apply_btn: gtk4::Button,
    package_names: Rc<PackageNames>,
}

#[derive(Clone)]
struct TxUi {
    step: Rc<RefCell<TxStep>>,
    step_label: gtk4::Label,
    results_label: gtk4::Label,
    results_list: gtk4::ListBox,
    build_btn: gtk4::Button,
    primary_btn: gtk4::Button,
}

impl TxUi {
    fn transition(&self, event: TxEvent) -> Option<TxStep> {
        advance_transaction(&self.step, event)
    }

    fn render(&self) {
        let step = *self.step.borrow();
        let view = updates::view_for(step, activation::can_activate());
        self.step_label.set_text(step.label());
        self.build_btn.set_sensitive(view.prepare_enabled);
        self.primary_btn.set_label(view.primary_label);
        self.primary_btn.set_sensitive(view.primary_enabled);
        self.primary_btn.set_tooltip_text(view.primary_tooltip);
    }
}

fn advance_transaction(step: &RefCell<TxStep>, event: TxEvent) -> Option<TxStep> {
    let current = *step.borrow();
    let next = updates::advance(current, event)?;
    *step.borrow_mut() = next;
    Some(next)
}

struct PrepareResult {
    report: updates::BuildReport,
    candidate: Option<build::CandidateBuild>,
}

/// Result of an activation attempt on the worker thread.
struct ActivateResult {
    action: activation::ActivateAction,
    result: activation::ActivateResult,
}

/// Verify every selected package, then build only when all checks pass.
///
/// The candidate remains absent if there are no changes, any check fails, or
/// the supported live configuration cannot be discovered. The caller treats
/// an absent candidate as not prepared for application.
fn prepare_changes(
    nix: &std::path::Path,
    changes: &[PackageIntent],
    doc: &DesiredDoc,
) -> PrepareResult {
    let report = updates::verify_package_intents(nix, changes, std::time::Duration::from_secs(60));
    let candidate = if report.all_ok() && !changes.is_empty() {
        if let Ok(sources) = crate::config_source::discover() {
            let state_version = build::state_version_or_default();
            Some(build::build_from_desired(
                doc,
                &state_version,
                Some(&sources.configuration),
                sources.hardware_configuration.as_deref(),
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
/// `storage::cleanup` on a worker thread (only Send values cross the thread
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
            storage::CleanupAction::GcEstimate,
            false,
        )
    });
    let (est, cln, opt) = (estimate.clone(), cleanup.clone(), optimise.clone());
    cleanup.connect_clicked(move |_| {
        run_storage_action(&cln_r, &est, &cln, &opt, storage::CleanupAction::Gc, true)
    });
    let (est, cln, opt) = (estimate.clone(), cleanup.clone(), optimise.clone());
    optimise.connect_clicked(move |_| {
        run_storage_action(
            &opt_r,
            &est,
            &cln,
            &opt,
            storage::CleanupAction::Optimise,
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
    action: storage::CleanupAction,
    busy: bool,
) {
    result.set_text("Running…");
    for b in [estimate, cleanup, optimise] {
        b.set_sensitive(false);
    }
    let (tx, rx) = std::sync::mpsc::channel::<storage::CleanupResult>();
    std::thread::spawn(move || {
        let r = storage::cleanup(action, std::time::Duration::from_secs(600));
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
                        b.set_sensitive(activation::can_activate());
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
    snapshot: DesiredDoc,
    ui: &TxUi,
    prepared_doc: &RefCell<Option<DesiredDoc>>,
    package_names: &PackageNames,
) {
    while let Some(child) = ui.results_list.first_child() {
        ui.results_list.remove(&child);
    }
    for item in &result.report.items {
        let text = if item.ok {
            format!("{} — buildable", package_names.display_name(&item.attr))
        } else {
            format!("{} — {}", package_names.display_name(&item.attr), item.note)
        };
        append_result_row(&ui.results_list, &text, false);
    }

    let verdict = result.candidate.as_ref().map(|candidate| {
        append_log_tail(&ui.results_list, &candidate.log);
        candidate.note.clone()
    });

    // A green report with no candidate means there was no NixOS system config
    // to build for — say so instead of claiming everything is ready.
    let classification = classify_prepare(
        result.report.all_ok(),
        result.report.items.is_empty(),
        result.candidate.as_ref().map(|c| c.ok),
    );
    let event = match classification {
        PrepareVerdict::Ready => TxEvent::Prepared,
        _ => TxEvent::BuildFailed,
    };
    if ui.transition(event).is_none() {
        *prepared_doc.borrow_mut() = None;
        ui.results_label
            .set_text("The preparation result did not match the current transaction.");
        ui.render();
        return;
    }
    if classification == PrepareVerdict::Ready {
        *prepared_doc.borrow_mut() = Some(snapshot);
    } else {
        *prepared_doc.borrow_mut() = None;
    }
    ui.results_label.set_text(&verdict.unwrap_or_else(|| {
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
    ui.render();
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
    /// Build the Updates surface and its initially disabled action controls.
    pub fn new(package_names: Rc<PackageNames>) -> UpdatesPage {
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
        let mut auto_cards = Vec::new();
        for spec in crate::settings::SPECS
            .iter()
            .filter(|s| s.category == "updates")
        {
            let card = setting_card::SettingCard::new(spec);
            automatic_box.append(card.card());
            auto_cards.push(card);
        }
        automatic_card.append(&automatic_heading);
        automatic_card.append(&automatic_blurb);
        automatic_card.append(&automatic_box);
        page.append(&automatic_card);

        // Storage upkeep: read-only usage readout + run-now actions,
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

        // System versions: read-only generations list + go-back control.
        // Rollback goes to the previous generation; every earlier one stays
        // on the boot menu as the recovery path (REQ-4).
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
            prepared_doc: Rc::new(RefCell::new(None)),
            auto_cards,
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
            package_names,
        }
    }

    fn transaction_ui(&self) -> TxUi {
        TxUi {
            step: self.step.clone(),
            step_label: self.step_label.clone(),
            results_label: self.results_label.clone(),
            results_list: self.results_list.clone(),
            build_btn: self.build_btn.clone(),
            primary_btn: self.apply_btn.clone(),
        }
    }

    /// Read the store size on a worker thread and show it (read-only).
    /// The widget never crosses the thread boundary: the worker sends a plain
    /// String back and the GTK thread polls for it.
    fn refresh_storage_usage(&self) {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            let _ = tx.send(match storage::store_usage_bytes() {
                Some(bytes) => format!(
                    "The package store holds {} on this computer.",
                    storage::human_bytes(bytes)
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

    /// Repopulate the System-versions card (read-only, shown when the page
    /// opens). Runs on the GTK thread; the listing is a cheap read of
    /// world-readable profile symlinks. Shows the newest few generations with
    /// the rest behind a "Show older" expander, per the recovery spec.
    fn refresh_generations(&self) {
        while let Some(child) = self.gens_list.first_child() {
            self.gens_list.remove(&child);
        }
        let gens = generation::list_generations();
        if gens.is_empty() {
            let l = gtk4::Label::new(Some("No system versions could be read on this computer."));
            l.set_xalign(0.0);
            l.add_css_class("dim-label");
            self.gens_list.append(&l);
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut by_number = gens.into_iter();
        let recent: Vec<_> = by_number.by_ref().take(RECENT_GENERATIONS).collect();
        let older: Vec<_> = by_number.collect();
        for gen in &recent {
            self.gens_list.append(&generation_row(gen, now));
        }
        if !older.is_empty() {
            self.gens_list.append(&show_older_rows(&older, now));
        }
    }

    /// Reset then repopulate the queued-change list (call when shown).
    pub fn refresh(&self, desired: &Rc<RefCell<DesiredDoc>>, account: &str) {
        setting_card::refresh_live_values(&self.auto_cards, desired);
        self.refresh_storage_usage();
        self.storage_result.set_text("");
        self.refresh_generations();
        self.gens_result.set_text("");
        while let Some(child) = self.queue_list.first_child() {
            self.queue_list.remove(&child);
        }
        let document = desired.borrow().clone();
        let intents = document.package_intents_for(account);
        if *self.step.borrow() == TxStep::BuildCheck
            && self.prepared_doc.borrow().as_ref() != Some(&document)
        {
            self.prepared_doc.borrow_mut().take();
            let ui = self.transaction_ui();
            let _ = ui.transition(TxEvent::InputsChanged);
            self.results_label
                .set_text("Your changes have changed since preparation. Prepare them again.");
            while let Some(child) = self.results_list.first_child() {
                self.results_list.remove(&child);
            }
        }
        self.transaction_ui().render();
        // Desired-state-driven status: the Ready label must not claim changes
        // are ready when none are.
        if *self.step.borrow() == TxStep::Ready {
            let label = if intents.is_empty() {
                "No changes waiting."
            } else {
                let n = intents.len();
                let suffix = if n == 1 { "" } else { "s" };
                &format!("{n} change{suffix} ready — nothing has been applied yet")
            };
            self.step_label.set_text(label);
        }
        if intents.is_empty() {
            let l = gtk4::Label::new(Some(
                "Nothing ready to change yet — add packages from the store.",
            ));
            l.set_xalign(0.0);
            l.add_css_class("dim-label");
            self.queue_list.append(&l);
            return;
        }
        for intent in intents {
            let l = gtk4::Label::new(Some(&format!(
                "{} · {}",
                self.package_names.display_name(&intent.attr),
                match intent.scope {
                    PackageScope::System => "system",
                    PackageScope::User => "user",
                }
            )));
            l.set_xalign(0.0);
            let row = gtk4::ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            row.set_child(Some(&l));
            self.queue_list.append(&row);
        }
    }

    /// Wire Prepare and Apply on worker threads.
    pub fn wire(
        &self,
        desired: &Rc<RefCell<DesiredDoc>>,
        account: &str,
        nix: &Option<std::path::PathBuf>,
        save: Rc<dyn Fn()>,
    ) {
        // Automatic upkeep cards: unified controls, wired like the Settings
        // page (mirror-reality preselection, always-visible Reset, validation).
        for card in &self.auto_cards {
            card.wire(desired, save.clone());
        }

        // Storage upkeep: the run-now actions route through the sanctioned
        // helper, so they are enabled only when activation is (module+flag).
        let storage_result = self.storage_result.clone();
        let estimate_btn = self.estimate_btn.clone();
        let cleanup_btn = self.cleanup_btn.clone();
        let optimise_btn = self.optimise_btn.clone();
        let activate = activation::can_activate();
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
            let (tx, rx) = std::sync::mpsc::channel::<activation::ActivateResult>();
            let doc = doc_rc.borrow().clone();
            std::thread::spawn(move || {
                let r = activation::activate(
                    activation::ActivateAction::Rollback,
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
                        btn.set_sensitive(activation::can_activate());
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => {
                        btn.set_sensitive(activation::can_activate());
                        glib::ControlFlow::Break
                    }
                }
            });
        });

        let ui = self.transaction_ui();
        let prepared_doc = self.prepared_doc.clone();
        let desired_for_prepare = desired.clone();
        let account_for_prepare = account.to_string();
        let nix = nix.clone();
        let config_view = self.config_view.clone();
        let package_names = self.package_names.clone();

        // --- Prepare (Build) ---
        let prepare_ui = ui.clone();
        self.build_btn.connect_clicked(move |_| {
            if prepare_ui.transition(TxEvent::Prepare).is_none() {
                return;
            }
            *prepared_doc.borrow_mut() = None;
            while let Some(child) = prepare_ui.results_list.first_child() {
                prepare_ui.results_list.remove(&child);
            }
            prepare_ui.results_label.set_text("");
            prepare_ui.render();

            let Some(nixp) = nix.clone() else {
                fail_prepare(
                    &prepare_ui,
                    &prepared_doc,
                    "The package checker is not available on this system.",
                );
                return;
            };

            let doc = desired_for_prepare.borrow().clone();
            match doc.render_module(build::detect_user_backend()) {
                Ok(config) => config_view.set_text(&config),
                Err(error) => {
                    config_view
                        .set_text(&format!("Cannot render the desired state safely: {error}"));
                    fail_prepare(
                        &prepare_ui,
                        &prepared_doc,
                        "The desired state could not be rendered safely.",
                    );
                    return;
                }
            }
            let snapshot = doc.clone();
            let changes = doc.package_intents_for(&account_for_prepare);
            let (tx, rx) = std::sync::mpsc::channel::<PrepareResult>();
            std::thread::spawn(move || {
                let result = prepare_changes(&nixp, &changes, &doc);
                let _ = tx.send(result);
            });
            let result_ui = prepare_ui.clone();
            let result_snapshot = snapshot.clone();
            let result_prepared_doc = prepared_doc.clone();
            let result_package_names = package_names.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                match rx.try_recv() {
                    Ok(result) => {
                        render_prepare_result(
                            result,
                            result_snapshot.clone(),
                            &result_ui,
                            &result_prepared_doc,
                            &result_package_names,
                        );
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        fail_prepare(
                            &result_ui,
                            &result_prepared_doc,
                            "Preparation ended before a result was received.",
                        );
                        glib::ControlFlow::Break
                    }
                }
            });
        });

        // One stable handler dispatches Apply or confirmed rollback according
        // to the current transaction state; it is never reconnected.
        let action_ui = ui.clone();
        let action_desired = desired.clone();
        let action_prepared_doc = self.prepared_doc.clone();
        self.apply_btn.connect_clicked(move |_| {
            let view = updates::view_for(*action_ui.step.borrow(), activation::can_activate());
            if !view.primary_enabled {
                return;
            }
            match view.primary_action {
                updates::PrimaryAction::Apply => start_apply(
                    action_ui.clone(),
                    action_desired.clone(),
                    action_prepared_doc.clone(),
                ),
                updates::PrimaryAction::Rollback => {
                    show_rollback_confirmation(action_ui.clone(), action_desired.clone())
                }
                updates::PrimaryAction::None => {}
            }
        });
    }
}

fn fail_prepare(ui: &TxUi, prepared_doc: &RefCell<Option<DesiredDoc>>, message: &str) {
    *prepared_doc.borrow_mut() = None;
    if ui.transition(TxEvent::BuildFailed).is_some() {
        ui.results_label.set_text(message);
    } else {
        ui.results_label
            .set_text("The preparation result did not match the current transaction.");
    }
    ui.render();
}

fn start_apply(
    ui: TxUi,
    desired: Rc<RefCell<DesiredDoc>>,
    prepared_doc: Rc<RefCell<Option<DesiredDoc>>>,
) {
    if !activation::can_activate() {
        ui.render();
        return;
    }
    let doc = desired.borrow().clone();
    if prepared_doc.borrow().as_ref() != Some(&doc) {
        if ui.transition(TxEvent::InputsChanged).is_some() {
            *prepared_doc.borrow_mut() = None;
            ui.results_label
                .set_text("Your changes have changed since preparation. Prepare them again.");
            while let Some(child) = ui.results_list.first_child() {
                ui.results_list.remove(&child);
            }
        }
        ui.render();
        return;
    }
    if ui.transition(TxEvent::Apply).is_none() {
        return;
    }
    *prepared_doc.borrow_mut() = None;
    ui.results_label.set_text("Applying the change…");
    ui.render();
    start_activation(ui, activation::ActivateAction::Switch, doc, 1800);
}

fn show_rollback_confirmation(ui: TxUi, desired: Rc<RefCell<DesiredDoc>>) {
    let Some(parent) = ui
        .primary_btn
        .root()
        .and_then(|root| root.downcast::<gtk4::Window>().ok())
    else {
        return;
    };
    let dialog = gtk4::MessageDialog::builder()
        .transient_for(&parent)
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
    dialog.connect_response(move |dialog, response| {
        dialog.close();
        if response != gtk4::ResponseType::Accept {
            return;
        }
        let view = updates::view_for(*ui.step.borrow(), activation::can_activate());
        if view.primary_action != updates::PrimaryAction::Rollback || !view.primary_enabled {
            return;
        }
        if ui.transition(TxEvent::Rollback).is_none() {
            return;
        }
        ui.results_label.set_text("Restoring the previous system…");
        ui.render();
        start_activation(
            ui.clone(),
            activation::ActivateAction::Rollback,
            desired.borrow().clone(),
            600,
        );
    });
    dialog.present();
}

/// Run activation or rollback off the GTK thread and handle completion on it.
fn start_activation(
    ui: TxUi,
    action: activation::ActivateAction,
    doc: DesiredDoc,
    timeout_secs: u64,
) {
    let (tx, rx) = std::sync::mpsc::channel::<ActivateResult>();
    std::thread::spawn(move || {
        let result =
            activation::activate(action, &doc, std::time::Duration::from_secs(timeout_secs));
        let _ = tx.send(ActivateResult { action, result });
    });

    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        match rx.try_recv() {
            Ok(result) => {
                handle_activate_result(result, &ui);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                handle_activate_result(
                    ActivateResult {
                        action,
                        result: activation::ActivateResult {
                            ok: false,
                            note: "The operation ended before a result was received.".into(),
                            log: "activation worker disconnected before reporting its result"
                                .into(),
                        },
                    },
                    &ui,
                );
                glib::ControlFlow::Break
            }
        }
    });
}

/// Apply operation results through the transaction reducer on the GTK thread.
fn handle_activate_result(ar: ActivateResult, ui: &TxUi) {
    let event = match (ar.action, ar.result.ok) {
        (activation::ActivateAction::Switch, true) => TxEvent::ApplyOk,
        (activation::ActivateAction::Switch, false) => TxEvent::ApplyFailed,
        (activation::ActivateAction::Rollback, true) => TxEvent::RollbackOk,
        (activation::ActivateAction::Rollback, false) => TxEvent::RollbackFailed,
    };
    if ui.transition(event).is_none() {
        ui.results_label
            .set_text("The operation result did not match the current transaction.");
        ui.render();
        return;
    }

    while let Some(child) = ui.results_list.first_child() {
        ui.results_list.remove(&child);
    }
    if !ar.result.log.is_empty() {
        append_log_tail(&ui.results_list, &ar.result.log);
    }
    let note = if ar.action == activation::ActivateAction::Rollback && !ar.result.ok {
        format!("Rollback failed: {}", ar.result.note)
    } else {
        ar.result.note
    };
    ui.results_label.set_text(&note);
    ui.render();
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
