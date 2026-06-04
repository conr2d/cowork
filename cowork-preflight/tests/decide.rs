use std::collections::BTreeSet;

use cowork_errors::{Code, Envelope, Kind, Stage};
use cowork_preflight::{
    ARCH_ARM64, ARCH_X64, CheckId, CheckStatus, ElevationFacts, PreflightReport, RawFacts,
    StaticProbe, decide, run_preflight,
};

fn pass_baseline() -> RawFacts {
    RawFacts {
        build_number: Some(22631),
        ubr: Some(2715),
        arch: ARCH_X64,
        virt_feature_present: true,
        vm_monitor_mode_extensions: Some(true),
        virtualization_firmware_enabled: Some(true),
        hypervisor_present: Some(false),
        known_vmm_services: vec![],
        free_bytes_available: Some(64 * 1024 * 1024 * 1024),
        elevation: ElevationFacts {
            is_member_filtered: false,
            elevation_type: 3,
        },
        wsl_blocked: false,
        inbox_wsl_blocked: false,
        store_disabled: false,
        cfa_mode: Some(0),
        cfa_protected_path: None,
    }
}

fn status(report: &PreflightReport, check: CheckId) -> &CheckStatus {
    match report
        .outcomes
        .iter()
        .find(|outcome| outcome.check == check)
    {
        Some(outcome) => &outcome.status,
        None => panic!("missing check {check:?}"),
    }
}

fn fail(status: &CheckStatus) -> &Envelope {
    match status {
        CheckStatus::Fail(env) => env,
        other => panic!("expected failure, got {other:?}"),
    }
}

fn assert_pass(status: &CheckStatus) {
    assert!(matches!(status, CheckStatus::Pass));
}

fn assert_unknown(status: &CheckStatus) {
    assert!(matches!(status, CheckStatus::Unknown));
}

fn assert_fail(env: &Envelope, code: Code, kind: Kind) {
    assert_eq!(env.code, code);
    assert_eq!(env.code.as_str(), code.as_str());
    assert_eq!(env.kind, kind);
    assert_eq!(env.stage, Stage::Preflight);
}

#[test]
fn baseline_all_pass() {
    let report = decide(&pass_baseline());
    assert!(report.can_proceed);
    assert_eq!(report.outcomes.len(), 9);
    for outcome in report.outcomes {
        assert_pass(&outcome.status);
    }
}

#[test]
fn build_boundary() {
    let mut facts = pass_baseline();
    facts.build_number = Some(19041);
    assert_pass(status(&decide(&facts), CheckId::WindowsBuild));

    facts.build_number = Some(19040);
    let report = decide(&facts);
    let env = fail(status(&report, CheckId::WindowsBuild));
    assert_fail(env, Code::PreflightWindowsBuildUnsupported, Kind::Blocker);
    assert_eq!(env.code.as_str(), "preflight.windows_build_unsupported");
    assert_eq!(env.context.get("build").map(String::as_str), Some("19040"));
    assert_eq!(
        env.context.get("minBuild").map(String::as_str),
        Some("19041")
    );
}

#[test]
fn build_unknown() {
    let mut facts = pass_baseline();
    facts.build_number = None;
    assert_unknown(status(&decide(&facts), CheckId::WindowsBuild));
}

#[test]
fn arch_x64_and_arm64_pass() {
    let mut facts = pass_baseline();
    facts.arch = ARCH_X64;
    assert_pass(status(&decide(&facts), CheckId::Arch));

    facts.arch = ARCH_ARM64;
    assert_pass(status(&decide(&facts), CheckId::Arch));
}

#[test]
fn arch_reject() {
    let mut facts = pass_baseline();
    facts.arch = 0;
    let report = decide(&facts);
    let env = fail(status(&report, CheckId::Arch));
    assert_fail(env, Code::PreflightArchUnsupported, Kind::Blocker);
    assert_eq!(env.code.as_str(), "preflight.arch_unsupported");
    assert_eq!(env.context.get("arch").map(String::as_str), Some("0"));
}

#[test]
fn virt_hyperv_owns_vtx() {
    let mut facts = pass_baseline();
    facts.virtualization_firmware_enabled = Some(false);
    facts.vm_monitor_mode_extensions = Some(false);
    facts.hypervisor_present = Some(true);
    assert_pass(status(&decide(&facts), CheckId::Virtualization));
}

