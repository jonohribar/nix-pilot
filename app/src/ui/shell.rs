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
use crate::ui::{
    detail, global_search, settings_page, setup_page, store_search, updates_page, your_system,
};

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

/// Scoped styles for the sidebar nav, loaded once for the display. Scoped to
/// `.np-sidebar` so the bespoke classes below do not fight libadwaita's hull
/// and only touch the sidebar. The app otherwise loads no CSS.
const NP_CSS: &str = "
.np-sidebar .nav-item {
  margin: 1px 6px;
  padding: 6px 8px;
  border-radius: 8px;
}
.np-sidebar .nav-item.indented {
  margin-left: 14px;
}
.np-sidebar .nav-item.active {
  background-color: alpha(@accent_bg_color, 0.25);
  color: @accent_color;
  font-weight: 600;
}
.np-sidebar .nav-section {
  margin: 12px 8px 2px 10px;
}
";

fn install_np_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(NP_CSS);
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// The single source of truth for which sidebar row is selected. Every nav
/// entry goes through NavSelection::mark, so a sidebar click and a
/// global-search deep-link keep one row highlighted in lockstep.
#[derive(Clone)]
struct NavSelection {
    active: std::rc::Rc<std::cell::RefCell<Option<gtk4::Button>>>,
}

impl NavSelection {
    fn new() -> Self {
        Self {
            active: std::rc::Rc::new(std::cell::RefCell::new(None)),
        }
    }

    fn mark(&self, button: &gtk4::Button) {
        if let Some(prev) = self.active.borrow_mut().take() {
            prev.remove_css_class("active");
        }
        *self.active.borrow_mut() = Some(button.clone());
        button.add_css_class("active");
    }
}

