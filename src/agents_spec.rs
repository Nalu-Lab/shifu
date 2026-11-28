//! One row per coding-agent CLI Shifu knows: how it is named, found, started, resumed and told to report status.
//! `launch.rs`, `agent.rs` and `clis.rs` read these rows; what still has its own code is listed in docs/AGENTS.md.
//!
//! Templates are argument lists; `{model}`, `{effort}` and `{id}` are replaced by the picked model, effort and the
//! saved session id. A row whose `kind` is `Kind::Other` is a plain agent: found and started, no status hooks.

use crate::agent::Kind;

/// How the agent is told to report its status to `shifu hook <id> <event>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hooks {
    /// `--settings <file>` with Claude Code's hook events (Claude, CodeBuddy).
    ClaudeSettings,
    /// One `-c hooks.<Event>=…` override per event on the command line.
    Codex,
    /// Shifu's entries in the user's `~/.kimi-code/config.toml`.
    KimiConfig,
    /// A `shifu-status` group in the user's `~/.gemini/config/hooks.json`.
    AntigravityHooksJson,
    /// `--extension <file>`: a Pi extension Shifu writes under `SHIFU_HOME` (Pi; StepFun's launcher loads its own).
    PiExtension,
    /// `OPENCODE_CONFIG_CONTENT` naming a plugin file Shifu writes under `SHIFU_HOME`; opencode merges it with the
    /// user's own config, which is never written.
    OpencodePlugin,
    /// No status: the sidebar shows it running while its pane lives.
    None,
}

/// How the start command is put together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Launch {
    /// `<program> [hook args] [permission args] [model args] [resume args]`, program = `commands[0]`.
    Direct,
    /// `shifu <id> …`: Shifu's own launcher sets up the runtime (StepFun, Qwen).
    Shifu,
    /// Its own code in `launch.rs` (Codex resumes through a `resume` subcommand).
    Bespoke,
}

#[derive(Debug)]
pub struct AgentSpec {
    /// The kind name in agent records, `shifu hook <id>` and the app.
    pub id: &'static str,
    pub kind: Kind,
    /// Command names that select this agent (`shifu new <name>`, the app); the first is the program started.
    pub commands: &'static [&'static str],
    /// Programs looked for on PATH to tell whether it is installed, first match wins.
    pub detect: &'static [&'static str],
    pub label: &'static str,
    pub install_url: &'static str,
    /// What to run in a terminal to install it, when there is one command for it (shown when it fails to start).
    pub install_command: Option<&'static str>,
    /// What to run in a terminal to sign in, for agents that have such a command; the others sign in inside the agent.
    pub login_command: Option<&'static str>,
    /// Added when Shifu's permission switch is on Bypass or Own (a fresh install is asked; see `launch::Permissions`).
    pub bypass_args: &'static [&'static str],
    pub own_args: &'static [&'static str],
    /// Environment set for Bypass, for agents whose no-approval switch is configuration rather than a flag.
    pub bypass_env: &'static [(&'static str, &'static str)],
    pub model_args: &'static [&'static str],
    pub effort_args: &'static [&'static str],
    /// Resume the saved conversation; with no saved id, `resume_latest` (empty: start a fresh one).
    pub resume_by_id: &'static [&'static str],
    pub resume_latest: &'static [&'static str],
    pub hooks: Hooks,
    pub launch: Launch,
    /// A plain agent offered only when it is found: not named under 未安装, so the list of what to install stays
    /// the core agents.
    pub extra: bool,
}

const NONE: &[&str] = &[];
const MODEL: &[&str] = &["--model", "{model}"];
const EFFORT: &[&str] = &["--effort", "{effort}"];


