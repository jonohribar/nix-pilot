//! Read-only listing and formatting for installed NixOS generations.
//!
//! Filesystem inspection is kept separate from configuration building and
//! privileged activation so generation views can be tested independently.

use std::path::Path;

/// One system generation as the Updates page displays it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationInfo {
    /// Generation number parsed from a `system-N-link` profile name.
    pub number: u64,
    /// Generation-link modification time in `YYYY-MM-DD HH:MM` form.
    pub date: String,
    /// Nixpkgs version from the linked store path, when it can be parsed.
    pub version: Option<String>,
    /// Whether this is the generation targeted by the current `system` link.
    pub current: bool,
}

/// Parse the generation number from a profile name such as `system-42-link`.
pub fn parse_generation_number(name: &str) -> Option<u64> {
    let rest = name.strip_prefix("system-")?.strip_suffix("-link")?;
    rest.parse().ok()
}

/// Extract the version suffix from a NixOS system store path.
pub fn parse_store_version(name: &str) -> Option<String> {
    let base = name.rsplit('/').next().unwrap_or(name);
    let fields: Vec<&str> = base.split('-').collect();
    let position = fields
        .iter()
        .rposition(|field| field.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let version = fields[position..].join("-");
    version.contains('.').then_some(version)
}

/// List world-readable system profile links, newest first.
///
/// Returns an empty list when the profile directory cannot be read. The
/// current generation is identified from the `system` symlink target.
pub fn list_generations() -> Vec<GenerationInfo> {
    list_generations_in(Path::new("/nix/var/nix/profiles"))
}

fn list_generations_in(directory: &Path) -> Vec<GenerationInfo> {
    let mut generations = Vec::new();
    let Ok(entries) = std::fs::read_dir(directory) else {
        return generations;
    };
    let current_target = std::fs::read_link(directory.join("system"))
        .ok()
        .and_then(|target| {
            target
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        });

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(number) = parse_generation_number(&name) else {
            continue;
        };
        let date = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .map(
                |modified| match modified.duration_since(std::time::UNIX_EPOCH) {
                    Ok(duration) => format_epoch(duration.as_secs()),
                    Err(_) => "unknown date".into(),
                },
            )
            .unwrap_or_else(|| "unknown date".into());
        let version = std::fs::read_link(entry.path())
            .ok()
            .and_then(|target| target.to_str().map(str::to_string))
            .and_then(|target| parse_store_version(&target));
        generations.push(GenerationInfo {
            number,
            date,
            version,
            current: current_target.as_deref() == Some(&name),
        });
    }
    generations.sort_by_key(|generation| std::cmp::Reverse(generation.number));
    generations
}

/// Format a Unix timestamp as UTC `YYYY-MM-DD HH:MM` without a date library.
pub fn format_epoch(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let remainder = secs % 86400;
    let (hour, minute) = (remainder / 3600, (remainder % 3600) / 60);
    // Howard Hinnant's civil-from-days algorithm.
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

/// Give a generation's `YYYY-MM-DD HH:MM` date a human day label.
///
/// Returns `Today HH:MM` or `Yesterday HH:MM` when the generation was
/// modified on the reference day or the day before it, otherwise the
/// original date. The reference uses the same UTC day as [`format_epoch`],
/// so a caller passes `now` as epoch seconds without a date library.
pub fn humanize_day(date: &str, now: u64) -> String {
    let Some(day) = date.get(..10) else {
        return date.to_string();
    };
    let today = &format_epoch(now)[..10];
    if day == today {
        return format!("Today{}", &date[10..]);
    }
    let yesterday = &format_epoch(now.saturating_sub(86400))[..10];
    if day == yesterday {
        return format!("Yesterday{}", &date[10..]);
    }
    date.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generation_numbers() {
        assert_eq!(parse_generation_number("system-42-link"), Some(42));
        assert_eq!(parse_generation_number("system-0-link"), Some(0));
        assert_eq!(parse_generation_number("default"), None);
        assert_eq!(parse_generation_number("system-link"), None);
        assert_eq!(parse_generation_number("system-42"), None);
    }

    #[test]
    fn parses_store_versions() {
        assert_eq!(
            parse_store_version("/nix/store/abc-nixos-system-laptop-26.05.20261020.cd4ea8d"),
            Some("26.05.20261020.cd4ea8d".into())
        );
        assert_eq!(
            parse_store_version("nixos-system-nixos-26.11.20261011.7fd4004"),
            Some("26.11.20261011.7fd4004".into())
        );
        assert_eq!(parse_store_version("nixos-system-host"), None);
        assert_eq!(parse_store_version(""), None);
    }

    #[test]
    fn humanizes_relative_days() {
        // A fixed reference day (2024-10-04 00:00 UTC): same-day, previous-day,
        // and older timestamps get Today / Yesterday / the raw date.
        let now = 1_728_000_000;
        assert_eq!(humanize_day("2024-10-04 12:35", now), "Today 12:35");
        assert_eq!(humanize_day("2024-10-03 09:05", now), "Yesterday 09:05");
        assert_eq!(humanize_day("2024-10-01 22:00", now), "2024-10-01 22:00");
        // Malformed input passes through unchanged.
        assert_eq!(humanize_day("unknown date", now), "unknown date");
    }

    #[test]
    fn formats_epoch_times() {
        assert_eq!(format_epoch(0), "1970-01-01 00:00");
        assert_eq!(format_epoch(86400), "1970-01-02 00:00");
        assert_eq!(format_epoch(1_700_000_000), "2023-11-14 22:13");
        assert_eq!(format_epoch(1_728_000_000), "2024-10-04 00:00");
    }

    #[test]
    fn lists_generations_newest_first_and_marks_the_current_link() {
        use std::os::unix::fs::symlink;

        let directory = std::env::temp_dir().join(format!("np-generations-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        for target in ["nixos-system-host-26.05.old", "nixos-system-host-26.11.new"] {
            std::fs::write(directory.join(target), "generation").unwrap();
        }
        symlink(
            directory.join("nixos-system-host-26.05.old"),
            directory.join("system-1-link"),
        )
        .unwrap();
        symlink(
            directory.join("nixos-system-host-26.11.new"),
            directory.join("system-2-link"),
        )
        .unwrap();
        symlink("system-2-link", directory.join("system")).unwrap();

        let generations = list_generations_in(&directory);
        let _ = std::fs::remove_dir_all(&directory);
        assert_eq!(
            generations
                .iter()
                .map(|generation| generation.number)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
        assert_ne!(generations[0].date, "unknown date");
        assert_eq!(generations[0].version.as_deref(), Some("26.11.new"));
        assert!(generations[0].current);
        assert!(!generations[1].current);
    }

    #[test]
    #[ignore = "requires a NixOS host with system generations"]
    fn live_list_generations() {
        let generations = list_generations();
        assert!(!generations.is_empty(), "no generations found");
        assert_eq!(
            generations
                .iter()
                .filter(|generation| generation.current)
                .count(),
            1,
            "exactly one generation is current"
        );
        assert!(generations.iter().all(|generation| generation.number > 0));
        assert!(generations
            .windows(2)
            .all(|pair| pair[0].number >= pair[1].number));
    }
}
