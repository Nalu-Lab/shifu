//! Why an agent ended right after it was started: no such command, not signed in, a Node.js too old for it.
//! The app shows this in the exited panel with the command that fixes it (docs/ONBOARDING.md, phase 4).
//!
//! The patterns are what shells and Node print themselves; the agents installed here (Claude, Codex, Antigravity,
//! Qwen Code) do not exit when signed out, they show their own sign-in screen, so the sign-in words are generic.

use crate::agent::{Agent, Kind};
use crate::agents_spec::{self, AgentSpec, Launch};
use crate::status::Status;
use crate::tmux::Tmux;
use serde::Serialize;

/// A pane that died this soon after its launch counts as "ended right away".
pub const QUICK_SECS: u64 = 10;
/// Lines of the screen kept to show what it printed last.
const TAIL: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    NotFound,
    Login,
    Node,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LaunchFailure {
    pub reason: Reason,
    /// One or two plain sentences for the panel.
    pub message: String,
    /// What to run in a terminal to fix it.
    pub command: Option<String>,
    pub install_url: Option<String>,
    /// The last lines it printed.
    pub output: Vec<String>,
}

/// What tmux says about a dead pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Death {
    pub status: Option<i32>,
    pub at: u64,
}

const DEATH_FORMAT: &str = "#{pane_dead}|#{pane_dead_status}|#{pane_dead_time}";

/// A `DEATH_FORMAT` line; `None` while the pane lives. A pane ended by a signal has no status.
pub fn parse_death(line: &str) -> Option<Death> {
    let mut words = line.trim().split('|');
    if words.next()? != "1" {
        return None;
    }
    let status = words.next().and_then(|w| w.parse().ok());
    let at = words.next()?.parse().ok()?;
    Some(Death { status, at })
}

/// The record still holds what the launcher wrote (no hook has changed it, so `status_since` is the launch time)
/// and the pane died within `QUICK_SECS` of it. Codex resumes are written as Idle; other agents start as Starting.
pub fn ended_at_launch(agent: &Agent, death: Death) -> bool {
    let untouched = agent.status == Status::Starting || (agent.kind == Kind::Codex && agent.status == Status::Idle);
    untouched && death.at >= agent.status_since && death.at - agent.status_since <= QUICK_SECS
}

fn spec_of(agent: &Agent) -> Option<&'static AgentSpec> {
    agents_spec::of_kind(agent.kind).or_else(|| agents_spec::of_command(&agent.command))
}

/// The screen's last non-blank lines, without the line tmux adds under a dead pane.
fn tail(screen: &str) -> Vec<String> {
    let lines: Vec<&str> = screen
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty() && !l.starts_with("Pane is dead"))
        .collect();
    lines[lines.len().saturating_sub(TAIL)..].iter().map(|l| l.chars().take(200).collect()).collect()
}

/// `v18.20.8` from Node's footer under an uncaught error (`Node.js v18.20.8`).
fn node_version(lines: &[String]) -> Option<String> {
    lines.iter().rev().find_map(|l| {
        let v = l.trim().strip_prefix("Node.js v")?;
        v.split('.').next()?.parse::<u32>().ok()?;
        Some(format!("v{v}"))
    })
}

const LOGIN_WORDS: &[&str] = &[
    "not logged in", "not signed in", "please log in", "please login", "please sign in", "log in again",
    "unauthorized", "invalid api key", "authentication failed", "authentication required",
];

pub fn classify(agent: &Agent, status: Option<i32>, screen: &str) -> LaunchFailure {
    let spec = spec_of(agent);
    let program = agent.command.as_str();
    let label = spec.map_or(program, |s| s.label);
    let install_url = spec.map(|s| s.install_url.to_string());
    let output = tail(screen);
    let text = screen.to_lowercase();
    // 127 is the shell's "command not found"; Shifu's own launchers (StepFun, Qwen) report the exec error instead.
    let not_found = status == Some(127)
        || text.contains("command not found")
        || (spec.is_some_and(|s| s.launch == Launch::Shifu) && text.contains("(os error 2)"));
    if not_found {
        let command = spec.and_then(|s| s.install_command).map(str::to_string);
        let how = if command.is_some() { "在终端里运行下面的命令装好" } else { "按安装说明装好" };
        return LaunchFailure {
            reason: Reason::NotFound,
            message: format!("没找到 {program} 命令，{label} 可能还没装。{how}，再点「重试」。"),
            command, install_url, output,
        };
    }
    if LOGIN_WORDS.iter().any(|w| text.contains(w)) {
        let command = spec.and_then(|s| s.login_command).map(str::to_string);
        let how = if command.is_some() { "在终端里运行下面的命令登录" } else { "按它的提示登录" };
        return LaunchFailure {
            reason: Reason::Login,
            message: format!("{label} 好像还没登录。{how}，再点「重试」。"),
            command, install_url, output,
        };
    }
    if let Some(version) = node_version(&output) {
        return LaunchFailure {
            reason: Reason::Node,
            message: format!("{label} 在 Node.js {version} 上出错退出了，常见原因是 Node.js 版本太旧。升级 Node.js 后再点「重试」。"),
            command: None, install_url, output,
        };
    }
    let code = status.map(|c| format!("（退出码 {c}）")).unwrap_or_default();
    LaunchFailure {
        reason: Reason::Other,
        message: format!("{label} 启动后马上退出了{code}。下面是它最后的输出。"),
        command: None, install_url, output,
    }
}

