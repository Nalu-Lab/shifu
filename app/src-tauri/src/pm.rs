//! 项目经理：侧栏最上面常驻的那个 Claude。它在自己的文件夹里开（`<home>/pm`），那里的 CLAUDE.md（Codex、Kimi
//! 读 AGENTS.md）写着它的职责和两条命令（`shifu status`、`shifu tell`），所以重开、续接、/clear 之后都还记得。

use crate::config::Config;
use std::path::Path;

pub const TASK_NAME: &str = "项目经理";

fn brief(shifu: &str) -> String {
    let shifu = shifu::launch::sh_word(shifu);
    format!(
        "# 你是 用户 的项目经理

你常驻在 Shifu 里。用户 同时开着很多个 agent 对话（Claude、Codex、CodeBuddy、Kimi），你替他盯全局，回答他「现在怎么样」「哪些在等我」「某件事做到哪了」，需要时替他给某个对话传话。用中文，说人话，先给结论。

## 看全局

- 回答之前先跑 `{shifu} status`。它按「等你批准 / 等你回复 / 在干活 / 启动中 / 空闲 / 已结束」列出每个对话：显示名、任务、哪家、在这个状态多久、方括号里的内部名、最后一句。
- 要细看某个对话做到哪：`{shifu} status --json` 里有每个对话的工作目录（cwd），去读那里的 HANDOFF.md、CHANGELOG.md 或最近的 git log。
- 汇报先说要他马上处理的（等批准、等回复），每个一句话说清是什么事、卡在哪；再说在干活的；空闲和已结束的只在他问时说。用显示名，不报内部名。

## 传话

- 只在 用户 明确让你传话时才发：`{shifu} tell <对话> --from 项目经理 <要说的话>`。<对话> 写方括号里的内部名最稳。
- 发之前跟他确认发给谁、原话是什么；他说「直接发」就不用再确认。
- 只能发给停着的对话（等你回复、空闲）。在干活或等批准的会被拒绝，告诉他稍后再发，不要绕过。

## 操作本机窗口

- 需要查看或操作本机窗口时，先读 `{shifu} computer guide`，按其中的目标选择、回读、错误恢复和授权要求执行。
- 需要读取或操作网页时，先读 `{shifu} browser guide`，优先用 DOM 快照和元素引用，保持会话隔离并核对结果。

## 不做

- 不结束、不新开、不改别的对话，不替他批准任何东西。
- 不改各个项目的代码和文件，除非他明确让你改。
"
    )
}

/// What the user is told when no `shifu` CLI can be run at all (sidebar notice, and why the 项目经理 does not start).
pub const NO_CLI: &str = "找不到 shifu 命令行：对话状态不会更新，不会有提醒，项目经理也开不了。请从安装包重新把 Shifu.app 拖进「应用程序」（App 里自带配套的命令行），然后重开 Shifu。";

/// The 项目经理 starts only with a `shifu` it can run: its instructions begin with `shifu status`.
pub fn require_cli(cfg: &Config) -> Result<(), String> {
    if shifu::clis::shifu_usable(&cfg.shifu_bin) { Ok(()) } else { Err(NO_CLI.into()) }
}

/// The folder exists and its instructions are current (rewritten when the text or the `shifu` path changed).
/// Returns the folder as tasks store it (canonical).
pub fn ensure_dir(cfg: &Config) -> Result<String, String> {
    ensure_role_dir(cfg, "pm", &brief(&cfg.shifu_bin))
}

/// Shifu's own text as of the last start, kept beside the instructions so an edited file can be told apart.
const SHIPPED: &str = ".shifu-default";

/// A pinned role's own folder `<home>/<folder>` with its instructions (CLAUDE.md for Claude, AGENTS.md for Codex
/// and Kimi). Shared by the 项目经理 and 每日分析 rows. A file the user has edited is theirs and stays as it is;
/// one that is missing, or still what Shifu last wrote, follows the current text.
pub fn ensure_role_dir(cfg: &Config, folder: &str, text: &str) -> Result<String, String> {
    let dir = cfg.home.join(folder);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {} failed: {e}", dir.display()))?;
    let shipped = std::fs::read_to_string(dir.join(SHIPPED)).ok();
    for name in ["CLAUDE.md", "AGENTS.md"] {
        let path = dir.join(name);
        let current = std::fs::read_to_string(&path).ok();
        if current.is_none() || current == shipped {
            write_if_changed(&path, text)?;
        }
    }
    write_if_changed(&dir.join(SHIPPED), text)?;
    let real = dir.canonicalize().map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(real.to_string_lossy().into_owned())
}

