//! Antigravity theme adapter (v0.2 WP4d): keep agy's persisted colorScheme in
//! sync with Cowork's light/dark app theme before mounting an agy terminal.

use std::fs;
use std::path::Path;

use clap::ValueEnum;
use cowork_errors::protocol::{Message, PROTOCOL_VERSION};
use cowork_errors::{Code, Envelope, Stage};
use serde_json::{Map, Value};

use crate::sink::ProgressSink;

use super::command::{self, Agent};

/// App theme the agy adapter maps to a colorScheme agy renders legibly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AppTheme {
    Light,
    Dark,
}

pub fn color_scheme(theme: AppTheme) -> &'static str {
    match theme {
        AppTheme::Light => "solarized light",
        AppTheme::Dark => "tokyo night",
    }
}

#[derive(Debug, Clone)]
pub enum AgyThemeOutcome {
    Done,
    Failed(Envelope),
}

pub fn run_agy_theme(sink: &mut dyn ProgressSink, home: &str, theme: AppTheme) -> AgyThemeOutcome {
    sink.emit(&Message::Hello {
        protocol_version: PROTOCOL_VERSION,
    });

    match write_agy_theme(home, theme) {
        Ok(()) => {
            sink.emit(&Message::Done {
                stage: Stage::Workspace,
            });
            AgyThemeOutcome::Done
        }
        Err(cause) => {
            let env = Envelope::new(Code::AgentThemeSyncFailed, Stage::Workspace)
                .with_context("agent", "antigravity")
                .with_cause(&cause);
            sink.emit(&Message::Error {
                envelope: env.clone(),
            });
            AgyThemeOutcome::Failed(env)
        }
    }
}

fn write_agy_theme(home: &str, theme: AppTheme) -> Result<(), String> {
    let path = Path::new(&command::config_dir(Agent::Antigravity, home)).join("settings.json");
    let parent = path.parent().expect("settings path has parent");
    fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;

    let mut object = match fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
    {
        Some(Value::Object(object)) => object,
        _ => Map::new(),
    };
    object.insert(
        "colorScheme".to_string(),
        Value::String(color_scheme(theme).to_string()),
    );

    let tmp = path.with_file_name("settings.json.tmp");
    let bytes = serde_json::to_vec(&Value::Object(object)).map_err(|e| e.to_string())?;
    fs::write(&tmp, bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename {}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Default)]
    struct CollectingSink {
        messages: Vec<Message>,
    }

    impl ProgressSink for CollectingSink {
        fn emit(&mut self, message: &Message) {
            self.messages.push(message.clone());
        }
    }

    struct TempHome {
        path: std::path::PathBuf,
    }

    impl TempHome {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock must be after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("cowork-theme-{name}-{nanos}"));
            fs::create_dir_all(&path).expect("create temp home");
            Self { path }
        }

        fn as_str(&self) -> &str {
            self.path.to_str().expect("temp path must be utf-8")
        }

        fn settings(&self) -> std::path::PathBuf {
            self.path
                .join(".gemini")
                .join("antigravity-cli")
                .join("settings.json")
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn read_settings(home: &TempHome) -> Value {
        serde_json::from_str(&fs::read_to_string(home.settings()).expect("read settings"))
            .expect("settings json")
    }

    #[test]
    fn color_scheme_maps_app_themes() {
        assert_eq!(color_scheme(AppTheme::Light), "solarized light");
        assert_eq!(color_scheme(AppTheme::Dark), "tokyo night");
    }

    #[test]
    fn fresh_home_creates_settings_file() {
        let home = TempHome::new("fresh");
        let mut sink = CollectingSink::default();
        assert!(matches!(
            run_agy_theme(&mut sink, home.as_str(), AppTheme::Light),
            AgyThemeOutcome::Done
        ));
        assert_eq!(read_settings(&home)["colorScheme"], "solarized light");
    }

    #[test]
    fn existing_settings_preserve_other_keys() {
        let home = TempHome::new("preserve");
        let settings = home.settings();
        fs::create_dir_all(settings.parent().expect("settings parent")).expect("create parent");
        fs::write(
            &settings,
            r#"{"colorScheme":"tokyo night","enableTelemetry":false}"#,
        )
        .expect("write settings");
        let mut sink = CollectingSink::default();
        assert!(matches!(
            run_agy_theme(&mut sink, home.as_str(), AppTheme::Light),
            AgyThemeOutcome::Done
        ));
        let value = read_settings(&home);
        assert_eq!(value["colorScheme"], "solarized light");
        assert_eq!(value["enableTelemetry"], false);
    }

    #[test]
    fn corrupt_settings_are_replaced_with_valid_object() {
        let home = TempHome::new("corrupt");
        let settings = home.settings();
        fs::create_dir_all(settings.parent().expect("settings parent")).expect("create parent");
        fs::write(&settings, "not json").expect("write corrupt settings");
        let mut sink = CollectingSink::default();
        assert!(matches!(
            run_agy_theme(&mut sink, home.as_str(), AppTheme::Dark),
            AgyThemeOutcome::Done
        ));
        assert_eq!(read_settings(&home)["colorScheme"], "tokyo night");
    }

    #[test]
    fn emits_hello_then_done() {
        let home = TempHome::new("messages");
        let mut sink = CollectingSink::default();
        assert!(matches!(
            run_agy_theme(&mut sink, home.as_str(), AppTheme::Dark),
            AgyThemeOutcome::Done
        ));
        assert!(matches!(sink.messages[0], Message::Hello { .. }));
        assert!(matches!(
            sink.messages[1],
            Message::Done {
                stage: Stage::Workspace
            }
        ));
        assert_eq!(sink.messages.len(), 2);
    }
}
