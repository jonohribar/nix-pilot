// nix-pilot desktop client — first scaffold (D19: Rust + GTK4 + libadwaita).
//
// A Software Store window that reads the generated catalogue index
// (experiments/phase0/results/catalogue-index.json) and lists the app
// universe with live search, grouped-labelled like the CLI search probe.
// This is deliberately the smallest real UI proving the stack: a header bar
// with a search entry and a scrollable list of entries, each labelled by its
// catalogue class (application / unverified application / terminal app /
// command-line tool). Layout and wireframes come next (msn.2).
use std::cell::RefCell;
use std::rc::Rc;

use glib::ExitCode;
use gtk4::prelude::*;
use serde::Deserialize;

// ------- catalogue index schema (subset we display) ---------------------------
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CatalogueIndex {
    entries: Vec<IndexEntry>,
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
    class: String,
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
}

// ------- index loading ----------------------------------------------------------
fn resolve_index_path() -> Option<String> {
    if let Some(p) = std::env::args().nth(1) {
        return Some(p);
    }
    for cand in [
        "experiments/phase0/results/catalogue-index.json", // repo root cwd
        "results/catalogue-index.json",                    // phase0 cwd
        "../experiments/phase0/results/catalogue-index.json",
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

fn entry_row(e: &IndexEntry) -> gtk4::Widget {
    let boxv = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    boxv.set_margin_top(6);
    boxv.set_margin_bottom(6);
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

    boxv.upcast()
}

fn rebuild_list(container: &gtk4::Box, entries: &[IndexEntry], query: &str) -> usize {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    let q = query.trim().to_lowercase();
    let mut shown = 0;
    for e in entries {
        let hay = format!(
            "{} {} {}",
            e.attr.to_lowercase(),
            e.display_name().to_lowercase(),
            e.description.clone().unwrap_or_default().to_lowercase()
        );
        if q.is_empty() || hay.contains(&q) {
            container.append(&entry_row(e));
            shown += 1;
        }
    }
    if shown == 0 {
        let none = gtk4::Label::new(Some("no matches — try another term"));
        none.add_css_class("dim-label");
        none.set_margin_top(18);
        none.set_xalign(0.0);
        container.append(&none);
    }
    shown
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
            .default_width(960)
            .default_height(720)
            .build();

        // header bar with live search
        let header = gtk4::HeaderBar::new();
        let search = gtk4::SearchEntry::new();
        search.set_placeholder_text(Some("Search apps…"));
        search.set_hexpand(true);
        header.pack_start(&search);
        window.set_titlebar(Some(&header));

        // index loading (fail loudly in the window, not silently)
        let (entries, error) = match resolve_index_path() {
            Some(p) => match load_index(&p) {
                Ok(idx) => (idx.entries, None),
                Err(e) => (vec![], Some(e)),
            },
            None => (
                vec![],
                Some("catalogue index not found — pass a path or run from the repo root".into()),
            ),
        };
        let entries = Rc::new(RefCell::new(entries));

        let summary = gtk4::Label::new(None);
        summary.set_xalign(0.0);
        summary.add_css_class("dim-label");

        let listbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let scrolled = gtk4::ScrolledWindow::builder()
            .child(&listbox)
            .vexpand(true)
            .build();

        let body = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        body.set_margin_top(6);
        body.set_margin_bottom(6);
        body.append(&summary);
        body.append(&scrolled);
        window.set_child(Some(&body));

        // initial paint
        let by_class = {
            let es = entries.borrow();
            rebuild_list(&listbox, &es, "");
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
                "{} entries · {} application · {} unverified · {} terminal · {} command-line — live grouped search",
                entries.borrow().len(), by_class[0], by_class[1], by_class[2], by_class[3]
            ));
        }

        // live filtering
        let listbox2 = listbox.clone();
        let entries2 = entries.clone();
        let summary2 = summary.clone();
        search.connect_search_changed(move |s| {
            let n = rebuild_list(&listbox2, &entries2.borrow(), &s.text());
            summary2.set_text(&format!("{} shown", n));
        });

        window.present();
    });

    app.run()
}
