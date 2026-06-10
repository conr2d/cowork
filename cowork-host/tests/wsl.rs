use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;

use cowork_errors::{Code, Envelope, Kind, Stage};
use cowork_host::wsl::{
    ResumeStage, ResumeState, WslEnableOutcome, WslOp, WslOps, WslRun, WslVersion,
    clear_resume_state, enable_wsl, is_inbox_unsupported, parse_wsl_version,
    reboot_required_envelope, run_failure_envelope, save_resume_state, version_too_old_envelope,
};

struct ScriptedOps {
    runs: RefCell<VecDeque<WslRun>>,
    online: bool,
}

impl ScriptedOps {
    fn new(runs: Vec<WslRun>) -> Self {
        Self {
            runs: RefCell::new(runs.into()),
            online: true,
        }
    }

    fn offline(runs: Vec<WslRun>) -> Self {
        Self {
            runs: RefCell::new(runs.into()),
            online: false,
        }
    }
}

impl WslOps for ScriptedOps {
    fn run(&self, _op: WslOp) -> WslRun {
        self.runs
            .borrow_mut()
            .pop_front()
            .expect("scripted WSL operation missing response")
    }

    fn is_online(&self) -> bool {
        self.online
    }
}

fn completed(exit_code: i32, output: &str) -> WslRun {
    WslRun::Completed {
        exit_code,
        output: output.to_string(),
    }
}

fn assert_failed(outcome: WslEnableOutcome, code: Code, kind: Kind) -> Envelope {
    let WslEnableOutcome::Failed(env) = outcome else {
        panic!("expected failure, got {outcome:?}");
    };
    assert_eq!(env.code, code);
    assert_eq!(env.code.as_str(), code.as_str());
    assert_eq!(env.kind, kind);
    assert_eq!(env.stage, Stage::WslEnable);
    env
}

fn assert_envelope(env: &Envelope, code: Code, kind: Kind) {
    assert_eq!(env.code, code);
    assert_eq!(env.code.as_str(), code.as_str());
    assert_eq!(env.kind, kind);
    assert_eq!(env.stage, Stage::WslEnable);
}

fn temp_path(suffix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("cowork-wsl-{}-{}.json", std::process::id(), suffix))
}

#[test]
fn version_parsing() {
    assert_eq!(
        parse_wsl_version("WSL version: 2.4.4.0\nKernel version: 5.15.167.4\n"),
        Some(WslVersion {
            major: 2,
            minor: 4,
            patch: 4,
        })
    );
    assert_eq!(
        parse_wsl_version("WSL バージョン: 2.5.7.0\nカーネル バージョン: 5.15.167.4\n"),
        Some(WslVersion {
            major: 2,
            minor: 5,
            patch: 7,
        })
    );
    assert_eq!(parse_wsl_version("no version token here"), None);
}

#[test]
fn meets_minimum_boundaries() {
    assert!(
        !WslVersion {
            major: 2,
            minor: 4,
            patch: 3
        }
        .meets_minimum()
    );
    assert!(
        WslVersion {
            major: 2,
            minor: 4,
            patch: 4
        }
        .meets_minimum()
    );
    assert!(
        WslVersion {
            major: 2,
            minor: 5,
            patch: 0
        }
        .meets_minimum()
    );
    assert!(
        WslVersion {
            major: 3,
            minor: 0,
            patch: 0
        }
        .meets_minimum()
    );
    assert!(
        !WslVersion {
            major: 1,
            minor: 9,
            patch: 9
        }
        .meets_minimum()
    );
}

#[test]
fn enable_wsl_ready() {
    let ops = ScriptedOps::new(vec![completed(0, "WSL version: 2.4.4.0\n...")]);
    assert!(matches!(enable_wsl(&ops), WslEnableOutcome::Ready));
}

