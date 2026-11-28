//! In-app updates with Tauri's updater plugin, and keeping an installed Computer Use helper in step.
//!
//! The feed is `latest.json` on the fixed `update-feed-preview` release (tauri.conf.json `plugins.updater`):
//! the release workflow attaches one to every draft, and `update-feed.yml` copies it there when the draft is
//! published, so the app is only ever offered a release people can download. The package it names is the
//! signed `Shifu.app` as a `.app.tar.gz`; the plugin checks its minisign signature (and that the signature was
//! made for the announced version) before anything is replaced.
//!
//! Relaunching is safe for the work in progress: agents run in tmux, which outlives the app.

use crate::config::Config;
use serde::Serialize;
use serde_json::{Value, json};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// The quiet check at launch runs at most this often.
const BACKGROUND_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
const HELPER: &str = "Shifu Computer Use.app";

/// `HOME` and the `SHIFU_*` variables this process was started with, before `config::init` exports its own:
/// the relaunch goes through LaunchServices, which would otherwise drop them (and with them a test's
/// separate home and tmux socket).
static LAUNCH_ENV: OnceLock<Vec<(String, String)>> = OnceLock::new();

pub fn remember_launch_env() {
    let _ = LAUNCH_ENV.set(std::env::vars().filter(|(k, _)| k == "HOME" || k.starts_with("SHIFU_")).collect());
}

/// For testing only: `SHIFU_UPDATE_ENDPOINT` replaces the feed URL. Plain http is accepted for a loopback address
/// only (the package's signature is checked either way).
pub fn configure<R: tauri::Runtime>(context: &mut tauri::Context<R>) {
    let Some(endpoint) = std::env::var("SHIFU_UPDATE_ENDPOINT").ok().filter(|e| !e.is_empty()) else { return };
    let Some(updater) = context.config_mut().plugins.0.get_mut("updater").and_then(Value::as_object_mut) else { return };
    updater.insert("endpoints".into(), json!([endpoint]));
    if loopback_http(&endpoint) {
        updater.insert("dangerousInsecureTransportProtocol".into(), json!(true));
    }
}

fn loopback_http(url: &str) -> bool {
    ["http://127.0.0.1", "http://localhost"].iter().any(|p| {
        url.strip_prefix(p).is_some_and(|rest| rest.is_empty() || rest.starts_with(':') || rest.starts_with('/'))
    })
}

/// The `.app` an executable at `X.app/Contents/MacOS/<name>` belongs to.
fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && app.extension()? == "app").then(|| app.to_path_buf())
}

/// Why this copy cannot update itself, as (Chinese, English).
fn unsupported(exe: &Path) -> Option<(&'static str, &'static str)> {
    if bundle_of(exe).is_none() {
        return Some(("开发版不检查更新：只有打包好的 Shifu.app 能更新自己。", "Development builds do not check for updates; only a packaged Shifu.app can update itself."));
    }
    if exe.to_string_lossy().contains("/AppTranslocation/") {
        return Some((
            "macOS 正从一个只读的临时位置运行 Shifu，没法原地更新。先退出 Shifu，把 Shifu.app 移到「应用程序」或 ~/Applications，再打开。",
            "macOS is running Shifu from a read-only temporary location, so it cannot update in place. Quit Shifu, move Shifu.app to Applications or ~/Applications, then open it again.",
        ));
    }
    None
}

/// `update_check: false` in settings.json turns the launch check off; the menu item still works.
fn background_enabled(home: &Path) -> bool {
    std::fs::read_to_string(home.join("settings.json")).ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["update_check"].as_bool())
        .unwrap_or(true)
}

fn last_check(home: &Path) -> Option<u64> {
    let text = std::fs::read_to_string(home.join("update-check.json")).ok()?;
    serde_json::from_str::<Value>(&text).ok()?["checked_at"].as_u64()
}

fn background_due(last: Option<u64>, now: u64) -> bool {
    last.is_none_or(|t| now < t || now - t >= BACKGROUND_EVERY.as_secs())
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Serialize, Default)]
pub struct Check {
    /// "available", "latest", "skipped" (the quiet check was not due or is off) or "unsupported".
    status: &'static str,
    current: String,
    version: Option<String>,
    notes: Option<String>,
    date: Option<String>,
    message: Option<String>,
}

