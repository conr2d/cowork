//! Guest-CLI injection + run-result classification (WP5③). Pure command/path
//! construction, failure→envelope mapping, and the rule that turns a guest's
//! JSON-lines stream + process exit code into one [`RunOutcome`]. The
//! `#[cfg(windows)]` side (UNC file copy, `wsl.exe` spawn, line streaming) lives
//! in `windows_inject`; this module is fully unit-tested off-Windows.
//!
//! The guest binary's *provenance* (cross-compiled `cowork` for the distro's
//! arch, embedded in or downloaded by the host) is deliberately out of scope
//! here: injection takes a host-side source path and copies it in. That keeps
//! the mechanism independent of the (still-open) build/delivery decision.

use cowork_errors::{Code, Envelope, Stage};

use crate::protocol::HostEvent;

use super::{DISTRO_NAME, WSL_USER};

/// Absolute path of the injected guest CLI inside the distro. `/usr/local/bin`
/// is on every login PATH and exists in a stock Ubuntu rootfs.
pub const GUEST_BIN_PATH: &str = "/usr/local/bin/cowork";

/// The Windows UNC path that maps to [`GUEST_BIN_PATH`] inside the `Cowork`
/// distro. `\\wsl.localhost\<distro>\...` is the modern share (WSL ≥ 2.4.4,
/// which WP4 guarantees). Built from [`DISTRO_NAME`] so the distro name stays
/// single-source.
pub fn unc_inject_path() -> String {
    format!(r"\\wsl.localhost\{DISTRO_NAME}\usr\local\bin\cowork")
}

/// `wsl -d Cowork -u root -- chmod +x /usr/local/bin/cowork`. A UNC file copy
/// does not carry the executable bit, so it is set inside the distro. `-u root`
/// because injection runs before any non-root firstboot user exists.
pub fn chmod_args() -> Vec<String> {
    vec![
        "-d".to_string(),
        DISTRO_NAME.to_string(),
        "-u".to_string(),
        "root".to_string(),
        "--".to_string(),
        "chmod".to_string(),
        "+x".to_string(),
        GUEST_BIN_PATH.to_string(),
    ]
}

/// `wsl -d Cowork -- /usr/local/bin/cowork <extra...>` — invoke the injected
/// guest CLI (it emits the JSON-lines protocol on stdout).
pub fn launch_args(extra: &[String]) -> Vec<String> {
    let mut v = vec![
        "-d".to_string(),
        DISTRO_NAME.to_string(),
        "--".to_string(),
        GUEST_BIN_PATH.to_string(),
    ];
    v.extend_from_slice(extra);
    v
}

/// The shell script (run as root inside the distro) that creates the non-root
/// firstboot user, grants it passwordless sudo (bootstrap needs sudo for apt and
/// locale-gen), and sets it as the distro's default user via `/etc/wsl.conf`.
/// Idempotent: the user is only created if absent; the sudoers and wsl.conf files
/// are rewritten each time. The wsl.conf default user takes effect after a
/// `wsl --terminate` (see [`terminate_args`]).
fn firstboot_script() -> String {
    format!(
        "set -e\n\
         id -u {user} >/dev/null 2>&1 || useradd -m -s /bin/bash {user}\n\
         echo '{user} ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/{user}\n\
         chmod 0440 /etc/sudoers.d/{user}\n\
         printf '[user]\\ndefault={user}\\n' > /etc/wsl.conf",
        user = WSL_USER
    )
}

/// `wsl -d Cowork -u root -- bash -c "<firstboot_script>"` — create the firstboot
/// user and set it as the distro default. Runs as root because the import default
/// user is root and only root can useradd / write /etc.
pub fn firstboot_setup_args() -> Vec<String> {
    vec![
        "-d".to_string(),
        DISTRO_NAME.to_string(),
        "-u".to_string(),
        "root".to_string(),
        "--".to_string(),
        "bash".to_string(),
        "-c".to_string(),
        firstboot_script(),
    ]
}

/// `wsl --terminate Cowork` — stop the distro so the new /etc/wsl.conf default
/// user is read on the next launch.
pub fn terminate_args() -> Vec<String> {
    vec!["--terminate".to_string(), DISTRO_NAME.to_string()]
}

/// `guest.cli_inject_failed` (Internal) — copying the binary in or setting its
/// executable bit failed.
pub fn cli_inject_failed_envelope(detail: &str) -> Envelope {
    Envelope::new(Code::GuestCliInjectFailed, Stage::Provision).with_context("detail", detail)
}

/// `guest.cli_launch_failed` (Internal) — `wsl.exe` itself could not be spawned
/// to run the guest.
pub fn cli_launch_failed_envelope(detail: &str) -> Envelope {
    Envelope::new(Code::GuestCliLaunchFailed, Stage::Provision).with_context("detail", detail)
}

