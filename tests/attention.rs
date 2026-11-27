//! 「在等你」靠的就是这些：Claude Code 和 Codex 的每个该让对话进列表的事件（要批准、提问、做完了）都把它放进去，
//! 人回应或 Agent 接着干活后又把它拿出来。`shifu hook` 作为子进程跑，喂各家真实的 hook JSON。
//! 需要 tmux 的那一条用独立 socket 和临时 SHIFU_HOME，不碰用户的 tmux。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn temp_home(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shifu-attention-{}-{tag}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("agents")).unwrap();
    dir
}

fn write_agent(home: &Path, name: &str, kind: &str, status: &str, pane: &str) {
    let record = serde_json::json!({
        "name": name, "kind": kind, "command": kind, "cwd": "/tmp", "created_at": 100,
        "status": status, "status_since": 100, "window_id": "@1", "pane_id": pane,
        "session_id": "s-1", "last_message": null,
    });
    fs::write(home.join("agents").join(format!("{name}.json")), record.to_string()).unwrap();
}

fn read_agent(home: &Path, name: &str) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(home.join("agents").join(format!("{name}.json"))).unwrap()).unwrap()
}

fn spawn_hook(home: &Path, kind: &str, name: &str, event: &str, stdin: &str) -> std::process::Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shifu"))
        .args(["hook", kind, event])
        .env("SHIFU_HOME", home)
        .env("SHIFU_ID", name)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child
}

fn hook(home: &Path, kind: &str, name: &str, event: &str, stdin: &str) {
    let out = spawn_hook(home, kind, name, event, stdin).wait_with_output().unwrap();
    assert!(out.status.success() && out.stderr.is_empty(), "{event}: {}", String::from_utf8_lossy(&out.stderr));
}

/// (status, waiting_for) after the event.
fn after(home: &Path, kind: &str, name: &str, event: &str, stdin: &str) -> (String, Option<String>) {
    hook(home, kind, name, event, stdin);
    let a = read_agent(home, name);
    (a["status"].as_str().unwrap().to_string(), a["waiting_for"].as_str().map(str::to_string))
}

fn waiting(status: &str, what: &str) -> (String, Option<String>) {
    (status.to_string(), Some(what.to_string()))
}

fn working() -> (String, Option<String>) {
    ("running".to_string(), None)
}

/// Claude Code's hook input: the common fields plus the event's own (see `bh` in Claude Code's bundle).
fn claude(event: &str, extra: &str) -> String {
    format!(
        r#"{{"session_id":"s-1","transcript_path":"/tmp/t.jsonl","cwd":"/tmp","permission_mode":"default","hook_event_name":"{event}"{extra}}}"#
    )
}

/// Codex's hook input (its `*.command.input` schemas): turn id and model on every event.
fn codex(event: &str, extra: &str) -> String {
    format!(
        r#"{{"session_id":"s-1","turn_id":"turn-1","transcript_path":null,"cwd":"/tmp","model":"gpt-5","permission_mode":"default","hook_event_name":"{event}"{extra}}}"#
    )
}

const BASH_PUSH: &str = r#","tool_name":"Bash","tool_input":{"command":"git push origin main","description":"Push"},"tool_use_id":"toolu_1""#;

