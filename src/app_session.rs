//! One agent = one tmux session on the app's own socket. Creating, resuming and ending them.

use crate::app_statusline as statusline;
use crate::agent::{self, Agent, Kind, Res, make_name, now_secs};
use crate::commands::write_claude_hooks;
use crate::launch::{agent_command, model_flags, resume_command};
use crate::menu::record_recent;
use crate::status::Status;
use crate::tmux::{Tmux, run_command};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug)]
pub struct Config {
    /// Holds agents/, tasks.json, layouts/, recent.json, tmux.conf, claude-hooks.json, statusline.sh, claude-statusline.json.
    pub home: PathBuf,
    pub socket: String,
    /// The `shifu` binary the agents' hooks call.
    pub shifu_bin: String,
    /// The `statusLine` command in the user's own `~/.claude/settings.json`, read once at start (APP-SPEC-B3).
    pub user_statusline: Option<String>,
}

/// An absent state file is normal on first use. Decoding and other I/O failures must reach the caller.
pub fn read_optional_text(path: &Path) -> std::io::Result<Option<String>> {
    use std::io::Read;
    let (mut file, _) = match open_regular_file(path) {
        Ok(opened) => opened,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    Ok(Some(text))
}

/// Metadata used by desktop watchers to notice replacements and in-place changes.
#[derive(Debug, Eq, PartialEq)]
pub struct FileStamp {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl FileStamp {
    pub fn read(path: &Path) -> std::io::Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::metadata(path)?;
        Ok(Self { device: m.dev(), inode: m.ino(), size: m.len(),
            modified: (m.mtime(), m.mtime_nsec()), changed: (m.ctime(), m.ctime_nsec()) })
    }
}

fn open_regular_file(path: &Path) -> std::io::Result<(std::fs::File, u64)> {
    use std::fs::OpenOptions;
    use std::io::{Error, ErrorKind};
    use std::os::unix::fs::OpenOptionsExt;

    let file = OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(Error::new(ErrorKind::InvalidData, format!("not a regular file: {}", path.display())));
    }
    Ok((file, meta.len()))
}

/// Read a size-limited app record without waiting for a pipe writer.
pub(crate) fn read_bounded_record(path: &Path, limit: usize, too_large: &'static str) -> std::io::Result<Vec<u8>> {
    use std::io::{Error, ErrorKind, Read};
    let (file, size) = open_regular_file(path)?;
    if size > limit as u64 {
        return Err(Error::new(ErrorKind::InvalidData, too_large));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(Error::new(ErrorKind::InvalidData, too_large));
    }
    Ok(bytes)
}

