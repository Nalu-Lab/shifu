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

/// The first `name` in `path` that is an executable file.
pub fn find_program(path: &str, names: &[&str]) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    names.iter().find_map(|name| {
        path.split(':').filter(|d| !d.is_empty()).map(|d| Path::new(d).join(name)).find(|p| {
            std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    })
}

/// Every agent in `agents_spec::AGENTS` with where it was found in `path`. StepFun counts as installed when node is
/// there and Shifu's Pi runtime is set up; Qwen honours `SHIFU_QWEN_BIN` as `shifu qwen` does.
pub fn detect(path: &str) -> Vec<Found> {
    crate::agents_spec::AGENTS
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
            }
        })
        .collect()
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
        let found = detect(&path);
        let path_of = |kind: &str| found.iter().find(|f| f.kind == kind).unwrap().path.clone();
        assert_eq!(path_of("claude"), Some(second.join("claude").to_string_lossy().into_owned()));
        assert_eq!(path_of("antigravity"), Some(second.join("agy").to_string_lossy().into_owned()));
        assert_eq!(path_of("gemini"), Some(first.join("gemini").to_string_lossy().into_owned()));
        assert_eq!(path_of("opencode"), Some(first.join("opencode").to_string_lossy().into_owned()));
        assert_eq!(path_of("pi"), Some(second.join("pi").to_string_lossy().into_owned()));
        assert_eq!(path_of("kimi"), None);
        let order: Vec<&str> = found.iter().map(|f| f.kind.as_str()).collect();
        assert_eq!(order, ["claude", "codex", "codebuddy", "kimi", "antigravity", "stepfun", "qwen", "opencode", "pi", "gemini"]);
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
