mod agents;
mod antigravity;
mod clamshell;
mod clis;
mod config;
mod files;
mod fullscreen;
mod goals;
mod models;
mod workbuddy;
mod layout;
mod limits;
mod quota;
mod qwen;
mod jev;
mod notify;
mod daily;
mod pm;
mod power;
mod lid;
mod pty;
mod resources;
mod remote;
mod remotecontrol;
mod search;
mod seen;
mod session;
mod statusline;
mod tasks;
mod taskgoals;

use agents::{AgentView, Branches, PendingQuestions};
use config::Config;
use portable_pty::PtySize;
use pty::Ptys;
use tasks::{Changed, Pick, TaskFile, Tasks};
use shifu::menu::load_recent;
use shifu::tmux::find_tmux_bin;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, State};

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }
}

#[tauri::command]
async fn pty_write(ptys: State<'_, Ptys>, id: u32, data: String) -> Result<(), String> {
    let ptys = ptys.inner().clone();
    tauri::async_runtime::spawn_blocking(move || ptys.write(id, &data)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pty_resize(ptys: State<'_, Ptys>, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    let ptys = ptys.inner().clone();
    tauri::async_runtime::spawn_blocking(move || ptys.resize(id, size(cols, rows))).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pty_kill(ptys: State<'_, Ptys>, id: u32) -> Result<(), String> {
    let ptys = ptys.inner().clone();
    tauri::async_runtime::spawn_blocking(move || ptys.kill(id)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn agents_list(branches: State<Branches>, pending: State<PendingQuestions>) -> Vec<AgentView> {
    agents::snapshot(&branches, &pending)
}

/// Progress page (GOALS.md files and where each agent works). Reads transcripts, so it runs off the main thread.
#[tauri::command]
async fn goals_get() -> Result<goals::Goals, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let (agents, _) = shifu::agent::list();
        let places: Vec<goals::AgentWhere> = agents
            .iter()
            .map(|a| goals::AgentWhere { name: &a.name, kind: a.kind.as_str(), cwd: &a.cwd, session_id: a.session_id.as_deref() })
            .collect();
        goals::collect(&places)
    }).await.map_err(|e| e.to_string())
}

/// The live agents whose conversations mention `query`. Reads transcripts, so it runs off the main thread.
#[tauri::command]
async fn search_conversations(cache: State<'_, search::SearchCache>, query: String) -> Result<Vec<search::Hit>, String> {
    let agents = shifu::agent::list().0;
    Ok(cache.search(&search_sessions(&agents), &query))
}

/// Fetch only the requested Codex session's file labels, using the existing incremental transcript cache.
#[tauri::command]
async fn codex_file_link_labels(app: AppHandle, name: String, session_id: String) -> Result<Vec<search::CodexFileLink>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !shifu::agent::is_safe_name(&name) {
            return Err("invalid conversation name".to_string());
        }
        let record = shifu::agent::load(&name)?.ok_or("conversation not found")?;
        if record.kind != shifu::agent::Kind::Codex || record.session_id.as_deref() != Some(session_id.as_str()) {
            return Err("conversation changed".to_string());
        }
        let session = search::Session { agent: &name, kind: "codex", session_id: &session_id };
        let labels = app.state::<search::SearchCache>().codex_file_link_labels(&session);
        Ok(labels)
    }).await.map_err(|e| e.to_string())?
}

fn search_sessions(agents: &[shifu::agent::Agent]) -> Vec<search::Session<'_>> {
    agents
        .iter()
        .filter_map(|a| Some(search::Session { agent: &a.name, kind: a.kind.as_str(), session_id: a.session_id.as_deref()? }))
        .collect()
}

/// Memory per live agent and for the machine. Spawns `ps` and tmux, so off the main thread.
#[tauri::command]
async fn resources_get() -> resources::Resources {
    resources::collect()
}

#[tauri::command]
async fn power_get(power: State<'_, power::Power>) -> Result<power::Status, String> {
    power.get()
}