/// The update found by the last check, kept for `update_install`.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

#[tauri::command]
pub async fn update_check(app: AppHandle, cfg: State<'_, Config>, pending: State<'_, Pending>, manual: bool) -> Result<Check, String> {
    let zh = crate::ui_chinese();
    let current = app.package_info().version.to_string();
    let mut check = Check { status: "skipped", current, ..Default::default() };
    let now = now_secs();
    if !manual && !(background_enabled(&cfg.home) && background_due(last_check(&cfg.home), now)) {
        return Ok(check);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if let Some((cn, en)) = unsupported(&exe) {
        if manual {
            check.status = "unsupported";
            check.message = Some(if zh { cn } else { en }.into());
        }
        return Ok(check);
    }
    // Recorded before asking, so a launch without network does not retry on every start.
    let _ = crate::config::write_atomic(&cfg.home.join("update-check.json"), &json!({ "checked_at": now }).to_string());
    let found = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| {
        let e = e.to_string();
        if zh { format!("检查更新失败：{e}") } else { format!("Could not check for updates: {e}") }
    })?;
    match found {
        Some(update) => {
            check.status = "available";
            check.version = Some(update.version.clone());
            check.notes = update.body.clone().filter(|b| !b.trim().is_empty());
            check.date = update.date.map(|d| d.date().to_string());
            *pending.0.lock().unwrap() = Some(update);
        }
        None => check.status = "latest",
    }
    Ok(check)
}

/// Downloads the pending update with progress (`update-progress`), checks its signature, replaces this Shifu.app
/// and relaunches it.
#[tauri::command]
pub async fn update_install(app: AppHandle, pending: State<'_, Pending>) -> Result<(), String> {
    let update = pending.0.lock().unwrap().take().ok_or("No update is waiting; check again.")?;
    let mut downloaded = 0u64;
    let mut last = Instant::now() - Duration::from_secs(1);
    let progress = app.clone();
    let installing = app.clone();
    let result = update.download_and_install(
        |chunk, total| {
            downloaded += chunk as u64;
            if last.elapsed() >= Duration::from_millis(100) || Some(downloaded) == total {
                last = Instant::now();
                let _ = progress.emit("update-progress", json!({ "phase": "download", "downloaded": downloaded, "total": total }));
            }
        },
        || { let _ = installing.emit("update-progress", json!({ "phase": "install" })); },
    ).await;
    if let Err(e) = result {
        // Kept so 「更新并重启」 can be tried again without another check.
        *pending.0.lock().unwrap() = Some(update);
        return Err(e.to_string());
    }
    let _ = app.emit("update-progress", json!({ "phase": "restart" }));
    relaunch(&app)
}

/// Opens the (now replaced) bundle again once this process has exited, through LaunchServices as Finder would.
fn relaunch(app: &AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundle = bundle_of(&exe).ok_or("not running from Shifu.app")?;
    let mut command = Command::new("/bin/sh");
    command.args(["-c", RELAUNCH, "sh", &std::process::id().to_string()]);
    for (k, v) in LAUNCH_ENV.get().into_iter().flatten() {
        command.arg("--env").arg(format!("{k}={v}"));
    }
    command.arg(&bundle).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0);
    command.spawn().map_err(|e| format!("cannot relaunch {}: {e}", bundle.display()))?;
    app.exit(0);
    Ok(())
}

/// Waits up to 30 s for the old process (`$1`) to go, then `open -n` with the rest of the arguments.
const RELAUNCH: &str = r#"pid=$1; shift; i=0
while kill -0 "$pid" 2>/dev/null && [ "$i" -lt 150 ]; do sleep 0.2; i=$((i+1)); done
exec /usr/bin/open -n "$@""#;

// ---------- Computer Use helper ----------

