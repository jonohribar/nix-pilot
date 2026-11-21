//! Store-selection helpers that do not own persistent package state.
//!
//! Package intent is owned by `desired_state::DesiredDoc`; this module keeps
//! the small selection-staleness rule used by asynchronous detail checks.

/// Whether a finished availability check still applies to the current
/// selection (a check for entry X must not update the card of entry Y).
pub fn result_matches_selection(selected_attr: &str, checked_attr: &str) -> bool {
    selected_attr == checked_attr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_result_does_not_apply() {
        assert!(result_matches_selection("firefox", "firefox"));
        assert!(!result_matches_selection("gimp", "firefox"));
    }
}