fn write_if_changed(path: &Path, text: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).ok().as_deref() == Some(text) {
        return Ok(());
    }
    std::fs::write(path, text).map_err(|e| format!("write {} failed: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brief_names_the_installed_shifu_for_both_commands() {
        let b = brief("/Users/e/.cargo/bin/shifu");
        assert!(b.contains("`/Users/e/.cargo/bin/shifu status`"));
        assert!(b.contains("`/Users/e/.cargo/bin/shifu tell <对话> --from 项目经理"));
    }
    #[test]
    fn brief_exposes_computer_guide_without_replacing_existing_duties() {
        let b = brief("/Users/e/.cargo/bin/shifu");
        assert!(b.contains("/Users/e/.cargo/bin/shifu computer guide"));
        assert!(!b.contains(" waimai"));
        assert!(b.contains("/Users/e/.cargo/bin/shifu status"));
    }

    #[test]
    fn a_path_with_spaces_is_quoted_so_the_commands_still_run() {
        assert!(brief("/Users/e/My Apps/Shifu.app/Contents/Resources/bin/shifu").contains("`'/Users/e/My Apps/Shifu.app/Contents/Resources/bin/shifu' status`"));
    }

    #[test]
    fn the_project_manager_starts_only_with_a_shifu_cli_that_runs() {
        use std::os::unix::fs::PermissionsExt;
        let mut cfg = role_home("gate");
        cfg.shifu_bin = cfg.home.join("bin/shifu").to_string_lossy().into_owned();
        assert_eq!(require_cli(&cfg), Err(NO_CLI.to_string()));
        assert!(NO_CLI.contains("项目经理") && NO_CLI.contains("对话状态") && NO_CLI.contains("提醒"));
        std::fs::create_dir_all(cfg.home.join("bin")).unwrap();
        std::fs::write(&cfg.shifu_bin, "#!/bin/sh\n").unwrap();
        assert!(require_cli(&cfg).is_err(), "not executable yet");
        std::fs::set_permissions(&cfg.shifu_bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(require_cli(&cfg), Ok(()));
        std::fs::remove_dir_all(&cfg.home).unwrap();
    }

    fn role_home(tag: &str) -> Config {
        let home = std::env::temp_dir().join(format!("shifu-role-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        Config { home, socket: "s".into(), shifu_bin: "shifu".into(), user_statusline: None }
    }

    #[test]
    fn instructions_the_user_edited_survive_a_restart_and_a_new_default() {
        let cfg = role_home("edited");
        let dir = ensure_role_dir(&cfg, "pm", "first default").unwrap();
        let claude = Path::new(&dir).join("CLAUDE.md");
        std::fs::write(&claude, "my own duties").unwrap();
        ensure_role_dir(&cfg, "pm", "first default").unwrap();
        ensure_role_dir(&cfg, "pm", "second default").unwrap();
        assert_eq!(std::fs::read_to_string(&claude).unwrap(), "my own duties");
        // The one left alone keeps following Shifu.
        assert_eq!(std::fs::read_to_string(Path::new(&dir).join("AGENTS.md")).unwrap(), "second default");
        std::fs::remove_dir_all(&cfg.home).unwrap();
    }

    #[test]
    fn instructions_from_before_shifu_kept_track_are_treated_as_the_users() {
        let cfg = role_home("legacy");
        let dir = cfg.home.join("pm");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "written by an older build or by hand").unwrap();
        ensure_role_dir(&cfg, "pm", "current default").unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap(), "written by an older build or by hand");
        assert_eq!(std::fs::read_to_string(dir.join("AGENTS.md")).unwrap(), "current default");
        std::fs::remove_dir_all(&cfg.home).unwrap();
    }
}
