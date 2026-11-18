// Navigable wireframe prototype: the page .blp files (compiled to .ui) become
// Gtk.Stack pages inside one shell with a sidebar, so navigation is clickable:
//   sidebar -> top-level pages; app card -> detail; store search -> results; back.
//
// Search is store-local: the SearchEntry lives INSIDE the store page (home +
// results), not in the header — the header global search is a separate surface.
//
// Screenshot-driven rendering: --page <home|search|detail|settings|updates>
// sets the initial page, --search <text> pre-fills the store search, and
// --detail <name> sets the detail page title (for captures).
use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    let i = a.iter().position(|x| x == name)?;
    a.get(i + 1).cloned()
}

fn load_box(build_dir: &str, file: &str, id: &str) -> gtk4::Box {
    let b = gtk4::Builder::from_file(format!("{}/{}", build_dir, file));
    b.object(id).unwrap_or_else(|| panic!("missing {id} in {file}"))
}

// Store-local search: any non-empty query shows the grouped results page.
fn wire_store_search(entry: &gtk4::SearchEntry, stack: &gtk4::Stack, prev: &Rc<RefCell<String>>) {
    let stack = stack.clone();
    let prev = prev.clone();
    entry.connect_search_changed(move |s| {
        let q = s.text();
        if q.is_empty() {
            stack.set_visible_child_name("home");
        } else if stack.visible_child_name().as_deref() != Some("search") {
            *prev.borrow_mut() = stack.visible_child_name().unwrap_or_default().to_string();
            stack.set_visible_child_name("search");
        }
    });
}

