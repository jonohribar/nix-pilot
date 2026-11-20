//! GTK UI surface: the app shell (window, header, store list + search) and its
//! focused components (detail + install, updates review dialog). Pure logic and
//! the nix adapter live in the sibling non-UI modules; this layer translates
//! user actions into store/updates/install actions and renders their outcomes.
pub mod detail;
pub mod settings_page;
pub mod updates_page;
pub mod widgets;
pub mod your_system;

mod shell;

use crate::model::IndexEntry;

/// Build and present the application window (thin composition entry point).
/// `entries` is the loaded catalogue; `nix` is the resolved client used for
/// install/update checks. The shell owns the window and all widgets.
pub fn run(
    app: &gtk4::Application,
    entries: Vec<IndexEntry>,
    load_error: Option<String>,
    nix: Option<std::path::PathBuf>,
) {
    shell::run(app, entries, load_error, nix);
}

#[cfg(test)]
mod tree_probe {
    //! GUI-contract probe (ignored; run under a display): build the real pages
    //! and walk the widget tree, asserting each shipped feature exists as a
    //! component. The GUI counterpart of the live Nix proofs — it catches a
    //! feature that silently fails to render (like the settings-category key
    //! swap in de027be). Run: xvfb-run cargo test -- --ignored gui_tree_probe
    use gtk4::prelude::*;
    use std::rc::Rc;

    fn collect(w: &impl IsA<gtk4::Widget>, out: &mut Vec<String>) {
        let w = w.upcast_ref::<gtk4::Widget>();
        let mut parts: Vec<String> = Vec::new();
        if let Some(l) = w.downcast_ref::<gtk4::Label>() {
            let t = l.text().to_string();
            if !t.is_empty() {
                parts.push(t);
            }
        }
        if let Some(b) = w.downcast_ref::<gtk4::Button>() {
            if let Some(t) = b.label() {
                let t = t.to_string();
                if !t.is_empty() {
                    parts.push(t);
                }
            }
        }
        if let Some(t) = w.tooltip_text() {
            let t = t.to_string();
            if !t.is_empty() {
                parts.push(format!("tip:{t}"));
            }
        }
        if !parts.is_empty() {
            out.push(parts.join(" | "));
        }
        let mut child = w.first_child();
        while let Some(c) = child {
            collect(&c, out);
            child = c.next_sibling();
        }
    }

    fn all_text(page: &impl IsA<gtk4::Widget>) -> Vec<String> {
        let mut out = Vec::new();
        collect(page, &mut out);
        out
    }

    #[test]
    #[ignore = "needs a display; run under xvfb"]
    fn gui_tree_probe() {
        // GTK widgets need an initialised toolkit; libadwaita init brings it
        // up (same as main.rs). The probe never opens a window.
        let _ = libadwaita::init();
        let account = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        let settings = super::settings_page::SettingsPage::new(&account);
        let updates = super::updates_page::UpdatesPage::new();
        let your_system = super::your_system::YourSystemPage::new(&account);

        let s = all_text(&settings.page);
        let u = all_text(&updates.page);
        let y = all_text(&your_system.page);

        for (page, want) in [
            (
                "settings",
                vec!["Bluetooth", "Bluetooth at startup", "Sound"],
            ),
            (
                "updates",
                vec![
                    "Automatic",
                    "Storage",
                    "System versions",
                    "Check what cleaning would free",
                    "Clean up old versions",
                    "Reclaim storage",
                    "Go back to the previous version",
                ],
            ),
            ("your-system", vec!["Your system"]),
        ] {
            let hay: String = match page {
                "settings" => s.join("\n"),
                "updates" => u.join("\n"),
                _ => y.join("\n"),
            };
            for w in want {
                assert!(
                    hay.contains(w),
                    "{page} page missing component {w:?}\n--- tree ---\n{hay}"
                );
                println!("OK [{page}] {w}");
            }
        }
        // The recipe page must render actual rows when the desired state has
        // content: seed a doc, refresh, and expect the app + a Remove button.
        let desired = Rc::new(std::cell::RefCell::new(
            crate::desired_state::DesiredDoc::current(),
        ));
        desired.borrow_mut().set_system(vec!["hello".to_string()]);
        desired.borrow_mut().settings.insert(
            "services.pipewire.enable".into(),
            crate::desired_state::SettingValue::Bool(true),
        );
        let queue = Rc::new(std::cell::RefCell::new(crate::store::ChangeQueue::new()));
        let on_change: Rc<dyn Fn()> = Rc::new(|| {});
        your_system.refresh(&desired, &queue, &on_change);
        let y2 = all_text(&your_system.page);
        let hay = y2.join("\n");
        assert!(
            hay.contains("hello"),
            "your-system missing seeded app row: {hay}"
        );
        assert!(
            hay.contains("Sound"),
            "your-system missing setting row: {hay}"
        );
        assert!(
            hay.contains("Remove"),
            "your-system missing Remove action: {hay}"
        );
        println!("OK [your-system] seeded rows + Remove action");
    }
}