#[test]
fn enable_wsl_old_update_success() {
    let ops = ScriptedOps::new(vec![
        completed(0, "WSL version: 2.4.3.0\n..."),
        completed(0, ""),
    ]);
    assert!(matches!(enable_wsl(&ops), WslEnableOutcome::Ready));
}

#[test]
fn enable_wsl_old_update_elevation_declined() {
    let ops = ScriptedOps::new(vec![
        completed(0, "WSL version: 2.4.3.0\n..."),
        WslRun::ElevationDeclined,
    ]);
    assert_failed(
        enable_wsl(&ops),
        Code::WslElevationDenied,
        Kind::NeedsUserAction,
    );
}

#[test]
fn enable_wsl_old_update_nonzero() {
    let ops = ScriptedOps::new(vec![
        completed(0, "WSL version: 2.4.3.0\n..."),
        completed(1, ""),
    ]);
    let env = assert_failed(enable_wsl(&ops), Code::WslUpdateFailed, Kind::Transient);
    assert_eq!(env.context.get("exitCode").map(String::as_str), Some("1"));
}

#[test]
fn update_failure_while_offline_maps_to_network_failed() {
    // Same script as enable_wsl_old_update_nonzero, but offline.
    let ops = ScriptedOps::offline(vec![
        completed(0, "WSL version: 2.4.3.0\n..."),
        completed(1, ""),
    ]);
    assert_failed(enable_wsl(&ops), Code::CommonNetworkFailed, Kind::Transient);
}

#[test]
fn enable_wsl_absent_install_success() {
    let ops = ScriptedOps::new(vec![completed(0, ""), completed(0, "")]);
    assert!(matches!(enable_wsl(&ops), WslEnableOutcome::RebootRequired));
}

#[test]
fn enable_wsl_absent_launch_failed_install_success() {
    let ops = ScriptedOps::new(vec![
        WslRun::LaunchFailed {
            detail: "not found".to_string(),
        },
        completed(0, ""),
    ]);
    assert!(matches!(enable_wsl(&ops), WslEnableOutcome::RebootRequired));
}

#[test]
fn enable_wsl_install_elevation_declined() {
    let ops = ScriptedOps::new(vec![completed(0, ""), WslRun::ElevationDeclined]);
    assert_failed(
        enable_wsl(&ops),
        Code::WslElevationDenied,
        Kind::NeedsUserAction,
    );
}

#[test]
fn enable_wsl_install_nonzero() {
    let ops = ScriptedOps::new(vec![completed(0, ""), completed(2, "")]);
    let env = assert_failed(enable_wsl(&ops), Code::WslInstallFailed, Kind::Transient);
    assert_eq!(env.context.get("exitCode").map(String::as_str), Some("2"));
}

#[test]
fn install_failure_while_offline_maps_to_network_failed() {
    // Same script as enable_wsl_install_nonzero, but offline -> the install
    // failure is attributed to connectivity, not WSL.
    let ops = ScriptedOps::offline(vec![completed(0, ""), completed(2, "")]);
    assert_failed(enable_wsl(&ops), Code::CommonNetworkFailed, Kind::Transient);
}

#[test]
fn enable_wsl_inbox_at_version_probe() {
    let ops = ScriptedOps::new(vec![completed(-1, "...invalid command line option...")]);
    assert_failed(
        enable_wsl(&ops),
        Code::WslUpdateUnsupportedInbox,
        Kind::NeedsUserAction,
    );
}

#[test]
fn inbox_unsupported_detection() {
    assert!(is_inbox_unsupported(
        1,
        "ERROR: INVALID COMMAND LINE OPTION"
    ));
    assert!(!is_inbox_unsupported(0, "invalid command line option"));
    assert!(!is_inbox_unsupported(1, "different failure"));
}

#[test]
fn command_args_and_elevation() {
    assert_eq!(WslOp::Install.args(), ["--install", "--no-distribution"]);
    assert_eq!(WslOp::Update.args(), ["--update"]);
    assert_eq!(WslOp::Version.args(), ["--version"]);
    assert!(WslOp::Install.needs_elevation());
    assert!(WslOp::Update.needs_elevation());
    assert!(!WslOp::Version.needs_elevation());
}