fn main() {
    if gtk4::init().is_err() {
        eprintln!("failed to initialise GTK");
        std::process::exit(1);
    }
    if libadwaita::init().is_err() {
        eprintln!("failed to initialise libadwaita");
        std::process::exit(1);
    }

    let build_dir = std::env::var("WIREFRAME_BUILD")
        .unwrap_or_else(|_| "wireframes/build".into());
    let css_path = std::env::var("WIREFRAME_CSS")
        .unwrap_or_else(|_| "wireframes/style.css".into());
    if let Ok(css) = std::fs::read_to_string(&css_path) {
        let provider = gtk4::CssProvider::new();
        provider.load_from_data(&css);
        if let Some(d) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(&d, &provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
    }

    let initial = arg("--page").unwrap_or_else(|| "home".into());
    let search_text = arg("--search").unwrap_or_default();
    let detail_name = arg("--detail").unwrap_or_default();

    // ---- window chrome (no global search here — that is a separate surface) ----
    let window = gtk4::Window::builder()
        .title("nix-pilot — wireframe prototype")
        .default_width(1180)
        .default_height(840)
        .build();

    let header = gtk4::HeaderBar::new();
    let back = gtk4::Button::from_icon_name("go-previous-symbolic");
    back.add_css_class("flat");
    back.set_tooltip_text(Some("Back"));
    let title = gtk4::Label::new(Some("nix-pilot"));
    title.add_css_class("title");
    header.pack_start(&back);
    header.set_title_widget(Some(&title));
    window.set_titlebar(Some(&header));

    // ---- pages ------------------------------------------------------------
    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);

    // home + its store-local search come from one builder (so we wire the entry)
    let home_b = gtk4::Builder::from_file(format!("{}/page-home.ui", build_dir));
    let page_home: gtk4::Box = home_b.object("page_home").expect("page_home");
    let store_search_home: gtk4::SearchEntry = home_b.object("store_search").expect("store_search in page-home");

    let search_b = gtk4::Builder::from_file(format!("{}/page-search.ui", build_dir));
    let page_search: gtk4::Box = search_b.object("page_search").expect("page_search");
    let store_search_results: gtk4::SearchEntry = search_b.object("store_search").expect("store_search in page-search");

    let page_detail = load_box(&build_dir, "page-detail.ui", "page_detail");
    let page_settings = load_box(&build_dir, "page-settings.ui", "page_settings");
    let page_updates = load_box(&build_dir, "page-updates.ui", "page_updates");

    stack.add_named(&page_home, Some("home"));
    stack.add_named(&page_search, Some("search"));
    stack.add_named(&page_detail, Some("detail"));
    stack.add_named(&page_settings, Some("settings"));
    stack.add_named(&page_updates, Some("updates"));
    stack.set_vexpand(true);
    stack.set_hexpand(true);

    // ---- sidebar -----------------------------------------------------------
    let sidebar = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    sidebar.add_css_class("nav");
    sidebar.set_width_request(236);
    let nav = |label: &str, page: &'static str, active: bool, stack: &gtk4::Stack| {
        let b = gtk4::Button::with_label(label);
        b.add_css_class("flat");
        b.add_css_class("nav-item");
        if active {
            b.add_css_class("active");
        }
        let stack = stack.clone();
        b.connect_clicked(move |_| {
            stack.set_visible_child_name(page);
        });
        b
    };
    let nb = gtk4::Label::new(Some("CATEGORIES"));
    nb.add_css_class("nav-title");
    nb.set_margin_top(14);
    nb.set_margin_start(14);
    nb.set_margin_bottom(4);
    sidebar.append(&nb);
    sidebar.append(&nav("Home", "home", initial == "home", &stack));
    sidebar.append(&nav("Applications", "home", false, &stack));
    sidebar.append(&nav("Unverified", "home", false, &stack));
    sidebar.append(&nav("Terminal apps", "home", false, &stack));
    sidebar.append(&nav("Command-line tools", "home", false, &stack));
    let sep = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    sep.set_margin_top(10);
    sep.set_margin_bottom(10);
    sidebar.append(&sep);
    sidebar.append(&nav("Updates", "updates", false, &stack));
    sidebar.append(&nav("Settings", "settings", false, &stack));

    let body = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    body.append(&sidebar);
    body.append(&stack);

    // ---- prev-page tracking for Back ---------------------------------------
    let prev: Rc<RefCell<String>> = Rc::new(RefCell::new("home".to_string()));

    // ---- app cards -> detail ------------------------------------------------
    let detail_title: gtk4::Label = {
        let b = gtk4::Builder::from_file(format!("{}/page-detail.ui", build_dir));
        b.object("detail_title").expect("detail_title")
    };
    let detail_title_c = detail_title.clone();
    let stack_c = stack.clone();
    let pc2 = prev.clone();
    let card_click = move |btn: &gtk4::Button, name: &str| {
        detail_title_c.set_text(name);
        let p = stack_c.visible_child_name().unwrap_or_default().to_string();
        if p != "detail" {
            *pc2.borrow_mut() = p;
            stack_c.set_visible_child_name("detail");
        }
        let _ = btn;
    };
    let hook_card = |id: &str, name: &str| {
        if let Some(btn) = home_b.object::<gtk4::Button>(id) {
            let card_click = card_click.clone();
            let name = name.to_string();
            btn.connect_clicked(move |b| card_click(b, &name));
        }
    };
    hook_card("card_gimp", "GIMP");
    hook_card("card_firefox", "Firefox");
    hook_card("card_webssh", "WebSSH");
    hook_card("card_vlc", "VLC");
    hook_card("card_bork", "Bork");
    hook_card("card_astroterm", "Astroterm");

    // ---- store-local search (home and results entries) -----------------------
    wire_store_search(&store_search_home, &stack, &prev);
    wire_store_search(&store_search_results, &stack, &prev);

    // ---- back -> previous page ----------------------------------------------
    let stack_b = stack.clone();
    let pc_b = prev.clone();
    back.connect_clicked(move |_| {
        let to = pc_b.borrow().clone();
        stack_b.set_visible_child_name(&to);
    });

    // ---- initial state for captures ------------------------------------------
    if !search_text.is_empty() {
        store_search_home.set_text(&search_text);
    }
    if !detail_name.is_empty() {
        detail_title.set_text(&detail_name);
    }
    window.set_child(Some(&body));
    stack.set_visible_child_name(&initial);

    window.present();
    glib::MainLoop::new(None, false).run();
}