#[tauri::command]
async fn power_set(power: State<'_, power::Power>, mode: power::Mode) -> Result<power::Status, String> {
    power.set(mode)
}

#[tauri::command]
async fn power_timer(power: State<'_, power::Power>, minutes: Option<u32>) -> Result<power::Status, String> {
    power.timer(minutes)
}

#[tauri::command]
async fn power_lid(power: State<'_, power::Power>, enabled: bool) -> Result<power::Status, String> {
    power.set_lid(enabled)
}

#[tauri::command]
async fn power_lid_setup(power: State<'_, power::Power>) -> Result<power::Status, String> {
    power.authorize_lid()
}

/// How new and resumed agents start: "bypass" (no-approval flags) or "own" (each agent's own settings).
#[tauri::command]
fn permissions_get() -> shifu::launch::Permissions {
    shifu::launch::permissions()
}

#[tauri::command]
fn permissions_set(mode: shifu::launch::Permissions) -> Result<(), String> {
    shifu::launch::save_permissions(mode)
}

/// Models each agent CLI offers here (Claude, Codex, CodeBuddy). Starts two CLIs briefly, so off the main thread.
#[tauri::command]
async fn models_list(cfg: State<'_, Config>) -> Result<Vec<models::AgentModels>, String> {
    Ok(models::collect(&cfg.home.join("model-probe")))
}

#[tauri::command]
fn agent_new(cfg: State<Config>, kind: String, cwd: String) -> Result<String, String> {
    session::create_agent(&cfg, &kind, &cwd, None, None)
}

/// Ends the agent and takes it off its task.
#[tauri::command]
fn agent_kill(cfg: State<Config>, tasks: State<Tasks>, name: String) -> Result<TaskFile, String> {
    tasks.kill_agent(&cfg, &name)
}

/// `null` before the first start of this version.
#[tauri::command]
fn seen_load(cfg: State<Config>) -> Result<Option<seen::Seen>, String> {
    seen::load(&cfg)
}

#[tauri::command]
fn seen_save(cfg: State<Config>, seen: seen::Seen) -> Result<(), String> {
    seen::save(&cfg, &seen)
}

/// Window title and Dock badge: how many agents need the user.
#[tauri::command]
fn set_attention(app: AppHandle, count: usize) {
    agents::show_waiting(&app, count);
}

/// A macOS notification for an agent that needs the user; a click on it comes back as the `notify-jump` event.
#[tauri::command]
fn notify_send(app: AppHandle, agent: String, title: String, body: String) -> Result<(), String> {
    notify::send(app, agent, title, body)
}

/// Takes these agents' notifications out of Notification Center; the others stay.
#[tauri::command]
fn notify_remove(agents: Vec<String>) {
    notify::remove(&agents)
}

/// The end of Jev's `waiting-log.jsonl` (APP-SPEC-C1); `None` when there is no such file.
#[tauri::command]
fn jev_log_tail() -> Option<String> {
    jev::log_tail()
}

/// Plan limits for the sidebar footer; later changes arrive as the `limits` event.
#[tauri::command]
fn limits_get(cfg: State<Config>, slow: State<limits::SlowLimits>) -> limits::Limits {
    limits::collect(&cfg, &mut limits::CodexCache::default(), &slow)
}

#[tauri::command]
fn limits_refresh(slow: State<limits::SlowLimits>) {
    slow.refresh.request();
}

#[tauri::command]
fn tasks_load(cfg: State<Config>, tasks: State<Tasks>) -> Result<TaskFile, String> {
    tasks.load(&cfg)
}

