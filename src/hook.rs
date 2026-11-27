//! `shifu hook <kind> <事件>`：agent 的 hook 回调。永远 exit 0，不写 stderr。
//! 默认 stdout 静默；三家新会话的首次启动可收到一次指南入口。

use crate::agent::{self, Agent, Res, now_secs};
use crate::status::{Status, asks_user, next_status};
use crate::tmux::{find_tmux_bin, run_command};
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

const LAST_MESSAGE_CHARS: usize = 300;
const WAITING_FOR_CHARS: usize = 200;

/// hook 送来的 JSON 里用得到的字段。
#[derive(Debug, Default, PartialEq)]
pub struct HookInput {
    pub tool_name: Option<String>,
    pub session_id: Option<String>,
    /// Why a session starts: Claude says `startup`, `resume`, `clear` or `compact`.
    pub source: Option<String>,
    pub last_message: Option<String>,
    pub prompt: Option<String>,
    /// What the tool call is about: the question it asks the user, else its command or the file it touches.
    pub tool_detail: Option<String>,
    /// The event came from a sub-agent's tool (Claude marks those with `agent_id`), not from the agent itself.
    /// Kimi Code sends `agent_id: "main"` for the agent itself.
    pub from_sub_agent: bool,
    /// That sub-agent's `agent_id`, to tell whose approval a later event answers.
    pub sub_agent: Option<String>,
    /// What the tool call touches, in full (command, path, URL or question): tells two calls of one tool apart.
    pub call: Option<String>,
    /// Antigravity can emit Stop while its background tasks are still running.
    pub background_work: bool,
    /// The folder the thread works in.
    pub cwd: Option<String>,
}

pub fn parse_input(raw: &str) -> HookInput {
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return HookInput::default();
    };
    let field = |key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
    HookInput {
        tool_name: field("tool_name").or_else(|| v["toolCall"]["name"].as_str().map(str::to_string)),
        session_id: field("session_id").or_else(|| field("conversationId")),
        source: field("source"),
        last_message: field("last_assistant_message"),
        prompt: field("prompt"),
        tool_detail: tool_detail(&v["tool_input"]),
        from_sub_agent: field("agent_id").is_some_and(|id| id != "main"),
        sub_agent: field("agent_id").filter(|id| id != "main"),
        call: call_detail(&v["tool_input"]),
        background_work: v.get("fullyIdle").and_then(Value::as_bool) == Some(false),
        cwd: field("cwd"),
    }
}

/// Codex runs background threads of its own (memory consolidation in `~/.codex/memories`) in the same process, so
/// their hooks carry the conversation's SHIFU_ID. A Codex thread keeps the folder it started in, so another folder
/// means another thread; its events must not replace the conversation's session or mark it exited.
pub fn foreign_thread(kind: &str, agent_cwd: &str, input: &HookInput) -> bool {
    let Some(cwd) = input.cwd.as_deref() else { return false };
    let same = |a: &str, b: &str| a == b || matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y);
    kind == "codex" && !agent_cwd.is_empty() && !same(cwd, agent_cwd)
}

/// Kinds whose hooks only ever touch a record of their own kind.
const OWN_RECORDS_ONLY: [&str; 5] = ["antigravity", "stepfun", "qwen", "opencode", "pi"];

/// An agy, StepFun, Qwen, opencode or Pi process started by another kind's agent (from its shell tool) inherits that
/// agent's SHIFU_ID; its events must not overwrite the parent's conversation ID or status. Qwen also ignores its
/// sub-agents.
pub fn foreign_caller(kind: &str, agent: &Agent, input: &HookInput) -> bool {
    (OWN_RECORDS_ONLY.contains(&kind) && agent.kind.as_str() != kind) || (kind == "qwen" && input.from_sub_agent)
}

/// What a tool call touches, in full: its command, the file, URL or pattern, else the question it asks.
fn call_detail(input: &Value) -> Option<String> {
    let text = if let Some(words) = input["command"].as_array() {
        words.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")
    } else {
        ["command", "file_path", "notebook_path", "path", "url", "pattern", "query", "question"]
            .iter()
            .find_map(|key| input[*key].as_str())
            .or_else(|| input["questions"][0]["question"].as_str())?
            .trim()
            .to_string()
    };
    (!text.is_empty()).then_some(text)
}

/// The pending approval, as `Agent::approval` keeps it: who asked (a sub-agent's id, empty for the agent itself),
/// the tool, and what the call touches.
pub fn approval_key(input: &HookInput) -> String {
    let part = |value: Option<&str>| value.unwrap_or_default().replace('\u{1f}', " ");
    [part(input.sub_agent.as_deref()), part(input.tool_name.as_deref()), part(input.call.as_deref())].join("\u{1f}")
}

/// How an event from the agent or one of its sub-agents relates to the approval it waits for (`key`).
#[derive(Debug, PartialEq)]
enum Answer {
    /// That call went ahead (or was answered): the approval is over.
    Answered,
    /// Whoever asked started another call: the approval was settled without a result, e.g. the approved command failed.
    MovedOn,
    /// Someone else's call, or a parallel one: the approval still waits.
    Unrelated,
}

fn answers(key: &str, event: &str, input: &HookInput) -> Answer {
    let mut parts = key.splitn(3, '\u{1f}');
    let (owner, tool, call) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
    let from = input.sub_agent.as_deref().unwrap_or_default();
    let same_call = input.tool_name.as_deref().unwrap_or_default() == tool
        && (call.is_empty() || input.call.as_deref().is_none_or(|c| c == call));
    if from != owner {
        // The agent getting a sub-agent's (Task/Agent tool) result back means that sub-agent has finished.
        let sub_agent_done = !owner.is_empty() && from.is_empty() && event == "PostToolUse"
            && matches!(input.tool_name.as_deref(), Some("Task" | "Agent"));
        return if sub_agent_done { Answer::Answered } else { Answer::Unrelated };
    }
    match event {
        // Kimi and opencode say so outright, without naming the call.
        "PermissionResult" => Answer::Answered,
        "PostToolUse" if same_call => Answer::Answered,
        "PreToolUse" if !same_call => Answer::MovedOn,
        _ => Answer::Unrelated,
    }
}

