//! Minimal GTK renderer for one Blueprint-compiled `.ui` file and stylesheet.
//!
//! Takes the UI file and CSS file as its two positional arguments, applies the
//! stylesheet before presenting the window, and runs the GTK event loop. It
//! is intended for screenshot capture under a virtual display.
use gtk4::prelude::*;
use log::{error, info};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("nix-pilot wireframe starting");

    let mut args = std::env::args().skip(1);
    let (ui, css) = match (args.next(), args.next()) {
        (Some(u), Some(c)) => (u, c),
        _ => {
            error!("usage: wireframe <window.ui> <style.css>");
            std::process::exit(1);
        }
    };

    if gtk4::init().is_err() {
        error!("failed to initialise GTK");
        std::process::exit(1);
    }
    if libadwaita::init().is_err() {
        error!("failed to initialise libadwaita");
        std::process::exit(1);
    }

    // stylesheet, before any window realises
    let css_text = match std::fs::read_to_string(&css) {
        Ok(t) => t,
        Err(e) => {
            error!("cannot read css {}: {}", css, e);
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

    glib::MainLoop::new(None, false).run()
}