/// A release Shifu.app carries the Computer Use helper built with it (Contents/Resources/helpers). When the person
/// installed the helper (`~/Applications/Shifu Computer Use.app`, from the download package) and it differs from
/// the one inside this app, it is replaced, so an in-app update brings the helper along. Only a helper with the
/// same designated requirement (same certificate and identifier) is replaced: macOS then keeps its Accessibility
/// and Screen Recording permissions. `Some(notice)` when it should have been updated and could not be.
#[tauri::command]
pub async fn computer_helper_sync(cfg: State<'_, Config>) -> Result<Option<String>, String> {
    let cli = cfg.shifu_bin.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let zh = crate::ui_chinese();
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let Some(bundled) = bundle_of(&exe).map(|b| b.join("Contents/Resources/helpers").join(HELPER)) else { return Ok(None) };
        let installed = installed_helper();
        match sync_helper(&bundled, &installed, &computer_home(), &cli) {
            Ok(Some(backup)) => {
                eprintln!("shifu-app: updated {} (previous copy in {})", installed.display(), backup.display());
                Ok(None)
            }
            Ok(None) => Ok(None),
            Err(e) => {
                eprintln!("shifu-app: Computer Use helper not updated: {e}");
                Ok(Some(if zh {
                    format!("电脑操作组件没能跟着 Shifu 一起更新（{e}）。可以从同版本下载包手动替换 {}，步骤见包内 INSTALL 说明。", installed.display())
                } else {
                    format!("The Computer Use helper was not updated with Shifu ({e}). Replace {} from the same version's download package; see INSTALL in the package.", installed.display())
                }))
            }
        }
    }).await.map_err(|e| e.to_string())?
}

/// Where `shifu computer` looks for the helper (`SHIFU_COMPUTER_HELPER` is its executable).
fn installed_helper() -> PathBuf {
    if let Some(exe) = std::env::var_os("SHIFU_COMPUTER_HELPER").map(PathBuf::from)
        && let Some(app) = exe.parent().and_then(Path::parent).and_then(Path::parent) {
        return app.to_path_buf();
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Applications").join(HELPER)
}

fn computer_home() -> PathBuf {
    std::env::var_os("SHIFU_COMPUTER_HOME").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".shifu/computer"))
}

/// `Ok(Some(backup))` after a replacement; `Ok(None)` when there is nothing to do.
fn sync_helper(bundled: &Path, installed: &Path, computer_home: &Path, cli: &str) -> Result<Option<PathBuf>, String> {
    if !bundled.is_dir() || !installed.is_dir() || same_helper(bundled, installed) {
        return Ok(None);
    }
    match (designated(bundled), designated(installed)) {
        (Ok(a), Ok(b)) if a == b => {}
        // Another signature (a local build, an old ad-hoc copy): replacing it would cost the person their
        // permissions, so it is left to them.
        _ => return Ok(None),
    }
    let parent = installed.parent().ok_or("no folder")?;
    let stage = parent.join(format!(".shifu-computer-update.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    let result = (|| {
        let staged = stage.join(HELPER);
        run(Command::new("/usr/bin/ditto").arg(bundled).arg(&staged))?;
        run(Command::new("/usr/bin/codesign").args(["--verify", "--deep", "--strict"]).arg(&staged))?;
        // The bridge runs the old helper until stopped; the next `shifu computer` starts the new one.
        run(Command::new(cli).args(["computer", "stop"]))?;
        let backups = computer_home.join("backups");
        std::fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
        // One previous copy is kept; older ones this app made are removed.
        for old in std::fs::read_dir(&backups).map_err(|e| e.to_string())?.flatten() {
            if old.file_name().to_string_lossy().starts_with("app-update-") {
                let _ = std::fs::remove_dir_all(old.path());
            }
        }
        let backup = backups.join(format!("app-update-{}", now_secs()));
        std::fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
        let kept = backup.join(HELPER);
        std::fs::rename(installed, &kept).map_err(|e| e.to_string())?;
        if let Err(e) = std::fs::rename(&staged, installed) {
            let _ = std::fs::rename(&kept, installed);
            return Err(e.to_string());
        }
        Ok(backup)
    })();
    let _ = std::fs::remove_dir_all(&stage);
    result.map(Some)
}