/// What to tell the user when the CLI the hooks call is not this app's match, given what `<bin> version` said.
/// A CLI from before the command existed fails it, which is a mismatch too.
fn cli_notice(bin: &str, reported: std::io::Result<std::process::Output>) -> Option<String> {
    const FIX: &str = "请用和 App 同一个安装包里的 bin/shifu 替换它，或从安装包重新把 Shifu.app 拖进「应用程序」（App 里自带配套的命令行），然后重开 Shifu。";
    match reported {
        Ok(out) if out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == shifu::SOURCE => None,
        Ok(_) => Some(format!("命令行 {bin} 和这个 App 不是同一版：对话状态、提醒和项目经理可能出错。{FIX}")),
        Err(e) => Some(format!("命令行 {bin} 运行不了（{e}）：对话状态不会更新，不会有提醒，项目经理也开不了。{FIX}")),
    }
}

/// Hooks and the 项目经理 run `cfg.shifu_bin`: the CLI inside the app, or one installed apart from it. A missing
/// one, or one from another build, goes unnoticed until a status looks wrong. `None` when it runs and matches.
#[tauri::command(async)]
fn cli_mismatch(cfg: State<Config>) -> Option<String> {
    if !shifu::clis::shifu_usable(&cfg.shifu_bin) {
        return Some(pm::NO_CLI.into());
    }
    cli_notice(&cfg.shifu_bin, std::process::Command::new(&cfg.shifu_bin).arg("version").output())
}

/// The project manager's folder (its task is the one whose project is this folder).
#[tauri::command]
fn pm_dir(cfg: State<Config>) -> Result<String, String> {
    pm::ensure_dir(&cfg)
}

/// The project manager's task with an agent in it (`kind`, as picked the first time): started the first time and
/// again after it was ended. It works in its own folder, which holds its instructions.
#[tauri::command]
fn pm_open(
    cfg: State<Config>,
    tasks: State<Tasks>,
    kind: String,
    model: Option<String>,
    effort: Option<String>,
) -> Result<Changed, String> {
    pm::require_cli(&cfg)?;
    let dir = pm::ensure_dir(&cfg)?;
    open_pinned(&cfg, &tasks, &dir, pm::TASK_NAME, &kind, Pick { model: model.as_deref(), effort: effort.as_deref() })
}

/// The 每日分析 folder (its task is pinned under 项目经理).
#[tauri::command]
fn daily_dir(cfg: State<Config>) -> Result<String, String> {
    daily::ensure_dir(&cfg)
}

/// The 每日分析 task with an agent in it, started like the project manager's. Scheduling is user-configured.
#[tauri::command]
fn daily_open(
    cfg: State<Config>,
    tasks: State<Tasks>,
    kind: String,
    model: Option<String>,
    effort: Option<String>,
) -> Result<Changed, String> {
    let dir = daily::ensure_dir(&cfg)?;
    open_pinned(&cfg, &tasks, &dir, daily::TASK_NAME, &kind, Pick { model: model.as_deref(), effort: effort.as_deref() })
}

/// A pinned role's task (the one whose project is `dir`): created the first time, given an agent again after it was
/// ended, left alone while it has one.
fn open_pinned(cfg: &Config, tasks: &Tasks, dir: &str, name: &str, kind: &str, pick: Pick) -> Result<Changed, String> {
    let file = tasks.load(cfg)?;
    match file.tasks.iter().find(|t| t.project == dir) {
        // Ending it (the tab's 叉) leaves the task empty; a click on the row starts it again.
        Some(t) if t.agents.is_empty() => {
            let id = t.id.clone();
            tasks.add_agent(cfg, &id, kind, pick)
        }
        Some(_) => Ok(Changed { file, agent: None }),
        None => tasks.create(cfg, dir, name, kind, pick),
    }
}

/// `name` may be empty (the task is then called "任务 N").
#[tauri::command]
fn task_new(
    cfg: State<Config>,
    tasks: State<Tasks>,
    project: String,
    name: String,
    kind: String,
    model: Option<String>,
    effort: Option<String>,
) -> Result<Changed, String> {
    tasks.create(&cfg, &project, &name, &kind, Pick { model: model.as_deref(), effort: effort.as_deref() })
}

