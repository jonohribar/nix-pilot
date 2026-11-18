//! Settings page — the product-authored settings surface (settings-schema.md,
//! decision D14). System-owned settings in the REQ-5 areas with plain-language
//! wording (REQ-6), a D14 pointer for out-of-scope desktop chrome, and the
//! recovery pointer to Updates. In v1 no setting workflow is shipped yet, so
//! the page states clearly that nothing here is changeable and every setting
//! will explain what changes before it is applied (REQ-3).
use gtk4::prelude::*;

/// (name, plain-language blurb) for each first-release REQ-5 area.
const AREAS: [(&str, &str); 7] = [
    (
        "Network",
        "How this computer connects to the internet and other devices.",
    ),
    ("Display", "Screens, resolution and brightness."),
    ("Sound", "Speakers, microphone and volume."),
    ("Printers", "Printers and scanning."),
    ("Users", "People who use this computer and their accounts."),
    ("Power", "Battery, sleep and power-saving."),
    ("Remote access", "Connect to this computer from elsewhere."),
];

pub struct SettingsPage {
    pub page: gtk4::Box,
}

fn area_row(name: &str, blurb: &str) -> gtk4::ListBoxRow {
    let box_ = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    let name_l = gtk4::Label::new(Some(name));
    name_l.set_xalign(0.0);
    let blurb_l = gtk4::Label::new(Some(blurb));
    blurb_l.set_xalign(0.0);
    blurb_l.set_wrap(true);
    blurb_l.add_css_class("dim-label");
    box_.append(&name_l);
    box_.append(&blurb_l);
    let row = gtk4::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.set_child(Some(&box_));
    row
}

impl SettingsPage {
    pub fn new() -> SettingsPage {
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        page.set_margin_top(18);
        page.set_margin_start(28);
        page.set_margin_end(28);

        let heading = gtk4::Label::new(Some("Settings"));
        heading.add_css_class("title-2");
        heading.set_xalign(0.0);
        page.append(&heading);

        let notice = gtk4::Label::new(Some(
            "Settings apply to this computer. Nothing here is changeable yet — \
             the settings backend is the next surface after Updates. Every \
             setting will explain what changes before it is applied.",
        ));
        notice.set_xalign(0.0);
        notice.set_wrap(true);
        notice.add_css_class("dim-label");
        page.append(&notice);

        let list = gtk4::ListBox::new();
        for (name, blurb) in AREAS {
            list.append(&area_row(name, blurb));
        }
        page.append(&list);

        // D14 pointer: desktop chrome is owned by the desktop, never the app
        let pointer = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        pointer.add_css_class("card");
        pointer.set_margin_top(10);
        let p_head = gtk4::Label::new(Some("Appearance"));
        p_head.add_css_class("heading");
        p_head.set_xalign(0.0);
        let p_body = gtk4::Label::new(Some(
            "Wallpaper, themes and how the desktop looks live in your desktop's \
             own settings app — this app does not manage them.",
        ));
        p_body.set_xalign(0.0);
        p_body.set_wrap(true);
        p_body.add_css_class("dim-label");
        pointer.append(&p_head);
        pointer.append(&p_body);
        page.append(&pointer);

        // recovery pointer: the recovery story lives in Updates
        let recover = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        recover.add_css_class("card");
        recover.set_margin_top(10);
        let r_head = gtk4::Label::new(Some("If something goes wrong"));
        r_head.add_css_class("heading");
        r_head.set_xalign(0.0);
        let r_body = gtk4::Label::new(Some(
            "System changes and the plain-language recovery controls (including \
             boot-menu recovery) are in Updates.",
        ));
        r_body.set_xalign(0.0);
        r_body.set_wrap(true);
        r_body.add_css_class("dim-label");
        recover.append(&r_head);
        recover.append(&r_body);
        page.append(&recover);

        page.set_vexpand(true);
        SettingsPage { page }
    }
}