fn tool_detail(input: &Value) -> Option<String> {
    let questions = input["questions"].as_array().map_or(0, Vec::len);
    let text = if let Some(question) = input["questions"][0]["question"].as_str().or_else(|| input["question"].as_str()) {
        if questions > 1 { format!("共 {questions} 问：{}", question.trim()) } else { question.trim().to_string() }
    } else if let Some(command) = input["command"].as_str() {
        command.trim().lines().next().unwrap_or_default().trim().to_string()
    } else if let Some(words) = input["command"].as_array() {
        words.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")
    } else if let Some(url) = input["url"].as_str() {
        url.trim().to_string()
    } else {
        let path = input["file_path"].as_str()?;
        std::path::Path::new(path).file_name()?.to_str()?.to_string()
    };
    (!text.is_empty()).then(|| text.chars().take(WAITING_FOR_CHARS).collect())
}

/// A line that points at something instead of saying something: a Codex directive (`::codex-file-citation{…}`), a
/// lone link, URL or path, or the label above such a list ("Sources:"). Replies often end on these; what the agent
/// wants is said before them.
fn points_elsewhere(line: &str) -> bool {
    let lone = !line.contains(char::is_whitespace);
    let directive = line.starts_with(':') && line.ends_with('}') && line.trim_start_matches(':').starts_with(|c: char| c.is_ascii_alphabetic());
    let link = line.starts_with('[') && line.ends_with(')') && line.matches("](").count() == 1;
    let address = lone && ["http://", "https://", "/", "~/"].iter().any(|start| line.starts_with(start));
    directive || link || address || line.ends_with([':', '：'])
}

/// The reply's last line that says something, without its Markdown marks: where an agent says what it wants next.
/// Commands and output inside code fences are not it. A long line keeps its end, since that is where the question is.
fn closing_line(message: &str) -> Option<String> {
    let mut fenced = false;
    let prose: Vec<&str> = message
        .lines()
        .filter(|line| {
            let fence = line.trim_start().starts_with("```");
            fenced ^= fence;
            !fence && !fenced
        })
        .collect();
    prose.into_iter().rev().find_map(|line| {
        let line = line.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '>' | '#' | '-' | '*' | '+'));
        let numbered = line.trim_start_matches(|c: char| c.is_ascii_digit());
        let line = match numbered.strip_prefix(['.', ')', '、']) {
            Some(rest) if numbered.len() < line.len() && (rest.starts_with(char::is_whitespace) || numbered.starts_with('、')) => rest,
            _ => line,
        };
        let plain = line.replace("**", "").replace("__", "").replace('`', "");
        let plain = plain.trim();
        if plain.is_empty() || points_elsewhere(plain) {
            return None;
        }
        let chars: Vec<char> = plain.chars().collect();
        Some(chars[chars.len().saturating_sub(WAITING_FOR_CHARS)..].iter().collect())
    })
}

/// 事件作用到 agent 上，返回新值。状态变了才更新 `status_since`。
pub fn apply_event(agent: &Agent, event: &str, input: &HookInput, now: u64) -> Agent {
    // A sub-agent working in the background says nothing about whether the agent itself waits for you.
    // Kimi Code's /new ends the old session after the new one started: that end is not the agent's.
    let old_session_ended = event == "SessionEnd"
        && matches!((&input.session_id, &agent.session_id), (Some(ended), Some(current)) if ended != current);
    if old_session_ended {
        return agent.clone();
    }
    let tool = input.tool_name.as_deref();
    // The approval this record waits for, when the hook that asked said which call it was (older records did not).
    let pending = agent.approval.as_deref().filter(|_| agent.status == Status::NeedsApproval);
    let status = if event == "PermissionRequest" {
        // Asked by the agent or by one of its sub-agents, the prompt blocks on the user all the same. Claude's
        // question tool also goes through a permission prompt; it stays a question.
        if asks_user(tool) { Status::NeedsReply } else { Status::NeedsApproval }
    } else if let Some(key) = pending
        && matches!(event, "PreToolUse" | "PostToolUse" | "PermissionResult")
    {
        match answers(key, event, input) {
            Answer::Answered => Status::Running,
            Answer::MovedOn => next_status(Status::Running, event, tool),
            Answer::Unrelated => Status::NeedsApproval,
        }
    } else if input.from_sub_agent {
        agent.status
    } else if event == "Stop" && input.background_work {
        Status::Running
    } else {
        next_status(agent.status, event, tool)
    };
    let mut next = agent.clone();
    if status != agent.status {
        next.status = status;
        next.status_since = now;
    }
    next.approval = if status != Status::NeedsApproval {
        None
    } else if event == "PermissionRequest" {
        Some(approval_key(input))
    } else {
        agent.approval.clone()
    };
    // A sub-agent's events never name the conversation (a Codex sub-agent runs as a thread of its own).
    if input.session_id.is_some() && !input.from_sub_agent {
        next.session_id = input.session_id.clone();
    }
    if event == "Stop"
        && let Some(msg) = &input.last_message
    {
        next.last_message = Some(msg.chars().take(LAST_MESSAGE_CHARS).collect());
    }
    next.waiting_for = if !status.is_waiting() {
        None
    } else if asks_user(tool) && matches!(event, "PreToolUse" | "PermissionRequest") {
        // An explicit question tool can itself quote the question. Preserve its
        // request context when only this one-line detail reaches the sidebar.
        input.tool_detail.as_ref().map(|detail| {
            if detail.starts_with(['“', '「', '『', '‘', '"'])
                || (detail.starts_with("共 ") && detail.split_once('：').is_some_and(|(_, question)| question.starts_with(['“', '「', '『', '‘', '"'])))
            {
                format!("请回答{detail}")
            } else {
                detail.clone()
            }
        })
    } else if event == "PermissionRequest" {
        match (tool, &input.tool_detail) {
            (Some(tool), Some(detail)) => Some(format!("{tool}：{detail}")),
            (tool, detail) => tool.map(str::to_string).or_else(|| detail.clone()),
        }
    } else if event == "Stop" && !input.from_sub_agent {
        input.last_message.as_deref().and_then(closing_line)
    } else if status != agent.status {
        None
    } else {
        agent.waiting_for.clone()
    };
    next
}

