//! App shell: sidebar + stack of surfaces (Store list, detail, Updates). This
//! is the UI composition point; the detail/install flow is in `detail`, the
//! updates page in `updates_page`. Pure ranking lives in crate::search.
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use log::error;

use crate::desired_state::DesiredDoc;
use crate::model::{IndexEntry, PackageNames};
use crate::search::SearchCorpus;
use crate::ui::{detail, global_search, settings_page, store_search, updates_page, your_system};

fn wire_search_shortcut(search: &gtk4::SearchEntry, window: &gtk4::ApplicationWindow) {
    let search = search.clone();
    let keys = gtk4::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if key == gtk4::gdk::Key::k && modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
            search.grab_focus();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);
}

/// Wrap a page root in a vertical scrolled window so content taller than the
/// window (Settings home, Updates with its upkeep cards, the recipe list) is
/// reachable by scrolling rather than clipped off-screen.
fn scrolled_page(child: &impl IsA<gtk4::Widget>) -> gtk4::ScrolledWindow {
    gtk4::ScrolledWindow::builder()
        .child(child)
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build()
}

/// A sidebar nav button that switches the stack and (for Updates) refreshes it.
fn nav_button(
    label: &str,
    page: &'static str,
    stack: &gtk4::Stack,
    on_show: Option<Rc<dyn Fn()>>,
) -> gtk4::Button {
    let b = gtk4::Button::with_label(label);
    b.add_css_class("flat");
    b.add_css_class("nav-item");
    let stack = stack.clone();
    b.connect_clicked(move |_| {
        if let Some(f) = on_show.as_ref() {
            f();
        }
        stack.set_visible_child_name(page);
    });
    b
}

struct HeaderWidgets {
    back: gtk4::Button,
    pending: gtk4::Button,
}

fn build_header(window: &gtk4::ApplicationWindow) -> HeaderWidgets {
    let header = gtk4::HeaderBar::new();
    let back = gtk4::Button::with_label("‹ Back to results");
    back.set_visible(false);
    header.pack_start(&back);

    let pending = gtk4::Button::new();
    pending.add_css_class("flat");
    pending.add_css_class("dim-label");
    pending.set_visible(false);
    header.pack_end(&pending);
    window.set_titlebar(Some(&header));

    HeaderWidgets { back, pending }
}

/// Load persisted state, preserving unreadable files before allowing writes.
///
/// Missing state starts empty and remains writable. A newer-schema file also
/// starts an empty session, but persistence is disabled to avoid overwriting
/// data this version cannot understand. Other load failures start empty only
/// after a backup succeeds; if backup fails, persistence remains disabled.
fn load_desired_state(notice: &gtk4::Label) -> (DesiredDoc, bool) {
    let path = crate::desired_state::default_desired_path();
    let (document, warning, persistence_enabled) = match crate::desired_state::load(&path) {
        Ok(document) => (document, None, true),
        Err(error) if error.is_missing() => (DesiredDoc::current(), None, true),
        Err(error @ crate::desired_state::LoadError::NewerVersion { .. }) => (
            DesiredDoc::current(),
            Some(format!(
                "Saved changes were written by a newer version ({error}). This session starts empty and will not overwrite the saved file."
            )),
            false,
        ),
        Err(error) => match crate::desired_state::preserve_unreadable_file(&path) {
            Ok(backup) => (
                DesiredDoc::current(),
                Some(format!(
                    "Saved changes could not be loaded ({error}). The original was preserved at {}. This session starts with an empty desired state.",
                    backup.display()
                )),
                true,
            ),
            Err(backup_error) => (
                DesiredDoc::current(),
                Some(format!(
                    "Saved changes could not be loaded ({error}), and a backup could not be created ({backup_error}). This session's changes will not be saved, and the original file was left untouched."
                )),
                false,
            ),
        },
    };

    if let Some(warning) = warning {
        notice.set_text(&warning);
        notice.set_visible(true);
    }
    (document, persistence_enabled)
}

/// Create the save callback used by UI edits.
///
/// When persistence is disabled (for example, because a newer file could not
/// be read), the callback is deliberately a no-op so the original file stays
/// untouched. Save failures are logged and surfaced in the state notice.
fn wire_desired_state_persistence(
    desired: &Rc<RefCell<DesiredDoc>>,
    persistence_enabled: bool,
    notice: &gtk4::Label,
) -> Rc<dyn Fn()> {
    let desired = desired.clone();
    let notice = notice.clone();
    let path = crate::desired_state::default_desired_path();

    Rc::new(move || {
        if !persistence_enabled {
            return;
        }
        if let Err(error) = crate::desired_state::save(&desired.borrow(), &path) {
            error!("could not save desired state: {error}");
            notice.set_text(&format!(
                "Your changes could not be saved and will be lost on restart ({error})."
            ));
            notice.set_visible(true);
        }
    })
}

