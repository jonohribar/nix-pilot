//! Catalogue consumption (7wo.4, first slice): resolve where the catalogue
//! index comes from and load it honestly.
//!
//! Sources, in precedence order: the first CLI argument, then
//! `$NIX_PILOT_CATALOGUE` (a local path or an https:// URL), then the
//! conventional local paths under the repo. A URL is downloaded with curl
//! into the app cache and re-fetched on each start (freshness surfaces as an
//! install-check R07 later). When a `.sha256` sidecar exists next to the
//! index (or next to the cached URL copy), the loaded bytes are verified
//! against it; a mismatch is a first-class error, never silently used.
use std::path::{Path, PathBuf};

use crate::model;

/// Where the index should come from.
#[derive(Debug, Clone)]
pub enum Source {
    Local(PathBuf),
    Remote(String),
}

/// Parse a user-supplied source string (path or URL).
fn parse(source: &str) -> Source {
    if source.starts_with("http://") || source.starts_with("https://") {
        Source::Remote(source.to_string())
    } else {
        Source::Local(PathBuf::from(source))
    }
}

/// Resolve the index source: CLI arg, then env, then conventional local
/// paths under the repo.
pub fn resolve_source() -> Option<Source> {
    if let Some(p) = std::env::args().nth(1) {
        return Some(parse(&p));
    }
    if let Some(p) = std::env::var_os("NIX_PILOT_CATALOGUE") {
        let p = p.to_string_lossy().into_owned();
        if !p.is_empty() {
            return Some(parse(&p));
        }
    }
    for cand in [
        "experiments/catalogue/results/catalogue-index.json", // repo root cwd
        "results/catalogue-index.json",                       // catalogue cwd
        "../experiments/catalogue/results/catalogue-index.json",
    ] {
        if Path::new(cand).exists() {
            return Some(Source::Local(PathBuf::from(cand)));
        }
    }
    None
}

/// Cache directory for remote catalogues.
fn cache_path() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join("nix-pilot/catalogue-index.json")
}

/// Load the index bytes for the given source, verifying the `.sha256`
/// sidecar when one exists.
pub fn load_raw(source: &Source) -> Result<Vec<u8>, String> {
    match source {
        Source::Local(path) => {
            let raw = std::fs::read(path)
                .map_err(|e| format!("cannot read catalogue index {}: {e}", path.display()))?;
            verify_sidecar(path, &raw)?;
            Ok(raw)
        }
        Source::Remote(url) => {
            let raw = fetch_remote(url)?;
            let cached = cache_path();
            if let Some(dir) = cached.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&cached, &raw);
            verify_sidecar(&cached, &raw)?;
            Ok(raw)
        }
    }
}

/// Fetch a URL with curl (present on every NixOS), with a timeout.
fn fetch_remote(url: &str) -> Result<Vec<u8>, String> {
    let curl = which::which("curl")
        .map_err(|_| "curl is not available to download the catalogue".to_string())?;
    let output = std::process::Command::new(&curl)
        .args(["-fsSL", "--max-time", "60", url])
        .output()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "could not download the catalogue: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

/// Verify `path` against the `<path>.sha256` sidecar (hex digest, first
/// whitespace-separated token) when the sidecar exists; absence means "no
/// verification available", a mismatch is an error.
fn verify_sidecar(index_path: &Path, raw: &[u8]) -> Result<(), String> {
    let sidecar = PathBuf::from(format!("{}.sha256", index_path.display()));
    if !sidecar.exists() {
        return Ok(());
    }
    let expected = std::fs::read_to_string(&sidecar)
        .map_err(|e| format!("cannot read sidecar {}: {e}", sidecar.display()))?;
    let expected = expected
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("sidecar {} is empty", sidecar.display()))?;
    let actual = sha256_hex(raw);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "catalogue checksum mismatch: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

/// sha256 of a byte slice as lowercase hex, via `sha256sum` (no new crate).
fn sha256_hex(raw: &[u8]) -> String {
    use std::io::Write;
    let mut cmd = std::process::Command::new("sha256sum");
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(raw);
    }
    let output = match child.wait_with_output() {
        Ok(o) if o.status.success() => o,
        _ => return String::new(),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace().next().unwrap_or("").to_string()
}

/// Convenience: parse the loaded bytes into entries, typed for the app.
pub fn load_entries(source: Option<&Source>) -> Result<Vec<model::IndexEntry>, String> {
    let Some(source) = source else {
        return Err("catalogue index not found — pass a path, set NIX_PILOT_CATALOGUE, or run from the repo root".into());
    };
    let raw = load_raw(source)?;
    let idx: model::CatalogueIndex =
        serde_json::from_slice(&raw).map_err(|e| format!("cannot parse catalogue index: {e}"))?;
    Ok(idx.entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_parsing_distinguishes_remote_and_local() {
        assert!(matches!(
            parse("https://example.com/i.json"),
            Source::Remote(_)
        ));
        assert!(matches!(parse("/tmp/i.json"), Source::Local(_)));
    }

    #[test]
    fn sha256_verifies_and_rejects() {
        let raw = b"hello catalogue";
        let good = sha256_hex(raw);
        assert_eq!(good.len(), 64);
        // sidecar present + matching -> ok (with the expected digest written by
        // the caller; here we only exercise the hash function)
        assert!(good.chars().all(|c| c.is_ascii_hexdigit()));

        let dir = std::env::temp_dir().join(format!("np-cat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let index = dir.join("index.json");
        std::fs::write(&index, raw).unwrap();
        // no sidecar: loads without verification
        assert!(verify_sidecar(&index, raw).is_ok());
        // matching sidecar: ok
        std::fs::write(format!("{}.sha256", index.display()), &good).unwrap();
        assert!(verify_sidecar(&index, raw).is_ok());
        // mismatching sidecar: hard error
        std::fs::write(format!("{}.sha256", index.display()), "0".repeat(64)).unwrap();
        assert!(verify_sidecar(&index, raw).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
