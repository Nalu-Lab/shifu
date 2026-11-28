//! Agents the user adds themselves in `<SHIFU_HOME>/agents.json` (the app's is `~/.shifu/app/agents.json`):
//!
//! ```json
//! [{"id": "mybot", "label": "My Bot", "command": "mybot", "args": ["--foo"], "install_url": "https://…"}]
//! ```
//!
//! They are plain agents (no status hooks): found on PATH like the built-in ones, offered in the picker when found,
//! started as `command args…` (each word shell-quoted), and resumed by running that again. The file is read at
//! every detection and start, so an edit applies to the next one. An entry that does not pass the checks is left
//! out with a warning; the file never stops anything else from working.

use serde_json::Value;
use std::path::{Path, PathBuf};

pub const FILE: &str = "agents.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserAgent {
    pub id: String,
    pub label: String,
    /// A bare program name (looked up on PATH) or an absolute path.
    pub command: String,
    pub args: Vec<String>,
    pub install_url: Option<String>,
}

pub fn path_in(home: &Path) -> PathBuf {
    home.join(FILE)
}

/// The agents in `<SHIFU_HOME>/agents.json` and a warning per entry left out. No file: nothing, no warning.
pub fn load() -> (Vec<UserAgent>, Vec<String>) {
    load_from(&path_in(&crate::agent::shifu_home()))
}

pub fn load_from(file: &Path) -> (Vec<UserAgent>, Vec<String>) {
    match std::fs::read_to_string(file) {
        Ok(text) => {
            let (agents, warnings) = parse(&text);
            (agents, warnings.into_iter().map(|w| format!("{}: {w}", file.display())).collect())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Vec::new(), Vec::new()),
        Err(e) => (Vec::new(), vec![format!("{}: not read ({e}); no agents from it", file.display())]),
    }
}

/// Names a user agent may not take: the built-in rows' ids and programs, and what Shifu itself uses.
fn reserved(id: &str) -> bool {
    matches!(id, "terminal" | "other" | "shifu")
        || crate::agents_spec::AGENTS.iter().any(|s| s.id == id || s.commands.contains(&id) || s.detect.contains(&id))
}

fn has_control(s: &str) -> bool {
    s.chars().any(char::is_control)
}

/// A program name as typed in a terminal, without anything a shell would read specially.
fn bare_name(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('-') && s.chars().all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c))
}

/// The entries that pass, in file order, and why each other one was left out.
pub fn parse(text: &str) -> (Vec<UserAgent>, Vec<String>) {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return (Vec::new(), vec![format!("not valid JSON ({e}); no agents from it")]),
    };
    let Some(entries) = value.as_array() else {
        return (Vec::new(), vec!["must be a JSON array of agents, like [{\"id\": \"mybot\", \"command\": \"mybot\"}]".into()]);
    };
    let (mut agents, mut warnings): (Vec<UserAgent>, Vec<String>) = (Vec::new(), Vec::new());
    for (i, entry) in entries.iter().enumerate() {
        match check(entry) {
            Ok(agent) if agents.iter().any(|a| a.id == agent.id) => {
                warnings.push(format!("entry {} left out: id {:?} is already used by an earlier entry", i + 1, agent.id));
            }
            Ok(agent) => agents.push(agent),
            Err(why) => {
                let id = entry.get("id").and_then(Value::as_str).map(|id| format!(" ({id:?})")).unwrap_or_default();
                warnings.push(format!("entry {}{id} left out: {why}", i + 1));
            }
        }
    }
    (agents, warnings)
}