fn update_pending_button(
    document: &DesiredDoc,
    account: &str,
    package_names: &PackageNames,
    button: &gtk4::Button,
) {
    let intents = document.package_intents_for(account);
    if intents.is_empty() {
        button.set_visible(false);
        button.set_label("");
        return;
    }

    let suffix = if intents.len() == 1 { "" } else { "s" };
    button.set_label(&format!("{} change{suffix} ready to apply", intents.len()));
    let items = intents
        .iter()
        .map(|intent| {
            format!(
                "- {} ({})",
                package_names.display_name(&intent.attr),
                match intent.scope {
                    crate::desired_state::PackageScope::System => "system",
                    crate::desired_state::PackageScope::User => "user",
                }
            )
        })
        .collect::<Vec<_>>();
    button.set_tooltip_text(Some(&items.join("\n")));
    button.set_visible(true);
}

fn wire_pending_button(
    button: &gtk4::Button,
    stack: &gtk4::Stack,
    updates: &Rc<updates_page::UpdatesPage>,
    desired: &Rc<RefCell<DesiredDoc>>,
    account: &str,
) {
    let stack = stack.clone();
    let updates = updates.clone();
    let desired = desired.clone();
    let account = account.to_string();
    button.connect_clicked(move |_| {
        updates.refresh(&desired, &account);
        stack.set_visible_child_name("updates");
    });
}

fn build_sidebar(
    stack: &gtk4::Stack,
    updates: &Rc<updates_page::UpdatesPage>,
    your_system: &Rc<your_system::YourSystemPage>,
    desired: &Rc<RefCell<DesiredDoc>>,
    account: &str,
    on_package_intent_change: &Rc<dyn Fn()>,
    global_search: &gtk4::SearchEntry,
) -> gtk4::Box {
    let sidebar = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    sidebar.set_width_request(220);
    // The app-wide search sits atop the sidebar, above the surface nav; it
    // spans apps + settings, so it cannot live inside one surface.
    let search_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    search_box.set_margin_top(8);
    search_box.set_margin_bottom(4);
    search_box.set_margin_start(8);
    search_box.set_margin_end(8);
    global_search.set_placeholder_text(Some("Search packages and settings…"));
    global_search.set_tooltip_text(Some("Searches packages and settings everywhere"));
    search_box.append(global_search);
    sidebar.append(&search_box);
    sidebar.append(&nav_button("Packages", "list", stack, None));
    sidebar.append(&nav_button("Settings", "settings", stack, None));

    let updates = updates.clone();
    let updates_desired = desired.clone();
    let updates_account = account.to_string();
    let refresh_updates = Rc::new(move || updates.refresh(&updates_desired, &updates_account));
    sidebar.append(&nav_button(
        "Updates",
        "updates",
        stack,
        Some(refresh_updates),
    ));

    // Your system: refresh the recipe list each time it is shown so removals
    // made elsewhere (Store, Settings) show up here.
    let your_system = your_system.clone();
    let desired = desired.clone();
    let on_change = on_package_intent_change.clone();
    let refresh_your_system = Rc::new(move || your_system.refresh(&desired, &on_change));
    sidebar.append(&nav_button(
        "Your system",
        "your-system",
        stack,
        Some(refresh_your_system),
    ));
    sidebar
}