pub fn run(kind: &str, event: &str) {
    // 错误只记本地日志；不能污染 CLI 的 hook 协议输出。
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = std::panic::catch_unwind(|| handle(kind, event));
    let error = match outcome {
        Ok(Ok(())) => return,
        Ok(Err(e)) => e,
        Err(_) => "hook 内部 panic".to_string(),
    };
    log_error(event, &error);
}

fn handle(kind: &str, event: &str) -> Res<()> {
    let Some(name) = std::env::var("SHIFU_ID").ok().filter(|n| !n.is_empty()) else {
        return Ok(());
    };
    if !agent::is_safe_name(&name) {
        return Err(format!("SHIFU_ID 不是合法名字: {name:?}"));
    }
    let mut raw = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut raw) {
        log_error(event, &format!("读 stdin 失败: {e}"));
    }
    let input = parse_input(&raw);
    let lock = agent::lock()?;
    let Some(agent) = agent::load(&name)? else {
        return Ok(());
    };
    if foreign_caller(kind, &agent, &input) { return Ok(()); }
    if foreign_thread(kind, &agent.cwd, &input) { return Ok(()); }
    let updated = apply_event(&agent, event, &input, now_secs());
    agent::save(&updated)?;
    drop(lock);
    if event == "UserPromptSubmit" && !input.from_sub_agent {
        if let Some(prompt) = input.prompt.as_deref() {
            crate::inbox::acknowledge(&agent::shifu_home(), &updated, prompt)?;
        }
    }
    if updated.status != agent.status {
        announce_status(updated.status)?;
    }
    if let Some(context) = guide_context(kind, event, &name, &agent, &input)? {
        writeln!(std::io::stdout().lock(), "{context}").map_err(|e| format!("写指南上下文失败: {e}"))?;
    }
    Ok(())
}

fn guide_context(kind: &str, event: &str, name: &str, registered: &Agent, input: &HookInput) -> Res<Option<String>> {
    if !matches!(kind, "claude" | "codex" | "kimi")
        || kind != registered.kind.as_str()
        || input.from_sub_agent
    {
        return Ok(None);
    }
    let Some(session) = input.session_id.as_deref().filter(|s| !s.trim().is_empty()) else {
        return Ok(None);
    };
    let home = agent::shifu_home();
    // A brand-new conversation gets the computer guide and its task. A started one gets only its task again after
    // /clear, compaction or resume, which drop what it was told; any other start under its name (a CLI run inside
    // the conversation) gets nothing.
    let fresh = registered.session_id.is_none();
    let restarted = matches!(input.source.as_deref(), Some("clear" | "compact" | "resume"));
    if event == "SessionStart" && !fresh && !(restarted && kind != "kimi") {
        return Ok(None);
    }
    if event != "SessionStart" && registered.session_id.as_deref() != Some(session) {
        return Ok(None);
    }
    let pending = home.join("computer-guide-pending").join(name);
    if kind == "kimi" {
        // Kimi ignores SessionStart output; its first prompt hook accepts plain context.
        if event == "SessionStart" {
            std::fs::create_dir_all(pending.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&pending, session).map_err(|e| e.to_string())?;
            return Ok(None);
        }
        if event != "UserPromptSubmit" { return Ok(None); }
        match std::fs::read_to_string(&pending) {
            Ok(expected) if expected == session => std::fs::remove_file(&pending).map_err(|e| e.to_string())?,
            Ok(_) => return Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        }
    } else if event != "SessionStart" {
        return Ok(None);
    }
    let task = task_note(&home, name).unwrap_or_else(|e| {
        log_error(event, &e);
        None
    });
    let guide = if fresh || kind == "kimi" { Some(computer_note()?) } else { None };
    let context = match (guide, task) {
        (Some(g), Some(t)) => format!("{g}\n\n{t}"),
        (Some(only), None) | (None, Some(only)) => only,
        (None, None) => return Ok(None),
    };
    let context = format!("{}\n\n{context}", talk_note()?);
    if kind == "kimi" { return Ok(Some(context)); }
    Ok(Some(serde_json::json!({"hookSpecificOutput": {
        "hookEventName":"SessionStart", "additionalContext":context
    }}).to_string()))
}

fn computer_note() -> Res<String> {
    // `shifu` is spelled out once, at the start of the context (`talk_note`).
    let (command, browser, find) = ("shifu computer guide", "shifu browser guide", "shifu find");
    Ok(format!(
        "优先用已有 API/CLI；网页先读 {browser}，本机窗口先读 {command}；用户提到“我发的、之前那份、微信里的”文件时先用 {find} 查候选。Shifu 自带后台截图、点击、输入和滚动，不依赖 Orca；普通后台保持鼠标位置，应用仍可能改变焦点或弹窗。--background-only 仅允许读取和文本框赋值，不得去掉限制重试。截图路径须打开查看；不扩大前台接管、发送或交易授权。"
    ))
}

/// How to reach the other conversations. Without it agents opened them by clicking through the window.
fn talk_note() -> Res<String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let shifu = crate::launch::sh_quote(executable.to_str().ok_or("Shifu 程序路径不是 UTF-8")?);
    Ok(format!(
        "下面的 shifu 指 {shifu}。找 Shifu 里别的对话用命令，别截图点开：shifu status 看有谁；shifu read <对话> 看它最近输出；shifu ask <对话> <问题> 问并等回复；shifu tell <对话> <话> 只通知。收到带「编号 ask-…」的提问，答完用 shifu reply <编号> <答复>。"
    ))
}

#[derive(serde::Deserialize)]
struct TaskList {
    #[serde(default)]
    tasks: Vec<TaskEntry>,
}

#[derive(serde::Deserialize)]
struct TaskEntry {
    id: String,
    name: String,
    #[serde(default)]
    project: String,
    #[serde(default)]
    agents: Vec<String>,
}