#[test]
fn run_failure_envelopes() {
    let install = run_failure_envelope(WslOp::Install, 2).expect("install maps");
    assert_envelope(&install, Code::WslInstallFailed, Kind::Transient);
    assert_eq!(
        install.context.get("exitCode").map(String::as_str),
        Some("2")
    );

    let update = run_failure_envelope(WslOp::Update, 1).expect("update maps");
    assert_envelope(&update, Code::WslUpdateFailed, Kind::Transient);
    assert_eq!(
        update.context.get("exitCode").map(String::as_str),
        Some("1")
    );

    assert!(run_failure_envelope(WslOp::Version, 1).is_none());
}

#[test]
fn version_too_old_envelope_fields() {
    let env = version_too_old_envelope(&WslVersion {
        major: 2,
        minor: 4,
        patch: 3,
    });
    assert_envelope(&env, Code::WslVersionTooOld, Kind::NeedsUserAction);
    assert_eq!(
        env.context.get("version").map(String::as_str),
        Some("2.4.3")
    );
    assert_eq!(
        env.context.get("minVersion").map(String::as_str),
        Some("2.4.4")
    );
}

#[test]
fn reboot_required_envelope_fields() {
    let env = reboot_required_envelope();
    assert_envelope(&env, Code::WslRebootRequired, Kind::NeedsUserAction);
}

#[test]
fn resume_round_trip() {
    let path = temp_path("round-trip");
    let _ = clear_resume_state(&path);
    let state = ResumeState::new(ResumeStage::WslReady, vec!["claude".into(), "codex".into()]);

    save_resume_state(&path, &state).expect("save resume state");
    let loaded = cowork_host::wsl::load_resume_state(&path).expect("load resume state");
    assert_eq!(loaded, state);
    clear_resume_state(&path).expect("clear resume state");
}

#[test]
fn resume_corrupt_bad_json() {
    let path = temp_path("bad-json");
    let _ = clear_resume_state(&path);
    std::fs::write(&path, "{ not json").expect("write bad JSON");

    let env = cowork_host::wsl::load_resume_state(&path).expect_err("bad JSON must fail");
    assert_envelope(&env, Code::HostResumeStateCorrupt, Kind::Internal);
    clear_resume_state(&path).expect("clear resume state");
}

#[test]
fn resume_corrupt_wrong_schema_version() {
    let path = temp_path("wrong-schema-version");
    let _ = clear_resume_state(&path);
    std::fs::write(
        &path,
        r#"{"schema_version":999,"stage":"WslReady","selected_agents":["claude"]}"#,
    )
    .expect("write wrong schema");

    let env = cowork_host::wsl::load_resume_state(&path).expect_err("wrong schema must fail");
    assert_envelope(&env, Code::HostResumeStateCorrupt, Kind::Internal);
    clear_resume_state(&path).expect("clear resume state");
}

#[test]
fn resume_corrupt_unknown_field() {
    let path = temp_path("unknown-field");
    let _ = clear_resume_state(&path);
    std::fs::write(
        &path,
        r#"{"schema_version":1,"stage":"WslReady","selected_agents":["claude"],"bogus":1}"#,
    )
    .expect("write unknown field");

    let env = cowork_host::wsl::load_resume_state(&path).expect_err("unknown field must fail");
    assert_envelope(&env, Code::HostResumeStateCorrupt, Kind::Internal);
    clear_resume_state(&path).expect("clear resume state");
}

#[test]
fn resume_clear() {
    let path = temp_path("clear");
    let _ = clear_resume_state(&path);
    clear_resume_state(&path).expect("clear missing");

    std::fs::write(&path, "{}").expect("write file");
    clear_resume_state(&path).expect("clear existing");
    assert!(!path.exists());
}
