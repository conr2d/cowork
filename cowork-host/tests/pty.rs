use cowork_errors::{Code, Stage};
use cowork_host::pty::{
    PtyCommand, locale_to_lang, pty_bridge_failed_envelope, pty_spawn_failed_envelope,
    terminal_launch,
};

fn args_of(cmd: &PtyCommand) -> Vec<&str> {
    cmd.args.iter().map(String::as_str).collect()
}

#[test]
fn terminal_launch_builds_interactive_wsl_shell() {
    let cmd = terminal_launch("Cowork", "/home/u/cowork", "ko", None);
    assert_eq!(cmd.program, "wsl.exe");
    let args = args_of(&cmd);
    for expected in [
        "-d",
        "Cowork",
        "--cd",
        "/home/u/cowork",
        "--",
        "env",
        "COLORTERM=truecolor",
        "TERM=xterm-256color",
        "TERM_PROGRAM=Cowork",
        "LANG=ko_KR.UTF-8",
        "LC_ALL=ko_KR.UTF-8",
    ] {
        assert!(args.contains(&expected), "missing arg: {expected}");
    }
    // The interactive login shell is the final two args.
    assert_eq!(
        &cmd.args[cmd.args.len() - 2..],
        &["bash".to_string(), "-li".to_string()]
    );
}

#[test]
fn terminal_launch_locale_maps_lang() {
    assert!(args_of(&terminal_launch("Cowork", "/h", "ja", None)).contains(&"LANG=ja_JP.UTF-8"));
    assert!(args_of(&terminal_launch("Cowork", "/h", "en", None)).contains(&"LANG=en_US.UTF-8"));
}

#[test]
fn terminal_launch_execs_escaped_autorun_in_place_of_shell() {
    let cmd = terminal_launch(
        "Cowork",
        "/home/u/cowork",
        "en",
        Some("agent --flag o'neal"),
    );
    assert_eq!(cmd.program, "wsl.exe");
    assert_eq!(
        &cmd.args[cmd.args.len() - 3..],
        &[
            "bash".to_string(),
            "-lic".to_string(),
            "exec 'agent' '--flag' 'o'\\''neal'".to_string(),
        ]
    );
}

#[test]
fn locale_to_lang_maps_known_and_falls_back() {
    assert_eq!(locale_to_lang("ja"), "ja_JP.UTF-8");
    assert_eq!(locale_to_lang("ko"), "ko_KR.UTF-8");
    assert_eq!(locale_to_lang("en"), "en_US.UTF-8");
    assert_eq!(locale_to_lang("fr"), "en_US.UTF-8");
}

#[test]
fn envelopes_carry_stage_and_detail() {
    let e = pty_spawn_failed_envelope(Stage::Auth, "boom");
    assert_eq!(e.code, Code::HostPtySpawnFailed);
    assert_eq!(e.stage, Stage::Auth);
    assert_eq!(e.context.get("detail").map(String::as_str), Some("boom"));

    let e = pty_bridge_failed_envelope(Stage::Done, "x");
    assert_eq!(e.code, Code::HostPtyBridgeFailed);
    assert_eq!(e.stage, Stage::Done);
}