#[test]
fn virt_unsupported() {
    let mut facts = pass_baseline();
    facts.vm_monitor_mode_extensions = Some(false);
    facts.hypervisor_present = Some(false);
    facts.virt_feature_present = false;
    let report = decide(&facts);
    let env = fail(status(&report, CheckId::Virtualization));
    assert_fail(env, Code::PreflightVirtualizationUnsupported, Kind::Blocker);
    assert_eq!(env.code.as_str(), "preflight.virtualization_unsupported");
}

#[test]
fn virt_disabled() {
    let mut facts = pass_baseline();
    facts.vm_monitor_mode_extensions = Some(true);
    facts.virtualization_firmware_enabled = Some(false);
    facts.hypervisor_present = Some(false);
    facts.virt_feature_present = false;
    let report = decide(&facts);
    let env = fail(status(&report, CheckId::Virtualization));
    assert_fail(
        env,
        Code::PreflightVirtualizationDisabled,
        Kind::NeedsUserAction,
    );
    assert_eq!(env.code.as_str(), "preflight.virtualization_disabled");
}

#[test]
fn hypervisor_conflict_flagged() {
    let mut facts = pass_baseline();
    facts.hypervisor_present = Some(false);
    facts.known_vmm_services = vec!["vmci".into()];
    let report = decide(&facts);
    let env = fail(status(&report, CheckId::HypervisorConflict));
    assert_fail(
        env,
        Code::PreflightHypervisorConflict,
        Kind::NeedsUserAction,
    );
    assert_eq!(env.code.as_str(), "preflight.hypervisor_conflict");
    assert!(
        env.context
            .get("detail")
            .is_some_and(|value| !value.is_empty())
    );
}

#[test]
fn hypervisor_conflict_suppressed_when_hv_present() {
    let mut facts = pass_baseline();
    facts.hypervisor_present = Some(true);
    facts.known_vmm_services = vec!["vmci".into()];
    assert_pass(status(&decide(&facts), CheckId::HypervisorConflict));
}

#[test]
fn disk_boundary() {
    let mut facts = pass_baseline();
    facts.free_bytes_available = Some(16 * 1024 * 1024 * 1024);
    assert_pass(status(&decide(&facts), CheckId::Disk));

    facts.free_bytes_available = Some(16 * 1024 * 1024 * 1024 - 1);
    let report = decide(&facts);
    let env = fail(status(&report, CheckId::Disk));
    assert_fail(env, Code::PreflightInsufficientDisk, Kind::NeedsUserAction);
    assert_eq!(env.code.as_str(), "preflight.insufficient_disk");
    assert_eq!(
        env.context.get("requiredBytes").map(String::as_str),
        Some("17179869184")
    );
    assert_eq!(
        env.context.get("availableBytes").map(String::as_str),
        Some("17179869183")
    );
}

#[test]
fn disk_unknown() {
    let mut facts = pass_baseline();
    facts.free_bytes_available = None;
    assert_unknown(status(&decide(&facts), CheckId::Disk));
}

#[test]
fn elevation_full_and_limited_pass() {
    let mut facts = pass_baseline();
    facts.elevation.elevation_type = 2;
    assert_pass(status(&decide(&facts), CheckId::Elevation));

    facts.elevation.elevation_type = 3;
    assert_pass(status(&decide(&facts), CheckId::Elevation));
}

#[test]
fn elevation_standard_blocks() {
    let mut facts = pass_baseline();
    facts.elevation.elevation_type = 1;
    facts.elevation.is_member_filtered = false;
    let report = decide(&facts);
    let env = fail(status(&report, CheckId::Elevation));
    assert_fail(env, Code::PreflightElevationUnavailable, Kind::Blocker);
    assert_eq!(env.code.as_str(), "preflight.elevation_unavailable");
}

#[test]
fn elevation_admin_default_passes() {
    let mut facts = pass_baseline();
    facts.elevation.elevation_type = 1;
    facts.elevation.is_member_filtered = true;
    assert_pass(status(&decide(&facts), CheckId::Elevation));
}

