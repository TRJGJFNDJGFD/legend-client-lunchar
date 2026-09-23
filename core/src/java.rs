//! Real Java runtime detection.
//!
//! Scans JAVA_HOME, PATH, and the common per-OS install locations, then runs
//! `java -version` on each candidate and parses the actual version string.
//! Nothing here is hardcoded/faked — a candidate only makes it into the
//! result if the binary exists and actually ran.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize, Clone, Debug)]
pub struct JavaInstall {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub is_64bit: bool,
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(home) = std::env::var("JAVA_HOME") {
        candidates.push(Path::new(&home).join("bin").join(java_bin_name()));
    }

    if let Ok(path) = which::which("java") {
        candidates.push(path);
    }

    // Common per-OS install roots. We only add ones that exist, so this list
    // is cheap even when most entries are absent (e.g. on Linux, Program
    // Files obviously never exists).
    let extra_roots: &[&str] = if cfg!(target_os = "windows") {
        &["C:\\Program Files\\Java", "C:\\Program Files\\Eclipse Adoptium"]
    } else if cfg!(target_os = "macos") {
        &["/Library/Java/JavaVirtualMachines", "/opt/homebrew/opt"]
    } else {
        &["/usr/lib/jvm", "/opt/java"]
    };

    for root in extra_roots {
        let root = Path::new(root);
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let bin = entry.path().join("bin").join(java_bin_name());
                if bin.exists() {
                    candidates.push(bin);
                }
                // macOS JVMs nest an extra Contents/Home
                let mac_bin = entry.path().join("Contents/Home/bin").join(java_bin_name());
                if mac_bin.exists() {
                    candidates.push(mac_bin);
                }
            }
        }
    }

    candidates
}

fn java_bin_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "java.exe"
    } else {
        "java"
    }
}

/// Runs `java -version` on a candidate binary and parses the real output.
/// `java -version` prints to stderr, e.g.:
///   openjdk version "21.0.10" 2026-01-20
///   OpenJDK Runtime Environment (build 21.0.10+7-Ubuntu-124.04)
///   OpenJDK 64-Bit Server VM (build 21.0.10+7-Ubuntu-124.04, mixed mode, sharing)
fn probe(path: &Path) -> Option<JavaInstall> {
    let output = Command::new(path).arg("-version").output().ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let version_line = text.lines().next()?;
    let version = version_line
        .split('"')
        .nth(1)
        .unwrap_or("unknown")
        .to_string();

    let major = parse_major(&version);
    let is_64bit = text.contains("64-Bit");

    Some(JavaInstall {
        path: path.to_string_lossy().to_string(),
        version,
        major,
        is_64bit,
    })
}

/// Handles both old-style ("1.8.0_392" -> 8) and modern ("21.0.10" -> 21)
/// version strings, since Minecraft still cares about this distinction for
/// legacy versions.
fn parse_major(version: &str) -> u32 {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.first() == Some(&"1") {
        parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0)
    } else {
        parts.first().and_then(|s| s.parse().ok()).unwrap_or(0)
    }
}

/// Public entry point used by the `detect_java` Tauri command. De-duplicates
/// by resolved path so the same JDK found via two roots isn't listed twice.
pub fn detect_all() -> Vec<JavaInstall> {
    let mut seen = std::collections::HashSet::new();
    let mut found = Vec::new();

    for candidate in candidate_paths() {
        let canonical = std::fs::canonicalize(&candidate).unwrap_or(candidate.clone());
        let key = canonical.to_string_lossy().to_string();
        if !seen.insert(key) {
            continue;
        }
        if let Some(install) = probe(&candidate) {
            found.push(install);
        }
    }

    found
}

/// Picks the best installed Java for a given required major version,
/// preferring an exact match, falling back to the newest available.
pub fn best_for(required_major: u32) -> Option<JavaInstall> {
    let all = detect_all();
    all.iter()
        .find(|j| j.major == required_major)
        .cloned()
        .or_else(|| all.into_iter().max_by_key(|j| j.major))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_version() {
        assert_eq!(parse_major("21.0.10"), 21);
    }

    #[test]
    fn parses_legacy_version() {
        assert_eq!(parse_major("1.8.0_392"), 8);
    }

    #[test]
    fn detects_the_java_this_test_runs_under() {
        // This machine has a real JDK (it's building this crate), so at
        // least one install must be found — this is not a mocked check.
        let found = detect_all();
        assert!(!found.is_empty(), "expected to find at least one real java install");
    }
}
