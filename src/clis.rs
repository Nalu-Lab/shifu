//! Which agent CLIs are installed on this Mac, the way the user's own terminal finds them.
//!
//! A Finder-launched app gets PATH=/usr/bin:/bin:/usr/sbin:/sbin, so a fixed list of folders used to stand in for
//! the user's PATH and the new-agent picker offered every agent Shifu knows, installed or not. Like Orca, Shifu now
//! asks the user's login shell for its PATH (cached for the next launch), adds the common install folders a shell
//! might not list (nvm, volta, asdf, mise, pnpm, bun, …), and looks for each agent's program there: a file check,
//! nothing is run. Sessions the app starts get the same PATH, so what was found also starts.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::RwLock;
use std::time::{Duration, Instant};

// Which agents exist, their names, programs and install pages: `agents_spec::AGENTS`, in the picker's order.

/// What was found for one agent.
#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct Found {
    pub kind: String,
    pub name: String,
    /// Full path of the program; `None` when it is not installed.
    pub path: Option<String>,
    pub install_url: String,
    /// One terminal command that installs it, when there is one (`agents_spec`); else only `install_url`.
    pub install_command: Option<String>,
    /// The command that signs in, for agents that have one; the others sign in inside the agent.
    pub login_command: Option<String>,
    /// Whether its sign-in was found: only for agents whose credentials sit in a known file (`signed_in`), only
    /// once it is installed, and filled in by the app's report (`None` from `detect`, which is called often).
    pub signed_in: Option<bool>,
    /// `built-in` (status hooks), `plain` (a table row without hooks) or `agents.json` (the user's own).
    pub source: String,
    /// How it reports status (`agents_spec::Hooks::name`); `none` for plain agents.
    pub hooks: String,
    /// Offered only when found: a missing one is not named under 未安装 (extra plain rows, the user's own).
    pub extra: bool,
}

/// The common install folders a login shell may not list (or there is no login shell answer yet). Missing folders
/// are left out; nvm's are one per installed Node version.
pub fn fallback_dirs(home: &str) -> Vec<String> {
    let mut dirs = vec![
        format!("{home}/.kimi-code/bin"),
        format!("{home}/.local/bin"),
        format!("{home}/.cargo/bin"),
        "/opt/homebrew/bin".into(),
        "/opt/homebrew/sbin".into(),
        "/usr/local/bin".into(),
        "/opt/local/bin".into(),
        format!("{home}/.volta/bin"),
        format!("{home}/.asdf/shims"),
        format!("{home}/.local/share/mise/shims"),
        format!("{home}/Library/pnpm"),
        format!("{home}/.yarn/bin"),
        format!("{home}/.bun/bin"),
        format!("{home}/.npm-global/bin"),
        format!("{home}/.nix-profile/bin"),
        "/nix/var/nix/profiles/default/bin".into(),
    ];
    // Newest Node first, as `nvm use` of the latest would put it.
    if let Ok(entries) = std::fs::read_dir(format!("{home}/.nvm/versions/node")) {
        let mut versions: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path().join("bin"))).collect();
        versions.sort_by_key(|v| std::cmp::Reverse(node_version(v)));
        dirs.extend(versions.into_iter().map(|p| p.to_string_lossy().into_owned()));
    }
    dirs
}

