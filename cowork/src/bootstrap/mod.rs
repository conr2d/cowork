//! Toolchain bootstrap orchestration (WP6): the ordered sequence of setup steps
//! run inside the `Cowork` distro, emitting the guest→host JSON-lines protocol.
//!
//! The sequence is: apt prerequisites → Linuxbrew → mise → shellrc activation →
//! (optional) pinned Node → locales → default workspace. Each step emits a
//! `Progress` first; on failure the step emits a structured `Error` envelope and
//! the run stops. brew/mise installs are skipped when already present, and the
//! shellrc lines are appended only when absent, so a re-run is idempotent.
//!
//! All decision logic is pure and unit-tested here against a mock
//! [`BootstrapOps`]; the real process/filesystem glue is [`LinuxOps`].

mod command;
mod ops;
mod sink;

use cowork_errors::Envelope;
use cowork_errors::Stage;
use cowork_errors::protocol::{Message, PROTOCOL_VERSION};

pub use ops::{BootstrapOps, ExecOutcome, LinuxOps};
pub use sink::{ProgressSink, StdoutSink};

use command::Cmd;

/// What the bootstrap was asked to do.
pub struct Config {
    /// The invoking user's home directory (resolved from `$HOME` by the caller).
    pub home: String,
    /// Install the pinned Node toolchain (set iff codex is selected).
    pub with_node: bool,
}

/// Outcome of [`run_bootstrap`].
///
/// NOTE: no `PartialEq`/`Eq` — `Failed` carries [`Envelope`]. Consumers
/// destructure + `matches!`.
#[derive(Debug, Clone)]
pub enum BootstrapOutcome {
    /// Every step completed; a `Done` was emitted.
    Done,
    /// A step failed; the carried envelope was already emitted as `Error`.
    Failed(Envelope),
}

/// Max chars of captured command output attached as an envelope `cause`.
const CAUSE_TAIL: usize = 1500;

/// How a single [`Cmd`] failed.
enum CmdFail {
    /// Ran to completion with a nonzero exit.
    Exited { code: i32, output: String },
    /// Could not be launched.
    Launch { detail: String },
}

impl CmdFail {
    fn exit_code(&self) -> i32 {
        match self {
            CmdFail::Exited { code, .. } => *code,
            CmdFail::Launch { .. } => -1,
        }
    }

    /// The short diagnostic to attach as `cause` (output tail, or launch detail).
    fn diagnostic(&self) -> String {
        match self {
            CmdFail::Exited { output, .. } => tail(output, CAUSE_TAIL),
            CmdFail::Launch { detail } => detail.clone(),
        }
    }
}

/// Run the toolchain bootstrap, emitting the JSON-lines protocol through `sink`.
pub fn run_bootstrap(
    ops: &mut dyn BootstrapOps,
    sink: &mut dyn ProgressSink,
    config: &Config,
) -> BootstrapOutcome {
    sink.emit(&Message::Hello {
        protocol_version: PROTOCOL_VERSION,
    });

    if let Err(env) = run_steps(ops, sink, config) {
        sink.emit(&Message::Error {
            envelope: env.clone(),
        });
        return BootstrapOutcome::Failed(env);
    }

    sink.emit(&Message::Done {
        stage: Stage::Toolchain,
    });
    BootstrapOutcome::Done
}

