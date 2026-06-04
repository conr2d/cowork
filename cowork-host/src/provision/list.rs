//! Parser for `wsl --list --verbose` output. `wsl.exe` prints this as UTF-16LE
//! with a header row that is TRANSLATED in non-English Windows locales, and
//! marks the default distro with a leading `*`. The `#[cfg(windows)]` layer
//! decodes the bytes to UTF-8 before calling here, so this stays a pure `&str`
//! parser.
//!
//! Locale robustness: we never match header text. A line is treated as a distro
//! row iff its last whitespace-separated column parses as a number (the WSL
//! version) — the header's last column never does. Distribution NAMES are not
//! localized by `wsl.exe`, so name matching is reliable.

/// One installed WSL distribution as reported by `wsl --list --verbose`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistroEntry {
    /// Distribution name (not localized by `wsl.exe`).
    pub name: String,
    /// WSL version (1 or 2).
    pub version: u8,
    /// Whether this is the default distro (the `*`-marked row).
    pub default: bool,
}

/// Parse `wsl --list --verbose` stdout (already decoded to UTF-8) into entries.
/// Blank lines, a UTF-16 BOM remnant, the localized header, and any line whose
/// last column is not a number are skipped.
pub fn parse_distro_list(output: &str) -> Vec<DistroEntry> {
    let mut entries = Vec::new();
    for raw in output.lines() {
        let line = raw.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }
        let default = line.starts_with('*');
        let body = line.trim_start_matches('*').trim();
        let cols: Vec<&str> = body.split_whitespace().collect();
        if cols.len() < 3 {
            continue;
        }
        let Ok(version) = cols[cols.len() - 1].parse::<u8>() else {
            continue;
        };
        entries.push(DistroEntry {
            name: cols[0].to_string(),
            version,
            default,
        });
    }
    entries
}

/// Whether a distro named `name` is present. Comparison is ASCII-case-insensitive
/// to mirror `wsl.exe`'s case-insensitive name resolution (so we never register a
/// near-duplicate of an existing distro).
pub fn distro_present(entries: &[DistroEntry], name: &str) -> bool {
    entries.iter().any(|e| e.name.eq_ignore_ascii_case(name))
}