/// `…/v22.11.0/bin` → (22, 11, 0); anything else sorts last.
fn node_version(bin: &Path) -> (u32, u32, u32) {
    let name = bin.parent().and_then(Path::file_name).and_then(|n| n.to_str()).unwrap_or("");
    let mut parts = name.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

/// The lists in order, each folder once (first place wins), empty entries dropped.
pub fn merge_paths(lists: &[&str]) -> String {
    let mut dirs: Vec<&str> = Vec::new();
    for list in lists {
        for dir in list.split(':').filter(|d| !d.is_empty()) {
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
    }
    dirs.join(":")
}

const START: &str = "__SHIFU_PATH_START__";
const END: &str = "__SHIFU_PATH_END__";

/// The PATH between the markers; a shell's greeting or rc-file output around it is ignored.
pub fn parse_shell_output(out: &str) -> Option<String> {
    let rest = &out[out.find(START)? + START.len()..];
    let path = rest[..rest.find(END)?].trim();
    (!path.is_empty()).then(|| path.to_string())
}

/// Asks the user's login shell (`$SHELL -ilc`, as their terminal starts it) for its PATH. `None` on a shell that
/// fails, prints no PATH, or takes longer than `timeout` (it is killed). Runs rc files, so call it off the UI thread.
pub fn login_shell_path(shell: &str, timeout: Duration) -> Option<String> {
    let script = format!("printf '%s%s%s' '{START}' \"$PATH\" '{END}'");
    let mut child = Command::new(shell)
        .args(["-ilc", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        // Some rc files only skip their interactive extras (prompts, auto-updates) for a dumb terminal.
        .env("TERM", "dumb")
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut out = String::new();
    std::io::Read::read_to_string(child.stdout.as_mut()?, &mut out).ok()?;
    parse_shell_output(&out)
}

/// `path` is a file this user may run (symlinks followed).
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// The first `name` in `path` that is an executable file.
pub fn find_program(path: &str, names: &[&str]) -> Option<PathBuf> {
    names.iter().find_map(|name| path.split(':').filter(|d| !d.is_empty()).map(|d| Path::new(d).join(name)).find(|p| is_executable(p)))
}

/// The `shifu` CLI the app's agents, hooks and 项目经理 call, as a path that runs. Tried in order: `explicit`
/// (`SHIFU_APP_SHIFU`; a bare name is looked up in `path`), `bundled` (the copy inside Shifu.app, or for the CLI
/// itself its own executable), `~/.cargo/bin/shifu` (where a separately installed CLI goes), then `shifu` in `path`.
/// A candidate that is not an executable file is skipped. `None` when none runs.
pub fn resolve_shifu(explicit: Option<&str>, bundled: Option<&Path>, user_home: &str, path: &str) -> Option<String> {
    let explicit = explicit.filter(|b| !b.is_empty()).and_then(|b| {
        if b.contains('/') { Some(PathBuf::from(b)).filter(|p| is_executable(p)) } else { find_program(path, &[b]) }
    });
    let cargo = (!user_home.is_empty()).then(|| Path::new(user_home).join(".cargo/bin/shifu"));
    explicit
        .or_else(|| bundled.filter(|p| is_executable(p)).map(Path::to_path_buf))
        .or_else(|| cargo.filter(|p| is_executable(p)))
        .or_else(|| find_program(path, &["shifu"]))
        .map(|p| p.to_string_lossy().into_owned())
}

/// `bin` (a `Config::shifu_bin`) can be run: a path that is an executable file, or a bare name found on the agents'
/// PATH. What gates the 项目经理, whose instructions start with `shifu status`.
pub fn shifu_usable(bin: &str) -> bool {
    if bin.contains('/') {
        return is_executable(Path::new(bin));
    }
    let path = agent_path().or_else(|| std::env::var("PATH").ok()).unwrap_or_default();
    !bin.is_empty() && find_program(&path, &[bin]).is_some()
}

/// Every agent in `agents_spec::AGENTS`, then the user's own from `<SHIFU_HOME>/agents.json`, with where each was
/// found in `path`. StepFun counts as installed when node is there and Shifu's Pi runtime is set up; Qwen honours
/// `SHIFU_QWEN_BIN` as `shifu qwen` does.
pub fn detect(path: &str) -> Vec<Found> {
    detect_with(path, &crate::user_agents::load().0)
}

pub fn detect_with(path: &str, users: &[crate::user_agents::UserAgent]) -> Vec<Found> {
    let mut found: Vec<Found> = crate::agents_spec::AGENTS
        .iter()
        .map(|spec| {
            let found = match spec.id {
                "stepfun" => find_program(path, spec.detect).filter(|_| crate::stepfun::runtime_path().is_file()),
                "qwen" => std::env::var_os("SHIFU_QWEN_BIN")
                    .filter(|p| !p.is_empty())
                    .map(PathBuf::from)
                    .filter(|p| p.is_file())
                    .or_else(|| find_program(path, spec.detect)),
                _ => find_program(path, spec.detect),
            };
            Found {
                kind: spec.id.into(),
                name: spec.label.into(),
                path: found.map(|p| p.to_string_lossy().into_owned()),
                install_url: spec.install_url.into(),
                install_command: spec.install_command.map(Into::into),
                login_command: spec.login_command.map(Into::into),
                signed_in: None,
                source: crate::agents_spec::source(spec).into(),
                hooks: spec.hooks.name().into(),
                extra: spec.extra,
            }
        })
        .collect();
    found.extend(users.iter().map(|user| {
        let program = if user.command.starts_with('/') {
            Some(PathBuf::from(&user.command)).filter(|p| is_executable(p))
        } else {
            find_program(path, &[user.command.as_str()])
        };
        Found {
            kind: user.id.clone(),
            name: user.label.clone(),
            path: program.map(|p| p.to_string_lossy().into_owned()),
            install_url: user.install_url.clone().unwrap_or_default(),
            install_command: None,
            login_command: None,
            signed_in: None,
            source: "agents.json".into(),
            hooks: crate::agents_spec::Hooks::None.name().into(),
            extra: true,
        }
    }));
    found
}

/// Whether `kind` looks signed in, from a file or variable its CLI itself keeps: nothing is run and no secret is
/// read out (only whether the file is there, or for Claude whether `oauthAccount` is in its config). `None` for
/// agents without such a place (their sign-in lives in the keychain or their own store), so nothing is guessed.
/// `Some(false)` means only that none was found: the agent still offers to sign in when it starts.
///
/// - Claude Code: `oauthAccount` in `$CLAUDE_CONFIG_DIR/.claude.json` (default `~/.claude.json`), or `ANTHROPIC_API_KEY`.
/// - Codex: `$CODEX_HOME/auth.json` (default `~/.codex/auth.json`), written by `codex login`.
/// - Gemini CLI: `~/.gemini/oauth_creds.json`, or `GEMINI_API_KEY`.
pub fn signed_in(kind: &str, user_home: &Path, var: impl Fn(&str) -> Option<String>) -> Option<bool> {
    let set = |key: &str| var(key).is_some_and(|v| !v.is_empty());
    let dir = |key: &str, default: &str| var(key).filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| user_home.join(default));
    match kind {
        "claude" => Some(
            set("ANTHROPIC_API_KEY")
                || std::fs::read_to_string(dir("CLAUDE_CONFIG_DIR", "").join(".claude.json"))
                    .ok()
                    .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                    .is_some_and(|v| v["oauthAccount"].is_object()),
        ),
        "codex" => Some(dir("CODEX_HOME", ".codex").join("auth.json").is_file()),
        "gemini" => Some(set("GEMINI_API_KEY") || user_home.join(".gemini/oauth_creds.json").is_file()),
        _ => None,
    }
}

pub const AGENTS_USAGE: &str = "用法: shifu agents [--json]
  列出 Shifu 认识的每个 Agent：在哪里找到（或 not installed）、是否已登录（yes / no / unknown）、
  状态怎么回报（hook 类型，或 none = 只看窗格是否在运行）、来源（built-in / plain / agents.json）。
  和桌面应用一样找：登录 shell 的 PATH、当前 PATH、常见安装目录（nvm、volta、bun…）；只看文件，不运行它们。
  自己的 Agent 写在 <SHIFU_HOME>/agents.json（桌面应用的是 ~/.shifu/app/agents.json）：
    [{\"id\":\"mybot\",\"label\":\"My Bot\",\"command\":\"mybot\",\"args\":[\"--foo\"],\"install_url\":\"https://…\"}]
  写错的条目会被跳过，并在这里列出原因。";

/// The PATH `shifu agents` looks in, as the app builds it: the login shell's (asked now; last saved answer if it
/// does not reply), the inherited one, then the usual install folders.
fn cli_path() -> String {
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let user_home = var("HOME").unwrap_or_default();
    let shell = var("SHELL").unwrap_or_else(|| "/bin/zsh".into());
    let fresh = login_shell_path(&shell, Duration::from_secs(10))
        .or_else(|| std::fs::read_to_string(crate::agent::shifu_home().join("shell-path")).ok().map(|s| s.trim().to_string()));
    merge_paths(&[fresh.as_deref().unwrap_or(""), &var("PATH").unwrap_or_default(), &fallback_dirs(&user_home).join(":")])
}

/// `shifu agents [--json]`.
pub fn cmd_agents(args: &[String]) -> Result<(), String> {
    let json = match args.first().map(String::as_str) {
        None => false,
        Some("--json") if args.len() == 1 => true,
        Some("-h" | "--help") => {
            println!("{AGENTS_USAGE}");
            return Ok(());
        }
        Some(other) => return Err(format!("shifu agents 不认识 {other:?}。{AGENTS_USAGE}")),
    };
    let file = crate::user_agents::path_in(&crate::agent::shifu_home());
    let (users, warnings) = crate::user_agents::load_from(&file);
    let user_home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let mut agents = detect_with(&cli_path(), &users);
    for agent in agents.iter_mut().filter(|a| a.path.is_some()) {
        agent.signed_in = signed_in(&agent.kind, &user_home, |key| std::env::var(key).ok());
    }
    if json {
        let out = serde_json::json!({ "agents": agents, "agents_json": file, "warnings": warnings });
        println!("{}", serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?);
        return Ok(());
    }
    print!("{}", agents_table(&agents));
    let shown = if file.is_file() { format!("{}（{} 个）", file.display(), users.len()) } else { format!("{}（没有这个文件）", file.display()) };
    println!("\n自己的 Agent：{shown}");
    for warning in &warnings {
        eprintln!("shifu: {warning}");
    }
    Ok(())
}

/// One line per agent: label, path or `not installed`, signed in, status reporting, source.
pub fn agents_table(agents: &[Found]) -> String {
    let rows: Vec<[String; 5]> = agents
        .iter()
        .map(|a| {
            let signed = match a.signed_in {
                Some(true) => "yes",
                Some(false) => "no",
                None => "unknown",
            };
            [
                a.name.clone(),
                a.path.clone().unwrap_or_else(|| "not installed".into()),
                format!("signed in: {signed}"),
                format!("status: {}", a.hooks),
                a.source.clone(),
            ]
        })
        .collect();
    let width = |i: usize| rows.iter().map(|r| r[i].chars().count()).max().unwrap_or(0);
    let widths = [width(0), width(1), width(2), width(3)];
    let mut out = String::new();
    for r in &rows {
        for (i, w) in widths.iter().enumerate() {
            out.push_str(&format!("{:<w$}  ", r[i], w = w));
        }
        out.push_str(&r[4]);
        out.push('\n');
    }
    out
}

/// The PATH sessions the app starts are given; `None` until the app sets it (then tmux's own environment is used).
static AGENT_PATH: RwLock<Option<String>> = RwLock::new(None);

/// Orca's tmux stand-in folder is left out, as when the tmux server starts (`tmux::server_env_changes`).
pub fn set_agent_path(path: &str) {
    let kept: Vec<&str> = path.split(':').filter(|d| !d.trim_end_matches('/').ends_with("/.orca/claude-agent-teams-bin")).collect();
    *AGENT_PATH.write().unwrap_or_else(|e| e.into_inner()) = Some(kept.join(":"));
}

pub fn agent_path() -> Option<String> {
    AGENT_PATH.read().unwrap_or_else(|e| e.into_inner()).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_in_is_read_only_from_the_files_each_cli_keeps() {
        let home = std::env::temp_dir().join(format!("shifu-signin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let none = |_: &str| None;
        assert_eq!(signed_in("claude", &home, none), Some(false));
        assert_eq!(signed_in("codex", &home, none), Some(false));
        assert_eq!(signed_in("gemini", &home, none), Some(false));
        // Agents whose sign-in is not in a known file are not guessed.
        for kind in ["kimi", "codebuddy", "qwen", "opencode", "pi", "antigravity", "stepfun", "terminal"] {
            assert_eq!(signed_in(kind, &home, none), None, "{kind}");
        }
        // Claude's config without an account (first run, or signed out) is not signed in.
        std::fs::write(home.join(".claude.json"), r#"{"numStartups": 3}"#).unwrap();
        assert_eq!(signed_in("claude", &home, none), Some(false));
        std::fs::write(home.join(".claude.json"), r#"{"oauthAccount": {"emailAddress": "x"}}"#).unwrap();
        assert_eq!(signed_in("claude", &home, none), Some(true));
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::write(home.join(".codex/auth.json"), "{}").unwrap();
        assert_eq!(signed_in("codex", &home, none), Some(true));
        std::fs::create_dir_all(home.join(".gemini")).unwrap();
        std::fs::write(home.join(".gemini/oauth_creds.json"), "{}").unwrap();
        assert_eq!(signed_in("gemini", &home, none), Some(true));
        // The variables that move or replace those files.
        let elsewhere = home.join("elsewhere");
        let moved = |key: &str| (key == "CODEX_HOME" || key == "CLAUDE_CONFIG_DIR").then(|| elsewhere.to_string_lossy().into_owned());
        assert_eq!(signed_in("codex", &home, moved), Some(false));
        assert_eq!(signed_in("claude", &home, moved), Some(false));
        let keyed = |key: &str| (key == "ANTHROPIC_API_KEY" || key == "CLAUDE_CONFIG_DIR").then(|| if key == "ANTHROPIC_API_KEY" { "k".to_string() } else { elsewhere.to_string_lossy().into_owned() });
        assert_eq!(signed_in("claude", &home, keyed), Some(true));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn detect_carries_the_install_and_sign_in_commands_from_the_table() {
        let found = detect("/nonexistent-shifu-path");
        let claude = found.iter().find(|f| f.kind == "claude").unwrap();
        assert_eq!(claude.path, None);
        assert_eq!(claude.install_command.as_deref(), Some("curl -fsSL https://claude.ai/install.sh | bash"));
        assert_eq!(claude.login_command.as_deref(), Some("claude auth login"));
        assert_eq!(claude.signed_in, None);
        let kimi = found.iter().find(|f| f.kind == "kimi").unwrap();
        assert_eq!((kimi.install_command.as_deref(), kimi.login_command.as_deref()), (None, None));
    }

    #[test]
    fn the_path_is_read_between_the_markers_whatever_the_rc_files_print() {
        let out = format!("Welcome!\n{START}/a/bin:/usr/bin{END}\nbye");
        assert_eq!(parse_shell_output(&out).as_deref(), Some("/a/bin:/usr/bin"));
        assert_eq!(parse_shell_output("no markers"), None);
        assert_eq!(parse_shell_output(&format!("{START}{END}")), None);
    }

    #[test]
    fn merged_paths_keep_the_first_place_of_each_folder() {
        assert_eq!(merge_paths(&["/a:/b", "", "/b:/c::/a"]), "/a:/b:/c");
    }

    #[test]
    fn a_login_shell_answers_with_its_path_and_a_hung_one_is_given_up() {
        let path = login_shell_path("/bin/sh", Duration::from_secs(10)).expect("sh prints its PATH");
        assert!(path.contains('/'), "{path}");
        assert_eq!(login_shell_path("/nonexistent/shell", Duration::from_secs(1)), None);

        use std::os::unix::fs::PermissionsExt;
        let hung = std::env::temp_dir().join(format!("shifu-hung-shell-{}", std::process::id()));
        std::fs::write(&hung, "#!/bin/sh\nsleep 30\n").unwrap();
        std::fs::set_permissions(&hung, std::fs::Permissions::from_mode(0o755)).unwrap();
        let started = Instant::now();
        assert_eq!(login_shell_path(&hung.to_string_lossy(), Duration::from_millis(300)), None);
        assert!(started.elapsed() < Duration::from_secs(5));
        std::fs::remove_file(&hung).unwrap();
    }

    #[test]
    fn programs_are_found_only_as_executable_files_in_order() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("shifu-clis-{}", std::process::id()));
        let (first, second) = (root.join("first"), root.join("second"));
        for dir in [&first, &second] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(first.join("codex"), "").unwrap(); // not executable
        std::fs::create_dir_all(first.join("claude")).unwrap(); // a folder
        for (dir, name) in [(&second, "codex"), (&second, "claude"), (&second, "agy"), (&first, "gemini"), (&first, "opencode"), (&second, "pi")] {
            let p = dir.join(name);
            std::fs::write(&p, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = format!("{}:{}", first.display(), second.display());
        assert_eq!(find_program(&path, &["codex"]), Some(second.join("codex")));
        let found = detect_with(&path, &[]);
        let path_of = |kind: &str| found.iter().find(|f| f.kind == kind).unwrap().path.clone();
        assert_eq!(path_of("claude"), Some(second.join("claude").to_string_lossy().into_owned()));
        assert_eq!(path_of("antigravity"), Some(second.join("agy").to_string_lossy().into_owned()));
        assert_eq!(path_of("gemini"), Some(first.join("gemini").to_string_lossy().into_owned()));
        assert_eq!(path_of("opencode"), Some(first.join("opencode").to_string_lossy().into_owned()));
        assert_eq!(path_of("pi"), Some(second.join("pi").to_string_lossy().into_owned()));
        assert_eq!(path_of("kimi"), None);
        let order: Vec<&str> = found.iter().map(|f| f.kind.as_str()).collect();
        assert_eq!(order, [
            "claude", "codex", "codebuddy", "kimi", "antigravity", "stepfun", "qwen", "opencode", "pi", "gemini",
            "aider", "goose", "amp", "cursor-agent", "droid", "copilot", "crush",
        ]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_agents_table_has_one_aligned_line_per_agent() {
        let mut found = detect_with("/nonexistent-shifu-path", &[]);
        found[0].path = Some("/bin/claude".into());
        found[0].signed_in = Some(true);
        let table = agents_table(&found[..2]);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("Claude  /bin/claude    signed in: yes      status: claude-settings  built-in"), "{table}");
        assert!(lines[1].starts_with("Codex   not installed  signed in: unknown  status: codex"), "{table}");
    }

    #[test]
    fn extra_and_user_agents_are_found_the_same_way_and_marked_extra() {
        use crate::user_agents::UserAgent;
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("shifu-clis-extra-{}", std::process::id()));
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        for name in ["aider", "mybot", "elsewhere"] {
            let p = bin.join(name);
            std::fs::write(&p, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let user = |id: &str, command: String| UserAgent { id: id.into(), label: id.to_uppercase(), command, args: vec![], install_url: None };
        let users = [
            user("mybot", "mybot".into()),
            user("byPath", bin.join("elsewhere").to_string_lossy().into_owned()),
            user("gone", "not-installed-here".into()),
            user("gone2", "/no/such/program".into()),
        ];
        let found = detect_with(&bin.to_string_lossy(), &users);
        let get = |kind: &str| found.iter().find(|f| f.kind == kind).unwrap();
        assert_eq!(get("aider").path, Some(bin.join("aider").to_string_lossy().into_owned()));
        assert_eq!((get("aider").source.as_str(), get("aider").hooks.as_str(), get("aider").extra), ("plain", "none", true));
        assert_eq!((get("gemini").source.as_str(), get("gemini").extra), ("plain", false));
        assert_eq!((get("claude").source.as_str(), get("claude").hooks.as_str(), get("claude").extra), ("built-in", "claude-settings", false));
        assert_eq!(get("droid").path, None);
        let mybot = get("mybot");
        assert_eq!((mybot.name.as_str(), mybot.source.as_str(), mybot.extra), ("MYBOT", "agents.json", true));
        assert_eq!(mybot.path, Some(bin.join("mybot").to_string_lossy().into_owned()));
        assert_eq!(get("byPath").path, Some(bin.join("elsewhere").to_string_lossy().into_owned()));
        assert_eq!((get("gone").path.clone(), get("gone2").path.clone()), (None, None));
        // The user's own come after the table, in file order.
        let tail: Vec<&str> = found.iter().rev().take(4).rev().map(|f| f.kind.as_str()).collect();
        assert_eq!(tail, ["mybot", "byPath", "gone", "gone2"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_shifu_cli_is_the_override_then_the_bundled_one_then_cargo_then_path() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("shifu-resolve-{}", std::process::id()));
        let home = root.join("home");
        let (over, bundled, cargo, on_path) = (root.join("over/shifu"), root.join("app/bin/shifu"), home.join(".cargo/bin/shifu"), root.join("path/shifu"));
        let exe = |p: &Path| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        let (h, path) = (home.to_string_lossy().into_owned(), root.join("path").to_string_lossy().into_owned());
        let s = |p: &Path| Some(p.to_string_lossy().into_owned());
        let resolve = |explicit: Option<&str>| resolve_shifu(explicit, Some(&bundled), &h, &path);

        // Nothing installed anywhere: no CLI, whatever the override names.
        assert_eq!(resolve(None), None);
        assert_eq!(resolve(Some("/no/such/shifu")), None);
        exe(&on_path);
        assert_eq!(resolve(None), s(&on_path));
        exe(&cargo);
        assert_eq!(resolve(None), s(&cargo));
        exe(&bundled);
        assert_eq!(resolve(None), s(&bundled));
        assert_eq!(resolve(Some("")), s(&bundled));
        // An override that does not run is skipped; one that does wins; a bare name is looked up on PATH.
        assert_eq!(resolve(Some(&over.to_string_lossy())), s(&bundled));
        exe(&over);
        assert_eq!(resolve(Some(&over.to_string_lossy())), s(&over));
        assert_eq!(resolve(Some("shifu")), s(&on_path));
        // A file that is not executable does not count.
        std::fs::set_permissions(&bundled, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(resolve(None), s(&cargo));
        assert!(shifu_usable(&cargo.to_string_lossy()));
        assert!(!shifu_usable(&bundled.to_string_lossy()));
        assert!(!shifu_usable("/no/such/shifu"));
        assert!(!shifu_usable(""));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn nvm_versions_come_newest_first() {
        let root = std::env::temp_dir().join(format!("shifu-nvm-{}", std::process::id()));
        for v in ["v9.0.0", "v22.11.0", "v20.18.1"] {
            std::fs::create_dir_all(root.join(".nvm/versions/node").join(v).join("bin")).unwrap();
        }
        let home = root.to_string_lossy().into_owned();
        let nvm: Vec<String> = fallback_dirs(&home).into_iter().filter(|d| d.contains(".nvm")).collect();
        let names: Vec<&str> = nvm.iter().map(|d| d.rsplit('/').nth(1).unwrap()).collect();
        assert_eq!(names, ["v22.11.0", "v20.18.1", "v9.0.0"]);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
