//! Pure URL splitting for the rootfs download. WinHTTP needs the host and the
//! object path as separate UTF-16 strings (`WinHttpConnect` takes the host,
//! `WinHttpOpenRequest` takes the path), so this splits our pinned `https` URL
//! into those parts. Pure and unit-tested on every platform, so the only
//! untested code in the Windows download path is the FFI itself.

/// Host + object-path split of an `https://` URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitUrl {
    /// Host authority, e.g. `github.com`. No scheme, no port, no path.
    pub host: String,
    /// Object path including the leading `/` (and any query), e.g. `/a/b.tar.gz`.
    pub path: String,
}

/// Split an `https://host/path` URL into host + path.
///
/// Returns `None` unless the input begins with `https://` and has a non-empty
/// host. A URL with no path component yields `path = "/"`. An authority that
/// carries userinfo (`@`) or an explicit port (`:`) is rejected (`None`): the
/// download path is HTTPS/443-only and deliberately does not parse a port, and
/// our pinned mirror URL has neither.
pub fn split_https_url(url: &str) -> Option<SplitUrl> {
    let rest = url.strip_prefix("https://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() || authority.contains('@') || authority.contains(':') {
        return None;
    }
    Some(SplitUrl {
        host: authority.to_string(),
        path: path.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_a_github_releases_url() {
        let s = split_https_url(
            "https://github.com/conr2d/cowork/releases/download/rootfs/cowork.tar.gz",
        )
        .expect("valid https url splits");
        assert_eq!(s.host, "github.com");
        assert_eq!(
            s.path,
            "/conr2d/cowork/releases/download/rootfs/cowork.tar.gz"
        );
    }

    #[test]
    fn preserves_a_query_string_in_the_path() {
        let s = split_https_url("https://host.example/obj?token=abc&x=1").expect("splits");
        assert_eq!(s.host, "host.example");
        assert_eq!(s.path, "/obj?token=abc&x=1");
    }

    #[test]
    fn no_path_yields_root() {
        let s = split_https_url("https://github.com").expect("splits");
        assert_eq!(s.host, "github.com");
        assert_eq!(s.path, "/");
    }

    #[test]
    fn rejects_non_https_scheme() {
        assert!(split_https_url("http://github.com/x").is_none());
        assert!(split_https_url("ftp://github.com/x").is_none());
        assert!(split_https_url("github.com/x").is_none());
    }

    #[test]
    fn rejects_empty_host() {
        assert!(split_https_url("https:///path").is_none());
    }

    #[test]
    fn rejects_explicit_port() {
        assert!(split_https_url("https://host.example:8443/p").is_none());
    }

    #[test]
    fn rejects_userinfo() {
        assert!(split_https_url("https://user@host.example/p").is_none());
    }
}
