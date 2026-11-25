//! Dev-only widget geometry dump (env `NIX_PILOT_DEBUG_GEOM`).
//!
//! The screenshot/AT-SPI harness needs reliable click targets, but AT-SPI's
//! extents are awkward to read across coordinate spaces. When the env var is
//! set, this module writes every clickable/labeled widget's window-space
//! rectangle to a file (default `/tmp/nix-pilot-geom.json`, override with
//! `NIX_PILOT_DEBUG_GEOM_FILE`) so a driver can compute exact click points.
//! It dumps on startup, on every stack pane switch, and periodically while
//! enabled, so the file always reflects the current visible surface.
//! No effect unless the env var is set; never active in production.

use gtk4::prelude::*;
use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

/// One clickable or labeled widget with its resolved geometry and state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub kind: &'static str,
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub visible: bool,
    pub sensitive: bool,
}

fn classify(w: &gtk4::Widget) -> Option<&'static str> {
    if w.downcast_ref::<gtk4::Button>().is_some() {
        Some("button")
    } else if w.downcast_ref::<gtk4::Switch>().is_some() {
        Some("switch")
    } else if w.downcast_ref::<gtk4::ListBoxRow>().is_some() {
        Some("row")
    } else if w.downcast_ref::<gtk4::ListBox>().is_some() {
        Some("list")
    } else if w.downcast_ref::<gtk4::ScrolledWindow>().is_some() {
        Some("scroll")
    } else if w.downcast_ref::<gtk4::Entry>().is_some()
        || w.downcast_ref::<gtk4::SearchEntry>().is_some()
    {
        Some("entry")
    } else {
        None
    }
}

/// Collect geometry nodes for the whole widget tree under `toplevel`.
/// Unrealized widgets report no window position (translate_coordinates is
/// None) and are skipped; the periodic refresh picks them up once shown.
pub fn collect<T: IsA<gtk4::Widget>>(toplevel: &T) -> Vec<Node> {
    let toplevel: &gtk4::Widget = toplevel.upcast_ref();
    let mut out = Vec::new();
    // The toplevel itself: gives the window position/size on its screen area.
    if let Some((wx, wy)) = toplevel.translate_coordinates(toplevel, 0.0, 0.0) {
        let a = toplevel.allocation();
        out.push(Node {
            kind: "window",
            label: String::new(),
            x: wx as i32,
            y: wy as i32,
            w: a.width(),
            h: a.height(),
            visible: toplevel.is_visible(),
            sensitive: true,
        });
    }
    collect_into(toplevel, toplevel, &mut out);
    out
}

fn collect_into(w: &gtk4::Widget, toplevel: &gtk4::Widget, out: &mut Vec<Node>) {
    if let Some(kind) = classify(w) {
        if let Some((x, y)) = w.translate_coordinates(toplevel, 0.0, 0.0) {
            let a = w.allocation();
            let mut text = Vec::new();
            super::widgets::widget_text(w, &mut text);
            out.push(Node {
                kind,
                label: text.join(" | "),
                x: x as i32,
                y: y as i32,
                w: a.width(),
                h: a.height(),
                visible: w.is_visible(),
                sensitive: w.is_sensitive(),
            });
        }
    }
    let mut child = w.first_child();
    while let Some(c) = child {
        collect_into(&c, toplevel, out);
        child = c.next_sibling();
    }
}

/// Serialize nodes to a JSON array (JSON, not JSON-lite, so any parser works).
pub fn render(nodes: &[Node]) -> String {
    let mut s = String::from("[");
    for (i, n) in nodes.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"kind\":\"{}\",\"label\":\"{}\",\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"visible\":{},\"sensitive\":{}}}",
            n.kind,
            escape(&n.label),
            n.x,
            n.y,
            n.w,
            n.h,
            n.visible,
            n.sensitive
        ));
    }
    s.push(']');
    s
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out
}

struct Dumper {
    last: Rc<RefCell<Option<String>>>,
    file: String,
}

impl Dumper {
    fn dump(&self, window: &gtk4::ApplicationWindow) {
        let text = render(&collect(window));
        if *self.last.borrow() == Some(text.clone()) {
            return;
        }
        *self.last.borrow_mut() = Some(text.clone());
        if let Ok(mut f) = std::fs::File::create(&self.file) {
            let _ = f.write_all(text.as_bytes());
        }
    }
}

/// Wire the dump under `NIX_PILOT_DEBUG_GEOM`. Safe to call unconditionally;
/// it is a no-op when the env var is unset.
pub fn wire(window: &gtk4::ApplicationWindow, stack: &gtk4::Stack) {
    if std::env::var("NIX_PILOT_DEBUG_GEOM").is_err() {
        return;
    }
    let file = std::env::var("NIX_PILOT_DEBUG_GEOM_FILE")
        .unwrap_or_else(|_| "/tmp/nix-pilot-geom.json".to_string());
    let dumper = Dumper {
        last: Rc::new(RefCell::new(None)),
        file,
    };

    // Initial dump, then re-dump whenever the visible pane changes.
    let d = Rc::new(dumper);
    d.dump(window);

    let d2 = d.clone();
    let win2 = window.clone();
    stack.connect_visible_child_name_notify(move |_| d2.dump(&win2));

    // Periodic refresh so the file tracks transient states (e.g. opening a
    // detail page, a button relabeling) without extra wiring at every site.
    let d3 = d.clone();
    let win3 = window.clone();
    gtk4::glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
        d3.dump(&win3);
        gtk4::glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_emits_json_array_with_escaped_labels() {
        let nodes = vec![
            Node {
                kind: "button",
                label: "Check \"what\" \\ cleaning".to_string(),
                x: 1,
                y: 2,
                w: 3,
                h: 4,
                visible: true,
                sensitive: true,
            },
            Node {
                kind: "row",
                label: "firefox  Application".to_string(),
                x: 5,
                y: 6,
                w: 7,
                h: 8,
                visible: true,
                sensitive: true,
            },
        ];
        let json = render(&nodes);
        // Bracketed array with both nodes and properly escaped labels (no
        // unescaped interior quotes that would break a JSON parser).
        assert!(json.starts_with('[') && json.ends_with(']'));
        assert_eq!(json.matches("{\"kind\":").count(), 2);
        assert!(json.contains(r#"\"what\""#));
        assert!(json.contains("firefox  Application"));
    }
}
