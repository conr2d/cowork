//! A fully-specified command to run inside the guest distro: program, args, and
//! extra environment. Pure data shared by the guest's setup modules (the
//! toolchain bootstrap and the agent install); the `*Ops` seams turn a [`Cmd`]
//! into a real process, while tests inspect it directly.

/// A fully-specified command: `program`, `args`, and extra environment
/// variables (applied on top of the inherited environment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
    /// Extra environment variables (applied on top of the inherited environment).
    pub env: Vec<(String, String)>,
}

impl Cmd {
    /// A command with `program` and `args` and no extra environment.
    pub fn new(program: &str, args: &[&str]) -> Self {
        Self {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            env: Vec::new(),
        }
    }

    /// Add one environment variable, returning `self` for chaining.
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.to_string(), value.to_string()));
        self
    }
}
