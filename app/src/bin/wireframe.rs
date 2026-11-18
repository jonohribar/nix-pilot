// Wireframe renderer: loads a GTK .ui file (compiled from Blueprint) + a CSS
// file, applies the stylesheet, presents the window, and runs the main loop.
// Used with Xvfb + ImageMagick to produce wireframe screenshots:
//   xvfb-run -a sh -c 'target/debug/wireframe build/foo.ui style.css & \
//                      sleep 2; import -window root foo.png'
use gtk4::prelude::*;

fn main() {
    let mut args = std::env::args().skip(1);
    let (ui, css) = match (args.next(), args.next()) {
        (Some(u), Some(c)) => (u, c),
        _ => {
            eprintln!("usage: wireframe <window.ui> <style.css>");
            std::process::exit(1);
        }
    };

    if gtk4::init().is_err() {
        eprintln!("failed to initialise GTK");
        std::process::exit(1);
    }
    if libadwaita::init().is_err() {
        eprintln!("failed to initialise libadwaita");
        std::process::exit(1);
    }

    // stylesheet, before any window realises
    let css_text = match std::fs::read_to_string(&css) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read css {}: {}", css, e);
            std::process::exit(1);
        }
    };
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(&css_text);
    if let Some(d) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &d,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    // window from blueprint-compiled ui
    let builder = gtk4::Builder::from_file(ui);
    let window: gtk4::Window = builder
        .object("window")
        .expect("ui must define a widget id `window`");
    window.present();

    glib::MainLoop::new(None, false).run();
}
