//! `#[cfg(windows)]` implementation of [`ProvisionOps`]: real `wsl.exe`
//! invocations plus the WinHTTP rootfs download. Not compiled on Linux/CI-ubuntu;
//! verified by compile+clippy on the windows-latest runner and the local
//! windows-gnu cross-check. The pure decision flow this feeds lives in
//! `provision/mod.rs`; URL splitting lives in `provision/url.rs`.
//!
//! `decode_wsl` and `wide_null` are intentionally duplicated from
//! `wsl::windows_exec` (this is the second occurrence; per the rule of three we
//! do not promote a shared helper yet, and re-implementing an 11-line, stable
//! `wsl.exe` output-decoding convention avoids touching sealed WP4 code).

use std::ffi::{OsStr, c_void};
use std::fs::File;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::ptr;

use sha2::{Digest, Sha256};
use windows_sys::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect,
    WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetTimeouts,
};
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

use super::url::split_https_url;
use super::{
    ExecResult, FetchResult, ProvisionOps, ROOTFS_URL, import_args, install_named_args,
    unregister_args,
};

/// Real provisioning side effects on Windows.
pub struct WindowsProvisionOps;

impl ProvisionOps for WindowsProvisionOps {
    fn list_distros(&self) -> ExecResult {
        run_wsl(&["--list".to_string(), "--verbose".to_string()])
    }

    fn fetch_rootfs(&self) -> FetchResult {
        download_rootfs(ROOTFS_URL)
    }

    fn import(&self, rootfs_path: &str) -> ExecResult {
        let location = match install_location() {
            Some(p) => p,
            None => {
                return ExecResult::LaunchFailed {
                    detail: "LOCALAPPDATA is not set; cannot choose an install location"
                        .to_string(),
                };
            }
        };
        if let Err(e) = std::fs::create_dir_all(&location) {
            return ExecResult::LaunchFailed {
                detail: format!("create install dir {location}: {e}"),
            };
        }
        run_wsl(&import_args(&location, rootfs_path))
    }

    fn install_named(&self) -> ExecResult {
        run_wsl(&install_named_args())
    }

    fn unregister(&self) -> ExecResult {
        run_wsl(&unregister_args())
    }
}

/// Run `wsl.exe <args>` capturing output (provisioning runs unelevated — WSL is
/// already enabled by WP4 — so there is no elevation path here).
fn run_wsl(args: &[String]) -> ExecResult {
    match Command::new("wsl.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args(args)
        .output()
    {
        Ok(out) => {
            let mut text = decode_wsl(&out.stdout);
            text.push_str(&decode_wsl(&out.stderr));
            ExecResult::Completed {
                exit_code: out.status.code().unwrap_or(-1),
                output: text,
            }
        }
        Err(e) => ExecResult::LaunchFailed {
            detail: e.to_string(),
        },
    }
}

/// `%LOCALAPPDATA%\Cowork\distro` — the per-user directory that holds the
/// imported distro's VHDX. `None` if `LOCALAPPDATA` is unset.
fn install_location() -> Option<String> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let mut p = PathBuf::from(base);
    p.push("Cowork");
    p.push("distro");
    Some(p.to_string_lossy().into_owned())
}

/// `%TEMP%\cowork-ubuntu-24.04-rootfs.tar.gz` — where the downloaded rootfs lands.
fn temp_rootfs_path() -> String {
    let mut p = std::env::temp_dir();
    p.push("cowork-ubuntu-24.04-rootfs.tar.gz");
    p.to_string_lossy().into_owned()
}

/// RAII guard that closes a WinHTTP handle on drop. Locals drop in reverse
/// declaration order, so a request handle closes before its connection before
/// its session — the order WinHTTP expects.
struct WinHttpHandle(*mut c_void);

impl Drop for WinHttpHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                WinHttpCloseHandle(self.0);
            }
        }
    }
}

