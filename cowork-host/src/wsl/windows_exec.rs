//! cfg(windows) implementation of [`WslOps`] plus RunOnce reboot-resume arming.
//! Not compiled on Linux/CI-ubuntu; verified by compile+clippy on the
//! windows-latest runner and by the local windows-gnu cross-check.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use cowork_errors::{Code, Envelope, Stage};

use super::command::WslOp;
use super::{WslOps, WslRun};

pub struct WindowsWslOps;

impl WslOps for WindowsWslOps {
    fn run(&self, op: WslOp) -> WslRun {
        if op.needs_elevation() {
            run_elevated(op)
        } else {
            run_captured(op)
        }
    }
}

/// Non-elevated run that captures output (used for the `--version` probe).
fn run_captured(op: WslOp) -> WslRun {
    use std::process::Command;

    match Command::new("wsl.exe").args(op.args()).output() {
        Ok(out) => {
            let mut text = decode_wsl(&out.stdout);
            text.push_str(&decode_wsl(&out.stderr));
            WslRun::Completed {
                exit_code: out.status.code().unwrap_or(-1),
                output: text,
            }
        }
        Err(e) => WslRun::LaunchFailed {
            detail: e.to_string(),
        },
    }
}

/// Elevated run via UAC (`runas`). Output cannot be captured for an elevated
/// child launched this way, so `output` is empty; only the exit code is
/// observed. A declined UAC prompt → `ElevationDeclined`.
fn run_elevated(op: WslOp) -> WslRun {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, GetLastError};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, INFINITE, WaitForSingleObject,
    };
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };

    let verb = wide_null("runas");
    let file = wide_null("wsl.exe");
    let params = wide_null(&op.args().join(" "));

    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = 0; // SW_HIDE

        if ShellExecuteExW(&mut info) == 0 {
            let err = GetLastError();
            if err == ERROR_CANCELLED {
                return WslRun::ElevationDeclined;
            }
            return WslRun::LaunchFailed {
                detail: format!("ShellExecuteExW failed (GetLastError={err})"),
            };
        }

        if info.hProcess.is_null() {
            return WslRun::LaunchFailed {
                detail: "ShellExecuteExW returned no process handle".to_string(),
            };
        }

        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code: u32 = 0;
        let got = GetExitCodeProcess(info.hProcess, &mut code) != 0;
        CloseHandle(info.hProcess);

        if got {
            WslRun::Completed {
                exit_code: code as i32,
                output: String::new(),
            }
        } else {
            WslRun::LaunchFailed {
                detail: "GetExitCodeProcess failed".to_string(),
            }
        }
    }
}

/// Decode wsl.exe output. `wsl.exe` emits UTF-16LE for `--version`/`--status`;
/// detect that (even length with a NUL high byte) else fall back to UTF-8.
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

const RUNONCE_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\RunOnce";
const RUNONCE_VALUE: &str = "Cowork";

/// Arm `"<exe_path>" --resume` in HKCU RunOnce so the wizard relaunches once
/// after the reboot. Windows deletes the RunOnce value automatically when it
/// fires, so a normal resume self-consumes; `disarm_resume` covers the abort path.
pub fn arm_resume(exe_path: &str) -> Result<(), Envelope> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(RUNONCE_KEY)
        .map_err(|e| internal(format!("open RunOnce: {e}")))?;
    let command = format!("\"{exe_path}\" --resume");
    key.set_value(RUNONCE_VALUE, &command)
        .map_err(|e| internal(format!("set RunOnce value: {e}")))?;
    Ok(())
}

/// Remove the RunOnce value (abort path; the normal resume self-consumes it).
pub fn disarm_resume() -> Result<(), Envelope> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(RUNONCE_KEY, KEY_SET_VALUE) {
        let _ = key.delete_value(RUNONCE_VALUE);
    }
    Ok(())
}

fn internal(detail: String) -> Envelope {
    Envelope::new(Code::InternalUnknown, Stage::WslEnable).with_context("detail", detail)
}

fn wide_null(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}