#[tauri::command]
fn task_add_agent(
    cfg: State<Config>,
    tasks: State<Tasks>,
    id: String,
    kind: String,
    model: Option<String>,
    effort: Option<String>,
) -> Result<Changed, String> {
    tasks.add_agent(&cfg, &id, &kind, Pick { model: model.as_deref(), effort: effort.as_deref() })
}

/// The user's own name for an agent (its tab); blank goes back to the conversation topic.
#[tauri::command]
fn agent_rename(name: String, label: String) -> Result<(), String> {
    let _lock = shifu::agent::lock()?;
    let mut record = shifu::agent::load(&name)?.ok_or_else(|| format!("no agent named {name}"))?;
    let label = label.trim();
    record.label = (!label.is_empty()).then(|| label.to_string());
    shifu::agent::save(&record)
}

#[tauri::command]
fn task_rename(cfg: State<Config>, tasks: State<Tasks>, id: String, name: String) -> Result<TaskFile, String> {
    tasks.rename(&cfg, &id, &name)
}

#[tauri::command]
fn task_set_current(cfg: State<Config>, tasks: State<Tasks>, id: String) -> Result<TaskFile, String> {
    tasks.set_current(&cfg, &id)
}

#[tauri::command]
fn task_move_agent(
    cfg: State<Config>,
    tasks: State<Tasks>,
    name: String,
    to_id: String,
) -> Result<TaskFile, String> {
    tasks.move_agent(&cfg, &name, &to_id)
}

#[tauri::command]
fn task_delete(cfg: State<Config>, tasks: State<Tasks>, id: String) -> Result<TaskFile, String> {
    tasks.delete(&cfg, &id)
}

#[tauri::command]
fn project_reorder(cfg: State<Config>, tasks: State<Tasks>, paths: Vec<String>) -> Result<TaskFile, String> {
    tasks.reorder_projects(&cfg, &paths)
}

#[tauri::command]
fn task_reorder(cfg: State<Config>, tasks: State<Tasks>, project: String, ids: Vec<String>) -> Result<TaskFile, String> {
    tasks.reorder_tasks(&cfg, &project, &ids)
}

fn task_file(cfg: &Config) -> Result<TaskFile, String> {
    Ok(tasks::read(cfg)?.unwrap_or_default())
}

/// Each task's 终点／重点／清单 file, by task id.
#[tauri::command]
async fn task_goals_get(cfg: State<'_, Config>) -> Result<std::collections::BTreeMap<String, taskgoals::TaskGoal>, String> {
    let cfg = cfg.inner().clone();
    tauri::async_runtime::spawn_blocking(move || Ok(taskgoals::read_all(&cfg.home, &task_file(&cfg)?)))
        .await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn task_goal_save(cfg: State<'_, Config>, id: String, expected: String, text: String) -> Result<taskgoals::Saved, String> {
    let cfg = cfg.inner().clone();
    tauri::async_runtime::spawn_blocking(move || taskgoals::save(&cfg, &task_file(&cfg)?, &id, &expected, &text))
        .await.map_err(|e| e.to_string())?
}

/// Queues a message to the task's conversations: `draft` asks one of them to write the file, otherwise all are
/// told where it is. Returns each submission outcome.
#[tauri::command]
async fn task_goal_tell(cfg: State<'_, Config>, ptys: State<'_, pty::Ptys>, id: String, draft: bool) -> Result<taskgoals::Told, String> {
    let cfg = cfg.inner().clone();
    let ptys = ptys.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (mut records, _) = shifu::agent::list();
        let tmux = shifu::tmux::Tmux::from_env();
        let panes = tmux.panes();
        for record in &mut records { record.status = record.effective_status(&panes); }
        // A conversation nobody has typed in yet gets the text pasted straight in; that must not mix with a keystroke.
        taskgoals::tell(&cfg, &task_file(&cfg)?, &id, draft, &records, &tmux, &ptys)
    }).await.map_err(|e| e.to_string())?
}