fn check(entry: &Value) -> Result<UserAgent, String> {
    let obj = entry.as_object().ok_or("must be an object")?;
    let text = |key: &str| -> Result<Option<String>, String> {
        match obj.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) if has_control(s) => Err(format!("{key} has a line break or control character")),
            Some(Value::String(s)) => Ok(Some(s.trim().to_string()).filter(|s| !s.is_empty())),
            Some(_) => Err(format!("{key} must be a string")),
        }
    };
    let id = text("id")?.ok_or("id is required")?;
    if !bare_name(&id) || id.len() > 40 {
        return Err("id may only use letters, digits, '.', '_', '+' and '-' (at most 40, not starting with '-')".into());
    }
    if reserved(&id) {
        return Err(format!("id {id:?} is a built-in agent's name; pick another"));
    }
    let command = text("command")?.ok_or("command is required")?;
    let absolute = command.starts_with('/');
    if !absolute && !bare_name(&command) {
        return Err(format!(
            "command {command:?} must be a program name or an absolute path, not a shell command; put its arguments in \"args\""
        ));
    }
    let args = match obj.get("args") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|a| match a {
                Value::String(s) if has_control(s) => Err("args has a line break or control character".to_string()),
                Value::String(s) => Ok(s.clone()),
                _ => Err("args must be a list of strings".to_string()),
            })
            .collect::<Result<_, _>>()?,
        Some(_) => return Err("args must be a list of strings".into()),
    };
    let install_url = text("install_url")?;
    if let Some(url) = &install_url
        && !(url.starts_with("https://") || url.starts_with("http://"))
    {
        return Err("install_url must start with https:// or http://".into());
    }
    let label = text("label")?.unwrap_or_else(|| id.clone());
    Ok(UserAgent { id, label, command, args, install_url })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_entries_are_kept_in_order_with_defaults() {
        let (agents, warnings) = parse(
            r#"[{"id":"mybot","label":"My Bot","command":"mybot","args":["--foo","a b"],"install_url":"https://example.com"},
                {"id":"local","command":"/opt/tools/my agent"}]"#,
        );
        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0], UserAgent {
            id: "mybot".into(), label: "My Bot".into(), command: "mybot".into(),
            args: vec!["--foo".into(), "a b".into()], install_url: Some("https://example.com".into()),
        });
        assert_eq!((agents[1].label.as_str(), agents[1].command.as_str(), agents[1].args.len()), ("local", "/opt/tools/my agent", 0));
    }

    #[test]
    fn bad_entries_are_left_out_with_a_reason_and_the_rest_kept() {
        let (agents, warnings) = parse(
            r#"[{"id":"ok","command":"ok"},
                {"command":"x"},
                {"id":"noprog"},
                {"id":"claude","command":"claude"},
                {"id":"kimi-cli","command":"x"},
                {"id":"terminal","command":"x"},
                {"id":"sh","command":"bash -c 'rm -rf ~'"},
                {"id":"rel","command":"./bin/x"},
                {"id":"bad id","command":"x"},
                {"id":"-x","command":"x"},
                {"id":"argy","command":"x","args":"--foo"},
                {"id":"argn","command":"x","args":[1]},
                {"id":"nl","command":"x","args":["a\nb"]},
                {"id":"url","command":"x","install_url":"javascript:alert(1)"},
                {"id":"ok","command":"again"},
                "not an object"]"#,
        );
        assert_eq!(agents.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["ok"]);
        assert_eq!(warnings.len(), 15, "{warnings:#?}");
        assert!(warnings[0].contains("id is required"));
        assert!(warnings[1].contains("command is required"));
        assert!(warnings[2].contains("built-in"), "{}", warnings[2]);
        assert!(warnings[3].contains("built-in"), "{}", warnings[3]);
        assert!(warnings[5].contains("not a shell command"), "{}", warnings[5]);
        assert!(warnings[13].contains("already used"), "{}", warnings[13]);
        assert!(warnings[14].contains("entry 16"));
    }

    #[test]
    fn a_broken_or_missing_file_gives_no_agents_and_never_fails() {
        assert_eq!(parse("{nope").0, []);
        assert!(parse("{nope").1[0].contains("not valid JSON"));
        assert!(parse(r#"{"id":"x"}"#).1[0].contains("JSON array"));
        assert_eq!(parse("[]"), (Vec::new(), Vec::new()));
        let missing = std::env::temp_dir().join(format!("shifu-no-agents-{}/agents.json", std::process::id()));
        assert_eq!(load_from(&missing), (Vec::new(), Vec::new()));
    }

    #[test]
    fn every_extra_built_in_name_is_reserved() {
        for id in ["aider", "goose", "amp", "cursor-agent", "droid", "copilot", "crush", "gemini", "agy", "node"] {
            assert!(reserved(id), "{id}");
        }
        assert!(!reserved("mybot"));
    }
}
