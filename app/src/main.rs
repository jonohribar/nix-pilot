// nix-pilot desktop client — Software Store (D19: Rust + GTK4 + libadwaita).
//
// Reads the generated catalogue index
// (experiments/catalogue/results/catalogue-index.json) and offers ranked
// grouped search over the app universe. Ranking mirrors the verified
// search.js scorer (exact/prefix/contains on attr+pname, description bonus)
// so results sort identically to the HTML viewer. Clicking a result opens a
// detail/install surface (per-user scope, D9) populated from the live entry.
use std::cell::RefCell;
use std::rc::Rc;

use glib::ExitCode;
use gtk4::prelude::*;
use serde::Deserialize;

// ------- catalogue index schema (subset we display) ---------------------------
#[derive(Deserialize, Debug)]
struct CatalogueIndex {
    entries: Vec<IndexEntry>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct InstallPlan {
    #[serde(default)]
    scope_default: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct IndexEntry {
    attr: String,
    #[serde(default)]
    pname: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    home_page: Option<String>,
    #[serde(default)]
    main_program: Option<String>,
    #[serde(default)]
    unfree: bool,
    #[serde(default)]
    linux_ok: bool,
    #[serde(default)]
    class: String,
    #[serde(default)]
    tier: String,
    #[serde(default)]
    install: Option<InstallPlan>,
}

impl IndexEntry {
    fn group_label(&self) -> &'static str {
        match self.class.as_str() {
            "app" => "Application",
            "unverified" => "Unverified application",
            "term-app" => "Terminal app",
            "cli" => "Command-line tool",
            _ => "Other",
        }
    }
    fn display_name(&self) -> &str {
        self.pname.as_deref().unwrap_or(self.attr.as_str())
    }
    fn tier_label(&self) -> String {
        match self.tier.as_str() {
            "rich" => "Verified — rich metadata".into(),
            "basic" => "Verified — desktop entry".into(),
            "unfree-deferred" => "Unfree — listed, no cache evidence".into(),
            "unverified" => "Unverified — no desktop evidence".into(),
            other => other.to_string(),
        }
    }
}

// ------- scored ranking (ported 1:1 from experiments/catalogue/search.js) -----
// tokenize(): split on anything that is not [a-z0-9.+-], lowercase.
fn tokenize(q: &str) -> Vec<String> {
    q.to_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '+' | '-')))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

// score(): exact attr/pname = 100, prefix = 30, contains = 10, in description = 3.
fn score(e: &IndexEntry, terms: &[String]) -> i64 {
    let name_t = e.attr.to_lowercase();
    let pn = e.pname.clone().unwrap_or_default().to_lowercase();
    let desc = e.description.clone().unwrap_or_default().to_lowercase();
    let mut s = 0i64;
    for t in terms {
        if t.len() < 2 {
            continue;
        }
        if name_t == *t || pn == *t {
            s += 100;
        } else if name_t.starts_with(t) || pn.starts_with(t) {
            s += 30;
        } else if name_t.contains(t.as_str()) || pn.contains(t.as_str()) {
            s += 10;
        }
        if desc.contains(t.as_str()) {
            s += 3;
        }
    }
    s
}

/// How many rows the list shows at once (empty query is a bounded browse).
const MAX_ROWS: usize = 60;

/// Group order for global search results (D16/D18 labelled groups).
const GROUP_ORDER: [&str; 4] = ["app", "unverified", "term-app", "cli"];

fn class_label(class: &str) -> &'static str {
    match class {
        "app" => "Applications",
        "unverified" => "Unverified applications",
        "term-app" => "Terminal apps",
        "cli" => "Command-line tools",
        _ => "Other",
    }
}

// ------- index loading ----------------------------------------------------------
fn resolve_index_path() -> Option<String> {
    if let Some(p) = std::env::args().nth(1) {
        return Some(p);
    }
    for cand in [
        "experiments/catalogue/results/catalogue-index.json", // repo root cwd
        "results/catalogue-index.json",                    // catalogue cwd
        "../experiments/catalogue/results/catalogue-index.json",
    ] {
        if std::path::Path::new(cand).exists() {
            return Some(cand.to_string());
        }
    }
    None
}