/// The task this conversation belongs to (the app's `tasks.json`) and what to do with the task's goal file
/// (`goals/<id>.md`, the app's `taskgoals.rs`). Told at every start, so a conversation never works without it.
fn task_note(home: &std::path::Path, name: &str) -> Res<Option<String>> {
    let Some(text) = crate::app_session::read_optional_text(&home.join("tasks.json"))
        .map_err(|e| format!("读 tasks.json 失败: {e}"))? else { return Ok(None) };
    let list: TaskList = serde_json::from_str(&text).map_err(|e| format!("tasks.json 解析失败: {e}"))?;
    let Some(task) = list.tasks.into_iter().find(|t| t.agents.iter().any(|a| a == name)) else {
        return Ok(None);
    };
    // Resident roles take their purpose from the instructions in their own folder.
    let project = std::path::Path::new(&task.project);
    if project == home.join("pm") || project == home.join("daily") || project.parent() == Some(home.join("bots").as_path()) {
        return Ok(None);
    }
    let file = home.join("goals").join(format!("{}.md", task.id));
    let (task_name, path) = (task.name, file.display());
    let goal = match crate::app_session::read_optional_text(&file) {
        Ok(text) => text.unwrap_or_default(),
        Err(e) => return Ok(Some(format!(
            "你在 Shifu 里属于任务「{task_name}」，你的名字是 `{name}`。任务文件 {path} 暂时无法读取（{e}）。保留原文件，继续处理用户当前交代的工作。"
        ))),
    };
    Ok(Some(if written(&goal) {
        format!(
            "你在 Shifu 里属于任务「{task_name}」，你的名字是 `{name}`。这个任务的终点、重点和清单在 {path}，动手前先读。做完清单里的一步，把那一行的 `- [ ]` 改成 `- [x]`；行末标了别的 `@名字` 的不要做，没标的等我交给你再做。别的对话需要知道的发现和决定，在「# 记录」下面追加一行 `- 日期 内容`。要改终点或重点，先跟我说。用户交代的工作和这个终点对不上时，照用户说的做，不按文件推进也不改它，并告诉我一声。"
        )
    } else {
        format!(
            "你在 Shifu 里属于任务「{task_name}」，你的名字是 `{name}`。这个任务的终点、重点和清单还没写，文件是 {path}（可能还不存在）。等你从对话里弄清这个任务要做成什么样，就起草它：「# 终点」一两句写做完是什么样子，「# 重点」写现在最该推的一件事，「# 清单」每步一行 `- [ ] `，「# 记录」先留空。起草前再读一次这个文件，别的对话已经写了就照它做，不要覆盖。写完告诉我，我来确认；不要为了起草打断我交代的事。如果别的对话已经写了，而你手上的工作和它对不上，照用户说的做，不改它，告诉我一声。"
        )
    }))
}

/// Same rule as the app's `taskgoals::written`: a goal file counts once it says more than its headings.
fn written(text: &str) -> bool {
    text.lines().map(str::trim).any(|l| !l.is_empty() && !l.starts_with('#'))
}

/// 状态真的变了才写窗格选项 `@shifu_status`，标签栏靠它画圆点。
/// 用环境里 `TMUX` 指的那个服务（`-S <socket 路径>`）；没有 TMUX 或 TMUX_PANE 就跳过。
/// 这是 hook 唯一会调的 tmux 命令。
fn announce_status(status: Status) -> Res<()> {
    let var = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    let (Some(tmux_var), Some(pane)) = (var("TMUX"), var("TMUX_PANE")) else {
        return Ok(());
    };
    let socket = tmux_var.split(',').next().unwrap_or_default();
    let mut cmd = Command::new(find_tmux_bin());
    cmd.args(["-S", socket, "set-option", "-p", "-t", &pane, "@shifu_status", status.name()]).stdin(Stdio::null());
    run_command(cmd).map(|_| ())
}

