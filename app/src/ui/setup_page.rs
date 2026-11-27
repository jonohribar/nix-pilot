//! First-run setup surface (config-authority.md, "Install and first run"):
//! the initial stack page until the user's configuration references
//! nix-pilot — detect, show the exact snippet, parse-verify after pasting,
//! then hand over to the store. Stays in the stack so Settings can reopen it.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::config_source::ConfigRoot;
use crate::setup;

pub struct SetupPage {
    pub page: gtk4::Box,
    root: Rc<RefCell<Option<ConfigRoot>>>,
    status: gtk4::Label,
    snippet: gtk4::Label,
    snippet_section: gtk4::Box,
    copy_btn: gtk4::Button,
    open_btn: gtk4::Button,
    verify_btn: gtk4::Button,
    message: gtk4::Label,
    recheck_btn: gtk4::Button,
}

impl SetupPage {
    /// Build the setup surface and run the initial detection.
    pub fn new(window: &gtk4::ApplicationWindow, stack: &gtk4::Stack) -> SetupPage {
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        page.set_margin_top(24);
        page.set_margin_bottom(24);
        page.set_margin_start(24);
        page.set_margin_end(24);
        page.set_valign(gtk4::Align::Start);
        page.set_halign(gtk4::Align::Center);
        page.set_width_request(720);

        let title = gtk4::Label::new(Some("Set up nix-pilot"));
        title.add_css_class("title-2");
        title.set_halign(gtk4::Align::Start);
        page.append(&title);

        let blurb = gtk4::Label::new(Some(
            "nix-pilot manages your NixOS configuration through a small module \
             inside it. One-time setup: add two lines, rebuild once, and every \
             change after that happens in this app.",
        ));
        blurb.set_wrap(true);
        blurb.set_xalign(0.0);
        blurb.set_halign(gtk4::Align::Start);
        page.append(&blurb);

        let status = gtk4::Label::new(None);
        status.set_wrap(true);
        status.set_xalign(0.0);
        status.set_halign(gtk4::Align::Start);
        page.append(&status);

        // The snippet section: the exact lines to paste, plus the actions.
        let snippet_section = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        let snippet = gtk4::Label::new(None);
        snippet.add_css_class("monospace");
        snippet.add_css_class("view");
        snippet.set_wrap(true);
        snippet.set_xalign(0.0);
        snippet.set_selectable(true);
        snippet_section.append(&snippet);

        let actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let copy_btn = gtk4::Button::with_label("Copy");
        let open_btn = gtk4::Button::with_label("Open my config");
        let verify_btn = gtk4::Button::with_label("I've added it — verify");
        verify_btn.add_css_class("suggested-action");
        actions.append(&copy_btn);
        actions.append(&open_btn);
        actions.append(&verify_btn);
        snippet_section.append(&actions);
        page.append(&snippet_section);

        let message = gtk4::Label::new(None);
        message.set_wrap(true);
        message.set_xalign(0.0);
        message.set_halign(gtk4::Align::Start);
        page.append(&message);

        let recheck_btn = gtk4::Button::with_label("Check again");
        page.append(&recheck_btn);

        let setup = SetupPage {
            page,
            root: Rc::new(RefCell::new(None)),
            status,
            snippet,
            snippet_section,
            copy_btn,
            open_btn,
            verify_btn,
            message,
            recheck_btn,
        };
        setup.wire(window, stack);
        setup.refresh();
        setup
    }

    /// Re-run detection and re-render the page state.
    pub fn refresh(&self) {
        *self.root.borrow_mut() = setup::resolve_root();
        self.message.set_text("");
        let root = self.root.borrow().clone();
        let integrated = root.as_ref().is_some_and(setup::is_integrated);
        render_state(
            root.as_ref(),
            integrated,
            &self.status,
            &self.snippet,
            &self.snippet_section,
            &self.recheck_btn,
        );
    }

