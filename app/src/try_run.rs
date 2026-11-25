//! "Try without installing" lifecycle.
//!
//! Runs `nix run nixpkgs#<attr>` as a detached child process with a
//! four-state machine: Starting → Running → Ended/Stopped. The child is
//! killed on cancel, stop, or parent exit (no orphaned processes).

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::install::discover_nix;

/// User-visible state of a try session.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TryState {
    /// Idle — no session active, button shows "Try it".
    Idle,
    /// Building/downloading — first run may take minutes. Cancel available.
    Starting,
    /// App window is up; user is interacting with it. Stop available.
    Running,
    /// Session ended (app closed or stopped). Returns to Idle after acknowledgement.
    Ended { stopped_by_user: bool },
}

impl TryState {
    /// Label for the primary action button in this state.
    pub fn button_label(self) -> &'static str {
        match self {
            TryState::Idle => "Try it",
            TryState::Starting => "Cancel",
            TryState::Running => "Stop",
            TryState::Ended { .. } => "Try it",
        }
    }

    /// Tooltip text for the button in this state.
    pub fn button_tooltip(self) -> &'static str {
        match self {
            TryState::Idle => "Run this app without installing it",
            TryState::Starting => "Cancel the build and download",
            TryState::Running => "Stop the running app",
            TryState::Ended {
                stopped_by_user: true,
            } => "Run again — the app was stopped",
            TryState::Ended {
                stopped_by_user: false,
            } => "Run again — the app exited",
        }
    }

    /// Status text shown below the button.
    pub fn status_text(self, app_name: &str) -> String {
        match self {
            TryState::Idle => String::new(),
            TryState::Starting => format!("Building {app_name} — the first run downloads and builds it; this can take a few minutes."),
            TryState::Running => format!("Running {app_name} — close the app window or press Stop to end the session."),
            TryState::Ended { stopped_by_user: true } => {
                "Stopped. It wasn't installed — choose 'Install for me' to keep it.".to_string()
            }
            TryState::Ended { stopped_by_user: false } => {
                "Finished. It wasn't installed — choose 'Install for me' to keep it.".to_string()
            }
        }
    }

    /// Whether the button is sensitive (clickable) in this state.
    pub fn is_sensitive(self) -> bool {
        matches!(
            self,
            TryState::Idle | TryState::Starting | TryState::Running | TryState::Ended { .. }
        )
    }
}

/// Handle to a running try session. Keeps the child process alive and
/// provides cancellation. When dropped without explicit stop, the child
/// is reaped (parent-exit guard).
pub struct TrySession {
    child: Option<Arc<Mutex<Option<std::process::Child>>>>,
    /// Whether the underlying child is still alive (not yet exited).
    alive: Arc<AtomicBool>,
}

impl TrySession {
    /// Spawn `nix run nixpkgs#<attr>` as a detached child and return the session.
    pub fn spawn(
        attr: String,
        allow_unfree: bool,
    ) -> Result<(Self, mpsc::Receiver<TryState>), String> {
        let nix_path = discover_nix().ok_or("nix not found on this system")?;

        let mut cmd = Command::new(&nix_path);
        cmd.args(["run", "--", &format!("nixpkgs#{attr}")]);
        if allow_unfree {
            cmd.env("NIXPKGS_ALLOW_UNFREE", "1");
        }

        // Create a new process group so we can kill the whole tree.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("failed to start nix run: {e}"))?;

        let child_shared = Arc::new(Mutex::new(Some(child)));
        let alive = Arc::new(AtomicBool::new(true));
        let child_alive = alive.clone();

        let (monitor_tx, monitor_rx) = mpsc::channel();
        let child_for_monitor = child_shared.clone();
        thread::spawn(move || {
            let exit_status = {
                let mut guard = child_for_monitor.lock().unwrap();
                if let Some(child) = guard.as_mut() {
                    child.wait()
                } else {
                    return;
                }
            };
            child_alive.store(false, Ordering::SeqCst);
            let stopped_by_user = exit_status.is_err();
            let terminal_state = TryState::Ended { stopped_by_user };
            let _ = monitor_tx.send(terminal_state);
        });

        let session = TrySession {
            child: Some(child_shared),
            alive,
        };

        Ok((session, monitor_rx))
    }