fn load_index(path: &str) -> Result<CatalogueIndex, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path, e))?;
    serde_json::from_str(&raw).map_err(|e| format!("cannot parse index: {}", e))
}

// ------- widget helpers --------------------------------------------------------
fn esc(s: &str) -> String {
    glib::markup_escape_text(s).to_string()
}

fn entry_row(e: &IndexEntry) -> gtk4::ListBoxRow {
    let boxv = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    boxv.set_margin_top(8);
    boxv.set_margin_bottom(8);
    boxv.set_margin_start(10);
    boxv.set_margin_end(10);

    let title = gtk4::Label::new(None);
    title.set_xalign(0.0);
    let version = e.version.clone().unwrap_or_default();
    title.set_markup(&format!(
        "<b>{}</b>  <span size='small'>{} · v{}</span>",
        esc(e.display_name()),
        esc(e.group_label()),
        esc(&version)
    ));
    boxv.append(&title);

    if let Some(d) = &e.description {
        let desc = gtk4::Label::new(Some(d));
        desc.set_xalign(0.0);
        desc.set_wrap(false);
        desc.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        desc.add_css_class("dim-label");
        boxv.append(&desc);
    }

    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&boxv));
    row
}

/// Non-clickable group header row ("Applications · 12 results").
fn group_header_row(label: &str, count: usize) -> gtk4::ListBoxRow {
    let s = if count == 1 { "" } else { "s" };
    let l = gtk4::Label::new(None);
    l.set_xalign(0.0);
    l.set_margin_top(12);
    l.set_margin_start(10);
    l.set_margin_end(10);
    l.set_markup(&format!(
        "<b>{}</b>  <span size='small' color='gray'>{} result{s}</span>",
        esc(label),
        count
    ));
    let row = gtk4::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.set_child(Some(&l));
    row
}

fn meta_row(key: &str, value: &str) -> gtk4::Box {
    let h = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
    let k = gtk4::Label::new(Some(key));
    k.set_xalign(0.0);
    k.set_width_chars(14);
    k.add_css_class("dim-label");
    let v = gtk4::Label::new(Some(value));
    v.set_xalign(0.0);
    v.set_wrap(true);
    v.set_hexpand(true);
    h.append(&k);
    h.append(&v);
    h
}

/// Detail/install page. GTK objects have interior mutability, so an Rc is
/// enough to share it across activation closures while populate() takes &self.
struct DetailUi {
    title: gtk4::Label,
    sub: gtk4::Label,
    desc: gtk4::Label,
    scope: gtk4::Switch,
    meta: gtk4::Box,
    home: gtk4::Button,
    page: gtk4::Box,
}