/// Build and present the window for a loaded catalogue.
pub fn run(
    app: &gtk4::Application,
    entries: Vec<IndexEntry>,
    load_error: Option<String>,
    nix: Option<std::path::PathBuf>,
) {
    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("nix-pilot")
        .default_width(1180)
        .default_height(840)
        .build();
    let header = build_header(&window);
    let catalogue = store_search::StoreSearch::new();
    let global = global_search::GlobalSearch::new();
    let full_entries = entries;
    let package_names = Rc::new(PackageNames::from_entries(&full_entries));
    let corpus = Rc::new(RefCell::new(SearchCorpus::new(&full_entries, false)));

    let account = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let (initial_document, persistence_enabled) = load_desired_state(&catalogue.state_notice);
    let desired = Rc::new(RefCell::new(initial_document));

    let detail = Rc::new(detail::DetailUi::new(
        "Installs into your user account — no administrator rights needed",
    ));
    let updates = Rc::new(updates_page::UpdatesPage::new(package_names.clone()));
    let settings = Rc::new(settings_page::SettingsPage::new(&account));
    let your_system = Rc::new(your_system::YourSystemPage::new(
        &account,
        package_names.clone(),
    ));

    let persist_desired_state =
        wire_desired_state_persistence(&desired, persistence_enabled, &catalogue.state_notice);
    persist_desired_state();
    updates.wire(&desired, &account, &nix, persist_desired_state.clone());
    settings.wire(&desired, persist_desired_state.clone());

    let pending = header.pending.clone();
    let desired_for_label = desired.clone();
    let account_for_label = account.clone();
    let package_names_for_label = package_names.clone();
    let update_pending = Rc::new(move || {
        update_pending_button(
            &desired_for_label.borrow(),
            &account_for_label,
            &package_names_for_label,
            &pending,
        );
    });
    let on_package_intent_change: Rc<dyn Fn()> = {
        let update_pending = update_pending.clone();
        let persist_desired_state = persist_desired_state.clone();
        Rc::new(move || {
            update_pending();
            persist_desired_state();
        })
    };
    let on_package_intent_change_for_sidebar = on_package_intent_change.clone();
    detail::wire(
        &detail,
        &desired,
        &account,
        &nix,
        &window,
        on_package_intent_change,
    );
    update_pending();

    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    stack.add_named(&catalogue.page, Some("list"));
    stack.add_named(&detail.page, Some("detail"));
    stack.add_named(&scrolled_page(&global.page), Some("global-search"));
    // Tall surfaces scroll; a plain Box would push content below the window
    // edge with no way to reach it. Wrap them so everything is shown.
    stack.add_named(&scrolled_page(&updates.page), Some("updates"));
    stack.add_named(&scrolled_page(&settings.page), Some("settings"));
    stack.add_named(&scrolled_page(&your_system.page), Some("your-system"));
    stack.set_visible_child_name("list");

    // Load current config values when Settings page becomes visible (lazy, non-blocking)
    let settings_for_load = settings.clone();
    stack.connect_visible_child_name_notify(move |s| {
        if s.visible_child_name().as_deref() == Some("settings") {
            settings_for_load.load_current_values();
        }
    });

    let sidebar = build_sidebar(
        &stack,
        &updates,
        &your_system,
        &desired,
        &account,
        &on_package_intent_change_for_sidebar,
        &global.entry,
    );
    let body = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    body.append(&sidebar);
    body.append(&stack);
    window.set_child(Some(&body));

    wire_pending_button(&header.pending, &stack, &updates, &desired, &account);
    let back_button = header.back.clone();
    stack.connect_visible_child_name_notify(move |s| {
        back_button.set_visible(s.visible_child_name().as_deref() == Some("detail"));
    });
    let stack_for_back = stack.clone();
    header
        .back
        .connect_clicked(move |_| stack_for_back.set_visible_child_name("list"));

    catalogue.set_initial_summary(&corpus.borrow(), load_error.as_deref());
    catalogue.render(&corpus.borrow(), &catalogue.search.text());
    let stack_for_store_query = stack.clone();
    let on_store_query: Rc<dyn Fn()> = Rc::new(move || {
        stack_for_store_query.set_visible_child_name("list");
    });
    let detail_for_store = detail.clone();
    let desired_for_store = desired.clone();
    let account_for_store = account.clone();
    let stack_for_store = stack.clone();
    let on_store_package: Rc<dyn Fn(IndexEntry)> = Rc::new(move |entry| {
        detail_for_store.populate(&entry);
        detail_for_store.sync_added(&desired_for_store.borrow(), &account_for_store);
        stack_for_store.set_visible_child_name("detail");
    });
    let full_entries_for_filter = full_entries.clone();
    let corpus_for_filter = corpus.clone();
    let catalogue_for_filter = catalogue.clone_handles();
    let search_for_filter = catalogue_for_filter.search.clone();
    let on_filter: Rc<dyn Fn(bool)> = Rc::new(move |hide_unfree| {
        let next = SearchCorpus::new(&full_entries_for_filter, hide_unfree);
        *corpus_for_filter.borrow_mut() = next;
        catalogue_for_filter.refresh(&corpus_for_filter.borrow(), &search_for_filter.text());
    });
    catalogue.wire(&corpus, on_store_query, on_filter, on_store_package);

    wire_search_shortcut(&global.entry, &window);
    let stack_for_global_query = stack.clone();
    let on_global_query: Rc<dyn Fn()> = Rc::new(move || {
        stack_for_global_query.set_visible_child_name("global-search");
    });
    let stack_for_escape = stack.clone();
    let on_return_to_store: Rc<dyn Fn()> = Rc::new(move || {
        stack_for_escape.set_visible_child_name("list");
    });
    let detail_for_global = detail.clone();
    let desired_for_global = desired.clone();
    let account_for_global = account.clone();
    let settings_for_global = settings.clone();
    let stack_for_global = stack.clone();
    let on_global_selection: Rc<dyn Fn(global_search::GlobalSelection)> =
        Rc::new(move |target| match target {
            global_search::GlobalSelection::Package(entry) => {
                detail_for_global.populate(&entry);
                detail_for_global.sync_added(&desired_for_global.borrow(), &account_for_global);
                stack_for_global.set_visible_child_name("detail");
            }
            global_search::GlobalSelection::Setting(spec) => {
                settings_for_global.show_category(spec.category);
                stack_for_global.set_visible_child_name("settings");
            }
        });
    global.wire(
        &corpus,
        on_global_query,
        on_return_to_store,
        on_global_selection,
    );
    window.present();
}