/// Execute the ordered steps, returning the first failure's envelope.
fn run_steps(
    ops: &mut dyn BootstrapOps,
    sink: &mut dyn ProgressSink,
    config: &Config,
) -> Result<(), Envelope> {
    // 1. apt prerequisites (update, then install). Either failure is the same code.
    progress(sink, command::step::APT_PREREQS);
    if let Err(f) = run_cmd(ops, &command::apt_update_cmd()) {
        return Err(with_cause(command::prereq_apt_failed_envelope(), &f));
    }
    if let Err(f) = run_cmd(ops, &command::apt_install_cmd()) {
        return Err(with_cause(command::prereq_apt_failed_envelope(), &f));
    }

    // 2. Linuxbrew (skip the network install if already present).
    progress(sink, command::step::BREW);
    if !ops.path_exists(command::BREW_BIN) {
        if let Err(f) = run_cmd(ops, &command::brew_install_cmd()) {
            return Err(with_cause(
                command::brew_install_failed_envelope(f.exit_code()),
                &f,
            ));
        }
    }

    // 3. mise (skip the install if already present).
    progress(sink, command::step::MISE);
    if !ops.path_exists(&command::mise_bin(&config.home)) {
        if let Err(f) = run_cmd(ops, &command::mise_install_cmd()) {
            return Err(with_cause(
                command::mise_install_failed_envelope(f.exit_code()),
                &f,
            ));
        }
    }

    // 4. shellrc activation lines (idempotent append).
    progress(sink, command::step::SHELLRC);
    ensure_shellrc(ops, &config.home)?;

    // 5. pinned Node toolchain (only if codex is selected).
    if config.with_node {
        progress(sink, command::step::NODE);
        if let Err(f) = run_cmd(ops, &command::node_pin_cmd(&config.home)) {
            return Err(with_cause(
                command::node_install_failed_envelope(f.exit_code()),
                &f,
            ));
        }
    }

    // 6. locales (no dedicated code → internal.unknown).
    progress(sink, command::step::LOCALES);
    if let Err(f) = run_cmd(ops, &command::locale_gen_cmd()) {
        let env =
            command::internal_unknown_envelope(&format!("locale-gen exited {}", f.exit_code()));
        return Err(with_cause(env, &f));
    }

    // 7. default workspace (no dedicated code → internal.unknown).
    progress(sink, command::step::WORKSPACE);
    let workspace = command::workspace_path(&config.home);
    if let Err(e) = ops.create_dir_all(&workspace) {
        return Err(command::internal_unknown_envelope(&format!(
            "create {workspace}: {e}"
        )));
    }

    Ok(())
}

/// Append each shellrc activation line that is not already present.
fn ensure_shellrc(ops: &mut dyn BootstrapOps, home: &str) -> Result<(), Envelope> {
    let file = command::shellrc_path(home);
    let existing = ops.read_to_string(&file).unwrap_or_default();
    for line in command::shellrc_lines(home) {
        if existing.lines().any(|l| l.trim() == line) {
            continue;
        }
        if let Err(e) = ops.append_line(&file, &line) {
            return Err(command::shellrc_write_failed_envelope(&file).with_cause(&e));
        }
    }
    Ok(())
}

/// Run `cmd`; `Ok(())` on a zero exit, `Err(CmdFail)` otherwise.
fn run_cmd(ops: &mut dyn BootstrapOps, cmd: &Cmd) -> Result<(), CmdFail> {
    match ops.run(cmd) {
        ExecOutcome::Completed { exit_code: 0, .. } => Ok(()),
        ExecOutcome::Completed { exit_code, output } => Err(CmdFail::Exited {
            code: exit_code,
            output,
        }),
        ExecOutcome::LaunchFailed { detail } => Err(CmdFail::Launch { detail }),
    }
}

/// Emit a `Progress` for `step` at the toolchain stage.
fn progress(sink: &mut dyn ProgressSink, step: &str) {
    sink.emit(&Message::Progress {
        stage: Stage::Toolchain,
        step: step.to_string(),
    });
}

/// Attach a command failure's diagnostic to `env` as a redacted `cause`.
fn with_cause(env: Envelope, fail: &CmdFail) -> Envelope {
    env.with_cause(&fail.diagnostic())
}

