use cowork_errors::{Code, Kind, Stage};
use cowork_host::provision::{
    DISTRO_NAME, FALLBACK_STORE_DISTRO, ROOTFS_SHA256, already_exists_envelope, import_args,
    import_failed_envelope, install_failed_envelope, install_named_args,
    rootfs_fetch_failed_envelope, unregister_args, unregister_failed_envelope,
    user_create_failed_envelope, verify_checksum,
};

#[test]
fn import_args_shape() {
    assert_eq!(
        import_args("C:\\cowork\\distro", "C:\\cowork\\rootfs.tar.gz"),
        vec![
            "--import",
            "Cowork",
            "C:\\cowork\\distro",
            "C:\\cowork\\rootfs.tar.gz",
            "--version",
            "2",
        ]
    );
}

#[test]
fn install_named_args_shape() {
    assert_eq!(
        install_named_args(),
        vec![
            "--install",
            "Ubuntu-24.04",
            "--name",
            "Cowork",
            "--no-launch"
        ]
    );
    assert_eq!(FALLBACK_STORE_DISTRO, "Ubuntu-24.04");
    assert_eq!(DISTRO_NAME, "Cowork");
}

#[test]
fn unregister_args_shape() {
    assert_eq!(unregister_args(), vec!["--unregister", "Cowork"]);
}

#[test]
fn failure_envelopes_carry_exit_code() {
    let import = import_failed_envelope(3);
    assert_eq!(import.code, Code::DistroImportFailed);
    assert_eq!(import.kind, Kind::Internal);
    assert_eq!(import.stage, Stage::Provision);
    assert_eq!(
        import.context.get("exitCode").map(String::as_str),
        Some("3")
    );

    let install = install_failed_envelope(5);
    assert_eq!(install.code, Code::DistroInstallFailed);
    assert_eq!(install.kind, Kind::Transient);
    assert_eq!(
        install.context.get("exitCode").map(String::as_str),
        Some("5")
    );

    let unregister = unregister_failed_envelope(1);
    assert_eq!(unregister.code, Code::DistroUnregisterFailed);
    assert_eq!(unregister.kind, Kind::Internal);
    assert_eq!(
        unregister.context.get("exitCode").map(String::as_str),
        Some("1")
    );
}

#[test]
fn rootfs_fetch_failed_carries_url_and_status() {
    let env = rootfs_fetch_failed_envelope("https://example.test/rootfs.tar.gz", 503);
    assert_eq!(env.code, Code::DistroRootfsFetchFailed);
    assert_eq!(env.kind, Kind::Transient);
    assert_eq!(
        env.context.get("url").map(String::as_str),
        Some("https://example.test/rootfs.tar.gz")
    );
    assert_eq!(
        env.context.get("httpStatus").map(String::as_str),
        Some("503")
    );
}

#[test]
fn already_exists_carries_name() {
    let env = already_exists_envelope();
    assert_eq!(env.code, Code::DistroAlreadyExists);
    assert_eq!(env.kind, Kind::NeedsUserAction);
    assert_eq!(env.context.get("name").map(String::as_str), Some("Cowork"));
}

#[test]
fn user_create_failed_carries_detail() {
    let env = user_create_failed_envelope("useradd exited 1");
    assert_eq!(env.code, Code::DistroUserCreateFailed);
    assert_eq!(env.kind, Kind::Internal);
    assert_eq!(
        env.context.get("detail").map(String::as_str),
        Some("useradd exited 1")
    );
}

#[test]
fn checksum_matches_case_insensitively() {
    let upper = ROOTFS_SHA256.to_ascii_uppercase();
    assert!(verify_checksum(ROOTFS_SHA256, &upper).is_ok());
    assert!(verify_checksum(ROOTFS_SHA256, ROOTFS_SHA256).is_ok());
}

#[test]
fn checksum_mismatch_yields_envelope() {
    let actual = "deadbeef";
    let env = verify_checksum(ROOTFS_SHA256, actual).expect_err("mismatch must fail");
    assert_eq!(env.code, Code::DistroChecksumMismatch);
    assert_eq!(env.kind, Kind::Internal);
    assert_eq!(env.stage, Stage::Provision);
    assert_eq!(
        env.context.get("expected").map(String::as_str),
        Some(ROOTFS_SHA256)
    );
    assert_eq!(
        env.context.get("actual").map(String::as_str),
        Some("deadbeef")
    );
}
