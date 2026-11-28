//! The one-click actions of the setup checklist and the first-run guide (setupcheck.ts): install an agent, sign in,
//! set up browser use in a visible Shifu terminal tab; install the Computer Use helper and open its permission
//! window; open the notification settings. Each runs only when the person clicks it. Which commands run is decided
//! here from the agent table (`clis::Found`), never sent by the page.

use crate::config::Config;
use crate::tasks::{Changed, Pick, Tasks};
use serde::Serialize;
use shifu::launch::sh_quote;
use std::process::{Command, Stdio};
use tauri::State;

/// What a checklist click runs in a terminal tab, or `None` when that agent has no such command.
pub fn terminal_command(found: &[shifu::clis::Found], kind: &str, action: &str, shifu_bin: &str) -> Option<String> {
    if action == "browser" {
        return Some(format!("{} browser install", sh_quote(shifu_bin)));
    }
    let agent = found.iter().find(|a| a.kind == kind)?;
    match action {
        "install" => agent.install_command.clone(),
        "login" => agent.login_command.clone(),
        _ => None,
    }
}

fn user_home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

/// Opens a tab for the action: in task `task` when it still exists, else in a new task in the home folder named
/// `label`. A command (install, sign-in command, browser runtime) is typed into a terminal there, so the person sees
/// it and its progress and can answer its questions; an agent without a sign-in command is opened itself, since it
/// asks to sign in when it starts.
#[tauri::command]
pub fn setup_run(
    cfg: State<Config>,
    tasks: State<Tasks>,
    kind: String,
    action: String,
    task: Option<String>,
    label: String,
) -> Result<Changed, String> {
    let found = shifu::clis::detect(&shifu::clis::agent_path().unwrap_or_default());
    let command = terminal_command(&found, &kind, &action, &cfg.shifu_bin);
    let opens = match (&command, action.as_str()) {
        (Some(_), _) => "terminal",
        (None, "login") if found.iter().any(|a| a.kind == kind && a.path.is_some()) => kind.as_str(),
        _ => return Err(format!("nothing to run for {kind} {action}")),
    };
    let pick = Pick { model: None, effort: None };
    let existing = task.filter(|id| tasks.load(&cfg).is_ok_and(|f| f.tasks.iter().any(|t| &t.id == id)));
    let changed = match existing {
        Some(id) => tasks.add_agent(&cfg, &id, opens, pick)?,
        None => tasks.create(&cfg, &user_home(), &label, opens, pick)?,
    };
    if let (Some(command), Some(name)) = (command, changed.agent.as_deref()) {
        type_line(name, &command, &label)?;
    }
    Ok(changed)
}

/// Names the tab and types `command` into its shell, with Return.
fn type_line(name: &str, command: &str, label: &str) -> Result<(), String> {
    let _lock = shifu::agent::lock()?;
    let mut record = shifu::agent::load(name)?.ok_or("the new terminal is gone")?;
    record.label = Some(label.to_string());
    shifu::agent::save(&record)?;
    let pane = record.pane_id.ok_or("the new terminal has no pane")?;
    let tmux = shifu::tmux::Tmux::from_env();
    tmux.run(["send-keys", "-t", &pane, "-l", command])?;
    tmux.run(["send-keys", "-t", &pane, "Enter"]).map(|_| ())
}

#[derive(Serialize, Debug, Default)]
pub struct Extras {
    /// The browser runtime is installed (`shifu browser doctor`).
    pub browser: bool,
    /// `ready`, `permissions` (installed, not yet allowed), `install` (Shifu carries it, not installed) or
    /// `missing` (not installed and not carried).
    pub computer: String,
}

fn cli_json(bin: &str, args: &[&str]) -> Option<serde_json::Value> {
    let out = Command::new(bin).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    serde_json::from_slice(&out.stdout).ok()
}

/// How far browser use and Computer Use are set up. Asks the CLI's read-only doctors; the Computer Use one starts
/// the helper in the background for a moment to read its permissions, so this is asked only while the checklist is
/// open.
#[tauri::command]
pub async fn setup_extras(cfg: State<'_, Config>) -> Result<Extras, String> {
    let bin = cfg.shifu_bin.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let browser = cli_json(&bin, &["browser", "doctor", "--json"]).is_some_and(|v| v["result"]["installed"] == true);
        let computer = match cli_json(&bin, &["computer", "doctor", "--json"]) {
            Some(v) if v["ok"] == true => "ready",
            Some(v) if v["result"]["installed"] == true => "permissions",
            Some(v) if v["result"]["nextStep"].as_str().is_some_and(|s| s.contains("shifu computer install")) => "install",
            _ => "missing",
        };
        Ok(Extras { browser, computer: computer.into() })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// `computer`: installs the helper Shifu carries (when it is not installed yet) and opens its permission window,
/// where the person allows it. `notifications`: opens the notification settings.
#[tauri::command]
pub async fn setup_open(cfg: State<'_, Config>, what: String) -> Result<(), String> {
    let bin = cfg.shifu_bin.clone();
    tauri::async_runtime::spawn_blocking(move || match what.as_str() {
        "computer" => {
            let installed = cli_json(&bin, &["computer", "install"]).ok_or("the Computer Use helper could not be installed")?;
            if installed["ok"] != true {
                return Err(installed["error"]["message"].as_str().unwrap_or("the Computer Use helper could not be installed").to_string());
            }
            run(Command::new(&bin).args(["computer", "permissions"]))
        }
        "notifications" => run(Command::new("/usr/bin/open").arg("x-apple.systempreferences:com.apple.Notifications-Settings.extension")),
        _ => Err(format!("unknown setup item {what}")),
    })
    .await
    .map_err(|e| e.to_string())?
}

fn run(command: &mut Command) -> Result<(), String> {
    let out = command.stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_commands_come_from_the_agent_table() {
        let mut found = shifu::clis::detect_with("/nonexistent-shifu-path", &[]);
        let claude = found.iter_mut().find(|a| a.kind == "claude").unwrap();
        claude.install_command = Some("curl -fsSL https://claude.ai/install.sh | bash".into());
        assert_eq!(terminal_command(&found, "claude", "install", "/s").as_deref(), Some("curl -fsSL https://claude.ai/install.sh | bash"));
        assert_eq!(terminal_command(&found, "claude", "login", "/s").as_deref(), Some("claude auth login"));
        assert_eq!(terminal_command(&found, "kimi", "install", "/s"), None);
        assert_eq!(terminal_command(&found, "nobody", "install", "/s"), None);
        assert_eq!(terminal_command(&found, "claude", "rm -rf", "/s"), None);
        assert_eq!(terminal_command(&found, "", "browser", "/a b/shifu").as_deref(), Some("'/a b/shifu' browser install"));
    }
}