/// One sidebar nav row, built uniformly so every entry shares the same
/// formatting and a single style change applies to them all. `indented`
/// marks the settings category sub-rows; `marking` drives the selected
/// state. `trailing` adds an end-aligned dim label (the pending-change
/// count on the Updates row) and returns it so the caller can refresh it.
fn nav_item(
    label: &str,
    indented: bool,
    trailing: Option<&str>,
    marking: &NavSelection,
    on_click: impl Fn(&gtk4::Button) + 'static,
) -> (gtk4::Button, Option<gtk4::Label>) {
    let b = gtk4::Button::new();
    b.add_css_class("flat");
    b.add_css_class("nav-item");
    if indented {
        b.add_css_class("indented");
    }
    b.set_halign(gtk4::Align::Fill);
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    let l = gtk4::Label::new(Some(label));
    l.set_halign(gtk4::Align::Start);
    l.set_xalign(0.0);
    l.set_hexpand(true);
    row.append(&l);
    let trailing_label = trailing.map(|text| {
        let t = gtk4::Label::new(Some(text));
        t.add_css_class("dim-label");
        t.set_margin_start(6);
        t.set_halign(gtk4::Align::End);
        t.set_xalign(1.0);
        t.set_tooltip_text(Some("Changes waiting to be applied"));
        row.append(&t);
        t
    });
    b.set_child(Some(&row));
    let marking = marking.clone();
    b.connect_clicked(move |b| {
        marking.mark(b);
        on_click(b);
    });
    (b, trailing_label)
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
    // Packages and settings share one pending count (mirror-reality: the
    // stored settings map holds exactly the user's changes).
    let total = document.pending_change_count(account);
    if total == 0 {
        button.set_visible(false);
        button.set_label("");
        return;
    }

    let suffix = if total == 1 { "" } else { "s" };
    button.set_label(&format!("{total} change{suffix} ready to apply"));
    let mut items: Vec<String> = document
        .package_intents_for(account)
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
        .collect();
    let unlisted_settings = document.settings.len();
    if unlisted_settings > 0 {
        let s = if unlisted_settings == 1 { "" } else { "s" };
        items.push(format!(
            "- {unlisted_settings} setting change{s} in Settings"
        ));
    }
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

/// Late-bound handle to the sidebar's pending-count setter, filled once the
/// sidebar exists so the count can be refreshed from before-build closures.
type PendingCountSetter = std::rc::Rc<std::cell::RefCell<Option<Rc<dyn Fn(usize)>>>>;

/// What `build_sidebar` returns: the sidebar box, a shared category-open
/// callback (used by both the sidebar and global search so the highlighted
/// row stays in sync), and a setter for the Updates row's pending-count badge.
struct Sidebar {
    sidebar: gtk4::Box,
    select_category: Rc<dyn Fn(&str)>,
    set_pending_count: Rc<dyn Fn(usize)>,
}

#[allow(clippy::too_many_arguments)]
fn build_sidebar(
    stack: &gtk4::Stack,
    settings: &Rc<settings_page::SettingsPage>,
    updates: &Rc<updates_page::UpdatesPage>,
    your_system: &Rc<your_system::YourSystemPage>,
    desired: &Rc<RefCell<DesiredDoc>>,
    account: &str,
    on_package_intent_change: &Rc<dyn Fn()>,
    global_search: &gtk4::SearchEntry,
) -> Sidebar {
    let sidebar = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    sidebar.set_width_request(240);
    sidebar.add_css_class("np-sidebar");
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

    // One selection drives every row: a sidebar click or a global-search
    // deep-link highlights one item in lockstep.
    let selection = NavSelection::new();
    let stack_for_packages = stack.clone();
    let (packages, _) = nav_item("Packages", false, None, &selection, move |_| {
        stack_for_packages.set_visible_child_name("list");
    });
    selection.mark(&packages);
    sidebar.append(&packages);

    // Settings is a two-column router: the category list always lives in the
    // sidebar (under a section header) and selects the category page shown on
    // the right, so there is no drill-in or hidden second level.
    let settings_head = gtk4::Label::new(Some("Settings"));
    settings_head.add_css_class("dim-label");
    settings_head.add_css_class("nav-section");
    settings_head.set_xalign(0.0);
    sidebar.append(&settings_head);

    // Category rows, keyed by child id so a global-search deep-link can mark
    // the same row active that a sidebar click would.
    let mut category_buttons: std::collections::BTreeMap<&'static str, gtk4::Button> =
        std::collections::BTreeMap::new();
    for (label, _summary, child) in crate::settings::SETTINGS_CATEGORIES {
        let stack = stack.clone();
        let settings = settings.clone();
        let child_id = child.to_string();
        let (button, _) = nav_item(label, true, None, &selection, move |_| {
            settings.show_category(&child_id);
            stack.set_visible_child_name("settings");
        });
        category_buttons.insert(child, button.clone());
        sidebar.append(&button);
    }

    let updates = updates.clone();
    let updates_desired = desired.clone();
    let updates_account = account.to_string();
    let refresh_updates = Rc::new(move || updates.refresh(&updates_desired, &updates_account));
    let stack_for_updates = stack.clone();
    let (updates_button, updates_count) =
        nav_item("Updates", false, Some(""), &selection, move |_| {
            refresh_updates();
            stack_for_updates.set_visible_child_name("updates");
        });
    sidebar.append(&updates_button);
    // The always-visible pending badge on the Updates row: shows “· N” while
    // changes wait, hidden at zero so “no changes” reads as done.
    let set_pending_count: Rc<dyn Fn(usize)> = {
        let count_label = updates_count.expect("the Updates row carries a pending-count badge");
        Rc::new(move |n| {
            if n > 0 {
                count_label.set_text(&format!("· {n}"));
                count_label.set_visible(true);
            } else {
                count_label.set_text("");
                count_label.set_visible(false);
            }
        })
    };

    // Your system: refresh the recipe list each time it is shown so removals
    // made elsewhere (Store, Settings) show up here.
    let your_system = your_system.clone();
    let desired = desired.clone();
    let on_change = on_package_intent_change.clone();
    let refresh_your_system = Rc::new(move || your_system.refresh(&desired, &on_change));
    let stack_for_system = stack.clone();
    let (your_system_button, _) = nav_item("Your system", false, None, &selection, move |_| {
        refresh_your_system();
        stack_for_system.set_visible_child_name("your-system");
    });
    sidebar.append(&your_system_button);

    // Shared entry point for opening a settings category from anywhere (nav
    // or search), keeping the highlighted row in sync.
    let select_category = {
        let selection = selection.clone();
        let settings = settings.clone();
        let stack = stack.clone();
        Rc::new(move |child_id: &str| {
            if let Some(button) = category_buttons.get(child_id) {
                selection.mark(button);
            }
            settings.show_category(child_id);
            stack.set_visible_child_name("settings");
        })
    };
    Sidebar {
        sidebar,
        select_category,
        set_pending_count,
    }
}

/// Build and present the window for a loaded catalogue.
/// The Store surface's callbacks: return-to-list, unfree-filter, and open-a-
/// package. Built here so `run` stays a thin composition and each surface's
/// wiring is one isolated function.
type StoreCallbacks = (Rc<dyn Fn()>, Rc<dyn Fn(bool)>, Rc<dyn Fn(IndexEntry)>);

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn wire_store_callbacks(
    stack: &gtk4::Stack,
    detail: &Rc<detail::DetailUi>,
    desired: &Rc<RefCell<DesiredDoc>>,
    account: &str,
    nix: &Option<std::path::PathBuf>,
    catalogue: &store_search::StoreSearch,
    corpus: &Rc<RefCell<SearchCorpus>>,
    full_entries: &[IndexEntry],
) -> StoreCallbacks {
    let stack_for_store_query = stack.clone();
    let on_store_query: Rc<dyn Fn()> = Rc::new(move || {
        stack_for_store_query.set_visible_child_name("list");
    });
    let detail_for_store = detail.clone();
    let desired_for_store = desired.clone();
    let account_for_store = account.to_string();
    let stack_for_store = stack.clone();
    let nix_for_store = nix.clone();
    let on_store_package: Rc<dyn Fn(IndexEntry)> = Rc::new(move |entry| {
        detail_for_store.populate(&entry);
        detail_for_store.sync_added(&desired_for_store.borrow(), &account_for_store);
        let allow_unfree = crate::settings::allow_unfree(&desired_for_store.borrow().settings);
        detail_for_store.auto_check(&nix_for_store, allow_unfree);
        stack_for_store.set_visible_child_name("detail");
    });
    let full_entries_for_filter = full_entries.to_vec();
    let corpus_for_filter = corpus.clone();
    let catalogue_for_filter = catalogue.clone_handles();
    let search_for_filter = catalogue_for_filter.search.clone();
    let on_filter: Rc<dyn Fn(bool)> = Rc::new(move |hide_unfree| {
        let next = SearchCorpus::new(&full_entries_for_filter, hide_unfree);
        *corpus_for_filter.borrow_mut() = next;
        catalogue_for_filter.refresh(&corpus_for_filter.borrow(), &search_for_filter.text());
    });
    (on_store_query, on_filter, on_store_package)
}

/// The app-wide search's callbacks: show the results page, return to the
/// store, and handle a package/setting selection.
type GlobalCallbacks = (
    Rc<dyn Fn()>,
    Rc<dyn Fn()>,
    Rc<dyn Fn(global_search::GlobalSelection)>,
);

#[allow(clippy::type_complexity)]
fn wire_global_callbacks(
    stack: &gtk4::Stack,
    detail: &Rc<detail::DetailUi>,
    desired: &Rc<RefCell<DesiredDoc>>,
    account: &str,
    nix: &Option<std::path::PathBuf>,
    select_category: &Rc<dyn Fn(&str)>,
) -> GlobalCallbacks {
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
    let account_for_global = account.to_string();
    let stack_for_global = stack.clone();
    let select_category_for_global = select_category.clone();
    let nix_for_global = nix.clone();
    let on_global_selection: Rc<dyn Fn(global_search::GlobalSelection)> =
        Rc::new(move |target| match target {
            global_search::GlobalSelection::Package(entry) => {
                detail_for_global.populate(&entry);
                detail_for_global.sync_added(&desired_for_global.borrow(), &account_for_global);
                let allow_unfree =
                    crate::settings::allow_unfree(&desired_for_global.borrow().settings);
                detail_for_global.auto_check(&nix_for_global, allow_unfree);
                stack_for_global.set_visible_child_name("detail");
            }
            global_search::GlobalSelection::Setting(spec) => {
                select_category_for_global(spec.category);
            }
        });
    (on_global_query, on_return_to_store, on_global_selection)
}

/// Shared handles for the shell surfaces. Built once in `run`; each `wire_*`
/// function owns one surface's wiring and reads what it needs from this
/// context, so the composition stays a thin ordered list.
struct ShellContext {
    window: gtk4::ApplicationWindow,
    header: HeaderWidgets,
    stack: gtk4::Stack,
    desired: Rc<RefCell<DesiredDoc>>,
    nix: Option<std::path::PathBuf>,
    account: String,
    detail: Rc<detail::DetailUi>,
    catalogue: store_search::StoreSearch,
    global: global_search::GlobalSearch,
    updates: Rc<updates_page::UpdatesPage>,
    settings: Rc<settings_page::SettingsPage>,
    your_system: Rc<your_system::YourSystemPage>,
    corpus: Rc<RefCell<SearchCorpus>>,
    select_category: Rc<dyn Fn(&str)>,
    on_package_intent_change: Rc<dyn Fn()>,
}

pub fn run(
    app: &gtk4::Application,
    entries: Vec<IndexEntry>,
    load_error: Option<String>,
    nix: Option<std::path::PathBuf>,
) {
    install_np_css();
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
    // The Integration card reopens the first-run setup page; the stack is
    // created below, so the shell fills this handle once it exists.
    let stack_handle: Rc<RefCell<Option<gtk4::Stack>>> = Rc::new(RefCell::new(None));
    let open_setup: Rc<dyn Fn()> = {
        let stack_handle = stack_handle.clone();
        Rc::new(move || {
            if let Some(stack) = stack_handle.borrow().as_ref() {
                stack.set_visible_child_name("setup");
            }
        })
    };
    let settings = Rc::new(settings_page::SettingsPage::new(&account, open_setup));
    let your_system = Rc::new(your_system::YourSystemPage::new(
        &account,
        package_names.clone(),
    ));

    let persist_desired_state =
        wire_desired_state_persistence(&desired, persistence_enabled, &catalogue.state_notice);
    persist_desired_state();

    // One shared pending count drives the header button and the sidebar badge;
    // the sidebar's count setter is filled in once the sidebar exists.
    let pending = header.pending.clone();
    let desired_for_label = desired.clone();
    let account_for_label = account.clone();
    let package_names_for_label = package_names.clone();
    let pending_setter: PendingCountSetter = std::rc::Rc::new(std::cell::RefCell::new(None));
    let pending_setter_handle = pending_setter.clone();
    let refresh_pending = Rc::new(move || {
        let document = desired_for_label.borrow();
        let count = document.pending_change_count(&account_for_label);
        update_pending_button(
            &document,
            &account_for_label,
            &package_names_for_label,
            &pending,
        );
        if let Some(set_count) = pending_setter_handle.borrow().as_ref() {
            set_count(count);
        }
    });
    let on_package_intent_change: Rc<dyn Fn()> = {
        let refresh_pending = refresh_pending.clone();
        let persist_desired_state = persist_desired_state.clone();
        Rc::new(move || {
            refresh_pending();
            persist_desired_state();
        })
    };
    let on_package_intent_change_for_sidebar = on_package_intent_change.clone();
    updates.wire(&desired, &account, &nix, persist_desired_state.clone());
    // Settings edits refresh the pending counts too, so the header and sidebar
    // badge stay in step with settings changes, not just package changes.
    let settings_save = {
        let persist_desired_state = persist_desired_state.clone();
        let refresh_pending = refresh_pending.clone();
        Rc::new(move || {
            persist_desired_state();
            refresh_pending();
        })
    };
    settings.wire(&desired, settings_save);
    detail::wire(
        &detail,
        &desired,
        &account,
        &nix,
        &window,
        on_package_intent_change.clone(),
    );
    refresh_pending();

    let stack = gtk4::Stack::new();
    *stack_handle.borrow_mut() = Some(stack.clone());
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    stack.add_named(&catalogue.page, Some("list")); // the store list scrolls internally
    stack.add_named(&scrolled_page(&detail.page), Some("detail"));
    stack.add_named(&scrolled_page(&global.page), Some("global-search"));
    // Tall surfaces scroll; a plain Box would push content below the window
    // edge with no way to reach it. Wrap them so everything is shown.
    stack.add_named(&scrolled_page(&updates.page), Some("updates"));
    stack.add_named(&scrolled_page(&settings.page), Some("settings"));
    stack.add_named(&scrolled_page(&your_system.page), Some("your-system"));
    // First-run: until the user's config references nix-pilot, the setup
    // page is the initial surface. Demo/screenshot mode always starts in the
    // store (dev hook; the harness has no integrated config to detect).
    let setup = Rc::new(setup_page::SetupPage::new(&window, &stack));
    stack.add_named(&setup.page, Some("setup"));
    {
        // Re-run detection whenever the setup page is shown, so reopening it
        // from Settings reflects the current integration state.
        let setup_for_refresh = setup.clone();
        stack.connect_visible_child_name_notify(move |s| {
            if s.visible_child_name().as_deref() == Some("setup") {
                setup_for_refresh.refresh();
            }
        });
    }
    let setup_skipped = std::env::var("NIX_PILOT_SKIP_SETUP").as_deref() == Ok("1")
        || std::env::var("NIX_PILOT_DEMO").as_deref() == Ok("1");
    let root = crate::setup::resolve_root();
    let integrated = root.as_ref().is_some_and(crate::setup::is_integrated);
    if !integrated && !setup_skipped {
        // The snippet imports ./nix-pilot/module.nix and state.nix; write
        // the app-owned files (and track them once) so the user's first
        // rebuild after pasting has real files to import. Best-effort: a
        // failure is logged and surfaces at verify/apply time.
        if let Some(root) = root.as_ref() {
            if let Err(error) = crate::setup::write_app_files(root, &desired.borrow()) {
                log::warn!("could not write the nix-pilot files into your configuration: {error}");
            }
            if let Err(error) = crate::system::state_file::git_add_once(&root.path) {
                log::warn!(
                    "could not track the nix-pilot files in your configuration's git repository: \
                     {error}"
                );
            }
        }
    }
    stack.set_visible_child_name(if integrated || setup_skipped {
        "list"
    } else {
        "setup"
    });

    let sidebar = build_sidebar(
        &stack,
        &settings,
        &updates,
        &your_system,
        &desired,
        &account,
        &on_package_intent_change_for_sidebar,
        &global.entry,
    );
    *pending_setter.borrow_mut() = Some(sidebar.set_pending_count.clone());
    refresh_pending();

    let ctx = ShellContext {
        window: window.clone(),
        header,
        stack: stack.clone(),
        desired,
        nix,
        account,
        detail,
        catalogue,
        global,
        updates,
        settings,
        your_system,
        corpus,
        select_category: sidebar.select_category.clone(),
        on_package_intent_change,
    };

    wire_settings_live_refresh(&ctx);
    wire_dev_hooks(&ctx);

    let body = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    body.append(&sidebar.sidebar);
    body.append(&stack);
    window.set_child(Some(&body));

    wire_pending(&ctx);
    wire_back(&ctx);
    wire_store(&ctx, load_error.as_deref(), &full_entries);
    wire_global_search(&ctx);
    window.present();
}

/// Refresh current config values whenever the Settings page becomes visible.
fn wire_settings_live_refresh(ctx: &ShellContext) {
    let settings_for_load = ctx.settings.clone();
    let desired_for_values = ctx.desired.clone();
    ctx.stack.connect_visible_child_name_notify(move |s| {
        if s.visible_child_name().as_deref() == Some("settings") {
            settings_for_load.load_current_values(&desired_for_values);
        }
    });
}

/// Dev/test demo seed and deep-link (screenshot and AT-SPI drives; never in
/// the core path).
fn wire_dev_hooks(ctx: &ShellContext) {
    super::geom_dump::wire(&ctx.window, &ctx.stack);
    if std::env::var("NIX_PILOT_DEMO").as_deref() == Ok("1") {
        ctx.desired.borrow_mut().settings.insert(
            "services.pipewire.enable".into(),
            crate::desired_state::SettingValue::Bool(true),
        );
        ctx.desired.borrow_mut().settings.insert(
            "time.timeZone".into(),
            crate::desired_state::SettingValue::Str("Europe/Berlin".into()),
        );
        ctx.desired.borrow_mut().settings.insert(
            "nix.gc.options".into(),
            crate::desired_state::SettingValue::Str("--delete-older-than 7d".into()),
        );
    }

    match std::env::var("NIX_PILOT_GO").as_deref() {
        Ok("setup") => ctx.stack.set_visible_child_name("setup"),
        Ok("settings") => {
            let first = crate::settings::SETTINGS_CATEGORIES
                .first()
                .map(|t| t.2)
                .unwrap_or("system");
            (ctx.select_category)(first);
        }
        Ok("updates") => ctx.stack.set_visible_child_name("updates"),
        Ok("your-system") => {
            ctx.your_system
                .refresh(&ctx.desired, &ctx.on_package_intent_change);
            ctx.stack.set_visible_child_name("your-system");
        }
        _ => {}
    }
}

/// The header pending button: review the waiting changes on the Updates page.
fn wire_pending(ctx: &ShellContext) {
    wire_pending_button(
        &ctx.header.pending,
        &ctx.stack,
        &ctx.updates,
        &ctx.desired,
        &ctx.account,
    );
}

/// The header back button, visible only on the detail page.
fn wire_back(ctx: &ShellContext) {
    let back_button = ctx.header.back.clone();
    let stack_for_back = ctx.stack.clone();
    ctx.stack.connect_visible_child_name_notify(move |s| {
        back_button.set_visible(s.visible_child_name().as_deref() == Some("detail"));
    });
    ctx.header
        .back
        .connect_clicked(move |_| stack_for_back.set_visible_child_name("list"));
}

/// The store list: initial summary + render + search/filter/open callbacks.
fn wire_store(ctx: &ShellContext, load_error: Option<&str>, full_entries: &[IndexEntry]) {
    ctx.catalogue
        .set_initial_summary(&ctx.corpus.borrow(), load_error);
    ctx.catalogue
        .render(&ctx.corpus.borrow(), &ctx.catalogue.search.text());
    let (on_store_query, on_filter, on_store_package) = wire_store_callbacks(
        &ctx.stack,
        &ctx.detail,
        &ctx.desired,
        &ctx.account,
        &ctx.nix,
        &ctx.catalogue,
        &ctx.corpus,
        full_entries,
    );
    ctx.catalogue
        .wire(&ctx.corpus, on_store_query, on_filter, on_store_package);
}

/// The app-wide search: shortcut, results page, and selection routing (with
/// settings deep-links keeping the sidebar highlight in sync).
fn wire_global_search(ctx: &ShellContext) {
    wire_search_shortcut(&ctx.global.entry, &ctx.window);
    let (on_global_query, on_return_to_store, on_global_selection) = wire_global_callbacks(
        &ctx.stack,
        &ctx.detail,
        &ctx.desired,
        &ctx.account,
        &ctx.nix,
        &ctx.select_category,
    );
    ctx.global.wire(
        &ctx.corpus,
        on_global_query,
        on_return_to_store,
        on_global_selection,
    );
}