fn run(command: &mut Command) -> Result<(), String> {
    let out = command.stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    if out.status.success() { return Ok(()); }
    let err = String::from_utf8_lossy(&out.stderr);
    Err(format!("{:?}: {}", command.get_program(), err.trim()))
}

/// Same executable, Info.plist and signature seal: the same build.
fn same_helper(a: &Path, b: &Path) -> bool {
    ["Contents/MacOS/shifu-computer-use-macos", "Contents/Info.plist", "Contents/_CodeSignature/CodeResources"].iter().all(|rel| {
        matches!((std::fs::read(a.join(rel)), std::fs::read(b.join(rel))), (Ok(x), Ok(y)) if x == y)
    })
}

/// The designated requirement macOS ties the helper's permissions to.
fn designated(app: &Path) -> Result<String, String> {
    let out = Command::new("/usr/bin/codesign").args(["-d", "-r-"]).arg(app).output().map_err(|e| e.to_string())?;
    if !out.status.success() { return Err("unsigned".into()); }
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    parse_designated(&text).ok_or_else(|| "no designated requirement".into())
}

fn parse_designated(text: &str) -> Option<String> {
    text.lines().find_map(|l| l.strip_prefix("designated => ")).map(|r| r.trim().to_string())
        // An ad-hoc signature's requirement is its own hash: no other build can match it.
        .filter(|r| !r.starts_with("cdhash "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_may_use_plain_http() {
        assert!(loopback_http("http://127.0.0.1:8765/latest.json") && loopback_http("http://localhost/latest.json"));
        assert!(!loopback_http("http://127.0.0.1.example.com/latest.json"));
        assert!(!loopback_http("http://example.com/latest.json") && !loopback_http("https://127.0.0.1/x"));
    }

    #[test]
    fn only_a_packaged_app_updates_itself() {
        let exe = Path::new("/Users/a/Applications/Shifu.app/Contents/MacOS/shifu-app");
        assert_eq!(bundle_of(exe), Some(PathBuf::from("/Users/a/Applications/Shifu.app")));
        assert!(unsupported(exe).is_none());
        assert!(unsupported(Path::new("/repo/target/release/shifu-app")).is_some());
        assert!(unsupported(Path::new("/private/var/folders/x/AppTranslocation/ABC/d/Shifu.app/Contents/MacOS/shifu-app")).is_some());
    }

    #[test]
    fn the_quiet_check_runs_once_a_day_unless_turned_off() {
        let now = 1_800_000_000;
        assert!(background_due(None, now));
        assert!(!background_due(Some(now - 3600), now));
        assert!(background_due(Some(now - 25 * 3600), now));
        assert!(background_due(Some(now + 3600), now), "a clock set back does not stop checks");
        let home = std::env::temp_dir().join(format!("shifu-update-test-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        assert!(background_enabled(&home));
        std::fs::write(home.join("settings.json"), r#"{"permissions":"own","update_check":false}"#).unwrap();
        assert!(!background_enabled(&home));
        std::fs::write(home.join("update-check.json"), r#"{"checked_at":42}"#).unwrap();
        assert_eq!(last_check(&home), Some(42));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn ad_hoc_helpers_never_match() {
        let signed = "Executable=/x\ndesignated => identifier \"dev.longado.shifu.computer-use\" and certificate leaf = H\"ab\"\n";
        assert_eq!(parse_designated(signed).as_deref(), Some("identifier \"dev.longado.shifu.computer-use\" and certificate leaf = H\"ab\""));
        assert_eq!(parse_designated("designated => cdhash H\"0123\""), None);
        assert_eq!(parse_designated("nothing"), None);
    }

    #[test]
    fn nothing_happens_without_an_installed_or_bundled_helper() {
        let dir = std::env::temp_dir().join(format!("shifu-helper-test-{}", std::process::id()));
        let r = sync_helper(&dir.join("bundled.app"), &dir.join("installed.app"), &dir, "/bin/false");
        assert_eq!(r, Ok(None));
    }
}