/// In the order the picker shows them; the first installed one is the default for a new user.
pub const AGENTS: &[AgentSpec] = &[
    AgentSpec {
        id: "claude",
        kind: Kind::Claude,
        commands: &["claude"],
        detect: &["claude"],
        label: "Claude",
        install_url: "https://code.claude.com/docs",
        install_command: Some("curl -fsSL https://claude.ai/install.sh | bash"),
        login_command: Some("claude auth login"),
        bypass_args: &["--dangerously-skip-permissions"],
        own_args: NONE,
        bypass_env: &[],
        model_args: MODEL,
        effort_args: EFFORT,
        resume_by_id: &["--resume", "{id}"],
        resume_latest: &["--continue"],
        hooks: Hooks::ClaudeSettings,
        launch: Launch::Direct,
        extra: false,
    },
    AgentSpec {
        id: "codex",
        kind: Kind::Codex,
        commands: &["codex"],
        detect: &["codex"],
        label: "Codex",
        install_url: "https://github.com/openai/codex",
        install_command: Some("npm install -g @openai/codex"),
        login_command: Some("codex login"),
        bypass_args: &["--dangerously-bypass-approvals-and-sandbox"],
        own_args: NONE,
        bypass_env: &[],
        model_args: &["-m", "{model}"],
        effort_args: &["-c", "model_reasoning_effort=\"{effort}\""],
        // `codex resume <overrides> <bypass> <model> <id|--last>`, built in launch.rs.
        resume_by_id: &["{id}"],
        resume_latest: &["--last"],
        hooks: Hooks::Codex,
        launch: Launch::Bespoke,
        extra: false,
    },
    AgentSpec {
        id: "codebuddy",
        kind: Kind::CodeBuddy,
        commands: &["codebuddy"],
        detect: &["codebuddy"],
        label: "CodeBuddy",
        install_url: "https://www.codebuddy.ai/cli",
        install_command: Some("npm install -g @tencent-ai/codebuddy-code"),
        login_command: None,
        bypass_args: &["--dangerously-skip-permissions"],
        own_args: NONE,
        bypass_env: &[],
        model_args: MODEL,
        effort_args: EFFORT,
        resume_by_id: &["--resume", "{id}"],
        resume_latest: &["--continue"],
        hooks: Hooks::ClaudeSettings,
        launch: Launch::Direct,
        extra: false,
    },
    AgentSpec {
        id: "kimi",
        kind: Kind::Kimi,
        // Kimi Code CLI (the TypeScript successor of kimi-cli). Its installer puts `kimi` in ~/.kimi-code/bin, which the
        // app's PATH lists before ~/.local/bin, where uv's old kimi-cli may put one. Agents recorded as `kimi-cli` start it too.
        commands: &["kimi", "kimi-cli"],
        detect: &["kimi"],
        label: "Kimi",
        install_url: "https://www.kimi.com/code/docs/en/kimi-code-cli/getting-started.html",
        install_command: None,
        login_command: None,
        // "Ask When Needed": risky commands, sensitive files and questions still ask.
        bypass_args: &["--yolo"],
        own_args: NONE,
        bypass_env: &[],
        model_args: MODEL,
        effort_args: NONE,
        resume_by_id: &["--session", "{id}"],
        resume_latest: &["--continue"],
        hooks: Hooks::KimiConfig,
        launch: Launch::Direct,
        extra: false,
    },
    AgentSpec {
        id: "antigravity",
        kind: Kind::Antigravity,
        commands: &["agy", "antigravity"],
        detect: &["agy", "antigravity"],
        label: "Antigravity",
        install_url: "https://antigravity.google/docs/cli-overview",
        install_command: None,
        login_command: None,
        bypass_args: NONE,
        own_args: NONE,
        bypass_env: &[(crate::launch::ANTIGRAVITY_BYPASS_ENV, "1")],
        model_args: MODEL,
        effort_args: EFFORT,
        resume_by_id: &["--conversation", "{id}"],
        // Without an id the task has not started a conversation; --continue could open another task's.
        resume_latest: NONE,
        hooks: Hooks::AntigravityHooksJson,
        launch: Launch::Direct,
        extra: false,
    },
    AgentSpec {
        id: "stepfun",
        kind: Kind::StepFun,
        commands: &["stepfun"],
        // Shifu's own Pi runtime under node; clis.rs also checks the runtime is there.
        detect: &["node"],
        label: "StepFun",
        install_url: "https://github.com/Nalu-Lab/shifu/blob/main/docs/STEPFUN.md",
        install_command: None,
        login_command: None,
        bypass_args: NONE,
        own_args: NONE,
        bypass_env: &[],
        model_args: MODEL,
        effort_args: NONE,
        resume_by_id: &["--session", "{id}"],
        resume_latest: NONE,
        hooks: Hooks::PiExtension,
        launch: Launch::Shifu,
        extra: false,
    },
    AgentSpec {
        id: "qwen",
        kind: Kind::Qwen,
        commands: &["qwen", "qwen-code"],
        detect: &["qwen"],
        label: "Qwen Code",
        install_url: "https://github.com/QwenLM/qwen-code",
        install_command: Some("npm install -g @qwen-code/qwen-code"),
        login_command: None,
        // `shifu qwen` adds `--approval-mode yolo` unless a mode is given.
        bypass_args: NONE,
        own_args: &["--approval-mode", "default"],
        bypass_env: &[],
        model_args: MODEL,
        effort_args: NONE,
        resume_by_id: &["--resume", "{id}"],
        resume_latest: NONE,
        hooks: Hooks::ClaudeSettings,
        launch: Launch::Shifu,
        extra: false,
    },
    AgentSpec {
        id: "opencode",
        kind: Kind::OpenCode,
        commands: &["opencode"],
        detect: &["opencode"],
        label: "opencode",
        install_url: "https://opencode.ai/docs/",
        install_command: Some("npm install -g opencode-ai"),
        login_command: None,
        bypass_args: &["--auto"],
        own_args: NONE,
        bypass_env: &[],
        model_args: &["-m", "{model}"],
        effort_args: NONE,
        resume_by_id: &["--session", "{id}"],
        resume_latest: &["--continue"],
        hooks: Hooks::OpencodePlugin,
        launch: Launch::Direct,
        extra: false,
    },
    AgentSpec {
        id: "pi",
        kind: Kind::Pi,
        commands: &["pi"],
        detect: &["pi"],
        label: "Pi",
        install_url: "https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent",
        install_command: Some("npm install -g @mariozechner/pi-coding-agent"),
        login_command: None,
        // Pi has no approval prompts: the permission switch changes nothing.
        bypass_args: NONE,
        own_args: NONE,
        bypass_env: &[],
        model_args: MODEL,
        effort_args: &["--thinking", "{effort}"],
        // The id Shifu's extension reports is the session file's path.
        resume_by_id: &["--session", "{id}"],
        resume_latest: &["--continue"],
        hooks: Hooks::PiExtension,
        launch: Launch::Direct,
        extra: false,
    },
    AgentSpec {
        id: "gemini",
        kind: Kind::Other,
        commands: &["gemini"],
        detect: &["gemini"],
        label: "Gemini CLI",
        install_url: "https://geminicli.com/docs/get-started/installation/",
        install_command: Some("npm install -g @google/gemini-cli"),
        login_command: None,
        bypass_args: NONE,
        own_args: NONE,
        bypass_env: &[],
        model_args: NONE,
        effort_args: NONE,
        resume_by_id: NONE,
        resume_latest: NONE,
        hooks: Hooks::None,
        launch: Launch::Direct,
        extra: false,
    },
    // Plain agents offered only when found. Command names and install pages checked 2026-11-27 against npm, PyPI,
    // Homebrew and each project's install page. No permission flags: none was checked against a real install.
    // PyPI aider-chat 0.86.2.
    plain(&["aider"], "Aider", "https://aider.chat/docs/install.html", Some("python -m pip install aider-install && aider-install")),
    // Homebrew block-goose-cli 1.61.1. The `goose` database migration tool installs the same name.
    plain(&["goose"], "Goose", "https://goose-docs.ai/docs/getting-started/installation/", Some("brew install block-goose-cli")),
    // npm @sourcegraph/amp.
    plain(&["amp"], "Amp", "https://ampcode.com/docs", Some("npm install -g @sourcegraph/amp")),
    // cursor.com/install links both `agent` and `cursor-agent`, the Homebrew cask cursor-cli only `cursor-agent`;
    // `agent` alone is too common a name to mean Cursor.
    plain(&["cursor-agent"], "Cursor CLI", "https://cursor.com/docs/cli/installation", Some("curl https://cursor.com/install -fsS | bash")),
    // npm @factory/cli 1.23.0 (bin `droid`).
    plain(&["droid"], "Factory Droid", "https://docs.factory.com/droid-cli/quickstart", Some("npm install -g @factory/cli")),
    // npm @github/copilot 1.0.117 (bin `copilot`).
    plain(&["copilot"], "GitHub Copilot CLI", "https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/install-copilot-cli", Some("npm install -g @github/copilot")),
    // npm @charmland/crush 0.104.0.
    plain(&["crush"], "Crush", "https://github.com/charmbracelet/crush", Some("npm install -g @charmland/crush")),
];

