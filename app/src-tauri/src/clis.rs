//! The agent CLIs on this Mac, for the new-agent picker and the first-run screen (see `shifu::clis`).
//!
//! At start the PATH is the user's login-shell PATH from last time (cached in `<home>/shell-path`), then the PATH
//! the app was given, then the common install folders. A thread asks the login shell again; when it answers, the
//! cache and the sessions' PATH are updated and the page gets a fresh `clis` event.

use serde::Serialize;
use shifu::clis::{self, Found};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const CACHE: &str = "shell-path";
/// Long enough for rc files that load nvm, conda and the like.
const SHELL_TIMEOUT: Duration = Duration::from_secs(10);

/// The login shell answered during this run.
static SHELL_READ: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Debug, Clone)]
pub struct Report {
    pub agents: Vec<Found>,
    /// Shifu runs every conversation in tmux; `None` when it is missing.
    pub tmux: Option<String>,
    /// The login shell's PATH was read this run (else it is last run's, or only the usual folders).
    pub shell_read: bool,
}

pub fn cached_shell_path(home: &Path) -> Option<String> {
    std::fs::read_to_string(home.join(CACHE)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// The PATH for the app and its sessions: login shell (cached), inherited, then the usual install folders.
pub fn startup_path(cached_shell: Option<&str>, inherited: &str, user_home: &str) -> String {
    clis::merge_paths(&[cached_shell.unwrap_or(""), inherited, &clis::fallback_dirs(user_home).join(":")])
}

fn current_path() -> String {
    clis::agent_path().or_else(|| std::env::var("PATH").ok()).unwrap_or_default()
}

pub fn report() -> Report {
    let path = current_path();
    let tmux = std::env::var_os("SHIFU_TMUX")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| clis::find_program(&path, &["tmux"]))
        .map(|p| p.to_string_lossy().into_owned());
    Report { agents: clis::detect(&path), tmux, shell_read: SHELL_READ.load(Ordering::Relaxed) }
}

/// Whether `kind`'s CLI was found, as `isInstalled` in models.ts reads the report: an agent the list does not know
/// about counts as installed.
pub fn is_installed(found: &[Found], kind: &str) -> bool {
    found.iter().find(|f| f.kind == kind).is_none_or(|f| f.path.is_some())
}

/// Whether `kind`'s CLI is on the PATH the app gives its sessions now. Only file checks, like `clis_detect`.
pub fn installed(kind: &str) -> bool {
    is_installed(&clis::detect(&current_path()), kind)
}

/// Asks the login shell for its PATH off the main thread; see the module comment.
pub fn spawn_shell_read(app: AppHandle, home: PathBuf) {
    std::thread::spawn(move || {
        let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into());
        let Some(fresh) = clis::login_shell_path(&shell, SHELL_TIMEOUT) else {
            eprintln!("shifu-app: {shell} did not tell its PATH; using the usual install folders");
            return;
        };
        if let Err(e) = crate::config::write_atomic(&home.join(CACHE), &fresh) {
            eprintln!("shifu-app: saving the shell PATH failed: {e}");
        }
        clis::set_agent_path(&clis::merge_paths(&[&fresh, &current_path()]));
        SHELL_READ.store(true, Ordering::Relaxed);
        if let Err(e) = app.emit("clis", report()) {
            eprintln!("shifu-app: emit clis failed: {e}");
        }
    });
}

/// Which agents are installed, and tmux. Only file checks: nothing is started.
#[tauri::command]
pub fn clis_detect() -> Report {
    report()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shell_path_comes_first_then_inherited_then_the_usual_folders() {
        let got = startup_path(Some("/Users/e/.nvm/versions/node/v22.0.0/bin:/usr/bin"), "/usr/bin:/bin", "/Users/e");
        assert!(got.starts_with("/Users/e/.nvm/versions/node/v22.0.0/bin:/usr/bin:/bin:/Users/e/.kimi-code/bin:/Users/e/.local/bin:"), "{got}");
        assert!(got.contains(":/opt/homebrew/bin:") && got.contains(":/Users/e/.bun/bin"), "{got}");
    }

    #[test]
    fn only_a_known_agent_without_a_path_counts_as_missing() {
        let found = |kind: &str, path: Option<&str>| Found {
            kind: kind.into(),
            name: kind.into(),
            path: path.map(Into::into),
            install_url: String::new(),
        };
        let report = [found("claude", Some("/bin/claude")), found("antigravity", None)];
        assert!(is_installed(&report, "claude"));
        assert!(!is_installed(&report, "antigravity"));
        assert!(is_installed(&report, "unknown"));
    }

    #[test]
    fn without_a_cached_shell_path_the_inherited_one_leads() {
        assert!(startup_path(None, "/usr/bin:/bin", "/Users/e").starts_with("/usr/bin:/bin:/Users/e/.kimi-code/bin:"));
    }
}