fn log_error(event: &str, message: &str) {
    let path = agent::shifu_home().join("hook-errors.log");
    let _ = std::fs::create_dir_all(agent::shifu_home());
    // 写日志本身失败时没有别的地方可报，只能放弃
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {event}: {}", now_secs(), message.replace('\n', " "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Kind;
    use crate::status::Status;

    fn agent(status: Status) -> Agent {
        Agent {
            name: "claude-p".to_string(),
            kind: Kind::Claude,
            command: "claude".to_string(),
            cwd: "/tmp".to_string(),
            created_at: 100,
            status,
            status_since: 100,
            window_id: Some("@1".to_string()),
            pane_id: Some("%1".to_string()),
            session_id: None,
            last_message: None,
            waiting_for: None, approval: None,
            model: None,
            effort: None,
            label: None,
        }
    }

    #[test]
    fn opencode_and_pi_hooks_touch_only_their_own_records() {
        let none = HookInput::default();
        let mut a = agent(Status::Running);
        for kind in ["opencode", "pi", "antigravity", "stepfun", "qwen"] {
            assert!(foreign_caller(kind, &a, &none), "{kind} must not touch a Claude record");
        }
        assert!(!foreign_caller("claude", &a, &none));
        a.kind = Kind::OpenCode;
        assert!(!foreign_caller("opencode", &a, &none));
        assert!(foreign_caller("pi", &a, &none));
        a.kind = Kind::Pi;
        assert!(!foreign_caller("pi", &a, &none));
        assert!(foreign_caller("opencode", &a, &none));
        a.kind = Kind::Qwen;
        assert!(!foreign_caller("qwen", &a, &none));
        assert!(foreign_caller("qwen", &a, &HookInput { from_sub_agent: true, ..HookInput::default() }));
    }

    /// What Shifu's opencode plugin and Pi extension send (integrations/*/shifu-status.*), through the usual rules.
    #[test]
    fn opencode_plugin_events_walk_start_run_done_ask_and_approval() {
        let mut a = agent(Status::Starting);
        a.kind = Kind::OpenCode;
        let step = |a: &Agent, event: &str, raw: &str| apply_event(a, event, &parse_input(raw), 200);
        let a1 = step(&a, "SessionStart", "{}");
        assert_eq!(a1.status, Status::Idle);
        let a2 = step(&a1, "SessionStart", r#"{"session_id":"ses_1","source":"startup"}"#);
        assert_eq!((a2.status, a2.session_id.as_deref()), (Status::Idle, Some("ses_1")));
        let a3 = step(&a2, "UserPromptSubmit", r#"{"session_id":"ses_1"}"#);
        assert_eq!(a3.status, Status::Running);
        let a4 = step(&a3, "PermissionRequest", r#"{"session_id":"ses_1","tool_name":"bash","tool_input":{"command":"echo shifu-mock"}}"#);
        assert_eq!((a4.status, a4.waiting_for.as_deref()), (Status::NeedsApproval, Some("bash：echo shifu-mock")));
        let a5 = step(&a4, "PermissionResult", r#"{"session_id":"ses_1","tool_name":"permission"}"#);
        assert_eq!(a5.status, Status::Running);
        let a6 = step(&a5, "Stop", r#"{"session_id":"ses_1","last_assistant_message":"I can do either.\n\nWhich option do you prefer?"}"#);
        assert_eq!(a6.status, Status::NeedsReply);
        assert_eq!(a6.waiting_for.as_deref(), Some("Which option do you prefer?"));
        let q = step(&a3, "PreToolUse", r#"{"session_id":"ses_1","tool_name":"ask_question","tool_input":{"questions":[{"question":"Which file?"}]}}"#);
        assert_eq!((q.status, q.waiting_for.as_deref()), (Status::NeedsReply, Some("Which file?")));
        assert_eq!(step(&q, "PostToolUse", r#"{"tool_name":"ask_question"}"#).status, Status::Running);
        assert_eq!(step(&a3, "Interrupt", r#"{"session_id":"ses_1"}"#).status, Status::NeedsReply);
    }

    #[test]
    fn pi_extension_reports_the_session_file_as_the_id() {
        let mut a = agent(Status::Starting);
        a.kind = Kind::Pi;
        let file = "/Users/x/.pi/agent/sessions/--w--/2026-11-23T10-00-00_abc.jsonl";
        let started = apply_event(&a, "SessionStart", &parse_input(&format!(r#"{{"session_id":"{file}"}}"#)), 1);
        assert_eq!((started.status, started.session_id.as_deref()), (Status::Idle, Some(file)));
        let done = apply_event(&started, "Stop", &parse_input(&format!(r#"{{"session_id":"{file}","last_assistant_message":"Done."}}"#)), 2);
        assert_eq!((done.status, done.last_message.as_deref()), (Status::NeedsReply, Some("Done.")));
    }

    #[test]
    fn parse_input_reads_fields() {
        let i = parse_input(r#"{"tool_name":"Bash","session_id":"abc","last_assistant_message":"好了","x":1}"#);
        assert_eq!(i.tool_name.as_deref(), Some("Bash"));
        assert_eq!(i.session_id.as_deref(), Some("abc"));
        assert_eq!(i.last_message.as_deref(), Some("好了"));
        assert_eq!(parse_input(""), HookInput::default());
        assert_eq!(parse_input("not json"), HookInput::default());
        assert_eq!(parse_input(r#"{"tool_name":5}"#), HookInput::default());
    }

    #[test]
    fn since_changes_only_with_status() {
        let a = agent(Status::Running);
        let same = apply_event(&a, "PreToolUse", &HookInput::default(), 500);
        assert_eq!(same.status_since, 100);
        let changed = apply_event(&a, "Stop", &HookInput::default(), 500);
        assert_eq!(changed.status, Status::NeedsReply);
        assert_eq!(changed.status_since, 500);
        assert_eq!(a.status_since, 100, "原值不被改");
    }

    #[test]
    fn stop_records_last_message_truncated_to_300_chars() {
        let long: String = "字".repeat(400);
        let input = HookInput { last_message: Some(long), ..Default::default() };
        let out = apply_event(&agent(Status::Running), "Stop", &input, 5);
        assert_eq!(out.last_message.as_ref().unwrap().chars().count(), 300);
        let other = apply_event(&agent(Status::Running), "UserPromptSubmit", &input, 5);
        assert_eq!(other.last_message, None);
    }

    #[test]
    fn a_waiting_agent_records_what_it_waits_for() {
        let asks = parse_input(r#"{"tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"用哪个数据库？"},{"question":"要不要迁移？"}]}}"#);
        let asked = apply_event(&agent(Status::Running), "PreToolUse", &asks, 5);
        assert_eq!(asked.waiting_for.as_deref(), Some("共 2 问：用哪个数据库？"));
        let answered = apply_event(&asked, "PostToolUse", &asks, 6);
        assert_eq!((answered.status, answered.waiting_for), (Status::Running, None));

        let shell = parse_input(r#"{"tool_name":"Bash","tool_input":{"command":"npm publish \\\n  --tag next"}}"#);
        let approval = apply_event(&agent(Status::Running), "PermissionRequest", &shell, 5);
        assert_eq!(approval.waiting_for.as_deref(), Some("Bash：npm publish \\"));
        let edit = parse_input(r#"{"tool_name":"Edit","tool_input":{"file_path":"/repo/app/config.toml"}}"#);
        assert_eq!(apply_event(&agent(Status::Running), "PermissionRequest", &edit, 5).waiting_for.as_deref(), Some("Edit：config.toml"));
        let codex = parse_input(r#"{"tool_name":"shell","tool_input":{"command":["bash","-lc","git push"]}}"#);
        assert_eq!(apply_event(&agent(Status::Running), "PermissionRequest", &codex, 5).waiting_for.as_deref(), Some("shell：bash -lc git push"));
        let bare = parse_input(r#"{"tool_name":"Bash"}"#);
        assert_eq!(apply_event(&agent(Status::Running), "PermissionRequest", &bare, 5).waiting_for.as_deref(), Some("Bash"));
        assert_eq!(apply_event(&approval, "PostToolUse", &shell, 6).waiting_for, None);
    }

    #[test]
    fn quoted_examples_and_explicit_question_tools_keep_their_context() {
        let reply = format!("{}\n可以这样问管理员：\n“设备 A 的使用编号是多少？”", "说明。".repeat(200));
        let input = HookInput { last_message: Some(reply), ..HookInput::default() };
        let stopped = apply_event(&agent(Status::Running), "Stop", &input, 5);
        assert_eq!(stopped.status, Status::NeedsReply);
        assert_eq!(stopped.waiting_for.as_deref(), Some("“设备 A 的使用编号是多少？”"));
        let input = HookInput { tool_name: Some("AskUserQuestion".into()), tool_detail: Some("“设备 A 的编号是多少？”".into()), ..HookInput::default() };
        let asked = apply_event(&agent(Status::Running), "PreToolUse", &input, 5);
        assert_eq!(asked.status, Status::NeedsReply);
        assert_eq!(asked.waiting_for.as_deref(), Some("请回答“设备 A 的编号是多少？”"));
        let input = HookInput { tool_detail: Some("共 2 问：“编号是多少？”".into()), ..input };
        let asked = apply_event(&agent(Status::Running), "PreToolUse", &input, 5);
        assert_eq!(asked.waiting_for.as_deref(), Some("请回答共 2 问：“编号是多少？”"));
    }

    #[test]
    fn a_finished_turn_keeps_its_closing_line_even_when_the_reply_is_long() {
        let reply = format!("{}\n\n- 改好了三处\n\n**要我现在合并吗？**\n", "前文。".repeat(200));
        let input = HookInput { last_message: Some(reply), ..Default::default() };
        let stopped = apply_event(&agent(Status::Running), "Stop", &input, 5);
        assert_eq!(stopped.waiting_for.as_deref(), Some("要我现在合并吗？"));
        let list = HookInput { last_message: Some("做完了：\n1. 第一步\n> - `cargo test` 通过".into()), ..Default::default() };
        assert_eq!(apply_event(&agent(Status::Running), "Stop", &list, 5).waiting_for.as_deref(), Some("cargo test 通过"));
        let long = HookInput { last_message: Some(format!("{}结尾？", "长".repeat(400))), ..Default::default() };
        let kept = apply_event(&agent(Status::Running), "Stop", &long, 5).waiting_for.unwrap();
        assert!(kept.chars().count() == 200 && kept.ends_with("结尾？"), "{kept}");
        let fenced = HookInput { last_message: Some("装好了，重启后生效。\n\n要看的话跑这条：\n```sh\nopen -a Preview '/tmp/a.pdf'\n```\n---\n".into()), ..Default::default() };
        assert_eq!(apply_event(&agent(Status::Running), "Stop", &fenced, 5).waiting_for.as_deref(), Some("装好了，重启后生效。"));
        // Replies often end on citations, sources or file paths; the closing line is the sentence before them.
        let closing = |reply: &str| {
            let input = HookInput { last_message: Some(reply.into()), ..Default::default() };
            apply_event(&agent(Status::Running), "Stop", &input, 5).waiting_for
        };
        assert_eq!(
            closing("表格已更新，当前 4/5 位。\n\n::codex-file-citation{path=\"/Users/example/out/进度.xlsx\" artifact_kind=\"workbook\"}").as_deref(),
            Some("表格已更新，当前 4/5 位。")
        );
        assert_eq!(
            closing("数据都核过了，要继续写第二节吗？\n\nSources:\n- [月活（某站）](https://example.com/a)\n- https://example.com/b").as_deref(),
            Some("数据都核过了，要继续写第二节吗？")
        );
        assert_eq!(closing("对照写好了，先看结论一节。\n\n文件在这里：\n/Users/example/项目/对照与借鉴.md\n~/notes/b.md").as_deref(), Some("对照写好了，先看结论一节。"));
        assert_eq!(closing("见 [设计稿](https://example.com/d) 和 /tmp/a.png 两处").as_deref(), Some("见 [设计稿](https://example.com/d) 和 /tmp/a.png 两处"));
        assert_eq!(closing("来源：\n- https://example.com/a"), None);
        let version = HookInput { last_message: Some("3.2 版已发布".into()), ..Default::default() };
        assert_eq!(apply_event(&agent(Status::Running), "Stop", &version, 5).waiting_for.as_deref(), Some("3.2 版已发布"));
        // The next prompt, a sub-agent's events and background work leave nothing stale behind.
        assert_eq!(apply_event(&stopped, "UserPromptSubmit", &HookInput::default(), 6).waiting_for, None);
        let sub = parse_input(r#"{"tool_name":"Bash","agent_id":"a80d08fb6413ebdaf"}"#);
        assert_eq!(apply_event(&stopped, "PostToolUse", &sub, 6).waiting_for.as_deref(), Some("要我现在合并吗？"));
        let busy = HookInput { last_message: Some("还在跑".into()), background_work: true, ..Default::default() };
        assert_eq!(apply_event(&agent(Status::Running), "Stop", &busy, 5).waiting_for, None);
        assert_eq!(apply_event(&agent(Status::Running), "Stop", &HookInput::default(), 5).waiting_for, None);
    }

    #[test]
    fn a_sub_agents_tools_do_not_wake_a_session_that_waits() {
        let sub = parse_input(r#"{"tool_name":"Bash","agent_id":"a80d08fb6413ebdaf","agent_type":"general-purpose"}"#);
        assert_eq!(apply_event(&agent(Status::NeedsReply), "PostToolUse", &sub, 5).status, Status::NeedsReply);
        let own = parse_input(r#"{"tool_name":"Bash"}"#);
        assert_eq!(apply_event(&agent(Status::NeedsReply), "PostToolUse", &own, 5).status, Status::Running);
        // Kimi Code names its own agent "main" when it stops to ask (11-09: Shifu kept showing running).
        let kimi_asks = parse_input(r#"{"hook_event_name":"PermissionRequest","tool_name":"Bash","agent_id":"main"}"#);
        assert_eq!(apply_event(&agent(Status::Running), "PermissionRequest", &kimi_asks, 5).status, Status::NeedsApproval);
    }

    #[test]
    fn an_old_session_ending_after_a_new_one_started_is_ignored() {
        // Kimi Code's /new: SessionStart for the new session, then SessionEnd for the old one (11-09).
        let started = apply_event(&agent(Status::Idle), "SessionStart", &HookInput { session_id: Some("new".into()), ..Default::default() }, 5);
        let old_end = HookInput { session_id: Some("old".into()), ..Default::default() };
        let after = apply_event(&started, "SessionEnd", &old_end, 6);
        assert_eq!((after.status, after.session_id.as_deref()), (Status::Idle, Some("new")));
        let own_end = HookInput { session_id: Some("new".into()), ..Default::default() };
        assert_eq!(apply_event(&started, "SessionEnd", &own_end, 7).status, Status::Exited);
        assert_eq!(apply_event(&agent(Status::Idle), "SessionEnd", &old_end, 7).status, Status::Exited);
    }

    #[test]
    fn activity_recovers_a_false_exit_but_a_dead_pane_still_shows_exited() {
        use crate::agent::PaneInfo;
        use std::collections::HashMap;
        let mut original = agent(Status::Running);
        original.session_id = Some("same-conversation".into());
        original.label = Some("双人 IM".into());
        let ended = apply_event(&original, "SessionEnd", &HookInput::default(), 200);
        let own = parse_input(r#"{"tool_name":"exec_command","session_id":"same-conversation"}"#);
        let active = apply_event(&ended, "PostToolUse", &own, 300);
        assert_eq!(active.status, Status::Running);
        assert_eq!(active.status_since, 300);
        assert_eq!(active.session_id, original.session_id);
        assert_eq!(active.label, original.label);
        let live = HashMap::from([("%1".into(), PaneInfo { dead: false, window_id: "@1".into(), index: 0 })]);
        assert_eq!(active.effective_status(&live), Status::Running);
        let dead = HashMap::from([("%1".into(), PaneInfo { dead: true, window_id: "@1".into(), index: 0 })]);
        assert_eq!(active.effective_status(&dead), Status::Exited);
        assert_eq!(active.effective_status(&HashMap::new()), Status::Exited);
        let sub = parse_input(r#"{"tool_name":"Bash","agent_id":"child"}"#);
        assert_eq!(apply_event(&ended, "PostToolUse", &sub, 300).status, Status::Exited);
        assert_eq!(apply_event(&active, "Stop", &HookInput::default(), 400).status, Status::NeedsReply);
    }

    #[test]
    fn codex_background_thread_in_another_folder_leaves_the_conversation_alone() {
        // 11-22: Codex's memory consolidation ran in ~/.codex/memories under the conversation's SHIFU_ID; its events
        // replaced the session ID and its end marked the live conversation exited.
        let memories = parse_input(r#"{"session_id":"memory","cwd":"/Users/x/.codex/memories","hook_event_name":"SessionEnd"}"#);
        assert_eq!(memories.cwd.as_deref(), Some("/Users/x/.codex/memories"));
        assert!(foreign_thread("codex", "/tmp", &memories));
        // The conversation itself, also when macOS names the same folder through its symlink.
        let own = parse_input(r#"{"session_id":"chat","cwd":"/private/tmp"}"#);
        assert!(!foreign_thread("codex", "/tmp", &own));
        assert!(!foreign_thread("codex", "/tmp", &parse_input(r#"{"session_id":"chat"}"#)));
        // Claude's hook cwd follows `cd` inside the conversation, so it says nothing about which thread it is.
        assert!(!foreign_thread("claude", "/tmp", &memories));
    }

    /// Walk one conversation through hook payloads, as `shifu hook` would apply them one after another.
    fn walk(start: &Agent, steps: &[(&str, &str)]) -> Agent {
        steps.iter().enumerate().fold(start.clone(), |a, (i, (event, raw))| apply_event(&a, event, &parse_input(raw), 200 + i as u64))
    }

    #[test]
    fn a_sub_agents_approval_request_puts_the_conversation_in_the_waiting_list() {
        // Claude Code and Codex mark a sub-agent's hooks with agent_id, PermissionRequest included (Claude's common
        // hook fields carry `agent_id`; Codex's permission-request schema lists it). The prompt still blocks on the
        // user: before, Shifu dropped the event and kept showing 运行中.
        for (kind, raw) in [
            (Kind::Claude, r#"{"session_id":"s","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"git push"},"agent_id":"a7f","agent_type":"general-purpose"}"#),
            (Kind::Codex, r#"{"session_id":"s","turn_id":"t","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"git push"},"agent_id":"a7f","agent_type":"worker"}"#),
        ] {
            let mut a = agent(Status::Running);
            a.kind = kind;
            a.session_id = Some("s".into());
            let asked = apply_event(&a, "PermissionRequest", &parse_input(raw), 5);
            assert_eq!((asked.status, asked.waiting_for.as_deref()), (Status::NeedsApproval, Some("Bash：git push")), "{kind:?}");
            // Another sub-agent's or the agent's own other tools finishing do not answer it.
            let other_sub = apply_event(&asked, "PostToolUse", &parse_input(r#"{"tool_name":"Read","tool_input":{"file_path":"/r/a"},"agent_id":"b22"}"#), 6);
            assert_eq!(other_sub.status, Status::NeedsApproval, "{kind:?}");
            // The approved command finishing in that sub-agent does.
            let done = apply_event(&other_sub, "PostToolUse", &parse_input(r#"{"tool_name":"Bash","tool_input":{"command":"git push"},"agent_id":"a7f"}"#), 7);
            assert_eq!((done.status, done.waiting_for.as_deref()), (Status::Running, None), "{kind:?}");
        }
    }

    #[test]
    fn claudes_question_tool_also_asks_for_permission_and_stays_a_question() {
        // Claude Code's AskUserQuestion answers checkPermissions with `ask`, so PermissionRequest follows its
        // PreToolUse. It was shown as 等你批准「AskUserQuestion：…」, sorted with approvals.
        let question = r#"{"tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"用哪个数据库？","options":[]}]}}"#;
        let asked = walk(&agent(Status::Running), &[("PreToolUse", question), ("PermissionRequest", question)]);
        assert_eq!((asked.status, asked.waiting_for.as_deref()), (Status::NeedsReply, Some("用哪个数据库？")));
        assert_eq!(walk(&asked, &[("PostToolUse", question)]).status, Status::Running);
    }

    #[test]
    fn a_parallel_tool_finishing_does_not_clear_another_calls_approval() {
        // Claude runs read-only tools side by side; a Read outside the project asks while another one finishes.
        let a = walk(&agent(Status::Running), &[
            ("PreToolUse", r#"{"tool_name":"Read","tool_input":{"file_path":"/repo/a.rs"}}"#),
            ("PreToolUse", r#"{"tool_name":"Read","tool_input":{"file_path":"/etc/hosts"}}"#),
            ("PermissionRequest", r#"{"tool_name":"Read","tool_input":{"file_path":"/etc/hosts"}}"#),
            ("PostToolUse", r#"{"tool_name":"Read","tool_input":{"file_path":"/repo/a.rs"}}"#),
        ]);
        assert_eq!((a.status, a.waiting_for.as_deref()), (Status::NeedsApproval, Some("Read：hosts")));
        let approved = walk(&a, &[("PostToolUse", r#"{"tool_name":"Read","tool_input":{"file_path":"/etc/hosts"}}"#)]);
        assert_eq!(approved.status, Status::Running);
        // Codex reports no PreToolUse; a parallel command finishing after the request is the same story.
        let codex = walk(&agent(Status::Running), &[
            ("PermissionRequest", r#"{"tool_name":"shell","tool_input":{"command":["bash","-lc","git push"]}}"#),
            ("PostToolUse", r#"{"tool_name":"shell","tool_input":{"command":["bash","-lc","ls"]}}"#),
        ]);
        assert_eq!(codex.status, Status::NeedsApproval);
        assert_eq!(walk(&codex, &[("PostToolUse", r#"{"tool_name":"shell","tool_input":{"command":["bash","-lc","git push"]}}"#)]).status, Status::Running);
    }

    #[test]
    fn an_approval_ends_when_its_owner_moves_on_even_without_a_result() {
        // Claude reports a failed call with PostToolUseFailure, which Shifu does not hook: the approved command
        // failed and the agent started its next tool. That next start ends the approval.
        let asked = walk(&agent(Status::Running), &[("PermissionRequest", r#"{"tool_name":"Bash","tool_input":{"command":"npm test"}}"#)]);
        let next = walk(&asked, &[("PreToolUse", r#"{"tool_name":"Edit","tool_input":{"file_path":"/r/a.rs"}}"#)]);
        assert_eq!((next.status, next.approval.as_deref(), next.waiting_for.as_deref()), (Status::Running, None, None));
        // The same call starting again (a duplicate report) changes nothing; a sub-agent starting one neither.
        assert_eq!(walk(&asked, &[("PreToolUse", r#"{"tool_name":"Bash","tool_input":{"command":"npm test"}}"#)]).status, Status::NeedsApproval);
        assert_eq!(walk(&asked, &[("PreToolUse", r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"agent_id":"s1"}"#)]).status, Status::NeedsApproval);
        // Moving on to a question is a question.
        let ask = walk(&asked, &[("PreToolUse", r#"{"tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"继续吗？"}]}}"#)]);
        assert_eq!((ask.status, ask.waiting_for.as_deref()), (Status::NeedsReply, Some("继续吗？")));
    }

    #[test]
    fn a_sub_agents_approval_ends_with_that_sub_agent_or_the_turn() {
        let sub = r#"{"tool_name":"WebFetch","tool_input":{"url":"https://example.com"},"agent_id":"a7f"}"#;
        let asked = walk(&agent(Status::Running), &[("PermissionRequest", sub)]);
        assert_eq!((asked.status, asked.waiting_for.as_deref()), (Status::NeedsApproval, Some("WebFetch：https://example.com")));
        // Denied: the sub-agent carries on with something else.
        assert_eq!(walk(&asked, &[("PreToolUse", r#"{"tool_name":"Grep","tool_input":{"pattern":"x"},"agent_id":"a7f"}"#)]).status, Status::Running);
        // The agent's own tools do not answer a sub-agent's prompt, but the sub-agent's result coming back does.
        assert_eq!(walk(&asked, &[("PostToolUse", r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#)]).status, Status::NeedsApproval);
        assert_eq!(walk(&asked, &[("PostToolUse", r#"{"tool_name":"Task","tool_input":{"prompt":"look"}}"#)]).status, Status::Running);
        // The turn ending or the user typing ends it too.
        assert_eq!(walk(&asked, &[("Stop", "{}")]).status, Status::NeedsReply);
        assert_eq!(walk(&asked, &[("UserPromptSubmit", "{}")]).status, Status::Running);
        // A sub-agent's own session (a Codex sub-agent thread) never replaces the conversation's.
        let mut named = agent(Status::Running);
        named.session_id = Some("main-session".into());
        let after = walk(&named, &[("PermissionRequest", r#"{"session_id":"sub-thread","tool_name":"Bash","agent_id":"a7f"}"#)]);
        assert_eq!(after.session_id.as_deref(), Some("main-session"));
    }

    #[test]
    fn a_record_from_an_older_shifu_keeps_the_plain_rules() {
        // Written before `approval` existed: any tool result ends the approval, as it used to.
        let mut old = agent(Status::NeedsApproval);
        old.waiting_for = Some("Bash：git push".into());
        assert_eq!(walk(&old, &[("PostToolUse", r#"{"tool_name":"Read"}"#)]).status, Status::Running);
        assert_eq!(walk(&old, &[("PreToolUse", r#"{"tool_name":"Read"}"#)]).status, Status::NeedsApproval);
        let raw = serde_json::to_string(&walk(&agent(Status::Running), &[("PermissionRequest", r#"{"tool_name":"Bash","tool_input":{"command":"git push"}}"#)])).unwrap();
        assert!(raw.contains(r#""approval":"\u001fBash\u001fgit push""#), "{raw}");
        assert!(!serde_json::to_string(&walk(&agent(Status::Running), &[("Stop", "{}")])).unwrap().contains("approval"));
    }

    #[test]
    fn session_id_recorded() {
        let input = HookInput { session_id: Some("s1".to_string()), ..Default::default() };
        assert_eq!(apply_event(&agent(Status::Starting), "SessionStart", &input, 5).session_id.as_deref(), Some("s1"));
    }
}