/// An `extra` plain agent named after its program (`id` is `commands[0]`): found by that name, started as it is,
/// no hooks, model or resume arguments.
const fn plain(commands: &'static [&'static str], label: &'static str, install_url: &'static str, install_command: Option<&'static str>) -> AgentSpec {
    AgentSpec {
        id: commands[0],
        kind: Kind::Other,
        commands,
        detect: commands,
        label,
        install_url,
        install_command,
        login_command: None,
        bypass_args: NONE,
        own_args: NONE,
        bypass_env: &[],
        model_args: NONE,
        effort_args: NONE,
        resume_by_id: NONE,
        resume_latest: NONE,
        hooks: Hooks::None,
        launch: Launch::Direct,
        extra: true,
    }
}

/// Where a row comes from, as `shifu agents` shows it: `built-in` (with status hooks) or `plain` (no hooks).
pub fn source(spec: &AgentSpec) -> &'static str {
    if spec.hooks == Hooks::None { "plain" } else { "built-in" }
}

impl Hooks {
    /// How `shifu agents` names it.
    pub fn name(self) -> &'static str {
        match self {
            Hooks::ClaudeSettings => "claude-settings",
            Hooks::Codex => "codex",
            Hooks::KimiConfig => "kimi-config",
            Hooks::AntigravityHooksJson => "antigravity-hooks-json",
            Hooks::PiExtension => "pi-extension",
            Hooks::OpencodePlugin => "opencode-plugin",
            Hooks::None => "none",
        }
    }
}