/// Download `url` over HTTPS via WinHTTP, streaming the body to a temp file
/// while computing its SHA-256. Uses `WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY` so
/// WPAD/PAC auto-proxy and the system root-CA store apply (works behind
/// TLS-intercepting corporate proxies). HTTPS→HTTPS redirects (GitHub Releases →
/// objects CDN) are followed by WinHTTP's default redirect policy; HTTPS→HTTP is
/// not. Returns [`FetchResult::Fetched`] only on HTTP 200; any transport
/// failure, non-200 status, or local IO error yields [`FetchResult::Failed`]
/// (`http_status` is the queried HTTP code, or `0` when no response was
/// obtained). A `Failed` result drives the caller's Store fallback; a corrupt
/// download is caught later by the checksum verify, never here.
fn download_rootfs(url: &str) -> FetchResult {
    let parts = match split_https_url(url) {
        Some(p) => p,
        None => return FetchResult::Failed { http_status: 0 },
    };

    let agent = wide_null("Cowork");
    let host = wide_null(&parts.host);
    let verb = wide_null("GET");
    let path = wide_null(&parts.path);

    unsafe {
        let session = WinHttpHandle(WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            ptr::null(), // WINHTTP_NO_PROXY_NAME
            ptr::null(), // WINHTTP_NO_PROXY_BYPASS
            0,
        ));
        if session.0.is_null() {
            return FetchResult::Failed { http_status: 0 };
        }

        // resolve / connect / send / receive timeouts (ms). API takes i32.
        WinHttpSetTimeouts(session.0, 60_000, 60_000, 60_000, 300_000);

        let connect = WinHttpHandle(WinHttpConnect(
            session.0,
            host.as_ptr(),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        ));
        if connect.0.is_null() {
            return FetchResult::Failed { http_status: 0 };
        }

        let request = WinHttpHandle(WinHttpOpenRequest(
            connect.0,
            verb.as_ptr(),
            path.as_ptr(),
            ptr::null(), // version → HTTP/1.1
            ptr::null(), // WINHTTP_NO_REFERER
            ptr::null(), // WINHTTP_DEFAULT_ACCEPT_TYPES (*const *const u16)
            WINHTTP_FLAG_SECURE,
        ));
        if request.0.is_null() {
            return FetchResult::Failed { http_status: 0 };
        }

        if WinHttpSendRequest(request.0, ptr::null(), 0, ptr::null(), 0, 0, 0) == 0 {
            return FetchResult::Failed { http_status: 0 };
        }
        if WinHttpReceiveResponse(request.0, ptr::null_mut()) == 0 {
            return FetchResult::Failed { http_status: 0 };
        }

        let mut status: u32 = 0;
        let mut status_len: u32 = std::mem::size_of::<u32>() as u32;
        if WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            ptr::null(), // WINHTTP_HEADER_NAME_BY_INDEX
            (&mut status as *mut u32).cast::<c_void>(),
            &mut status_len,
            ptr::null_mut(), // WINHTTP_NO_HEADER_INDEX
        ) == 0
        {
            return FetchResult::Failed { http_status: 0 };
        }
        if status != 200 {
            return FetchResult::Failed {
                http_status: status as u16,
            };
        }

        let rootfs_path = temp_rootfs_path();
        let mut file = match File::create(&rootfs_path) {
            Ok(f) => f,
            Err(_) => return FetchResult::Failed { http_status: 0 },
        };
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 65536];
        loop {
            let mut read: u32 = 0;
            if WinHttpReadData(
                request.0,
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as u32,
                &mut read,
            ) == 0
            {
                return FetchResult::Failed { http_status: 0 };
            }
            if read == 0 {
                break;
            }
            let chunk = &buf[..read as usize];
            if file.write_all(chunk).is_err() {
                return FetchResult::Failed { http_status: 0 };
            }
            hasher.update(chunk);
        }
        if file.flush().is_err() {
            return FetchResult::Failed { http_status: 0 };
        }

        let digest = hasher.finalize();
        let sha256 = digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        FetchResult::Fetched {
            rootfs_path,
            sha256,
        }
    }
}

/// Decode `wsl.exe` output: it emits UTF-16LE for some commands (even length
/// with a NUL high byte in the first unit) and UTF-8 otherwise. Duplicated from
/// `wsl::windows_exec` (see module docs).
fn decode_wsl(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes.len() % 2 == 0 && bytes[1] == 0 {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// UTF-16, NUL-terminated. Duplicated from `wsl::windows_exec` (see module docs).
fn wide_null(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}