impl DetailUi {
    fn new(install_hint: &str) -> DetailUi {
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        page.set_margin_start(42);
        page.set_margin_end(42);
        page.set_margin_top(20);

        let head = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        let title = gtk4::Label::new(None);
        title.add_css_class("title-1");
        title.set_xalign(0.0);
        let sub = gtk4::Label::new(None);
        sub.set_xalign(0.0);
        sub.add_css_class("dim-label");
        let desc = gtk4::Label::new(None);
        desc.set_xalign(0.0);
        desc.set_wrap(true);
        desc.set_visible(false);
        head.append(&title);
        head.append(&sub);
        head.append(&desc);
        page.append(&head);

        let sep = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        sep.set_margin_top(18);
        sep.set_margin_bottom(14);
        page.append(&sep);

        // install card (per-user scope, D9)
        let card = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        card.add_css_class("card");
        let inst_h = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        let install = gtk4::Button::with_label("Install for me");
        install.add_css_class("suggested-action");
        install.set_valign(gtk4::Align::Center);
        install.set_tooltip_text(Some("Installation is not wired up yet — this is the Store surface."));
        let hint = gtk4::Label::new(Some(install_hint));
        hint.set_wrap(true);
        hint.add_css_class("dim-label");
        hint.set_hexpand(true);
        inst_h.append(&install);
        inst_h.append(&hint);
        card.append(&inst_h);

        let scope_h = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        let scope_labelv = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        let scope_label = gtk4::Label::new(Some("Install for everyone on this computer"));
        scope_label.set_xalign(0.0);
        let scope_sub = gtk4::Label::new(Some(
            "Choose this only if you administer the system (system-wide installation)",
        ));
        scope_sub.set_xalign(0.0);
        scope_sub.add_css_class("dim-label");
        scope_labelv.append(&scope_label);
        scope_labelv.append(&scope_sub);
        scope_labelv.set_hexpand(true);
        let scope = gtk4::Switch::new();
        scope_h.append(&scope_labelv);
        scope_h.append(&scope);
        card.append(&scope_h);
        page.append(&card);

        let more = gtk4::Label::new(Some("More information"));
        more.add_css_class("heading");
        more.set_xalign(0.0);
        more.set_margin_top(14);
        page.append(&more);

        let meta_card = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        meta_card.add_css_class("card");
        let meta = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        meta_card.append(&meta);
        page.append(&meta_card);

        let home = gtk4::Button::with_label("Open project home page");
        home.set_visible(false);
        home.set_margin_top(8);
        page.append(&home);

        DetailUi {
            title,
            sub,
            desc,
            scope,
            meta,
            home,
            page,
        }
    }

    fn populate(&self, e: &IndexEntry) {
        let scope_default = e
            .install
            .as_ref()
            .and_then(|i| i.scope_default.as_deref())
            .unwrap_or("user");
        self.title.set_label(e.display_name());
        let version = e.version.clone().unwrap_or_default();
        self.sub
            .set_markup(&format!("{} · version {}", esc(e.group_label()), esc(&version)));
        if let Some(d) = &e.description {
            self.desc.set_label(d);
            self.desc.set_visible(true);
        } else {
            self.desc.set_label("");
            self.desc.set_visible(false);
        }
        self.scope.set_active(scope_default == "system");

        while let Some(child) = self.meta.first_child() {
            self.meta.remove(&child);
        }
        let licence = if e.unfree { "Unfree licence" } else { "Free licence" };
        let platforms = if e.linux_ok {
            "Linux (x86_64 / aarch64)"
        } else {
            "Unknown"
        };
        let main = e.main_program.as_deref().unwrap_or("—");
        self.meta.append(&meta_row("Licence", licence));
        self.meta.append(&meta_row("Evidence", &e.tier_label()));
        self.meta.append(&meta_row("Available for", platforms));
        self.meta.append(&meta_row("Command", main));
        self.meta.append(&meta_row(
            "Scope",
            if scope_default == "system" {
                "System-wide"
            } else {
                "Per user"
            },
        ));

        if let Some(hp) = &e.home_page {
            self.home.set_visible(true);
            self.home.set_tooltip_text(Some(hp));
        } else {
            self.home.set_visible(false);
            self.home.set_tooltip_text(None);
        }
    }
}

// ------- list rebuild (ranks + renders, writes row order into `shown`) ---------
/// Maps the row order back to entries (None marks a group header) and renders.
/// Empty query: bounded browse in index order. Search: ranked, bucketed into
/// the D16/D18 labelled groups, cap MAX_ROWS in total.
fn rebuild_list(listbox: &gtk4::ListBox, all: &[IndexEntry], shown: &RefCell<Vec<Option<IndexEntry>>>, query: &str) -> usize {
    while let Some(child) = listbox.first_child() {
        listbox.remove(&child);
    }
    let mut rows: Vec<Option<IndexEntry>> = Vec::new();
    let terms = tokenize(query);
    if query.trim().is_empty() {
        for e in all.iter().take(MAX_ROWS) {
            listbox.append(&entry_row(e));
            rows.push(Some(e.clone()));
        }
    } else {
        let mut scored: Vec<(i64, &IndexEntry)> = all
            .iter()
            .map(|e| (score(e, &terms), e))
            .filter(|(s, _)| *s > 0)
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        let picked: Vec<&IndexEntry> = scored
            .into_iter()
            .take(MAX_ROWS)
            .map(|(_, e)| e)
            .collect();
        let mut groups: Vec<(&str, Vec<&IndexEntry>)> = Vec::new();
        for c in GROUP_ORDER {
            let v: Vec<&IndexEntry> = picked.iter().filter(|e| e.class == c).cloned().collect();
            if !v.is_empty() {
                groups.push((c, v));
            }
        }
        let others: Vec<&IndexEntry> = picked
            .iter()
            .filter(|e| !GROUP_ORDER.contains(&e.class.as_str()))
            .cloned()
            .collect();
        if !others.is_empty() {
            groups.push(("other", others));
        }
        for (c, v) in groups {
            listbox.append(&group_header_row(class_label(c), v.len()));
            rows.push(None);
            for e in v {
                listbox.append(&entry_row(e));
                rows.push(Some(e.clone()));
            }
        }
    }
    let n = rows.iter().filter(|r| r.is_some()).count();
    *shown.borrow_mut() = rows;
    n
}