/// The program and its own arguments for a plain command recorded as `command`: an agent from
/// `<SHIFU_HOME>/agents.json` with that id runs its `command` and `args` (read again at every start and resume, so
/// an edit applies to the next one); a plain row of the table runs its program; anything else runs as it is.
pub fn plain_launch(command: &str) -> (String, Vec<String>) {
    let (users, _) = crate::user_agents::load();
    plain_launch_with(command, &users)
}

pub fn plain_launch_with(command: &str, users: &[crate::user_agents::UserAgent]) -> (String, Vec<String>) {
    if let Some(user) = users.iter().find(|u| u.id == command) {
        return (user.command.clone(), user.args.clone());
    }
    match of_command(command).filter(|s| s.kind == Kind::Other) {
        Some(spec) => (spec.commands[0].to_string(), Vec::new()),
        None => (command.to_string(), Vec::new()),
    }
}

/// The row a recorded kind runs by; `None` for `Kind::Other` (plain commands keep the command they were given).
pub fn of_kind(kind: Kind) -> Option<&'static AgentSpec> {
    if kind == Kind::Other {
        return None;
    }
    AGENTS.iter().find(|s| s.kind == kind)
}

/// The row whose command names include `command`.
pub fn of_command(command: &str) -> Option<&'static AgentSpec> {
    AGENTS.iter().find(|s| s.commands.contains(&command))
}

/// `template` with `{name}` replaced by `value`.
pub fn fill(template: &[&str], name: &str, value: &str) -> Vec<String> {
    let key = format!("{{{name}}}");
    template.iter().map(|w| w.replace(&key, value)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_one_row_named_as_it_is_saved() {
        for kind in [Kind::Claude, Kind::Codex, Kind::CodeBuddy, Kind::Kimi, Kind::Antigravity, Kind::StepFun, Kind::Qwen, Kind::OpenCode, Kind::Pi] {
            let spec = of_kind(kind).unwrap_or_else(|| panic!("{kind:?} has no row"));
            assert_eq!(spec.id, kind.as_str());
            assert_eq!(serde_json::to_value(kind).unwrap(), spec.id);
            assert_eq!(AGENTS.iter().filter(|s| s.kind == kind).count(), 1);
            assert_eq!(Kind::of_command(spec.commands[0]), kind);
        }
        assert!(of_kind(Kind::Other).is_none());
        let ids: Vec<&str> = AGENTS.iter().map(|s| s.id).collect();
        let mut unique = ids.clone();
        unique.dedup();
        assert_eq!(ids, unique);
    }

    #[test]
    fn templates_fill_every_placeholder() {
        assert_eq!(fill(&["-c", "model_reasoning_effort=\"{effort}\""], "effort", "low"), ["-c", "model_reasoning_effort=\"low\""]);
        for spec in AGENTS {
            for (t, name) in [(spec.model_args, "model"), (spec.effort_args, "effort"), (spec.resume_by_id, "id")] {
                assert!(fill(t, name, "x").iter().all(|w| !w.contains('{')), "{} {name}", spec.id);
            }
        }
    }

    #[test]
    fn plain_agents_take_no_hooks() {
        for spec in AGENTS.iter().filter(|s| s.kind == Kind::Other) {
            assert_eq!(spec.hooks, Hooks::None, "{}", spec.id);
            assert!(spec.bypass_args.is_empty() && spec.own_args.is_empty() && spec.bypass_env.is_empty(), "{}", spec.id);
            assert_eq!(source(spec), "plain");
        }
        // Extra rows are plain, named after their program, and only the core rows are listed when missing.
        for spec in AGENTS.iter().filter(|s| s.extra) {
            assert_eq!((spec.kind, spec.commands[0], spec.detect), (Kind::Other, spec.id, spec.commands), "{}", spec.id);
        }
        assert!(AGENTS.iter().filter(|s| s.kind != Kind::Other).all(|s| !s.extra && source(s) == "built-in"));
        assert!(!of_command("gemini").unwrap().extra);
    }

    #[test]
    fn plain_commands_run_the_user_entry_then_the_row_then_as_given() {
        use crate::user_agents::UserAgent;
        let users = [UserAgent {
            id: "mybot".into(), label: "My Bot".into(), command: "/opt/my bot/run".into(), args: vec!["--foo".into()], install_url: None,
        }];
        assert_eq!(plain_launch_with("mybot", &users), ("/opt/my bot/run".to_string(), vec!["--foo".to_string()]));
        assert_eq!(plain_launch_with("cursor-agent", &users), ("cursor-agent".to_string(), vec![]));
        assert_eq!(plain_launch_with("sh", &users), ("sh".to_string(), vec![]));
        // A hooked agent's command is not a plain one.
        assert_eq!(plain_launch_with("claude", &[]).0, "claude");
    }
}
