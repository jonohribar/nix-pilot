//! Dev-only UI snapshot emitter (env `NIX_PILOT_UI_DUMP`).
//!
//! The AI UI driver needs a canonical description of every user-visible unit:
//! geometry, role, name, and state. When the env var is set, this module
//! writes a JSON snapshot of the whole window to a file (default
//! `/tmp/nix-pilot-ui.json`, override with `NIX_PILOT_UI_DUMP_FILE`) so the
//! driver can address units and compute exact click points. It dumps on
//! startup, on every stack pane switch, and periodically while enabled, so
//! the file always reflects the current visible surface. No effect unless
//! the env var is set; never active in production.

use gtk4::prelude::*;
use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

/// One user-visible unit with its geometry, state, and a stable name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub kind: &'static str,
    pub role: String,
    /// Short identity for the driver: the first non-empty label fragment.
    pub name: String,
    /// Full concatenated text (labels, button text, tooltips).
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub visible: bool,
    pub sensitive: bool,
    pub focused: bool,
}

/// The whole-window snapshot: geometry, the visible surface, and units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Window rectangle in screen space: (x, y, width, height).
    pub window: (i32, i32, i32, i32),
    /// The visible stack pane name (list, detail, settings, ...).
    pub page: Option<String>,
    pub units: Vec<Node>,
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

/// Collect a snapshot for the whole widget tree under `toplevel`.
/// Unrealized widgets report no window position (translate_coordinates is
/// None) and are skipped; the periodic refresh picks them up once shown.
pub fn collect<T: IsA<gtk4::Widget>>(toplevel: &T, page: Option<&str>) -> Snapshot {
    let toplevel: &gtk4::Widget = toplevel.upcast_ref();
    let window = toplevel
        .translate_coordinates(toplevel, 0.0, 0.0)
        .map(|(x, y)| {
            let a = toplevel.allocation();
            (x as i32, y as i32, a.width(), a.height())
        })
        .unwrap_or((0, 0, 0, 0));
    let mut units = Vec::new();
    collect_into(toplevel, toplevel, &mut units);
    Snapshot {
        window,
        page: page.map(str::to_string),
        units,
    }
}

fn collect_into(w: &gtk4::Widget, toplevel: &gtk4::Widget, out: &mut Vec<Node>) {
    if let Some(kind) = classify(w) {
        if let Some((x, y)) = w.translate_coordinates(toplevel, 0.0, 0.0) {
            let a = w.allocation();
            let mut text = Vec::new();
            super::widgets::widget_text(w, &mut text);
            out.push(Node {
                kind,
                role: format!("{:?}", w.accessible_role()),
                name: text.first().cloned().unwrap_or_default(),
                label: text.join(" | "),
                x: x as i32,
                y: y as i32,
                w: a.width(),
                h: a.height(),
                visible: w.is_visible(),
                sensitive: w.is_sensitive(),
                focused: w.has_focus(),
            });
        }
    }
    let mut child = w.first_child();
    while let Some(c) = child {
        collect_into(&c, toplevel, out);
        child = c.next_sibling();
    }
}

/// Serialize a snapshot to JSON (JSON, not JSON-lite, so any parser works).
pub fn render(snapshot: &Snapshot) -> String {
    let (wx, wy, ww, wh) = snapshot.window;
    let mut out = format!("{{\"window\":{{\"x\":{wx},\"y\":{wy},\"w\":{ww},\"h\":{wh}}},\"page\":");
    match &snapshot.page {
        Some(page) => out.push_str(&format!("\"{}\"", escape(page))),
        None => out.push_str("null"),
    }
    out.push_str(",\"units\":[");
    for (i, n) in snapshot.units.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"id\":{i},\"kind\":\"{}\",\"role\":\"{}\",\"name\":\"{}\",\"label\":\"{}\",\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"visible\":{},\"sensitive\":{},\"focused\":{}}}",
            n.kind,
            escape(&n.role),
            escape(&n.name),
            escape(&n.label),
            n.x,
            n.y,
            n.w,
            n.h,
            n.visible,
            n.sensitive,
            n.focused
        ));
    }
    out.push_str("]}");
    out
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
    fn dump(&self, window: &gtk4::ApplicationWindow, stack: &gtk4::Stack) {
        let text = render(&collect(window, stack.visible_child_name().as_deref()));
        if *self.last.borrow() == Some(text.clone()) {
            return;
        }
        *self.last.borrow_mut() = Some(text.clone());
        if let Ok(mut f) = std::fs::File::create(&self.file) {
            let _ = f.write_all(text.as_bytes());
        }
    }
}

/// Wire the dump under `NIX_PILOT_UI_DUMP`. Safe to call unconditionally;
/// it is a no-op when the env var is unset.
pub fn wire(window: &gtk4::ApplicationWindow, stack: &gtk4::Stack) {
    if std::env::var("NIX_PILOT_UI_DUMP").is_err() {
        return;
    }
    let file = std::env::var("NIX_PILOT_UI_DUMP_FILE")
        .unwrap_or_else(|_| "/tmp/nix-pilot-ui.json".to_string());
    let dumper = Dumper {
        last: Rc::new(RefCell::new(None)),
        file,
    };

    // Initial dump, then re-dump whenever the visible pane changes.
    let d = Rc::new(dumper);
    d.dump(window, stack);

    let d2 = d.clone();
    let win2 = window.clone();
    let stack2 = stack.clone();
    stack.connect_visible_child_name_notify(move |_| d2.dump(&win2, &stack2));

    // Periodic refresh so the file tracks transient states (e.g. opening a
    // detail page, a button relabeling) without extra wiring at every site.
    let d3 = d.clone();
    let win3 = window.clone();
    let stack3 = stack.clone();
    gtk4::glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
        d3.dump(&win3, &stack3);
        gtk4::glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(kind: &'static str, name: &str) -> Node {
        Node {
            kind,
            role: "button".into(),
            name: name.into(),
            label: format!("{name} | tip:detail"),
            x: 1,
            y: 2,
            w: 3,
            h: 4,
            visible: true,
            sensitive: true,
            focused: false,
        }
    }

    #[test]
    fn render_emits_the_snapshot_object_with_escaped_text() {
        let snapshot = Snapshot {
            window: (0, 0, 800, 600),
            page: Some("settings".into()),
            units: vec![
                node("button", "Check \"what\" \\ cleaning"),
                node("row", "firefox  Application"),
            ],
        };
        let json = render(&snapshot);
        assert!(json.starts_with('{') && json.ends_with('}'));
        assert!(json.contains(r#""window":{"x":0,"y":0,"w":800,"h":600}"#));
        assert!(json.contains(r#""page":"settings""#));
        assert!(json.contains(r#""id":0"#) && json.contains(r#""id":1"#));
        assert!(json.contains(r#""name":"Check \"what\" \\ cleaning""#));
        assert!(json.contains(r#""focused":false"#));
        assert!(json.contains("firefox  Application"));
    }

    #[test]
    fn render_emits_null_page_when_none() {
        let snapshot = Snapshot {
            window: (0, 0, 10, 10),
            page: None,
            units: Vec::new(),
        };
        let json = render(&snapshot);
        assert!(json.contains(r#""page":null"#));
        assert!(json.contains(r#""units":[]"#));
    }
}