/// Publish one complete version, even when multiple threads save in the same directory.
pub fn write_atomic(dest: &Path, text: &str) -> Result<(), String> {
    use std::fs::{self, OpenOptions, Permissions};
    use std::io::{ErrorKind, Write};
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    let parent = dest.parent().ok_or_else(|| format!("missing parent for {}", dest.display()))?;
    let mode = match dest.symlink_metadata() {
        Ok(meta) if meta.is_file() => Some(meta.permissions().mode() & 0o777),
        Ok(_) => None,
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return Err(format!("inspect {} failed: {e}", dest.display())),
    };
    let (tmp, mut file) = loop {
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".shifu-write-{}-{id}.tmp", std::process::id()));
        if path == dest {
            continue;
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true).mode(mode.unwrap_or(0o666));
        match options.open(&path) {
            Ok(file) => break (path, file),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("create temporary file for {} failed: {e}", dest.display())),
        }
    };
    let result = (|| {
        file.write_all(text.as_bytes())?;
        if let Some(mode) = mode {
            file.set_permissions(Permissions::from_mode(mode))?;
        }
        drop(file);
        fs::rename(&tmp, dest)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(|e| format!("save {} failed: {e}", dest.display()))
}

/// Size of a freshly created session; the first attached terminal resizes it.
const NEW_COLS: &str = "120";
const NEW_ROWS: &str = "40";

/// tmux turns `.` and `:` in a session name into `_`, so `attach -t =<agent name>` would miss it.
pub fn session_name(agent_name: &str) -> String {
    agent_name.replace(['.', ':'], "_")
}

/// `=` makes tmux match the name exactly instead of by prefix.
pub fn exact_target(agent_name: &str) -> String {
    format!("={}", session_name(agent_name))
}

/// Resolve the named conversation and its active pane together. For a missing target,
/// tmux display-message can succeed with empty output; attaching to "" picks another session.
pub fn attachment_target(tmux: &Tmux, agent: &Agent) -> Res<(String, String)> {
    let target = format!("{}:", exact_target(&agent.name));
    let actual = tmux.run(["display-message", "-p", "-t", &target, "#{session_id} #{pane_id}"])?;
    let (session, pane) = actual.trim().split_once(' ').ok_or("会话还未接回")?;
    if !session.starts_with('$') || Some(pane) != agent.pane_id.as_deref() {
        return Err("会话窗格已经改变，请重新连接".into());
    }
    Ok((session.to_string(), pane.to_string()))
}

/// Config the app writes before it starts a tmux server (APP-SPEC-A1 §1).
pub fn conf_text() -> String {
    "\
set -g mouse on
set -g status off
set -s escape-time 10
set -g history-limit 50000
set -g default-terminal \"tmux-256color\"
set -as terminal-features \",*:RGB\"
set -as terminal-features \",*:extkeys\"
set -as terminal-features \",*:hyperlinks\"
set -s extended-keys on
set -g allow-passthrough on
set -g set-clipboard on
set -g focus-events on
set -g window-size latest
setw -g aggressive-resize on
# A dead pane must stay so the app can show \"exited\" and resume it; set globally so there is no window
# between creating the pane and flagging it.
setw -g remain-on-exit on
"
    .to_string()
}

/// Lets OSC 8 links (Claude shows a Markdown link as its label only, the path rides in the link) reach the app's
/// terminal. `conf_text` covers a new server; a server started by an older Shifu gets it here, before the window
/// attaches, since tmux reads a client's features only when it attaches.
pub fn ensure_links(tmux: &Tmux) -> Res<()> {
    if tmux.run(["show", "-s", "terminal-features"])?.contains("hyperlinks") {
        return Ok(());
    }
    tmux.run(["set", "-as", "terminal-features", ",*:hyperlinks"]).map(|_| ())
}

/// `new-session` arguments; prints `<pane_id> <window_id>`.
pub fn new_session_args(agent_name: &str, cwd: &str, launch: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "new-session", "-d", "-P", "-F", "#{pane_id} #{window_id}", "-s", &session_name(agent_name), "-c", cwd, "-e",
        &format!("SHIFU_ID={agent_name}"), "-x", NEW_COLS, "-y", NEW_ROWS,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.extend(path_env());
    args.push(launch.to_string());
    args
}

/// `-e PATH=…`: the PATH the app found the agents in (see `clis`), so a CLI installed through nvm, volta and the
/// like starts too. Empty before the app sets one: the tmux server's own PATH is used.
fn path_env() -> Vec<String> {
    crate::clis::agent_path().map(|path| vec!["-e".to_string(), format!("PATH={path}")]).unwrap_or_default()
}

fn write_conf(cfg: &Config) -> Res<()> {
    std::fs::create_dir_all(&cfg.home).map_err(|e| format!("create {} failed: {e}", cfg.home.display()))?;
    let path = cfg.home.join("tmux.conf");
    std::fs::write(&path, conf_text()).map_err(|e| format!("write {} failed: {e}", path.display()))
}

/// Starts the session (and the tmux server if it is not running) and records pane/window ids on the agent file.
/// The agent file must exist before the process starts: its first hook has to find it.
fn start_session(cfg: &Config, record: &Agent, launch: &str) -> Res<()> {
    write_conf(cfg)?;
    let tmux = Tmux::from_env();
    forget_lost_panes(&tmux, &record.name)?;
    let mut cmd = tmux.server_command();
    cmd.args(new_session_args(&record.name, &record.cwd, launch));
    let created = run_command(cmd)?;
    let (pane, window) = created.split_once(' ').ok_or_else(|| format!("unexpected new-session output: {created:?}"))?;
    // Re-read under the lock so a hook that already fired, or fires now, is not overwritten.
    let _lock = agent::lock()?;
    let current = agent::load(&record.name)?.unwrap_or_else(|| record.clone());
    agent::save(&Agent { pane_id: Some(pane.to_string()), window_id: Some(window.to_string()), ..current })
}

/// With no pane left, the tmux server is gone (the Mac restarted, or tmux was killed) and the session about to be
/// created starts a new one that numbers its panes from %0 again. The other records still name panes of the old
/// server; left alone, such an id soon belongs to another agent, whose live pane would then make a lost agent look
/// alive and take its restart. So they are marked exited with no pane, before the new server exists.
fn forget_lost_panes(tmux: &Tmux, starting: &str) -> Res<()> {
    if !tmux.panes().is_empty() {
        return Ok(());
    }
    let now = now_secs();
    for lost in agent::list().0 {
        if lost.name != starting && (lost.pane_id.is_some() || lost.status != Status::Exited) {
            agent::save(&Agent { pane_id: None, window_id: None, status: Status::Exited, status_since: now, ..lost })?;
        }
    }
    Ok(())
}

/// The shared library writes Claude's hooks file; the app adds its `statusLine` to it (APP-SPEC-B3).
fn write_settings(cfg: &Config, kind: Kind) -> Res<String> {
    let path = write_claude_hooks(kind, &cfg.shifu_bin)?;
    if kind == Kind::Claude {
        statusline::install(cfg, &path)?;
    }
    Ok(path)
}

/// `kind` is `claude`, `codex`, or any other program name. Returns the agent's name.
/// A model or effort picked in the new-agent popover goes into a shell command (quoted), but only ids like
/// `claude-opus-5-5`, `gpt-6.1-sol` or `high` are expected.
fn check_pick(what: &str, value: Option<&str>) -> Res<()> {
    match value {
        // '/' for Kimi's, opencode's and Pi's ids (`kimi-code/k3`, `openrouter/x:free`).
        Some(v) if v.is_empty() || !v.chars().all(|c| c.is_ascii_alphanumeric() || "._-/:".contains(c)) => {
            Err(format!("{what} {v:?} is not a model id"))
        }
        _ => Ok(()),
    }
}

/// Every agent name the app's task list (`<home>/tasks.json`) mentions; empty when there is none or it is unreadable.
fn names_in_tasks(home: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(home.join("tasks.json")) else { return Vec::new() };
    let Ok(file) = serde_json::from_str::<serde_json::Value>(&text) else { return Vec::new() };
    file["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|t| t["agents"].as_array().into_iter().flatten())
        .filter_map(|a| a.as_str().map(str::to_string))
        .collect()
}

pub fn create_agent(cfg: &Config, command: &str, cwd: &str, model: Option<&str>, effort: Option<&str>) -> Res<String> {
    check_pick("model", model)?;
    check_pick("effort", effort)?;
    let dir = Path::new(cwd).canonicalize().map_err(|e| format!("directory {cwd} unusable: {e}"))?;
    if !dir.is_dir() {
        return Err(format!("{} is not a directory", dir.display()));
    }
    let kind = Kind::of_command(command);
    let (existing, _) = agent::list();
    let mut taken: Vec<String> = existing.into_iter().map(|a| a.name).collect();
    // A task may still list an agent whose record is gone; reusing that name would put the new agent in two tasks.
    taken.extend(names_in_tasks(&cfg.home));
    let name = make_name(command, &dir, &taken);
    let settings = write_settings(cfg, kind)?;
    let launch = agent_command(kind, command, &cfg.shifu_bin, &settings, &model_flags(kind, model, effort), crate::launch::permissions());
    let now = now_secs();
    let record = Agent {
        name: name.clone(),
        kind,
        command: command.to_string(),
        cwd: dir.to_string_lossy().to_string(),
        created_at: now,
        status: Status::Starting,
        status_since: now,
        window_id: None,
        pane_id: None,
        session_id: None,
        last_message: None, waiting_for: None,
        model: model.map(str::to_string),
        effort: effort.map(str::to_string),
        label: (command == "terminal").then(|| "终端".to_string()),
    };
    agent::save(&record)?;
    if let Err(e) = start_session(cfg, &record, &launch) {
        agent::remove(&name)?;
        return Err(e);
    }
    if let Err(e) = record_recent(&record.cwd) {
        eprintln!("shifu-app: recent directory not recorded: {e}");
    }
    Ok(name)
}

/// Restarts an exited agent: in place when its dead pane is still there, otherwise in a new session.
pub fn resume_agent(cfg: &Config, name: &str) -> Res<()> {
    restart_agent(cfg, name, false)
}

/// Starts an agent that ended right at its launch again the way it was first started: it never began a
/// conversation, so resuming (`--continue`) could pick up another one in the same folder.
pub fn retry_agent(cfg: &Config, name: &str) -> Res<()> {
    restart_agent(cfg, name, true)
}

fn restart_agent(cfg: &Config, name: &str, fresh: bool) -> Res<()> {
    let record = agent::load(name)?.ok_or_else(|| format!("no agent named {name}"))?;
    let tmux = Tmux::from_env();
    let panes = tmux.panes();
    if record.effective_status(&panes) != Status::Exited {
        return Err(format!("{name} is still running"));
    }
    let settings = write_settings(cfg, record.kind)?;
    let pick = model_flags(record.kind, record.model.as_deref(), record.effort.as_deref());
    let launch = if fresh {
        agent_command(record.kind, &record.command, &cfg.shifu_bin, &settings, &pick, crate::launch::permissions())
    } else {
        resume_command(record.kind, &record.command, &cfg.shifu_bin, &settings, record.session_id.as_deref(), &pick, crate::launch::permissions())
    };
    // Reattaching a saved Codex conversation need not emit SessionStart, and does not submit a turn.
    // Write before launch so a prompt/tool hook during startup can still replace this idle state.
    let status = if !fresh && record.kind == Kind::Codex && record.session_id.is_some() { Status::Idle } else { Status::Starting };
    let restarted = Agent { status, status_since: now_secs(), ..record.clone() };
    agent::save(&restarted)?;
    let live_pane = record.pane_id.as_ref().filter(|p| panes.contains_key(*p));
    let launched = match live_pane {
        Some(pane) => {
            // A retry starts from a blank pane, so the next failure is read from its own output only.
            if fresh {
                let _ = tmux.run(["send-keys", "-R", "-t", pane.as_str()]);
                let _ = tmux.run(["clear-history", "-t", pane.as_str()]);
            }
            let id = format!("SHIFU_ID={name}");
            let mut args: Vec<String> =
                ["respawn-pane", "-k", "-t", pane.as_str(), "-c", record.cwd.as_str(), "-e", id.as_str()].map(String::from).to_vec();
            args.extend(path_env());
            args.push(launch.clone());
            tmux.run(args).map(|_| ())
        }
        None => start_session(cfg, &restarted, &launch),
    };
    if launched.is_err() {
        // Nothing was started: say so, and keep only a pane that is really there to restart in.
        let window_id = live_pane.and(record.window_id.clone());
        let failed = Agent { status: Status::Exited, status_since: now_secs(), pane_id: live_pane.cloned(), window_id, ..record };
        agent::save(&failed)?;
    }
    launched
}

/// Deletes the record first (the hooks of a dying process must not recreate state), then the session.
pub fn kill_agent(name: &str) -> Res<()> {
    let record = agent::load(name)?.ok_or_else(|| format!("no agent named {name}"))?;
    agent::remove(name)?;
    let tmux = Tmux::from_env();
    let session_alive = record.pane_id.as_ref().is_some_and(|p| tmux.panes().contains_key(p));
    if session_alive {
        tmux.run(["kill-session", "-t", &exact_target(name)])?;
    }
    Ok(())
}

/// Shift+Enter in the CSI u form, which Claude Code and Codex both read as "new line".
const SHIFT_ENTER: &str = "\x1b[13;2u";

/// Written straight into the agent's pane. Sent through the attached terminal instead, tmux parses it as a key and,
/// for programs that did not ask for extended keys (Codex), turns it into a plain Enter, which submits.
pub fn send_shift_enter(name: &str) -> Res<()> {
    let target = format!("{}:", exact_target(name));
    Tmux::from_env().run(["send-keys", "-t", &target, "-l", SHIFT_ENTER]).map(|_| ())
}

/// Leaves tmux's scroll mode, if the pane is in it. A wheel turn over a program that does not take the mouse
/// (CodeBuddy) starts that mode, and then Enter and the arrows move tmux's cursor instead of reaching the program.
/// The inner command names the pane too: left bare it goes to whichever session tmux thinks is current, which with
/// several agents open is another one, and that one answers "not in a mode".
pub fn leave_scroll(name: &str) -> Res<()> {
    let target = format!("{}:", exact_target(name));
    let cancel = format!("send-keys -t '{target}' -X cancel");
    Tmux::from_env().run(["if-shell", "-F", "-t", &target, "#{pane_in_mode}", &cancel]).map(|_| ())
}

pub fn reveal(cwd: &str) -> Res<()> {
    let mut cmd = Command::new("open");
    cmd.args(["-R", cwd]);
    run_command(cmd).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_a_task_still_lists_count_as_taken() {
        let home = std::env::temp_dir().join(format!("shifu-names-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        assert!(names_in_tasks(&home).is_empty());
        std::fs::write(home.join("tasks.json"), r#"{"tasks":[{"agents":["claude-w-10"]},{"agents":["codex-w","claude-w-10"]},{}]}"#).unwrap();
        assert_eq!(names_in_tasks(&home), ["claude-w-10", "codex-w", "claude-w-10"]);
        let mut taken = names_in_tasks(&home);
        taken.push("claude-w".into());
        assert_eq!(crate::agent::make_name("claude", Path::new("/x/w"), &taken), "claude-w-2");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn record_reads_bound_size_and_finish_when_a_path_is_a_pipe() {
        use std::fs;
        use std::os::unix::fs::{FileTypeExt, symlink};
        use std::sync::mpsc;
        use std::time::Duration;
        let dir = std::env::temp_dir().join(format!("shifu-bounded-record-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let regular = dir.join("regular");
        fs::write(&regular, b"1234").unwrap();
        let link = dir.join("link");
        symlink(&regular, &link).unwrap();
        assert_eq!(read_bounded_record(&link, 4, "too large").unwrap(), b"1234");
        assert_eq!(read_optional_text(&link).unwrap().as_deref(), Some("1234"));
        assert_eq!(read_bounded_record(&regular, 3, "too large").unwrap_err().to_string(), "too large");
        let pipe = dir.join("pipe");
        assert!(Command::new("mkfifo").arg(&pipe).status().unwrap().success());
        let (done, result) = mpsc::channel();
        let target = pipe.clone();
        std::thread::spawn(move || { let _ = done.send((read_bounded_record(&target, 4, "too large"), read_optional_text(&target))); });
        let (record, text) = result.recv_timeout(Duration::from_secs(2)).expect("state read waited for a pipe writer");
        assert_eq!(record.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(text.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
        assert!(fs::metadata(&pipe).unwrap().file_type().is_fifo());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn overlapping_saves_publish_whole_versions_to_the_requested_files() {
        use std::{fs, sync::Barrier, thread};
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("shifu-overlapping-writes-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let shared = dir.join("shared.json");
        fs::write(&shared, "before").unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o600)).unwrap();
        let versions: Vec<_> = (0..12).map(|n| format!("record {n:02}\n").repeat(24 * 1024)).collect();
        let barrier = Barrier::new(versions.len());
        let results = thread::scope(|scope| {
            let jobs: Vec<_> = versions.iter().enumerate().map(|(n, version)| {
                let distinct = dir.join(format!("state.{n}"));
                let (barrier, shared) = (&barrier, &shared);
                scope.spawn(move || {
                    barrier.wait();
                    let first = write_atomic(&distinct, version);
                    barrier.wait();
                    let second = write_atomic(shared, version);
                    (first, second)
                })
            }).collect();
            jobs.into_iter().map(|job| job.join().unwrap()).collect::<Vec<_>>()
        });
        let shared_contents = fs::read_to_string(&shared).unwrap();
        let permissions = fs::metadata(&shared).unwrap().permissions().mode() & 0o777;
        let individual: Vec<_> = (0..versions.len()).map(|n| fs::read_to_string(dir.join(format!("state.{n}"))).ok()).collect();
        let failed_dest = dir.join("existing-directory");
        fs::create_dir(&failed_dest).unwrap();
        assert!(write_atomic(&failed_dest, "incomplete").is_err());
        let remaining = fs::read_dir(&dir).unwrap().count();
        fs::remove_dir_all(&dir).unwrap();
        assert!(results.iter().all(|(a, b)| a.is_ok() && b.is_ok()), "{results:?}");
        assert!(versions.contains(&shared_contents));
        assert_eq!(permissions, 0o600);
        assert!(individual.iter().zip(&versions).all(|(saved, expected)| saved.as_ref() == Some(expected)));
        assert_eq!(remaining, versions.len() + 2);
    }

    #[test]
    fn session_name_follows_tmux_renaming_of_dots_and_colons() {
        assert_eq!(session_name("claude-proj"), "claude-proj");
        assert_eq!(session_name("claude-my.proj"), "claude-my_proj");
        assert_eq!(exact_target("codex-a.b"), "=codex-a_b");
    }

    #[test]
    fn new_session_args_pass_cwd_id_and_launch_verbatim() {
        let got = new_session_args("codex-a.b", "/tmp/中文 目录", "'codex' -c 'x'");
        let want = [
            "new-session", "-d", "-P", "-F", "#{pane_id} #{window_id}", "-s", "codex-a_b", "-c", "/tmp/中文 目录", "-e",
            "SHIFU_ID=codex-a.b", "-x", "120", "-y", "40", "'codex' -c 'x'",
        ];
        assert_eq!(got, want);
    }

    #[test]
    fn conf_has_the_required_options_and_no_status_bar_or_sidebar() {
        let conf = conf_text();
        for line in [
            "set -g mouse on",
            "set -g status off",
            "set -s escape-time 10",
            "set -g history-limit 50000",
            "set -g default-terminal \"tmux-256color\"",
            "set -as terminal-features \",*:RGB\"",
            "set -as terminal-features \",*:extkeys\"",
            "set -as terminal-features \",*:hyperlinks\"",
            "set -s extended-keys on",
            "set -g allow-passthrough on",
            "set -g set-clipboard on",
            "set -g focus-events on",
            "set -g window-size latest",
            "setw -g aggressive-resize on",
            "setw -g remain-on-exit on",
        ] {
            assert!(conf.lines().any(|l| l == line), "missing: {line}");
        }
        assert!(!conf.contains("sidebar") && !conf.contains("status-right"));
    }
}