/// `guest.cli_crashed` (Internal) — the guest ran but exited without honoring
/// the protocol (no clean `Done` + zero exit) and emitted no structured error.
pub fn cli_crashed_envelope(exit_code: i32, last_stage: Stage) -> Envelope {
    Envelope::new(Code::GuestCliCrashed, Stage::Provision)
        .with_context("exitCode", exit_code.to_string())
        .with_context("lastStage", stage_label(last_stage))
}

/// `Stage` as its canonical kebab string (`Stage::Provision` → `"provision"`),
/// via its serde representation so it stays in sync with the wire format.
fn stage_label(stage: Stage) -> String {
    serde_json::to_value(stage)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Outcome of running the injected guest for one stage.
///
/// NOTE: no `PartialEq`/`Eq` — the envelope-carrying variants hold [`Envelope`].
/// Consumers destructure + `matches!`.
#[derive(Debug, Clone)]
pub enum RunOutcome {
    /// The guest reported `stage` complete and exited zero.
    Done { stage: Stage },
    /// The guest emitted a structured error envelope; surface it as-is.
    GuestFailed(Envelope),
    /// The host detected a protocol fault (`protocol.*`) in the stream.
    ProtocolFault(Envelope),
    /// The guest ran but died without a clean completion (`guest.cli_crashed`).
    Crashed(Envelope),
    /// `wsl.exe` could not be launched at all (`guest.cli_launch_failed`).
    /// Never produced by [`classify_run`]; the spawn site sets it directly.
    LaunchFailed(Envelope),
}

/// Classify a finished guest run from its emitted [`HostEvent`]s and process
/// exit code. Precedence (first match wins):
/// 1. **`ProtocolFault`** — any `protocol.*` event. Stream-integrity faults rank
///    highest: if host and guest disagree on the protocol (version skew) or a
///    line is unparseable, no later parse can be trusted, and this is the
///    actionable bug to report.
/// 2. **`GuestFailed`** — any structured guest `Error`; the authoritative cause.
/// 3. **`Done`** — a `Done` event *and* `exit_code == 0`. A `Done` with a
///    nonzero exit is treated as a crash (a clean run exits zero).
/// 4. **`Crashed`** — everything else: nonzero exit, or a zero exit that never
///    produced the required `Done`. `lastStage` is the stage of the last
///    `Progress`/`Done` seen, else `default_stage`.
///
/// Never returns [`RunOutcome::LaunchFailed`] (that is a pre-stream spawn error).
pub fn classify_run(events: &[HostEvent], exit_code: i32, default_stage: Stage) -> RunOutcome {
    for ev in events {
        if let HostEvent::ProtocolError(env) = ev {
            return RunOutcome::ProtocolFault(env.clone());
        }
    }
    for ev in events {
        if let HostEvent::GuestError(env) = ev {
            return RunOutcome::GuestFailed(env.clone());
        }
    }
    let done_stage = events.iter().rev().find_map(|ev| match ev {
        HostEvent::Done { stage } => Some(*stage),
        _ => None,
    });
    if let Some(stage) = done_stage {
        if exit_code == 0 {
            return RunOutcome::Done { stage };
        }
    }
    let last_stage = events
        .iter()
        .rev()
        .find_map(|ev| match ev {
            HostEvent::Progress { stage, .. } => Some(*stage),
            HostEvent::Done { stage } => Some(*stage),
            HostEvent::AuthStatus { .. } | HostEvent::SessionUuid { .. } => None,
            HostEvent::GuestError(_) | HostEvent::ProtocolError(_) => None,
        })
        .unwrap_or(default_stage);
    RunOutcome::Crashed(cli_crashed_envelope(exit_code, last_stage))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(stage: Stage, step: &str) -> HostEvent {
        HostEvent::Progress {
            stage,
            step: step.to_string(),
        }
    }

    #[test]
    fn unc_path_targets_the_cowork_distro() {
        assert_eq!(
            unc_inject_path(),
            r"\\wsl.localhost\Cowork\usr\local\bin\cowork"
        );
    }

    #[test]
    fn launch_args_invoke_the_guest_with_extra_args() {
        assert_eq!(
            launch_args(&["--version".to_string()]),
            vec![
                "-d".to_string(),
                "Cowork".to_string(),
                "--".to_string(),
                "/usr/local/bin/cowork".to_string(),
                "--version".to_string(),
            ]
        );
    }

    #[test]
    fn chmod_args_set_the_executable_bit_as_root() {
        assert_eq!(
            chmod_args(),
            vec![
                "-d".to_string(),
                "Cowork".to_string(),
                "-u".to_string(),
                "root".to_string(),
                "--".to_string(),
                "chmod".to_string(),
                "+x".to_string(),
                "/usr/local/bin/cowork".to_string(),
            ]
        );
    }

    #[test]
    fn firstboot_setup_args_create_user_sudo_and_default_via_wsl_conf() {
        let a = firstboot_setup_args();
        assert_eq!(a[0], "-d");
        assert_eq!(a[1], DISTRO_NAME);
        assert_eq!(a[2], "-u");
        assert_eq!(a[3], "root");
        assert_eq!(a[4], "--");
        assert_eq!(a[5], "bash");
        assert_eq!(a[6], "-c");
        let script = &a[7];
        assert!(script.contains("useradd -m -s /bin/bash cowork"));
        assert!(script.contains("id -u cowork")); // idempotent guard
        assert!(script.contains("cowork ALL=(ALL) NOPASSWD:ALL"));
        assert!(script.contains("/etc/sudoers.d/cowork"));
        assert!(script.contains("default=cowork"));
        assert!(script.contains("/etc/wsl.conf"));
    }

    #[test]
    fn terminate_args_target_the_cowork_distro() {
        assert_eq!(
            terminate_args(),
            vec!["--terminate".to_string(), "Cowork".to_string()]
        );
    }

    #[test]
    fn clean_run_is_done() {
        let events = vec![
            progress(Stage::Provision, "x"),
            HostEvent::Done {
                stage: Stage::Provision,
            },
        ];
        match classify_run(&events, 0, Stage::Provision) {
            RunOutcome::Done { stage } => assert_eq!(stage, Stage::Provision),
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn done_with_nonzero_exit_is_a_crash() {
        let events = vec![
            progress(Stage::Provision, "x"),
            HostEvent::Done {
                stage: Stage::Provision,
            },
        ];
        match classify_run(&events, 1, Stage::Provision) {
            RunOutcome::Crashed(env) => {
                assert_eq!(env.code, Code::GuestCliCrashed);
                assert_eq!(env.context.get("exitCode").map(String::as_str), Some("1"));
                assert_eq!(
                    env.context.get("lastStage").map(String::as_str),
                    Some("provision")
                );
            }
            other => panic!("expected Crashed, got {other:?}"),
        }
    }

    #[test]
    fn zero_exit_without_done_is_a_crash() {
        let events = vec![progress(Stage::Provision, "x")];
        match classify_run(&events, 0, Stage::Provision) {
            RunOutcome::Crashed(env) => {
                assert_eq!(env.code, Code::GuestCliCrashed);
                assert_eq!(env.context.get("exitCode").map(String::as_str), Some("0"));
                assert_eq!(
                    env.context.get("lastStage").map(String::as_str),
                    Some("provision")
                );
            }
            other => panic!("expected Crashed, got {other:?}"),
        }
    }

    #[test]
    fn structured_guest_error_is_surfaced() {
        let env = Envelope::new(Code::ToolchainBrewInstallFailed, Stage::Toolchain);
        let events = vec![
            progress(Stage::Toolchain, "brew"),
            HostEvent::GuestError(env),
        ];
        match classify_run(&events, 1, Stage::Provision) {
            RunOutcome::GuestFailed(env) => assert_eq!(env.code, Code::ToolchainBrewInstallFailed),
            other => panic!("expected GuestFailed, got {other:?}"),
        }
    }

    #[test]
    fn protocol_fault_beats_a_guest_error() {
        let proto = Envelope::new(Code::ProtocolParseError, Stage::Provision);
        let guest = Envelope::new(Code::ToolchainBrewInstallFailed, Stage::Toolchain);
        let events = vec![
            HostEvent::ProtocolError(proto),
            HostEvent::GuestError(guest),
        ];
        match classify_run(&events, 1, Stage::Provision) {
            RunOutcome::ProtocolFault(env) => assert_eq!(env.code, Code::ProtocolParseError),
            other => panic!("expected ProtocolFault, got {other:?}"),
        }
    }

    #[test]
    fn no_events_falls_back_to_default_stage() {
        match classify_run(&[], 1, Stage::Provision) {
            RunOutcome::Crashed(env) => {
                assert_eq!(
                    env.context.get("lastStage").map(String::as_str),
                    Some("provision")
                );
            }
            other => panic!("expected Crashed, got {other:?}"),
        }
    }

    #[test]
    fn inject_failed_envelope_carries_detail() {
        let env = cli_inject_failed_envelope("copy x -> y: denied");
        assert_eq!(env.code, Code::GuestCliInjectFailed);
        assert_eq!(
            env.context.get("detail").map(String::as_str),
            Some("copy x -> y: denied")
        );
    }

    #[test]
    fn launch_failed_envelope_carries_detail() {
        let env = cli_launch_failed_envelope("program not found");
        assert_eq!(env.code, Code::GuestCliLaunchFailed);
        assert_eq!(
            env.context.get("detail").map(String::as_str),
            Some("program not found")
        );
    }
}