// ------- app -------------------------------------------------------------------
fn main() -> ExitCode {
    if libadwaita::init().is_err() {
        eprintln!("failed to initialise libadwaita");
        return ExitCode::FAILURE;
    }

    let app = gtk4::Application::builder()
        .application_id("dev.nixpilot.app")
        .build();

    app.connect_activate(|app| {
        let window = gtk4::ApplicationWindow::builder()
            .application(app)
            .title("nix-pilot — Software Store")
            .default_width(1020)
            .default_height(720)
            .build();

        // header: back (visible only on detail) + live search
        let header = gtk4::HeaderBar::new();
        let back = gtk4::Button::new();
        back.set_label("‹ Back to results");
        back.set_visible(false);
        header.pack_start(&back);
        let search = gtk4::SearchEntry::new();
        search.set_placeholder_text(Some("Search apps…"));
        search.set_hexpand(true);
        header.pack_end(&search);
        window.set_titlebar(Some(&header));

        // index loading (fail loudly in the window, not silently)
        let (all_entries, error) = match resolve_index_path() {
            Some(p) => match load_index(&p) {
                Ok(idx) => (idx.entries, None),
                Err(e) => (vec![], Some(e)),
            },
            None => (
                vec![],
                Some("catalogue index not found — pass a path or run from the repo root".into()),
            ),
        };
        let all = Rc::new(RefCell::new(all_entries));
        let shown = Rc::new(RefCell::new(Vec::<Option<IndexEntry>>::new()));

        let summary = gtk4::Label::new(None);
        summary.set_xalign(0.0);
        summary.add_css_class("dim-label");

        let listbox = gtk4::ListBox::new();
        let scrolled = gtk4::ScrolledWindow::builder()
            .child(&listbox)
            .vexpand(true)
            .build();
        let list_page = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        list_page.set_margin_top(6);
        list_page.set_margin_bottom(6);
        list_page.append(&summary);
        list_page.append(&scrolled);

        let detail = Rc::new(DetailUi::new(
            "Installs into your user account — no administrator rights needed",
        ));

        let stack = gtk4::Stack::new();
        stack.add_named(&list_page, Some("list"));
        stack.add_named(&detail.page, Some("detail"));
        stack.set_visible_child_name("list");
        window.set_child(Some(&stack));

        // header back button toggles with the page
        let back2 = back.clone();
        let stack_b = stack.clone();
        stack.connect_visible_child_name_notify(move |s| {
            back2.set_visible(s.visible_child_name().as_deref() == Some("detail"));
        });
        let _ = stack_b;
        let stack_back = stack.clone();
        back.connect_clicked(move |_| stack_back.set_visible_child_name("list"));

        // summary header across the top of the list page
        let by_class = {
            let es = all.borrow();
            es.iter().fold([0i64; 4], |mut a, e| {
                match e.class.as_str() {
                    "app" => a[0] += 1,
                    "unverified" => a[1] += 1,
                    "term-app" => a[2] += 1,
                    _ => a[3] += 1,
                }
                a
            })
        };
        if let Some(e) = error {
            summary.set_text(&e);
        } else {
            summary.set_text(&format!(
                "{} entries · {} application · {} unverified · {} terminal · {} command-line — click a result for details",
                all.borrow().len(), by_class[0], by_class[1], by_class[2], by_class[3]
            ));
        }

        // row activation opens the detail page for the selected entry
        let detail_for_row = detail.clone();
        let stack_for_row = stack.clone();
        let shown_for_row = shown.clone();
        listbox.connect_row_activated(move |_, row| {
            let i = row.index();
            if i >= 0 {
                if let Some(Some(e)) = shown_for_row.borrow().get(i as usize).cloned() {
                    detail_for_row.populate(&e);
                    stack_for_row.set_visible_child_name("detail");
                }
            }
        });

        // live global search: rerank into groups, return to the list page
        let stack_live = stack.clone();
        let summary_live = summary.clone();
        let listbox_live = listbox.clone();
        let all_live = all.clone();
        let shown_live = shown.clone();
        search.connect_search_changed(move |s| {
            let n = rebuild_list(&listbox_live, &all_live.borrow(), &shown_live, &s.text());
            summary_live.set_text(&format!("{} shown", n));
            stack_live.set_visible_child_name("list");
        });
        // Esc clears the query and shows the browse list again
        let listbox_esc = listbox.clone();
        let all_esc = all.clone();
        let shown_esc = shown.clone();
        let summary_esc = summary.clone();
        search.connect_stop_search(move |s| {
            if !s.text().is_empty() {
                s.set_text("");
            } else {
                let n = rebuild_list(&listbox_esc, &all_esc.borrow(), &shown_esc, "");
                summary_esc.set_text(&format!("{} shown", n));
            }
        });
        // Ctrl+K focuses the global search
        let kc = gtk4::EventControllerKey::new();
        let search_k = search.clone();
        kc.connect_key_pressed(move |_, key, _, mods| {
            if key == gtk4::gdk::Key::k && mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
                search_k.grab_focus();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(kc);

        // initial browse paint
        rebuild_list(&listbox, &all.borrow(), &shown, &search.text());

        window.present();
    });

    app.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(attr: &str, pname: Option<&str>, description: Option<&str>) -> IndexEntry {
        IndexEntry {
            attr: attr.into(),
            pname: pname.map(|s| s.into()),
            version: None,
            description: description.map(|s| s.into()),
            home_page: None,
            main_program: None,
            unfree: false,
            linux_ok: true,
            class: "app".into(),
            tier: "basic".into(),
            install: None,
        }
    }

    #[test]
    fn tokenize_splits_and_lowercases() {
        assert_eq!(
            tokenize("Firefox 160-esr +"),
            vec!["firefox".to_string(), "160-esr".to_string(), "+".to_string()]
        );
        assert_eq!(tokenize("::"), Vec::<String>::new());
    }

    #[test]
    fn scoring_matches_search_js_rules() {
        // exact attr == term -> 100
        let ff = e("firefox", None, None);
        assert_eq!(score(&ff, &["firefox".into()]), 100);
        // prefix -> 30
        assert_eq!(score(&ff, &["fire".into()]), 30);
        // contains -> 10
        assert_eq!(score(&ff, &["efo".into()]), 10);
        // description bonus only (+3)
        let htop = e("htop", None, Some("an interactive process viewer"));
        assert_eq!(score(&htop, &["viewer".into()]), 3);
        // multiple terms add up
        assert_eq!(score(&ff, &["firefox".into(), "efo".into()]), 110);
        // single-char terms are skipped by the scorer
        assert_eq!(score(&ff, &["f".into()]), 0);
    }

    #[test]
    fn group_labels_cover_the_classes() {
        let mut e = e("x", None, None);
        e.class = "cli".into();
        assert_eq!(e.group_label(), "Command-line tool");
        e.class = "unverified".into();
        assert_eq!(e.group_label(), "Unverified application");
        e.class = "term-app".into();
        assert_eq!(e.group_label(), "Terminal app");
        e.class = "app".into();
        assert_eq!(e.group_label(), "Application");
    }
}