/// The reason, when `agent` ended right after its launch and its dead pane is still there to read.
pub fn check(tmux: &Tmux, agent: &Agent) -> Option<LaunchFailure> {
    let pane = agent.pane_id.as_deref()?;
    let line = tmux.run(["display-message", "-p", "-t", pane, DEATH_FORMAT]).ok()?;
    let death = parse_death(&line)?;
    if !ended_at_launch(agent, death) {
        return None;
    }
    let screen = tmux.run(["capture-pane", "-J", "-p", "-S", "-200", "-t", pane]).unwrap_or_default();
    Some(classify(agent, death.status, &screen))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(kind: Kind, command: &str, status: Status) -> Agent {
        Agent {
            name: format!("{command}-x"), kind, command: command.into(), cwd: "/tmp".into(), created_at: 100,
            status, status_since: 100, window_id: None, pane_id: Some("%1".into()), session_id: None,
            last_message: None, waiting_for: None, model: None, effort: None, label: None,
        }
    }

    #[test]
    fn reads_tmux_death_line() {
        assert_eq!(parse_death("1|127|1763900000\n"), Some(Death { status: Some(127), at: 1763900000 }));
        assert_eq!(parse_death("1||1763900000"), Some(Death { status: None, at: 1763900000 }));
        assert_eq!(parse_death("0||"), None);
        assert_eq!(parse_death(""), None);
    }

    #[test]
    fn only_a_quick_death_before_any_hook_counts() {
        let starting = agent(Kind::Claude, "claude", Status::Starting);
        assert!(ended_at_launch(&starting, Death { status: Some(1), at: 103 }));
        assert!(!ended_at_launch(&starting, Death { status: Some(1), at: 100 + QUICK_SECS + 1 }));
        // A hook moved it on: it ran, and ending is the user's doing.
        assert!(!ended_at_launch(&agent(Kind::Claude, "claude", Status::Idle), Death { status: Some(0), at: 101 }));
        assert!(ended_at_launch(&agent(Kind::Codex, "codex", Status::Idle), Death { status: Some(1), at: 101 }));
        assert!(!ended_at_launch(&starting, Death { status: Some(1), at: 99 }));
    }

    #[test]
    fn command_not_found_offers_the_install_command() {
        // As zsh, sh and bash print it (checked 2026-11-23).
        for screen in ["zsh:1: command not found: claude", "sh: claude: command not found", "bash: claude: command not found"] {
            let f = classify(&agent(Kind::Claude, "claude", Status::Starting), Some(127), screen);
            assert_eq!(f.reason, Reason::NotFound, "{screen}");
            assert_eq!(f.command.as_deref(), Some("curl -fsSL https://claude.ai/install.sh | bash"));
            assert!(f.message.contains("没找到 claude 命令"), "{}", f.message);
            assert_eq!(f.output, [screen]);
        }
        let kimi = classify(&agent(Kind::Kimi, "kimi", Status::Starting), Some(127), "");
        assert_eq!((kimi.reason, kimi.command), (Reason::NotFound, None));
        assert!(kimi.message.contains("按安装说明") && kimi.install_url.is_some());
        let qwen = classify(&agent(Kind::Qwen, "qwen", Status::Starting), Some(1), "Error: No such file or directory (os error 2)");
        assert_eq!(qwen.reason, Reason::NotFound);
    }

    #[test]
    fn sign_in_messages_offer_the_login_command() {
        let f = classify(&agent(Kind::Codex, "codex", Status::Idle), Some(1), "Error: Not logged in. Run codex login");
        assert_eq!((f.reason, f.command.as_deref()), (Reason::Login, Some("codex login")));
        let pi = classify(&agent(Kind::Pi, "pi", Status::Starting), Some(1), "401 Unauthorized");
        assert_eq!((pi.reason, pi.command), (Reason::Login, None));
        assert!(pi.message.contains("按它的提示登录"));
    }

    #[test]
    fn node_footer_points_at_an_old_node() {
        // gemini-cli under Node 18, as it really ends (2026-11-23).
        let screen = "ReferenceError: File is not defined\n    at __require2 (file:///x.js:18:50)\n\nNode.js v18.20.8\n\nPane is dead (status 1, Sun Nov 23 10:00:00 2026)\n";
        let f = classify(&agent(Kind::Other, "gemini", Status::Starting), Some(1), screen);
        assert_eq!(f.reason, Reason::Node);
        assert!(f.message.starts_with("Gemini CLI 在 Node.js v18.20.8 上"), "{}", f.message);
        assert_eq!(f.output.last().map(String::as_str), Some("Node.js v18.20.8"));
    }

    #[test]
    fn anything_else_shows_the_last_lines() {
        let screen = (1..=12).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n") + "\n\n";
        let f = classify(&agent(Kind::Claude, "claude", Status::Starting), Some(3), &screen);
        assert_eq!(f.reason, Reason::Other);
        assert_eq!(f.message, "Claude 启动后马上退出了（退出码 3）。下面是它最后的输出。");
        assert_eq!(f.output.len(), TAIL);
        assert_eq!(f.output.last().map(String::as_str), Some("line 12"));
    }
}