/// Last `n` chars of `s` (char-boundary safe), for bounding `cause` size.
fn tail(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n {
        return s.to_string();
    }
    s.chars().skip(count - n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    /// Records emitted messages as `(tag, detail)` pairs for order assertions:
    /// `("hello", "")`, `("progress", step)`, `("error", "<code-debug>")`,
    /// `("done", "")`.
    #[derive(Default)]
    struct CollectingSink {
        events: Vec<(String, String)>,
    }

    impl ProgressSink for CollectingSink {
        fn emit(&mut self, message: &Message) {
            let pair = match message {
                Message::Hello { .. } => ("hello".to_string(), String::new()),
                Message::Progress { step, .. } => ("progress".to_string(), step.clone()),
                Message::Error { envelope } => {
                    ("error".to_string(), format!("{:?}", envelope.code))
                }
                Message::Done { .. } => ("done".to_string(), String::new()),
            };
            self.events.push(pair);
        }
    }

    /// A scriptable mock. `present` paths report as existing; `fail` maps a
    /// command key (see [`cmd_key`]) to the exit code it should return; commands
    /// not in `fail` succeed with exit 0.
    struct MockOps {
        present: HashSet<String>,
        files: HashMap<String, String>,
        fail: HashMap<String, i32>,
        append_fail: bool,
        dir_fail: bool,
        runs: Vec<Cmd>,
    }

    impl MockOps {
        fn new() -> Self {
            Self {
                present: HashSet::new(),
                files: HashMap::new(),
                fail: HashMap::new(),
                append_fail: false,
                dir_fail: false,
                runs: Vec::new(),
            }
        }

        fn ran(&self, key: &str) -> bool {
            self.runs.iter().any(|c| cmd_key(c) == key)
        }
    }

    /// A stable key identifying which step a [`Cmd`] belongs to, for scripting
    /// failures and asserting which installs ran.
    fn cmd_key(cmd: &Cmd) -> String {
        if cmd.program == "sudo" {
            // "sudo apt-get update" / "sudo env … apt-get install" / "sudo locale-gen"
            if cmd.args.iter().any(|a| a == "update") {
                return "apt-update".to_string();
            }
            if cmd.args.iter().any(|a| a == "install") {
                return "apt-install".to_string();
            }
            if cmd.args.iter().any(|a| a == "locale-gen") {
                return "locale".to_string();
            }
            return "sudo-other".to_string();
        }
        if cmd.program == "bash" {
            let script = cmd.args.get(1).cloned().unwrap_or_default();
            if script.contains("mise.run") {
                return "mise".to_string();
            }
            if script.contains("Homebrew") || script.contains("install.sh") {
                return "brew".to_string();
            }
            return "bash-other".to_string();
        }
        if cmd.program.ends_with("/mise") {
            return "node".to_string();
        }
        "other".to_string()
    }

    impl BootstrapOps for MockOps {
        fn run(&mut self, cmd: &Cmd) -> ExecOutcome {
            self.runs.push(cmd.clone());
            let key = cmd_key(cmd);
            match self.fail.get(&key) {
                Some(&code) => ExecOutcome::Completed {
                    exit_code: code,
                    output: format!("{key} failed"),
                },
                None => ExecOutcome::Completed {
                    exit_code: 0,
                    output: String::new(),
                },
            }
        }

        fn path_exists(&self, path: &str) -> bool {
            self.present.contains(path)
        }

        fn read_to_string(&self, path: &str) -> Option<String> {
            self.files.get(path).cloned()
        }

        fn append_line(&mut self, path: &str, line: &str) -> Result<(), String> {
            if self.append_fail {
                return Err("append denied".to_string());
            }
            let entry = self.files.entry(path.to_string()).or_default();
            entry.push_str(line);
            entry.push('\n');
            Ok(())
        }

        fn create_dir_all(&mut self, _path: &str) -> Result<(), String> {
            if self.dir_fail {
                Err("mkdir denied".to_string())
            } else {
                Ok(())
            }
        }
    }

    fn config(with_node: bool) -> Config {
        Config {
            home: "/home/u".to_string(),
            with_node,
        }
    }

    fn steps(sink: &CollectingSink) -> Vec<String> {
        sink.events
            .iter()
            .filter(|(tag, _)| tag == "progress")
            .map(|(_, step)| step.clone())
            .collect()
    }

    #[test]
    fn happy_path_emits_hello_all_progress_then_done() {
        let mut ops = MockOps::new();
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(true));
        assert!(matches!(out, BootstrapOutcome::Done));

        assert_eq!(sink.events.first().map(|(t, _)| t.as_str()), Some("hello"));
        assert_eq!(sink.events.last().map(|(t, _)| t.as_str()), Some("done"));
        assert_eq!(
            steps(&sink),
            vec![
                "apt-prereqs",
                "brew-install",
                "mise-install",
                "shellrc",
                "node-pin",
                "locale-gen",
                "workspace",
            ]
        );
        // brew + mise installs ran (not present), node ran (with_node).
        assert!(ops.ran("brew"));
        assert!(ops.ran("mise"));
        assert!(ops.ran("node"));
    }

    #[test]
    fn without_node_skips_node_step() {
        let mut ops = MockOps::new();
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(false));
        assert!(matches!(out, BootstrapOutcome::Done));
        assert!(!steps(&sink).iter().any(|s| s == "node-pin"));
        assert!(!ops.ran("node"));
    }

    #[test]
    fn brew_and_mise_skipped_when_already_present() {
        let mut ops = MockOps::new();
        ops.present.insert(command::BREW_BIN.to_string());
        ops.present.insert(command::mise_bin("/home/u"));
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(true));
        assert!(matches!(out, BootstrapOutcome::Done));
        // The expensive network installers must NOT have run.
        assert!(!ops.ran("brew"));
        assert!(!ops.ran("mise"));
        // But the steps are still reported.
        assert!(steps(&sink).iter().any(|s| s == "brew-install"));
        assert!(steps(&sink).iter().any(|s| s == "mise-install"));
    }

    #[test]
    fn shellrc_not_duplicated_when_lines_present() {
        let mut ops = MockOps::new();
        let existing = command::shellrc_lines("/home/u").join("\n");
        ops.files
            .insert(command::shellrc_path("/home/u"), existing.clone());
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(false));
        assert!(matches!(out, BootstrapOutcome::Done));
        // No new lines appended: the file is byte-for-byte the seeded content.
        assert_eq!(
            ops.files.get(&command::shellrc_path("/home/u")),
            Some(&existing)
        );
    }

    #[test]
    fn shellrc_appends_both_lines_when_absent() {
        let mut ops = MockOps::new();
        let mut sink = CollectingSink::default();
        run_bootstrap(&mut ops, &mut sink, &config(false));
        let content = ops
            .files
            .get(&command::shellrc_path("/home/u"))
            .cloned()
            .unwrap_or_default();
        for line in command::shellrc_lines("/home/u") {
            assert!(content.contains(&line), "missing shellrc line: {line}");
        }
    }

    fn assert_failed_with(out: BootstrapOutcome, expected: cowork_errors::Code) {
        match out {
            BootstrapOutcome::Failed(env) => assert_eq!(env.code, expected),
            BootstrapOutcome::Done => panic!("expected Failed, got Done"),
        }
    }

    #[test]
    fn apt_update_failure_maps_to_prereq() {
        let mut ops = MockOps::new();
        ops.fail.insert("apt-update".to_string(), 1);
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(true));
        assert_failed_with(out, cowork_errors::Code::ToolchainPrereqAptFailed);
        // Stopped at the first step: no later progress.
        assert_eq!(steps(&sink), vec!["apt-prereqs"]);
    }

    #[test]
    fn apt_install_failure_maps_to_prereq() {
        let mut ops = MockOps::new();
        ops.fail.insert("apt-install".to_string(), 100);
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(true));
        assert_failed_with(out, cowork_errors::Code::ToolchainPrereqAptFailed);
    }

    #[test]
    fn brew_failure_maps_to_brew_code() {
        let mut ops = MockOps::new();
        ops.fail.insert("brew".to_string(), 1);
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(true));
        assert_failed_with(out, cowork_errors::Code::ToolchainBrewInstallFailed);
    }

    #[test]
    fn mise_failure_maps_to_mise_code() {
        let mut ops = MockOps::new();
        ops.fail.insert("mise".to_string(), 2);
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(true));
        assert_failed_with(out, cowork_errors::Code::ToolchainMiseInstallFailed);
    }

    #[test]
    fn node_failure_maps_to_node_code() {
        let mut ops = MockOps::new();
        ops.fail.insert("node".to_string(), 3);
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(true));
        assert_failed_with(out, cowork_errors::Code::ToolchainNodeInstallFailed);
    }

    #[test]
    fn shellrc_append_failure_maps_to_shellrc_code() {
        let mut ops = MockOps::new();
        ops.append_fail = true;
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(false));
        assert_failed_with(out, cowork_errors::Code::ToolchainShellrcWriteFailed);
    }

    #[test]
    fn locale_failure_maps_to_internal_unknown() {
        let mut ops = MockOps::new();
        ops.fail.insert("locale".to_string(), 1);
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(false));
        assert_failed_with(out, cowork_errors::Code::InternalUnknown);
    }

    #[test]
    fn workspace_failure_maps_to_internal_unknown() {
        let mut ops = MockOps::new();
        ops.dir_fail = true;
        let mut sink = CollectingSink::default();
        let out = run_bootstrap(&mut ops, &mut sink, &config(false));
        assert_failed_with(out, cowork_errors::Code::InternalUnknown);
    }

    #[test]
    fn tail_keeps_last_n_chars() {
        assert_eq!(tail("abcdef", 3), "def");
        assert_eq!(tail("abc", 10), "abc");
    }
}
