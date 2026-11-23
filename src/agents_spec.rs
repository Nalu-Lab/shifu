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
    /// Added when Shifu's permission switch is on Bypass (the default) or Own.
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
        bypass_args: &["--dangerously-skip-permissions"],
        own_args: NONE,
        bypass_env: &[],
        model_args: MODEL,
        effort_args: EFFORT,
        resume_by_id: &["--resume", "{id}"],
        resume_latest: &["--continue"],
        hooks: Hooks::ClaudeSettings,
        launch: Launch::Direct,
    },
    AgentSpec {
        id: "codex",
        kind: Kind::Codex,
        commands: &["codex"],
        detect: &["codex"],
        label: "Codex",
        install_url: "https://github.com/openai/codex",
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
    },
    AgentSpec {
        id: "codebuddy",
        kind: Kind::CodeBuddy,
        commands: &["codebuddy"],
        detect: &["codebuddy"],
        label: "CodeBuddy",
        install_url: "https://www.codebuddy.ai/cli",
        bypass_args: &["--dangerously-skip-permissions"],
        own_args: NONE,
        bypass_env: &[],
        model_args: MODEL,
        effort_args: EFFORT,
        resume_by_id: &["--resume", "{id}"],
        resume_latest: &["--continue"],
        hooks: Hooks::ClaudeSettings,
        launch: Launch::Direct,
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
    },
    AgentSpec {
        id: "antigravity",
        kind: Kind::Antigravity,
        commands: &["agy", "antigravity"],
        detect: &["agy", "antigravity"],
        label: "Antigravity",
        install_url: "https://antigravity.google/docs/cli-overview",
        bypass_args: NONE,
        own_args: NONE,
        bypass_env: &[],
        model_args: MODEL,
        effort_args: EFFORT,
        resume_by_id: &["--conversation", "{id}"],
        // Without an id the task has not started a conversation; --continue could open another task's.
        resume_latest: NONE,
        hooks: Hooks::AntigravityHooksJson,
        launch: Launch::Direct,
    },
    AgentSpec {
        id: "stepfun",
        kind: Kind::StepFun,
        commands: &["stepfun"],
        // Shifu's own Pi runtime under node; clis.rs also checks the runtime is there.
        detect: &["node"],
        label: "StepFun",
        install_url: "https://github.com/Nalu-Lab/shifu/blob/main/docs/STEPFUN.md",
        bypass_args: NONE,
        own_args: NONE,
        bypass_env: &[],
        model_args: MODEL,
        effort_args: NONE,
        resume_by_id: &["--session", "{id}"],
        resume_latest: NONE,
        hooks: Hooks::PiExtension,
        launch: Launch::Shifu,
    },
    AgentSpec {
        id: "qwen",
        kind: Kind::Qwen,
        commands: &["qwen", "qwen-code"],
        detect: &["qwen"],
        label: "Qwen Code",
        install_url: "https://github.com/QwenLM/qwen-code",
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
    },
    AgentSpec {
        id: "opencode",
        kind: Kind::OpenCode,
        commands: &["opencode"],
        detect: &["opencode"],
        label: "opencode",
        install_url: "https://opencode.ai/docs/",
        bypass_args: &["--auto"],
        own_args: NONE,
        bypass_env: &[],
        model_args: &["-m", "{model}"],
        effort_args: NONE,
        resume_by_id: &["--session", "{id}"],
        resume_latest: &["--continue"],
        hooks: Hooks::OpencodePlugin,
        launch: Launch::Direct,
    },
    AgentSpec {
        id: "pi",
        kind: Kind::Pi,
        commands: &["pi"],
        detect: &["pi"],
        label: "Pi",
        install_url: "https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent",
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
    },
    AgentSpec {
        id: "gemini",
        kind: Kind::Other,
        commands: &["gemini"],
        detect: &["gemini"],
        label: "Gemini CLI",
        install_url: "https://geminicli.com/docs/get-started/installation/",
        bypass_args: NONE,
        own_args: NONE,
        bypass_env: &[],
        model_args: NONE,
        effort_args: NONE,
        resume_by_id: NONE,
        resume_latest: NONE,
        hooks: Hooks::None,
        launch: Launch::Direct,
    },
];

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
        }
    }
}
