use cowork_errors::{Code, Envelope, Kind, Stage};
use cowork_host::provision::{
    ExecResult, FetchResult, ProvisionOps, ProvisionOutcome, ROOTFS_SHA256, provision,
    remove_cowork,
};

struct MockOps {
    list: ExecResult,
    fetch: FetchResult,
    import: ExecResult,
    install: ExecResult,
    unregister: ExecResult,
}

impl ProvisionOps for MockOps {
    fn list_distros(&self) -> ExecResult {
        self.list.clone()
    }
    fn fetch_rootfs(&self) -> FetchResult {
        self.fetch.clone()
    }
    fn import(&self, _rootfs_path: &str) -> ExecResult {
        self.import.clone()
    }
    fn install_named(&self) -> ExecResult {
        self.install.clone()
    }
    fn unregister(&self) -> ExecResult {
        self.unregister.clone()
    }
}

fn completed(exit_code: i32, output: &str) -> ExecResult {
    ExecResult::Completed {
        exit_code,
        output: output.to_string(),
    }
}

/// Defaults: no `Cowork` yet (only `Ubuntu`), rootfs fetched with a matching
/// checksum, import/install/unregister all succeed. Tests override one field.
fn base() -> MockOps {
    MockOps {
        list: completed(0, "  NAME    STATE      VERSION\n* Ubuntu  Running    2\n"),
        fetch: FetchResult::Fetched {
            rootfs_path: "/tmp/cowork-rootfs.tar.gz".to_string(),
            sha256: ROOTFS_SHA256.to_string(),
        },
        import: completed(0, ""),
        install: completed(0, ""),
        unregister: completed(0, ""),
    }
}

fn assert_failed(outcome: ProvisionOutcome, code: Code, kind: Kind) -> Envelope {
    let ProvisionOutcome::Failed(env) = outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    assert_eq!(env.code, code);
    assert_eq!(env.kind, kind);
    assert_eq!(env.stage, Stage::Provision);
    env
}

#[test]
fn already_exists_when_cowork_present() {
    let mut ops = base();
    ops.list = completed(
        0,
        "  NAME    STATE      VERSION\n* Ubuntu  Running    2\n  Cowork  Stopped    2\n",
    );
    assert!(matches!(provision(&ops), ProvisionOutcome::AlreadyExists));
}

#[test]
fn import_success_is_ready() {
    let ops = base();
    assert!(matches!(provision(&ops), ProvisionOutcome::Ready));
}

#[test]
fn checksum_mismatch_stops_without_fallback() {
    let mut ops = base();
    ops.fetch = FetchResult::Fetched {
        rootfs_path: "/tmp/cowork-rootfs.tar.gz".to_string(),
        sha256: "deadbeef".to_string(),
    };
    // install would succeed if (wrongly) reached — proves no fallback occurs.
    ops.install = completed(0, "");
    assert_failed(
        provision(&ops),
        Code::DistroChecksumMismatch,
        Kind::Internal,
    );
}

#[test]
fn import_nonzero_surfaces_import_failed_without_fallback() {
    let mut ops = base();
    ops.import = completed(1, "");
    // install would succeed if (wrongly) reached — proves no fallback occurs.
    ops.install = completed(0, "");
    let env = assert_failed(provision(&ops), Code::DistroImportFailed, Kind::Internal);
    assert_eq!(env.context.get("exitCode").map(String::as_str), Some("1"));
}

#[test]
fn import_launch_failed_is_wsl_not_found() {
    let mut ops = base();
    ops.import = ExecResult::LaunchFailed {
        detail: "wsl.exe missing".to_string(),
    };
    assert_failed(provision(&ops), Code::HostWslNotFound, Kind::Internal);
}

#[test]
fn fetch_failure_falls_back_to_store_success() {
    let mut ops = base();
    ops.fetch = FetchResult::Failed { http_status: 503 };
    ops.install = completed(0, "");
    assert!(matches!(provision(&ops), ProvisionOutcome::Ready));
}

#[test]
fn fetch_failure_then_store_failure_surfaces_install_failed() {
    let mut ops = base();
    ops.fetch = FetchResult::Failed { http_status: 0 };
    ops.install = completed(7, "");
    let env = assert_failed(provision(&ops), Code::DistroInstallFailed, Kind::Transient);
    assert_eq!(env.context.get("exitCode").map(String::as_str), Some("7"));
}

#[test]
fn list_launch_failure_is_non_fatal() {
    let mut ops = base();
    ops.list = ExecResult::LaunchFailed {
        detail: "wsl.exe missing".to_string(),
    };
    // List probe failed, but provisioning still proceeds and imports.
    assert!(matches!(provision(&ops), ProvisionOutcome::Ready));
}

#[test]
fn remove_cowork_success() {
    let ops = base();
    assert!(remove_cowork(&ops).is_ok());
}

#[test]
fn remove_cowork_nonzero_is_unregister_failed() {
    let mut ops = base();
    ops.unregister = completed(1, "");
    let env = remove_cowork(&ops).expect_err("nonzero must fail");
    assert_eq!(env.code, Code::DistroUnregisterFailed);
    assert_eq!(env.kind, Kind::Internal);
    assert_eq!(env.context.get("exitCode").map(String::as_str), Some("1"));
}

#[test]
fn remove_cowork_launch_failed_is_wsl_not_found() {
    let mut ops = base();
    ops.unregister = ExecResult::LaunchFailed {
        detail: "wsl.exe missing".to_string(),
    };
    let env = remove_cowork(&ops).expect_err("launch failure must fail");
    assert_eq!(env.code, Code::HostWslNotFound);
}
