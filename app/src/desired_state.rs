//! Desired-state model (7wo.2): the app owns *what the user wants*, and the
//! NixOS configuration is generated output (architecture / Product model,
//! decision D9). This module is the pure owner layer: it holds the installed
//! package set by scope and renders it into the generated NixOS module that
//! the backend would feed to `nixos-rebuild`. Nothing here touches Nix or the
//! filesystem; the build/apply path that consumes the rendered module stays
//! gated until the backend lands (7wo.2) — this file only produces config.
//!
//! Scope per D9: per-user by default (`users.users.<name>.packages`),
//! system-wide only when explicitly chosen (`environment.systemPackages`).
//! Attributes are nixpkgs attribute names (e.g. `firefox`, `vscode`), rendered
//! under `with pkgs;`, and stay sorted so output is deterministic.
use std::collections::BTreeSet;

/// The system's user whose per-user packages are generated into the config.
/// Read once at apply time by the backend (e.g. the account that owns the
/// install); the renderer takes it as a parameter to stay pure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Account<'a>(pub &'a str);

/// The application-owned desired package set, split by install scope.
#[derive(Debug, Default, Clone)]
pub struct DesiredState {
    user_scope: BTreeSet<String>,
    system_scope: BTreeSet<String>,
}

impl DesiredState {
    /// Add or remove a package in the given scope (idempotent).
    pub fn set(&mut self, attr: impl Into<String>, system_wide: bool) {
        let s = attr.into();
        let (add, remove) = if system_wide {
            (&mut self.system_scope, &mut self.user_scope)
        } else {
            (&mut self.user_scope, &mut self.system_scope)
        };
        add.insert(s.clone());
        remove.remove(&s);
    }

    /// Render a complete NixOS module for the current desired state. Sections
    /// with no packages are omitted; output is sorted and byte-stable.
    pub fn render_module(&self, account: Account) -> String {
        let mut body = String::new();
        if !self.system_scope.is_empty() {
            let pkgs = join(&self.system_scope);
            body.push_str(&format!(
                "  environment.systemPackages = with pkgs; [ {pkgs} ];\n"
            ));
        }
        if !self.user_scope.is_empty() {
            let pkgs = join(&self.user_scope);
            body.push_str(&format!(
                "  users.users.{} = {{\n    packages = with pkgs; [ {pkgs} ];\n  }};\n",
                account.0
            ));
        }
        if body.is_empty() {
            body.push_str("  # no packages selected\n");
        }
        format!("{{ config, pkgs, ... }}:\n\n{{\n{body}}}\n")
    }

    #[cfg(test)]
    fn user(&self) -> &BTreeSet<String> {
        &self.user_scope
    }

    #[cfg(test)]
    fn system(&self) -> &BTreeSet<String> {
        &self.system_scope
    }
}

fn join(set: &BTreeSet<String>) -> String {
    set.iter().cloned().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_scope_renders_under_account() {
        let mut d = DesiredState::default();
        d.set("vscode", false);
        d.set("firefox", false);
        assert_eq!(d.render_module(Account("alice")),
            "{ config, pkgs, ... }:\n\n{\n  users.users.alice = {\n    packages = with pkgs; [ firefox vscode ];\n  };\n}\n");
    }

    #[test]
    fn system_scope_renders_system_packages() {
        let mut d = DesiredState::default();
        d.set("gimp", true);
        assert_eq!(
            d.render_module(Account("bob")),
            "{ config, pkgs, ... }:\n\n{\n  environment.systemPackages = with pkgs; [ gimp ];\n}\n"
        );
    }

    #[test]
    fn moving_scope_hides_from_other_scope() {
        let mut d = DesiredState::default();
        d.set("firefox", false);
        d.set("firefox", true); // user->system
        assert!(d.user().is_empty());
        assert!(d.system().contains("firefox"));
    }

    #[test]
    fn empty_emits_comment() {
        let d = DesiredState::default();
        assert_eq!(
            d.render_module(Account("alice")),
            "{ config, pkgs, ... }:\n\n{\n  # no packages selected\n}\n"
        );
    }

    /// Live proof (opt-in): the rendered module is *valid Nix*. Runs
    /// `nix-instantiate --parse` on it; a parse success means the generated
    /// config would be accepted by nixos-rebuild at the syntax level. Safe:
    /// parse only, never evaluates or builds. Needs nix on PATH.
    #[test]
    #[ignore = "requires nix-instantiate on PATH"]
    fn rendered_module_parses_with_nix() {
        let mut d = DesiredState::default();
        d.set("firefox", false);
        d.set("gimp", true);
        d.set("vscode", false);
        let nix = d.render_module(Account("tester"));

        let mut path = std::env::temp_dir();
        path.push(format!("nix-pilot-desired-{}.nix", std::process::id()));
        std::fs::write(&path, &nix).unwrap();
        let out = std::process::Command::new("nix-instantiate")
            .arg("--parse")
            .arg(&path)
            .output();
        let _ = std::fs::remove_file(&path);
        let out = out.expect("run nix-instantiate");
        assert!(
            out.status.success(),
            "generated config did not parse:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
