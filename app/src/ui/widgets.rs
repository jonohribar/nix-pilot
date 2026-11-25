//! Shared widget builders for Store, search, and detail metadata.
//!
//! Builders keep repeated row structure and safe Pango-markup interpolation
//! consistent across the GTK pages.
use gtk4::prelude::*;

use crate::model::IndexEntry;

/// Escape untrusted text for safe interpolation into GTK label markup.
pub fn esc(s: &str) -> String {
    glib::markup_escape_text(s).to_string()
}

/// Recursively collect the human-readable text of a widget and its subtree
/// into `out` (label text, button labels, tooltips). Shared by the geometry
/// dump and the tree-probe GUI test so both walk the tree the same way.
pub fn widget_text(w: &gtk4::Widget, out: &mut Vec<String>) {
    if let Some(l) = w.downcast_ref::<gtk4::Label>() {
        let t = l.text().to_string();
        if !t.is_empty() {
            out.push(t);
        }
    }
    if let Some(b) = w.downcast_ref::<gtk4::Button>() {
        if let Some(t) = b.label() {
            let t = t.to_string();
            if !t.is_empty() {
                out.push(t);
            }
        }
    }
    if let Some(t) = w.tooltip_text() {
        let t = t.to_string();
        if !t.is_empty() {
            out.push(format!("tip:{t}"));
        }
    }
    let mut child = w.first_child();
    while let Some(c) = child {
        widget_text(&c, out);
        child = c.next_sibling();
    }
}

/// Build a catalogue row showing its name, class, version, and description.
pub fn entry_row(e: &IndexEntry) -> gtk4::ListBoxRow {
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
        esc(e.class.label()),
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

/// A settings-result row for the global search: label + blurb, plain. It
/// does not render an install control; selecting it opens the Settings
/// category that owns it.
pub fn setting_row(spec: &crate::settings::Spec) -> gtk4::ListBoxRow {
    let boxv = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    boxv.set_margin_top(8);
    boxv.set_margin_bottom(8);
    boxv.set_margin_start(10);
    boxv.set_margin_end(10);

    let title = gtk4::Label::new(None);
    title.set_xalign(0.0);
    title.set_markup(&format!(
        "<b>{}</b>  <span size='small'>setting</span>",
        esc(spec.label)
    ));
    boxv.append(&title);

    let blurb = gtk4::Label::new(Some(spec.blurb));
    blurb.set_xalign(0.0);
    blurb.set_wrap(false);
    blurb.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    blurb.add_css_class("dim-label");
    boxv.append(&blurb);

    let row = gtk4::ListBoxRow::new();
    row.set_child(Some(&boxv));
    row
}

/// Non-clickable group header row ("Applications · 12 results").
pub fn group_header_row(label: &str, count: usize) -> gtk4::ListBoxRow {
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

/// A key/value row of the detail metadata card.
pub fn meta_row(key: &str, value: &str) -> gtk4::Box {
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
