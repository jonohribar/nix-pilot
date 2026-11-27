//! Where the catalogue index comes from, and how it is verified.
//!
//! Precedence: the first CLI argument, then `$NIX_PILOT_CATALOGUE` (a path or
//! an https URL), then the conventional repo paths. A URL is fetched on each
//! start and cached.
//!
//! A remote index must ship `<url>.sha256`. It is fetched with the same flags
//! and kept beside the cache; a missing or mismatched sidecar is a hard error,
//! so unverified bytes are never used. A local index is verified only when a
//! sidecar sits next to it.
use std::path::{Path, PathBuf};

use crate::model;

/// Where the index should come from.
#[derive(Debug, Clone)]
pub enum Source {
    /// Read index bytes from a local file.
    Local(PathBuf),
    /// Fetch fresh index bytes from this HTTP(S) URL.
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

/// Load the index bytes, verifying them before use.
///
/// Remote bytes are fetched on every call; the cache is never a fallback, so
/// a failed fetch is an error rather than stale data.
///
/// # Errors
/// Plain-language errors for read, download, or checksum failures.
pub fn load_raw(source: &Source) -> Result<Vec<u8>, String> {
    match source {
        Source::Local(path) => {
            let raw = std::fs::read(path)
                .map_err(|e| format!("cannot read catalogue index {}: {e}", path.display()))?;
            verify_sidecar_local(path, &raw)?;
            Ok(raw)
        }
        Source::Remote(url) => {
            let (raw, sidecar) = fetch_remote_with_sidecar(url)?;
            // Verify before caching, so unverified bytes never reach the disk.
            verify_remote_bytes(&raw, &sidecar)?;
            let cached = cache_path();
            if let Some(dir) = cached.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            std::fs::write(&cached, &raw)
                .map_err(|e| format!("cannot write cache {}: {e}", cached.display()))?;
            let sidecar_path = format!("{}.sha256", cached.display());
            std::fs::write(&sidecar_path, &sidecar)
                .map_err(|e| format!("cannot write sidecar {}: {e}", sidecar_path))?;
            Ok(raw)
        }
    }
}

/// Fetch a URL and its `.sha256` sidecar with curl (same flags).
/// Remote catalogues require a sidecar; missing sidecar is a hard error.
fn fetch_remote_with_sidecar(url: &str) -> Result<(Vec<u8>, String), String> {
    let curl = which::which("curl")
        .map_err(|_| "curl is not available to download the catalogue".to_string())?;
    let raw = fetch_with_curl(&curl, url)?;
    let sidecar_url = format!("{}.sha256", url);
    let sidecar = fetch_with_curl(&curl, &sidecar_url)
        .map_err(|e| format!("remote catalogue missing .sha256 sidecar: {e}"))?;
    let sidecar =
        String::from_utf8(sidecar).map_err(|_| "sidecar is not valid UTF-8".to_string())?;
    let sidecar = sidecar.trim().to_string();
    if sidecar.is_empty() {
        return Err("sidecar is empty".to_string());
    }
    Ok((raw, sidecar))
}

/// Fetch a single URL with curl using the standard flags.
fn fetch_with_curl(curl: &Path, url: &str) -> Result<Vec<u8>, String> {
    let output = std::process::Command::new(curl)
        .args(["-fsSL", "--max-time", "60", url])
        .output()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "could not download {}: {}",
            url,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

/// Verify a local source against its sidecar.
/// Sidecar absence is allowed (no verification available).
fn verify_sidecar_local(index_path: &Path, raw: &[u8]) -> Result<(), String> {
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

/// Verify a remote source against its sidecar.
/// Check downloaded bytes against the sidecar text fetched beside them.
fn verify_remote_bytes(raw: &[u8], sidecar: &str) -> Result<(), String> {
    let expected = sidecar
        .split_whitespace()
        .next()
        .ok_or_else(|| "sidecar is empty".to_string())?;
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

/// Load and deserialize catalogue entries from an optional source.
///
/// # Errors
/// Returns an error when no source was resolved, loading or checksum
/// verification fails, or the index is not valid catalogue JSON.
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
    fn local_sidecar_verifies_and_rejects() {
        let raw = b"hello catalogue";
        let good = sha256_hex(raw);
        assert_eq!(good.len(), 64);
        assert!(good.chars().all(|c| c.is_ascii_hexdigit()));

        let dir = std::env::temp_dir().join(format!("np-cat-local-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let index = dir.join("index.json");
        std::fs::write(&index, raw).unwrap();
        // no sidecar: loads without verification (local source)
        assert!(verify_sidecar_local(&index, raw).is_ok());
        // matching sidecar: ok
        std::fs::write(format!("{}.sha256", index.display()), &good).unwrap();
        assert!(verify_sidecar_local(&index, raw).is_ok());
        // mismatching sidecar: hard error
        std::fs::write(format!("{}.sha256", index.display()), "0".repeat(64)).unwrap();
        assert!(verify_sidecar_local(&index, raw).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remote_bytes_verify_against_the_fetched_sidecar() {
        let raw = b"remote index";
        let good = sha256_hex(raw);
        assert!(verify_remote_bytes(raw, &good).is_ok());
        // Sidecars usually carry the "<digest>  <name>" form.
        assert!(verify_remote_bytes(raw, &format!("{good}  index.json")).is_ok());
        // A mismatch or an empty sidecar must fail closed.
        let err = verify_remote_bytes(raw, &"0".repeat(64)).unwrap_err();
        assert!(err.contains("checksum mismatch"), "{err}");
        assert!(verify_remote_bytes(raw, "   ").is_err());
    }

    #[test]
    fn local_source_no_sidecar_loads() {
        let raw = b"local index";
        let dir = std::env::temp_dir().join(format!("np-cat-local2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let index = dir.join("index.json");
        std::fs::write(&index, raw).unwrap();
        // Local source without sidecar is allowed
        assert!(verify_sidecar_local(&index, raw).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