    fn wire(&self, window: &gtk4::ApplicationWindow, stack: &gtk4::Stack) {
        let copy_for = self.copy_btn.clone();
        let snippet_for_copy = self.snippet.clone();
        let window_for_copy = window.clone();
        copy_for.connect_clicked(move |_| {
            window_for_copy
                .clipboard()
                .set_text(&snippet_for_copy.text());
        });

        let open_for = self.open_btn.clone();
        let root_for_open = self.root.clone();
        open_for.connect_clicked(move |_| {
            if let Some(root) = root_for_open.borrow().clone() {
                if let Err(error) = crate::process::open_folder(&root.path) {
                    log::warn!("could not open the configuration folder: {error}");
                }
            }
        });

        let verify_for = self.verify_btn.clone();
        let root_for_verify = self.root.clone();
        let message_for = self.message.clone();
        let stack_for_done = stack.clone();
        let verify_handler = verify_for.clone();
        verify_for.connect_clicked(move |_| {
            let Some(root) = root_for_verify.borrow().clone() else {
                return;
            };
            let verify_inner = verify_handler.clone();
            let message_inner = message_for.clone();
            let stack_inner = stack_for_done.clone();
            verify_inner.set_sensitive(false);
            message_inner.set_text("Checking that your configuration parses…");
            crate::ui::async_task::run_on_main(
                std::time::Duration::from_millis(100),
                move || setup::verify_parse(&root),
                move |result| {
                    verify_inner.set_sensitive(true);
                    match result {
                        Ok(Ok(())) => {
                            message_inner
                                .set_text("Connected — nix-pilot is ready. Opening the store.");
                            stack_inner.set_visible_child_name("list");
                        }
                        Ok(Err(error)) => message_inner
                            .set_text(&format!("Your configuration does not parse yet: {error}")),
                        Err(()) => message_inner
                            .set_text("The check ended before a result was received — try again."),
                    }
                },
            );
        });

        let recheck_handler = self.recheck_btn.clone();
        let root_for_recheck = self.root.clone();
        let status_for = self.status.clone();
        let snippet_for_state = self.snippet.clone();
        let section_for = self.snippet_section.clone();
        let message_for_recheck = self.message.clone();
        let recheck_inner = recheck_handler.clone();
        recheck_handler.connect_clicked(move |_| {
            *root_for_recheck.borrow_mut() = setup::resolve_root();
            message_for_recheck.set_text("");
            let root = root_for_recheck.borrow().clone();
            let integrated = root.as_ref().is_some_and(setup::is_integrated);
            render_state(
                root.as_ref(),
                integrated,
                &status_for,
                &snippet_for_state,
                &section_for,
                &recheck_inner,
            );
        });
    }
}

/// Render the page for a detected root (or its absence). `integrated` is
/// only meaningful when a root exists.
fn render_state(
    root: Option<&ConfigRoot>,
    integrated: bool,
    status: &gtk4::Label,
    snippet: &gtk4::Label,
    snippet_section: &gtk4::Box,
    recheck_btn: &gtk4::Button,
) {
    match root {
        Some(root) if integrated => {
            status.set_text(&format!(
                "Connected — your configuration at {} imports the nix-pilot \
                 module. You can manage everything from the sidebar.",
                root.path.display()
            ));
            snippet_section.set_visible(false);
            recheck_btn.set_visible(false);
        }
        Some(root) => {
            let kind = match root.kind {
                crate::config_source::ConfigKind::Flake => "flake",
                crate::config_source::ConfigKind::Classic => "configuration.nix",
            };
            status.set_text(&format!(
                "Found your configuration: {} at {}",
                kind,
                root.path.display()
            ));
            snippet.set_text(&setup::snippet_for(root.kind));
            snippet_section.set_visible(true);
            recheck_btn.set_visible(false);
        }
        None => {
            status.set_text(
                "No NixOS configuration was found on this computer.\n\n\
                 nix-pilot manages a NixOS configuration file. Point it at \
                 one: a flake in a well-known place (for example \
                 ~/nixos-config), or /etc/nixos/configuration.nix.\n\n\
                 New machines: the one-click installer that creates a fresh \
                 configuration is planned; for now set up a configuration \
                 first, then connect it here.",
            );
            snippet_section.set_visible(false);
            recheck_btn.set_visible(true);
        }
    }
}