/// Gives the task's `index`th checklist line (shown as `text`) to the conversation `agent`.
#[tauri::command]
async fn task_goal_assign(cfg: State<'_, Config>, ptys: State<'_, pty::Ptys>, id: String, index: usize, text: String, agent: String) -> Result<taskgoals::Assigned, String> {
    let cfg = cfg.inner().clone();
    let ptys = ptys.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (mut records, _) = shifu::agent::list();
        let tmux = shifu::tmux::Tmux::from_env();
        let panes = tmux.panes();
        for record in &mut records { record.status = record.effective_status(&panes); }
        taskgoals::assign(&cfg, &task_file(&cfg)?, &id, index, &text, &agent, &records, &tmux, &ptys)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn agent_reorder(cfg: State<Config>, tasks: State<Tasks>, task: String, names: Vec<String>) -> Result<TaskFile, String> {
    tasks.reorder_agents(&cfg, &task, &names)
}

#[tauri::command]
fn path_reveal(path: String) -> Result<(), String> {
    session::reveal(&path)
}

/// Which of the paths written in a terminal exist; relative ones are taken from the agent's directory.
#[tauri::command]
fn paths_resolve(paths: Vec<String>, cwd: String) -> Vec<Option<files::Resolved>> {
    let home = std::env::var("HOME").unwrap_or_default();
    paths.iter().take(64).map(|p| files::resolve(p, &cwd, &home)).collect()
}

#[tauri::command]
fn path_open(target: String) -> Result<(), String> {
    files::open(&target)
}

/// Raw bytes, so a screenshot is not inflated into a JSON number array.
#[tauri::command]
fn file_read(path: String) -> Result<tauri::ipc::Response, String> {
    files::read(&path).map(tauri::ipc::Response::new)
}

#[tauri::command]
async fn docx_preview(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || files::docx_preview(&path)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn file_save(path: String, expected: String, text: String) -> Result<(), String> {
    files::save_markdown(&path, &expected, &text)
}

/// Body: the dropped file's bytes; header `x-name`: its file name (percent-encoded, headers are ASCII).
#[tauri::command]
fn drop_save(cfg: State<Config>, request: tauri::ipc::Request) -> Result<String, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else { return Err("没有收到文件内容".into()) };
    let name = request.headers().get("x-name").and_then(|v| v.to_str().ok()).unwrap_or("");
    let name = percent_decode(name);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    files::save_drop(&cfg.home.join("drops"), &name, bytes, stamp)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = (b[i] == b'%' && i + 2 < b.len())
            .then(|| std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()))
            .flatten();
        if let Some(v) = hex {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[tauri::command]
fn app_name() -> &'static str {
    config::app_name()
}

#[tauri::command]
fn agent_resume(cfg: State<Config>, name: String) -> Result<(), String> {
    session::resume_agent(&cfg, &name)
}

#[tauri::command]
fn agent_retry(cfg: State<Config>, name: String) -> Result<(), String> {
    session::retry_agent(&cfg, &name)
}

/// Why `name` ended right after it started; `None` when it did not (or its dead pane is gone).
#[tauri::command]
async fn agent_launch_failure(name: String) -> Result<Option<shifu::launch_failure::LaunchFailure>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(record) = shifu::agent::load(&name)? else { return Ok(None) };
        Ok(shifu::launch_failure::check(&shifu::tmux::Tmux::from_env(), &record))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn agent_reveal(name: String) -> Result<(), String> {
    let record = shifu::agent::load(&name)?.ok_or_else(|| format!("no agent named {name}"))?;
    session::reveal(&record.cwd)
}

/// Recently used directories that still exist.
#[tauri::command]
fn recent_dirs() -> Result<Vec<String>, String> {
    Ok(load_recent()?.into_iter().filter(|d| Path::new(d).is_dir()).collect())
}

/// Opens a PTY running `tmux attach` for the agent's session.
#[tauri::command]
async fn term_attach(
    app: AppHandle,
    ptys: State<'_, Ptys>,
    cfg: State<'_, Config>,
    name: String,
    cols: u16,
    rows: u16,
) -> Result<u32, String> {
    let ptys = ptys.inner().clone();
    let socket = cfg.socket.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let tmux = shifu::tmux::Tmux::from_env();
        if let Err(e) = session::ensure_links(&tmux) {
            eprintln!("shifu: tmux links: {e}");
        }
        let record = shifu::agent::load(&name)?.ok_or("conversation not found")?;
        let (target, pane) = session::attachment_target(&tmux, &record)?;
        let program = find_tmux_bin().to_string_lossy().into_owned();
        let args = vec!["-L".into(), socket, "attach".into(), "-t".into(), target];
        ptys.spawn(app, &program, &args, size(cols, rows), None, &[], &pane)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn term_shift_enter(ptys: State<'_, Ptys>, name: String, id: Option<u32>) -> Result<(), String> {
    let ptys = ptys.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(id) = id {
            return ptys.session_input(id, |pane| shifu::tmux::Tmux::from_env()
                .run(["send-keys", "-t", pane, "-l", "\x1b[13;2u"]).map(|_| ()));
        }
        let record = shifu::agent::load(&name)?.ok_or("conversation not found")?;
        let pane = record.pane_id.ok_or("conversation has no pane")?;
        ptys.serialize_input(&pane, || session::send_shift_enter(&name))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn term_leave_scroll(ptys: State<'_, Ptys>, name: String, id: Option<u32>) -> Result<(), String> {
    let ptys = ptys.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(id) = id {
            return ptys.session_input(id, |pane| {
                let cancel = format!("send-keys -t '{pane}' -X cancel");
                shifu::tmux::Tmux::from_env().run(["if-shell", "-F", "-t", pane, "#{pane_in_mode}", &cancel]).map(|_| ())
            });
        }
        session::leave_scroll(&name)
    }).await.map_err(|e| e.to_string())?
}

/// Text a mouse selection in tmux handed over (OSC 52). `pbcopy` reads UTF-8 because `config::init` set LANG.
#[tauri::command]
fn clipboard_write(text: String) -> Result<(), String> {
    let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn().map_err(|e| format!("pbcopy failed to start: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("pbcopy has no stdin")?;
    stdin.write_all(text.as_bytes()).map_err(|e| format!("pbcopy write failed: {e}"))?;
    drop(stdin);
    let status = child.wait().map_err(|e| format!("pbcopy failed: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("pbcopy exited with {status}")) }
}

#[tauri::command]
fn layout_save(cfg: State<Config>, task_id: String, json: String) -> Result<(), String> {
    layout::save(&cfg, &task_id, &json)
}

#[tauri::command]
fn layout_load(cfg: State<Config>, task_id: String) -> Result<Option<String>, String> {
    layout::load(&cfg, &task_id)
}

/// The first language in System Settings → Language & Region, e.g. "en-US" or "zh-Hans-CN".
fn preferred_language() -> Option<String> {
    objc2_foundation::NSLocale::preferredLanguages().firstObject().map(|l| l.to_string())
}

/// The menu bar follows the system language, as macOS's own items (About, Hide, Quit) do: Chinese for a Chinese
/// system, English for anything else. The page itself is Chinese either way for now.
fn menu_in_chinese(preferred: Option<&str>) -> bool {
    preferred.is_some_and(|l| l.starts_with("zh"))
}

/// Cmd+N / Cmd+T / Cmd+W / Cmd+J (and Cmd+K, Cmd+B, Cmd+D, Shift+Cmd+D) live in the native menu: macOS handles menu shortcuts before the page sees the key,
/// and the stock menu would make Cmd+W close the whole window.
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let chinese = menu_in_chinese(preferred_language().as_deref());
    let tr = |zh: &'static str, en: &'static str| if chinese { zh } else { en };
    let item = |id: &str, text: &str, accel: &str| MenuItemBuilder::with_id(id, text).accelerator(accel).build(app);
    let app_menu = SubmenuBuilder::new(app, "Shifu")
        .about(None)
        .separator()
        .item(&item("appearance", tr("外观…", "Appearance…"), "CmdOrCtrl+,")?)
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    let file_menu = SubmenuBuilder::new(app, tr("文件", "File"))
        .item(&item("new-task", tr("新建任务", "New Task"), "CmdOrCtrl+N")?)
        .item(&item("new", tr("在当前任务里加 agent", "Add Agent to Current Task"), "CmdOrCtrl+T")?)
        .item(&item("close", tr("关闭面板", "Close Panel"), "CmdOrCtrl+W")?)
        .item(&item("reopen-closed", tr("重新打开关闭的标签", "Reopen Closed Tab"), "CmdOrCtrl+Shift+T")?)
        .separator()
        .item(&item("jump", tr("跳到等你最久的 agent", "Jump to Longest-Waiting Agent"), "CmdOrCtrl+J")?)
        .build()?;
    let edit_menu = SubmenuBuilder::new(app, tr("编辑", "Edit"))
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let view_menu = SubmenuBuilder::new(app, tr("显示", "View"))
        .item(&item("search", tr("搜索对话", "Search Conversations"), "CmdOrCtrl+K")?)
        .item(&item("sidebar", tr("显示或隐藏侧栏", "Show or Hide Sidebar"), "CmdOrCtrl+B")?)
        .separator()
        .item(&item("font-bigger", tr("终端字号放大", "Bigger Terminal Text"), "CmdOrCtrl+=")?)
        .item(&item("font-smaller", tr("终端字号缩小", "Smaller Terminal Text"), "CmdOrCtrl+-")?)
        .item(&item("font-default", tr("终端默认字号", "Default Terminal Text Size"), "CmdOrCtrl+0")?)
        .separator()
        .item(&item("split-down", tr("上下分屏", "Split Down"), "CmdOrCtrl+Shift+D")?)
        .item(&item("split-right", tr("左右分屏", "Split Right"), "CmdOrCtrl+D")?)
        .build()?;
    let window_menu = SubmenuBuilder::new(app, tr("窗口", "Window")).minimize().maximize().build()?;
    MenuBuilder::new(app).items(&[&app_menu, &file_menu, &edit_menu, &view_menu, &window_menu]).build()
}

fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 2 && args[1] == "--source-version" {
        println!("{}", shifu::SOURCE);
        return;
    }
    if args.get(1).is_some_and(|a| a == "--power-watchdog") {
        if let Some(file) = args.get(2) {
            if let Err(e) = lid::watchdog(Path::new(file)) { eprintln!("Shifu 合盖恢复：{e}"); std::process::exit(1); }
        } else { std::process::exit(2); }
        return;
    }
    let cfg = config::init();
    std::fs::create_dir_all(&cfg.home).unwrap_or_else(|e| panic!("cannot create {}: {e}", cfg.home.display()));
    // Read before the window can start an agent: one new session and the tmux server no longer looks gone.
    let power = power::Power::new(&cfg.home);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(cfg)
        .manage(Ptys::default())
        .manage(Branches::default())
        .manage(PendingQuestions::default())
        .manage(Tasks::default())
        .manage(limits::SlowLimits::default())
        .manage(search::SearchCache::default())
        .manage(power)
        .manage(remotecontrol::RemoteMacs::default())
        .menu(build_menu)
        .on_menu_event(|app, event| {
            if let Err(e) = app.emit("menu", event.id().as_ref()) {
                eprintln!("shifu-app: emit menu failed: {e}");
            }
        })
        .setup(|app| {
            clis::spawn_shell_read(app.handle().clone(), app.state::<Config>().home.clone());
            power::spawn_watcher(app.handle().clone());
            agents::spawn_watcher(app.handle().clone());
            limits::spawn_watcher(app.handle().clone());
            fullscreen::spawn_watcher(app.handle().clone());
            // After a restart of the Mac nothing is reopened here: a conversation comes back when it is opened
            // (Eddie 11-22), so starting up does not relaunch every agent at once.
            // Read the open conversations once now, so the first search does not wait (about a second for 150 MB).
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let agents = shifu::agent::list().0;
                handle.state::<search::SearchCache>().warm(&search_sessions(&agents));
                // Async question tools can be answered without changing the agent's running status.
                // Keep this transcript work off the UI and the inbox pump's polling thread.
                loop {
                    let agents = shifu::agent::list().0;
                    let sessions: Vec<_> = agents.iter().filter_map(|agent| {
                        if agent.kind != shifu::agent::Kind::Codex || agent.status == shifu::status::Status::Exited {
                            return None;
                        }
                        Some(search::Session { agent: &agent.name, kind: "codex", session_id: agent.session_id.as_deref()? })
                    }).collect();
                    let pending = handle.state::<search::SearchCache>().pending_questions(&sessions);
                    handle.state::<PendingQuestions>().replace(&agents, pending);
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            remotecontrol::remote_macs_load,
            remotecontrol::remote_macs_save,
            remotecontrol::remote_macs_forget,
            remote::remote_status,
            remote::remote_start,
            remote::remote_stop,
            remote::remote_configure,
            remote::remote_pair,
            remote::remote_approve,
            remote::remote_revoke,
            remote::remote_autostart,
            power_get,
            power_set,
            power_timer,
            power_lid,
            power_lid_setup,
            pty_write,
            pty_resize,
            pty_kill,
            agents_list,
            goals_get,
            search_conversations,
            codex_file_link_labels,
            cli_mismatch,
            pm_dir,
            resources_get,
            pm_open,
            daily_dir,
            daily_open,
            models_list,
            permissions_get,
            permissions_set,
            clis::clis_detect,
            agent_new,
            agent_kill,
            seen_load,
            seen_save,
            set_attention,
            notify_send,
            notify_remove,
            jev_log_tail,
            limits_get,
            limits_refresh,
            tasks_load,
            task_new,
            task_add_agent,
            task_rename,
            agent_rename,
            task_set_current,
            task_move_agent,
            task_delete,
            project_reorder,
            task_reorder,
            agent_reorder,
            task_goals_get,
            task_goal_save,
            task_goal_tell,
            task_goal_assign,
            path_reveal,
            paths_resolve,
            path_open,
            file_read,
            docx_preview,
            file_save,
            drop_save,
            app_name,
            agent_resume,
            agent_retry,
            agent_launch_failure,
            agent_reveal,
            recent_dirs,
            term_attach,
            layout_save,
            layout_load,
            clipboard_write,
            term_shift_enter,
            term_leave_scroll,
            fullscreen::fullscreen_inset
        ])
        .build(tauri::generate_context!())
        .expect("error while building Shifu")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit)
                && let Err(e) = app.state::<power::Power>().shutdown() {
                eprintln!("Shifu 合盖恢复：{e}");
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{cli_notice, menu_in_chinese};
    use std::process::Command;

    #[test]
    fn the_menu_is_chinese_only_on_a_chinese_system() {
        assert!(menu_in_chinese(Some("zh-Hans-CN")) && menu_in_chinese(Some("zh-Hant-TW")));
        assert!(!menu_in_chinese(Some("en-US")) && !menu_in_chinese(Some("ja-JP")) && !menu_in_chinese(None));
    }

    #[test]
    fn only_a_cli_built_from_this_source_passes_without_a_notice() {
        assert_eq!(cli_notice("shifu", Command::new("echo").arg(shifu::SOURCE).output()), None);
        let other = cli_notice("/x/shifu", Command::new("echo").arg("0000000000000000").output()).unwrap();
        assert!(other.contains("/x/shifu") && other.contains("不是同一版"));
        // A CLI from before `version` existed rejects the command.
        assert!(cli_notice("shifu", Command::new("false").output()).unwrap().contains("不是同一版"));
        assert!(cli_notice("/no/such/shifu", Command::new("/no/such/shifu").output()).unwrap().contains("运行不了"));
    }
}