#[test]
fn claude_approval_enters_the_list_and_leaves_once_approved_or_answered() {
    let home = temp_home("claude-approval");
    write_agent(&home, "c", "claude", "running", "%1");
    let run = |event: &str, extra: &str| after(&home, "claude", "c", event, &claude(event, extra));
    assert_eq!(run("PreToolUse", BASH_PUSH), working());
    assert_eq!(run("PermissionRequest", r#","tool_name":"Bash","tool_input":{"command":"git push origin main"},"permission_suggestions":[]"#), waiting("needs_approval", "Bash：git push origin main"));
    // Approved: the command runs and reports back.
    assert_eq!(run("PostToolUse", &format!("{BASH_PUSH},\"tool_response\":{{\"stdout\":\"ok\"}}")), working());
    // Asked again and the user typed instead of approving.
    run("PermissionRequest", r#","tool_name":"Edit","tool_input":{"file_path":"/tmp/app/config.toml","old_string":"a","new_string":"b"}"#);
    assert_eq!(read_agent(&home, "c")["waiting_for"], "Edit：config.toml");
    assert_eq!(run("UserPromptSubmit", r#","prompt":"不要改这个"#), working());
}

#[test]
fn claude_question_and_finished_turn_enter_the_list_and_leave_on_reply() {
    let home = temp_home("claude-reply");
    write_agent(&home, "c", "claude", "running", "%1");
    let run = |event: &str, extra: &str| after(&home, "claude", "c", event, &claude(event, extra));
    let question = r#","tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"用 Postgres 还是 SQLite？","header":"数据库","options":[{"label":"Postgres"},{"label":"SQLite"}],"multiSelect":false}]},"tool_use_id":"toolu_q""#;
    assert_eq!(run("PreToolUse", question), waiting("needs_reply", "用 Postgres 还是 SQLite？"));
    // Claude puts the question tool through its permission prompt as well: still a question, not an approval.
    assert_eq!(run("PermissionRequest", question), waiting("needs_reply", "用 Postgres 还是 SQLite？"));
    assert_eq!(run("PostToolUse", question), working());
    assert_eq!(
        run("Stop", r#","stop_hook_active":false,"last_assistant_message":"改好了，测试都过了。\n\n要我现在提交吗？""#),
        waiting("needs_reply", "要我现在提交吗？")
    );
    assert_eq!(read_agent(&home, "c")["last_message"], "改好了，测试都过了。\n\n要我现在提交吗？");
    assert_eq!(run("UserPromptSubmit", r#","prompt":"提交吧"#), working());
    assert_eq!(run("Stop", r#","stop_hook_active":false,"last_assistant_message":"已提交。""#), waiting("needs_reply", "已提交。"));
    // Woken by a background job without a prompt: its own tool starting means it works again.
    assert_eq!(run("PreToolUse", r#","tool_name":"Bash","tool_input":{"command":"ls"}"#), working());
}

#[test]
fn claude_sub_agent_approval_is_not_lost() {
    let home = temp_home("claude-sub");
    write_agent(&home, "c", "claude", "running", "%1");
    let run = |event: &str, extra: &str| after(&home, "claude", "c", event, &claude(event, extra));
    let sub = r#","agent_id":"a80d08fb6413ebdaf","agent_type":"general-purpose""#;
    assert_eq!(run("PreToolUse", &format!("{BASH_PUSH}{sub}")), working());
    assert_eq!(run("PermissionRequest", &format!("{BASH_PUSH}{sub}")), waiting("needs_approval", "Bash：git push origin main"));
    // Another sub-agent's tool finishing meanwhile does not hide it.
    assert_eq!(run("PostToolUse", r#","tool_name":"Read","tool_input":{"file_path":"/tmp/a"},"agent_id":"other""#).0, "needs_approval");
    assert_eq!(run("PostToolUse", &format!("{BASH_PUSH}{sub}")), working());
    assert_eq!(read_agent(&home, "c")["session_id"], "s-1");
}

#[test]
fn codex_approval_question_and_finish_enter_and_leave_the_list() {
    let home = temp_home("codex");
    write_agent(&home, "x", "codex", "idle", "%1");
    let run = |event: &str, extra: &str| after(&home, "codex", "x", event, &codex(event, extra));
    assert_eq!(run("UserPromptSubmit", r#","prompt":"发布一下"#), working());
    let push = r#","tool_name":"Bash","tool_input":{"command":"git push"},"tool_use_id":"call_1""#;
    assert_eq!(run("PermissionRequest", push), waiting("needs_approval", "Bash：git push"));
    assert_eq!(run("PostToolUse", &format!("{push},\"tool_response\":\"done\"")), working());
    // The older shell tool form: the command as an argument list.
    let shell = r#","tool_name":"shell","tool_input":{"command":["bash","-lc","npm publish"]}"#;
    assert_eq!(run("PermissionRequest", shell), waiting("needs_approval", "shell：bash -lc npm publish"));
    assert_eq!(run("PostToolUse", shell), working());
    assert_eq!(
        run("Stop", r#","stop_hook_active":false,"last_assistant_message":"发布好了。\n\n要不要顺便打个 tag？""#),
        waiting("needs_reply", "要不要顺便打个 tag？")
    );
    assert_eq!(run("UserPromptSubmit", r#","prompt":"打吧"#), working());
    // Esc: the turn ends without Stop; it waits for the user all the same.
    assert_eq!(run("Interrupt", ""), ("needs_reply".to_string(), None));
    assert_eq!(run("UserPromptSubmit", r#","prompt":"继续"#), working());
    // A Codex sub-agent's approval.
    let sub = r#","tool_name":"Bash","tool_input":{"command":"rm -rf build"},"agent_id":"agent-2","agent_type":"worker""#;
    assert_eq!(run("PermissionRequest", sub), waiting("needs_approval", "Bash：rm -rf build"));
    assert_eq!(run("PostToolUse", sub), working());
    assert_eq!(run("SessionEnd", r#","reason":"exit""#), ("exited".to_string(), None));
}

#[test]
fn codex_parallel_command_finishing_after_the_request_keeps_the_approval() {
    let home = temp_home("codex-parallel");
    write_agent(&home, "x", "codex", "running", "%1");
    let run = |event: &str, extra: &str| after(&home, "codex", "x", event, &codex(event, extra));
    assert_eq!(run("PermissionRequest", r#","tool_name":"Bash","tool_input":{"command":"git push"}"#), waiting("needs_approval", "Bash：git push"));
    assert_eq!(run("PostToolUse", r#","tool_name":"Bash","tool_input":{"command":"git status"}"#), waiting("needs_approval", "Bash：git push"));
    assert_eq!(run("PostToolUse", r#","tool_name":"Bash","tool_input":{"command":"git push"}"#), working());
}

#[test]
fn simultaneous_hooks_never_lose_the_approval() {
    // Parallel tool calls fire their hooks at once; each hook is its own process. Whatever order they take the
    // record lock in, the approval that was asked for is what is left, and the record stays readable.
    for kind in ["claude", "codex"] {
        let home = temp_home(&format!("burst-{kind}"));
        write_agent(&home, "a", kind, "running", "%1");
        let event = |e: &str, extra: &str| if kind == "claude" { claude(e, extra) } else { codex(e, extra) };
        let mut children = Vec::new();
        for i in 0..8 {
            children.push(spawn_hook(&home, kind, "a", "PostToolUse", &event("PostToolUse", &format!(r#","tool_name":"Read","tool_input":{{"file_path":"/tmp/f{i}"}}"#))));
        }
        children.push(spawn_hook(&home, kind, "a", "PermissionRequest", &event("PermissionRequest", r#","tool_name":"WebFetch","tool_input":{"url":"https://example.com/x"}"#)));
        for child in children {
            assert!(child.wait_with_output().unwrap().status.success());
        }
        let a = read_agent(&home, "a");
        assert_eq!((a["status"].as_str(), a["waiting_for"].as_str()), (Some("needs_approval"), Some("WebFetch：https://example.com/x")), "{kind}");
    }
}

/// The app restarting while a conversation waits: everything it shows comes from the record and the live pane, so a
/// fresh process (here `shifu status --json`, which reads the same way the app's poller does) sees the same wait.
#[test]
fn a_waiting_conversation_is_still_waiting_after_a_restart() {
    let socket = format!("shifu-attention-test-{}", std::process::id());
    let tmux = shifu::tmux::find_tmux_bin();
    let started = Command::new(&tmux)
        .args(["-L", &socket, "-f", "/dev/null", "new-session", "-d", "-P", "-F", "#{pane_id}", "sleep 120"])
        .output();
    let Some(started) = started.ok().filter(|o| o.status.success()) else {
        eprintln!("tmux is not available; skipping");
        return;
    };
    let pane = String::from_utf8_lossy(&started.stdout).trim().to_string();
    let home = temp_home("restart");
    let result = std::panic::catch_unwind(|| {
        write_agent(&home, "c", "claude", "running", &pane);
        hook(&home, "claude", "c", "PermissionRequest", &claude("PermissionRequest", BASH_PUSH));
        let status = || {
            let out = Command::new(env!("CARGO_BIN_EXE_shifu"))
                .args(["status", "--json"])
                .env("SHIFU_HOME", &home)
                .env("SHIFU_SOCKET", &socket)
                .env_remove("TMUX")
                .output()
                .unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            let rows: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            (rows[0]["status"].as_str().unwrap().to_string(), rows[0]["waiting_for"].as_str().map(str::to_string))
        };
        assert_eq!(status(), waiting("needs_approval", "Bash：git push origin main"));
        hook(&home, "claude", "c", "PostToolUse", &claude("PostToolUse", BASH_PUSH));
        assert_eq!(status(), working());
        hook(&home, "claude", "c", "Stop", &claude("Stop", r#","last_assistant_message":"推好了。""#));
        assert_eq!(status(), waiting("needs_reply", "推好了。"));
        // The pane gone (agent quit while the app was closed): no longer waiting, offered for resume instead.
        Command::new(&tmux).args(["-L", &socket, "kill-pane", "-t", &pane]).output().unwrap();
        assert_eq!(status(), ("exited".to_string(), None));
    });
    let _ = Command::new(&tmux).args(["-L", &socket, "kill-server"]).output();
    let _ = fs::remove_dir_all(&home);
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
