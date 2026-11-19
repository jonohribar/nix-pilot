//! Application actions: the desired-change queue and the check-result rule.
//! Pure logic — no GTK, no Nix. The UI translates user actions into these and
//! renders the outcome; the persistent desired-state model lives in
//! `desired_state.rs` and is kept in sync with this queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingChange {
    pub attr: String,
    pub name: String,
    pub system_wide: bool,
}

/// In-memory queue of changes destined for the Updates transaction.
#[derive(Debug, Default)]
pub struct ChangeQueue {
    changes: Vec<PendingChange>,
}

impl ChangeQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a change; a duplicate attr is ignored (returns false).
    pub fn add(&mut self, c: PendingChange) -> bool {
        if self.changes.iter().any(|p| p.attr == c.attr) {
            return false;
        }
        self.changes.push(c);
        true
    }

    /// Remove by attr; returns whether anything was removed.
    pub fn remove(&mut self, attr: &str) -> bool {
        let before = self.changes.len();
        self.changes.retain(|p| p.attr != attr);
        self.changes.len() != before
    }

    pub fn contains(&self, attr: &str) -> bool {
        self.changes.iter().any(|p| p.attr == attr)
    }

    pub fn len(&self) -> usize {
        self.changes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// (name, system_wide) pairs for display.
    pub fn summary(&self) -> Vec<(String, bool)> {
        self.changes
            .iter()
            .map(|p| (p.name.clone(), p.system_wide))
            .collect()
    }

    /// The queued changes (attr + scope intact).
    pub fn items(&self) -> &[PendingChange] {
        &self.changes
    }
}

/// Whether a finished availability check still applies to the current
/// selection (a check for entry X must not update the card of entry Y).
pub fn result_matches_selection(selected_attr: &str, checked_attr: &str) -> bool {
    selected_attr == checked_attr
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(attr: &str) -> PendingChange {
        PendingChange {
            attr: attr.into(),
            name: attr.into(),
            system_wide: false,
        }
    }

    #[test]
    fn queue_adds_dedupes_removes() {
        let mut q = ChangeQueue::new();
        assert!(q.add(change("firefox")));
        assert!(!q.add(change("firefox"))); // duplicate ignored
        assert!(q.add(change("gimp")));
        assert_eq!(q.len(), 2);
        assert!(q.contains("gimp"));
        assert!(q.remove("gimp"));
        assert!(!q.remove("gimp"));
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn summary_lists_entries_with_scope() {
        let mut q = ChangeQueue::new();
        q.add(PendingChange {
            attr: "x".into(),
            name: "Firefox".into(),
            system_wide: true,
        });
        assert_eq!(q.summary(), vec![("Firefox".into(), true)]);
    }

    #[test]
    fn items_round_trip_attr_and_scope() {
        // underpins the desired-state seed path (shell loads the queue from
        // the persisted file through items())
        let mut q = ChangeQueue::new();
        q.add(PendingChange {
            attr: "firefox".into(),
            name: "Firefox".into(),
            system_wide: false,
        });
        q.add(PendingChange {
            attr: "gimp".into(),
            name: "GIMP".into(),
            system_wide: true,
        });
        let items = q.items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].attr, "firefox");
        assert!(!items[0].system_wide);
        assert_eq!(items[1].attr, "gimp");
        assert!(items[1].system_wide);
        assert!(!q.is_empty());
    }

    #[test]
    fn empty_queue_is_empty() {
        let q = ChangeQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert!(q.items().is_empty());
    }

    #[test]
    fn stale_result_does_not_apply() {
        assert!(result_matches_selection("firefox", "firefox"));
        assert!(!result_matches_selection("gimp", "firefox"));
    }
}
