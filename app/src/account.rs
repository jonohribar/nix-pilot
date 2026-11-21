//! Read-only account-directory queries used by Settings.
//!
//! This adapter owns the subprocess details and pure output classification;
//! callers receive typed results and decide how to present them.

use std::process::{Command, Stdio};

/// Outcome of checking whether an account name is already present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountPresence {
    /// The account is present in the system account database.
    Exists,
    /// The query completed and the account was not found.
    NotFound,
    /// The query could not be run, so absence cannot be confirmed safely.
    QueryFailed,
}

/// Best-effort display name obtained from the account's GECOS field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayNameResult {
    /// A non-empty GECOS field was found.
    Found(String),
    /// Lookup failed or had no usable GECOS field; use this account name.
    Fallback(String),
}

/// Look up an account's GECOS field, falling back to its name on any failure.
pub fn display_name(account: &str) -> DisplayNameResult {
    let output = Command::new("getent").args(["passwd", account]).output();
    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            match parse_gecos(&stdout) {
                Some(name) => DisplayNameResult::Found(name),
                None => DisplayNameResult::Fallback(account.to_string()),
            }
        }
        _ => DisplayNameResult::Fallback(account.to_string()),
    }
}

/// Check for a name collision using the system account database.
///
/// A command-launch failure is not treated as proof that the name is free.
pub fn check_presence(account: &str) -> AccountPresence {
    let status = Command::new("id")
        .args(["-u", "--", account])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .map(|status| status.success());
    presence_from_status(status)
}

fn presence_from_status(status: Option<bool>) -> AccountPresence {
    match status {
        Some(true) => AccountPresence::Exists,
        Some(false) => AccountPresence::NotFound,
        None => AccountPresence::QueryFailed,
    }
}

fn parse_gecos(passwd_output: &str) -> Option<String> {
    let gecos = passwd_output.lines().next()?.split(':').nth(4)?.trim();
    (!gecos.is_empty()).then(|| gecos.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gecos_display_name() {
        assert_eq!(
            parse_gecos("alice:x:1000:1000:Alice Example,,,:/home/alice:/bin/bash\n"),
            Some("Alice Example,,,".into())
        );
    }

    #[test]
    fn missing_or_empty_gecos_has_no_display_name() {
        assert_eq!(
            parse_gecos("alice:x:1000:1000::/home/alice:/bin/bash\n"),
            None
        );
        assert_eq!(parse_gecos("malformed"), None);
        assert_eq!(parse_gecos(""), None);
    }

    #[test]
    fn display_name_result_falls_back_to_account_name() {
        assert_eq!(
            display_name_from_output("alice", false, ""),
            DisplayNameResult::Fallback("alice".into())
        );
        assert_eq!(
            display_name_from_output("alice", true, "alice:x:1000:1000::/home/alice:/bin/bash"),
            DisplayNameResult::Fallback("alice".into())
        );
    }

    #[test]
    fn display_name_result_uses_gecos_when_available() {
        assert_eq!(
            display_name_from_output(
                "alice",
                true,
                "alice:x:1000:1000:Alice Example:/home/alice:/bin/bash"
            ),
            DisplayNameResult::Found("Alice Example".into())
        );
    }

    #[test]
    fn command_failure_is_not_treated_as_an_available_account_name() {
        assert_eq!(presence_from_status(None), AccountPresence::QueryFailed);
        assert_eq!(presence_from_status(Some(true)), AccountPresence::Exists);
        assert_eq!(presence_from_status(Some(false)), AccountPresence::NotFound);
    }

    fn display_name_from_output(account: &str, succeeded: bool, output: &str) -> DisplayNameResult {
        if succeeded {
            if let Some(name) = parse_gecos(output) {
                return DisplayNameResult::Found(name);
            }
        }
        DisplayNameResult::Fallback(account.to_string())
    }
}