#[test]
fn wsl_policy_blocks() {
    let mut facts = pass_baseline();
    facts.wsl_blocked = true;
    let report = decide(&facts);
    let env = fail(status(&report, CheckId::WslPolicy));
    assert_fail(env, Code::PreflightWslBlockedByPolicy, Kind::Blocker);
    assert_eq!(env.code.as_str(), "preflight.wsl_blocked_by_policy");
    assert!(
        env.context
            .get("detail")
            .is_some_and(|value| !value.is_empty())
    );
}

#[test]
fn store_disabled_flagged() {
    let mut facts = pass_baseline();
    facts.store_disabled = true;
    let report = decide(&facts);
    let env = fail(status(&report, CheckId::Store));
    assert_fail(env, Code::PreflightStoreDisabled, Kind::NeedsUserAction);
    assert_eq!(env.code.as_str(), "preflight.store_disabled");
}

#[test]
fn cfa_on_flagged() {
    let mut facts = pass_baseline();
    facts.cfa_mode = Some(1);
    facts.cfa_protected_path = Some(r"C:\Users\x\AppData\Local".into());
    let report = decide(&facts);
    let env = fail(status(&report, CheckId::ControlledFolderAccess));
    assert_fail(
        env,
        Code::PreflightControlledFolderAccess,
        Kind::NeedsUserAction,
    );
    assert_eq!(env.code.as_str(), "preflight.controlled_folder_access");
    assert_eq!(
        env.context.get("path").map(String::as_str),
        Some(r"C:\Users\x\AppData\Local")
    );
}

#[test]
fn cfa_audit_passes() {
    let mut facts = pass_baseline();
    facts.cfa_mode = Some(2);
    assert_pass(status(&decide(&facts), CheckId::ControlledFolderAccess));
}

#[test]
fn cfa_unknown() {
    let mut facts = pass_baseline();
    facts.cfa_mode = None;
    assert_unknown(status(&decide(&facts), CheckId::ControlledFolderAccess));
}

#[test]
fn can_proceed_false_on_any_fail() {
    let mut facts = pass_baseline();
    facts.store_disabled = true;
    let report = decide(&facts);
    assert!(!report.can_proceed);
}

#[test]
fn context_keys_match_registry() {
    let failing_facts = [
        {
            let mut facts = pass_baseline();
            facts.build_number = Some(19040);
            facts
        },
        {
            let mut facts = pass_baseline();
            facts.arch = 0;
            facts
        },
        {
            let mut facts = pass_baseline();
            facts.vm_monitor_mode_extensions = Some(false);
            facts.hypervisor_present = Some(false);
            facts.virt_feature_present = false;
            facts
        },
        {
            let mut facts = pass_baseline();
            facts.vm_monitor_mode_extensions = Some(true);
            facts.virtualization_firmware_enabled = Some(false);
            facts.hypervisor_present = Some(false);
            facts.virt_feature_present = false;
            facts
        },
        {
            let mut facts = pass_baseline();
            facts.hypervisor_present = Some(false);
            facts.known_vmm_services = vec!["vmci".into()];
            facts
        },
        {
            let mut facts = pass_baseline();
            facts.free_bytes_available = Some(16 * 1024 * 1024 * 1024 - 1);
            facts
        },
        {
            let mut facts = pass_baseline();
            facts.elevation.elevation_type = 1;
            facts.elevation.is_member_filtered = false;
            facts
        },
        {
            let mut facts = pass_baseline();
            facts.wsl_blocked = true;
            facts
        },
        {
            let mut facts = pass_baseline();
            facts.store_disabled = true;
            facts
        },
        {
            let mut facts = pass_baseline();
            facts.cfa_mode = Some(1);
            facts
        },
    ];

    for facts in failing_facts {
        let report = decide(&facts);
        for outcome in report.outcomes {
            if let CheckStatus::Fail(env) = outcome.status {
                let actual = env
                    .context
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                let expected = env
                    .code
                    .context_keys()
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>();
                assert_eq!(actual, expected, "context keys for {:?}", env.code);
            }
        }
    }
}

#[test]
fn run_preflight_seam() {
    let probe = StaticProbe {
        facts: pass_baseline(),
    };
    let report = run_preflight(&probe);
    assert!(report.can_proceed);
}