    /// Whether the try child is still alive; used to move from Starting to
    /// Running once the app is up (there is no build-complete signal from
    /// `nix run`, so the UI uses a grace period).
    pub fn is_running(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Cancel a Starting session or Stop a Running session.
    /// Kills the entire process group.
    pub fn terminate(&mut self) -> Result<(), String> {
        self.alive.store(false, Ordering::SeqCst);
        if let Some(child_shared) = self.child.take() {
            let mut guard = child_shared.lock().unwrap();
            if let Some(mut child) = guard.take() {
                #[cfg(unix)]
                {
                    let pgid = child.id();
                    unsafe {
                        libc::kill(-(pgid as i32), libc::SIGTERM);
                    }
                    thread::sleep(Duration::from_millis(500));
                    let _ = child.try_wait();
                    let _ = child.kill();
                }

                #[cfg(not(unix))]
                {
                    let _ = child.kill();
                }
            }
        }
        Ok(())
    }
}

impl Drop for TrySession {
    /// Parent-exit reaping: if the app quits while a session is running,
    /// kill the child process group so no orphaned "try" processes survive.
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

/// Determine if an entry should show the "Try it" button.
/// Per spec: rich and basic desktop applications only (Class::App).
/// CLI tools (Class::Cli) and terminal apps (Class::TermApp) are excluded.
pub fn should_show_try_button(entry: &crate::model::IndexEntry) -> bool {
    matches!(entry.class, crate::model::Class::App)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_state_labels() {
        assert_eq!(TryState::Idle.button_label(), "Try it");
        assert_eq!(TryState::Starting.button_label(), "Cancel");
        assert_eq!(TryState::Running.button_label(), "Stop");
        assert_eq!(
            TryState::Ended {
                stopped_by_user: false
            }
            .button_label(),
            "Try it"
        );
        assert_eq!(
            TryState::Ended {
                stopped_by_user: true
            }
            .button_label(),
            "Try it"
        );
    }

    #[test]
    fn try_state_status_texts() {
        let idle = TryState::Idle.status_text("Firefox");
        assert_eq!(idle, "");

        let starting = TryState::Starting.status_text("Firefox");
        assert!(starting.contains("Building Firefox"));
        assert!(starting.contains("first run"));

        let running = TryState::Running.status_text("Firefox");
        assert!(running.contains("Running Firefox"));
        assert!(running.contains("Stop"));

        let ended = TryState::Ended {
            stopped_by_user: false,
        }
        .status_text("Firefox");
        assert!(ended.contains("Finished"));
        assert!(ended.contains("wasn't installed"));

        let stopped = TryState::Ended {
            stopped_by_user: true,
        }
        .status_text("Firefox");
        assert!(stopped.contains("Stopped"));
        assert!(stopped.contains("wasn't installed"));
    }

    #[test]
    fn try_state_sensitivity() {
        assert!(TryState::Idle.is_sensitive());
        assert!(TryState::Starting.is_sensitive());
        assert!(TryState::Running.is_sensitive());
        assert!(TryState::Ended {
            stopped_by_user: false
        }
        .is_sensitive());
        assert!(TryState::Ended {
            stopped_by_user: true
        }
        .is_sensitive());
    }

    #[test]
    fn should_show_try_button_for_app_class() {
        use crate::model::{Class, IndexEntry, Tier};
        let entry = IndexEntry {
            attr: "firefox".into(),
            pname: Some("Firefox".into()),
            version: None,
            description: None,
            home_page: None,
            main_program: None,
            unfree: false,
            linux_ok: true,
            class: Class::App,
            tier: Tier::Rich,
            install: None,
        };
        assert!(should_show_try_button(&entry));
    }

    #[test]
    fn should_not_show_try_button_for_cli() {
        use crate::model::{Class, IndexEntry, Tier};
        let entry = IndexEntry {
            attr: "ripgrep".into(),
            pname: Some("ripgrep".into()),
            version: None,
            description: None,
            home_page: None,
            main_program: None,
            unfree: false,
            linux_ok: true,
            class: Class::Cli,
            tier: Tier::Other,
            install: None,
        };
        assert!(!should_show_try_button(&entry));
    }

    #[test]
    fn should_not_show_try_button_for_term_app() {
        use crate::model::{Class, IndexEntry, Tier};
        let entry = IndexEntry {
            attr: "htop".into(),
            pname: Some("htop".into()),
            version: None,
            description: None,
            home_page: None,
            main_program: None,
            unfree: false,
            linux_ok: true,
            class: Class::TermApp,
            tier: Tier::Basic,
            install: None,
        };
        assert!(!should_show_try_button(&entry));
    }

    #[test]
    fn should_not_show_try_button_for_unverified() {
        use crate::model::{Class, IndexEntry, Tier};
        let entry = IndexEntry {
            attr: "mystery-app".into(),
            pname: Some("Mystery App".into()),
            version: None,
            description: None,
            home_page: None,
            main_program: None,
            unfree: false,
            linux_ok: true,
            class: Class::Unverified,
            tier: Tier::Unverified,
            install: None,
        };
        assert!(!should_show_try_button(&entry));
    }
}
