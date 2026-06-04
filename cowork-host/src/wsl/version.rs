//! WSL app version parsing and the minimum-version gate. Pure: the cfg(windows)
//! executor captures `wsl --version` output and hands the decoded text here.

use cowork_errors::{Code, Envelope, Stage};

/// Minimum supported WSL app version. Below this, `wsl --update` is required
/// before provisioning works reliably.
pub const MIN_WSL_VERSION: WslVersion = WslVersion {
    major: 2,
    minor: 4,
    patch: 4,
};

/// A WSL app version. Ordering is by (major, minor, patch); any 4th component in
/// the source string (e.g. `2.4.4.0`) is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WslVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl WslVersion {
    pub fn meets_minimum(&self) -> bool {
        *self >= MIN_WSL_VERSION
    }

    pub fn to_dotted(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Parse the WSL app version from `wsl --version` output. The output is several
/// localized lines (`WSL version: 2.4.4.0`, kernel, WSLg, ...); the app version
/// is always the first line carrying a dotted-number token. We therefore scan
/// lines in order and return the first `major.minor.patch` token found, so the
/// parse does not depend on the (localized) label text.
pub fn parse_wsl_version(output: &str) -> Option<WslVersion> {
    output.lines().find_map(first_version_token)
}

/// Find the first whitespace-delimited token in `line` shaped like
/// `<u32>.<u32>.<u32>[.<u32>...]`. Returns its first three components.
fn first_version_token(line: &str) -> Option<WslVersion> {
    line.split_whitespace().find_map(|token| {
        let mut parts = token.split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor = parts.next()?.parse::<u32>().ok()?;
        let patch = parts.next()?.parse::<u32>().ok()?;
        Some(WslVersion {
            major,
            minor,
            patch,
        })
    })
}

/// `wsl.version_too_old` (NeedsUserAction) for a detected-but-too-old version.
pub fn version_too_old_envelope(found: &WslVersion) -> Envelope {
    Envelope::new(Code::WslVersionTooOld, Stage::WslEnable)
        .with_context("version", found.to_dotted())
        .with_context("minVersion", MIN_WSL_VERSION.to_dotted())
}
