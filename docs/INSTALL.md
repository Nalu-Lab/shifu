# 安装与使用

[回到 Shifu](../README.md)　[English](INSTALL.en.md)

Shifu 提供 [Apple Silicon 预发布包](https://github.com/Nalu-Lab/shifu/releases/tag/remote-v0.4.5-preview.1)，内含 App、匹配的 Shifu CLI 和安装说明；使用普通会话无需 Rust，StepFun 另需 Node/Pi；tmux、Agent 和账号登录需自行准备。[远程安装与配对](REMOTE-RELAY.md)只在连接另一台 Mac 时需要，首次安装用不到。

普通会话优先使用下载包；源码构建方式收在下方开发者说明。

<!-- permission-mode: the one place that describes the permission mode; the site's install notes are generated from it. -->
**权限模式：**第一次开对话前，Shifu 会问一次 Agent 的权限，默认选「按 Agent 自己的设置」；以后在新建任务的「权限」里随时能改。第一次打开的引导里就是一个「免确认」开关，默认关着（即「按 Agent 自己的设置」），点「开始」时保存；跳过引导的，第一个 Agent 启动前再问一次。选「按 Agent 自己的设置」时 Shifu 不加免确认参数，什么时候请求批准由 Agent 自己的配置决定。从旧版升级的不会被问，保持原来的设置（0.4.3 及更早的版本没改过的是免确认）。
<!-- /permission-mode -->

选「免确认（全权）」时，Claude、Codex、CodeBuddy、Kimi、Qwen Code 和 opencode 用免确认参数启动（opencode 加 `--auto`，你明确 deny 的规则仍然有效；如 `--dangerously-skip-permissions`、`--dangerously-bypass-approvals-and-sandbox`、`--yolo`）。改了设置后，新开或恢复的对话按新的设置启动。Pi 本身不弹批准，这个开关对它不起作用。

应用用 Shifu 自己的固定证书签名（不是 Apple 颁发的），尚未公证。首次打开若被 macOS 拦截，确认来源后按「系统设置 → 隐私与安全性」中的对应提示处理。

## 快速开始

[下载 Apple Silicon 预发布包](https://github.com/Nalu-Lab/shifu/releases/tag/remote-v0.4.5-preview.1)，内含 App、匹配 CLI 和安装说明。普通会话无需 Rust；另备 tmux 和已登录的 Agent 即可（从 0.4.5 起 Shifu 自带 tmux，第一屏点一下就能装缺的 Agent）。App 自带同版命令行，只拖进 `~/Applications` 就能用；要在终端里用 `shifu` 时再另装。第一次打开会出现一屏简短的引导：自动找出这台 Mac 上的 Agent 和常用代码文件夹里的项目，勾选项目后点「开始」，每个项目建一个任务并打开第一条对话。缺 tmux 或 Agent 时也在这一屏列出安装命令（0.4.5 起 tmux 内置，「安装」「登录」按钮在 Shifu 的终端标签里运行 Agent 的安装或登录）；以后可在 Shifu 菜单的「准备情况…」里再看。StepFun 需[单独设置 Node/Pi 和订阅密钥](STEPFUN.md)。

<!-- cli-step: optional (the app in the linked release carries its CLI; scripts/release/check-docs.py checks this against the release tag) -->

### 方式 A：让你的 AI 帮你装

把下面这段贴进 Claude Code 或 Codex：

```text
帮我安装 Shifu（https://github.com/Nalu-Lab/shifu）。先读 README 和 docs/INSTALL.md。
1. 确认当前 Mac 使用 Apple Silicon，检查 tmux 和我要使用的 Agent 是否已安装、登录；缺什么先告诉我。
2. 从 Release remote-v0.4.5-preview.1（https://github.com/Nalu-Lab/shifu/releases/tag/remote-v0.4.5-preview.1）下载 ZIP 和 SHA256SUMS，不要用别的 Release；核对下载文件及解压后的文件校验。默认使用现成 App 和 CLI，无需安装 Rust 或从源码编译。
3. 按包内说明，将 Shifu.app 放到 ~/Applications。App 自带同版命令行，对话状态、提醒和项目经理用它，不必另装；问我要不要在终端里用 shifu，要的话再将同包 bin/shifu 装到 ~/.cargo/bin/shifu，将 ~/.cargo/bin 加入当前终端 PATH，验证 shifu --help，并说明新终端可用完整路径运行。已有 Shifu 时先说明升级需要短暂退出，保留旧 App、CLI 和会话；不要覆盖运行中的 App。
4. 提醒我它用 Shifu 自己的证书签名、未经 Apple 公证；还要告诉我：第一次开对话前，Shifu 会问一次 Agent 的权限，默认选「按 Agent 自己的设置」；以后在新建任务的「权限」里随时能改。系统安全提示由我处理。StepFun、电脑操作、远程中继和后台托管仅在我要求时另行配置。
5. 打开后第一屏会找出 Agent 和项目：让我自己勾选项目、决定「免确认」开关（默认关，即「按 Agent 自己的设置」），再点「开始」；首次启动 Codex 时，让我检查它提示的 hooks 后决定是否信任。
```

### 方式 B：自己装

1. 下载同一 Release 的 ZIP 和 `SHA256SUMS`，先在下载目录执行 `shasum -a 256 -c SHA256SUMS`，再解压。
2. 已有 Shifu 时，先退出并备份旧 App 与 CLI。App 自带同版命令行（`Shifu.app/Contents/Resources/bin/shifu`），对话状态、提醒和项目经理用它；下面装 CLI 的命令只在终端里用 `shifu` 时才需要（0.4.4 及以前 tmux 仍要装）。先在解压后的包目录校验内容，再安装 CLI：

   先单独校验，看到每一行都是 `OK` 再继续：

   ```bash
   shasum -a 256 -c SHA256SUMS
   ```

   再安装命令行。这一段会先重新校验，校验不过就什么都不改；已有的 `web` 目录先改名备份，不直接删除：

   ```bash
   shasum -a 256 -c SHA256SUMS && (
     set -e
     mkdir -p "$HOME/Applications" "$HOME/.cargo/bin"
     install -m 755 bin/shifu "$HOME/.cargo/bin/shifu"
     if [ -e "$HOME/.cargo/bin/web" ]; then mv "$HOME/.cargo/bin/web" "$HOME/.cargo/bin/web.before-$(date +%Y%m%d%H%M%S)"; fi
     ditto bin/web "$HOME/.cargo/bin/web"   # shifu web 的运行文件
   )
   export PATH="$HOME/.cargo/bin:$PATH"
   brew install tmux   # 已有 tmux 时跳过；0.4.5 起不需要
   ```

3. 校验通过后，将 `Shifu.app` 移到 `~/Applications`，再打开 Shifu：第一屏会找出已装的 Agent 和项目，勾选项目后点「开始」就开出第一条对话（也可以「跳过」，之后在空白页点「添加项目」）。连接另一台 Mac 时才需要[远程安装与配对](REMOTE-RELAY.md)。

- 应用用 Shifu 自己的固定证书签名，尚未公证；首次打开若被拦，确认来源后按「系统设置 → 隐私与安全性」中的对应提示处理。
- 第一次启动 Codex 会提示检查 hooks，请阅读后决定是否信任。CLI 安装路径保持不变时，无需每次重复设置。
- PATH 设置只对当前终端生效。新终端尚未配置该路径时，可运行 `"$HOME/.cargo/bin/shifu"`，或自行将该目录加入 shell 配置。
- 只想用命令行：`shifu` 打开工作台，`shifu new claude ~/proj` 开一个 Agent，`shifu ls` 列出会话，`shifu agents` 看这台 Mac 上找到哪些 Agent CLI（自己的 CLI 可写进 `~/.shifu/app/agents.json`，见 [docs/AGENTS.md](AGENTS.md)），`shifu jump` 跳到等待处理的会话。

<details>
<summary>从源码构建（开发者）</summary>

需要 Rust，以及 Node 20.19 或 22.12 以上版本。

```bash
brew install tmux
git clone https://github.com/Nalu-Lab/shifu && cd shifu
cargo install --path .
cd app && npm install
npm run build:desktop -- --debug
# 按构建日志中的受管 .noindex App 路径打开
```

</details>

## 升级

从 0.4.4 起 Shifu 能在 App 里更新自己。0.4.3 及更早的版本还没有这个功能，要最后手动下载一次：按上面的方式 B 替换 `Shifu.app`，之后就在 App 里更新。

- 打开 Shifu 后它会悄悄查一次有没有新版（每天最多一次）。有新版时右下角出现一张小卡片，写着版本号和更新说明：点「更新并重启」就下载、核对签名、替换 `Shifu.app` 并重新打开；点「以后」先不更新。也可以随时点菜单 Shifu →「检查更新…」。
- 重启不打断工作：Agent 对话跑在 tmux 里，Shifu 退出时它们继续运行，重新打开后照常接上。
- 更新包只取 GitHub Releases 上已发布的预览版，下载后先用 Shifu 的更新密钥核对签名，不符就不安装，原来的 App 不动。App 仍用同一张 Shifu 证书签名，所以 macOS 里给过的权限保留。`Shifu.app` 要放在你有写权限的地方（如 `~/Applications`）；在 `/Applications` 时 macOS 会要管理员密码；直接从「下载」或解压目录里打开时，macOS 可能在只读的临时位置运行它，先把它移到 `~/Applications` 再更新。
- 一起更新的：App 自带的命令行（`Shifu.app/Contents/Resources/bin/shifu`，状态、通知和项目经理用它）；装过电脑操作组件（`~/Applications/Shifu Computer Use.app`）的，新版 Shifu 第一次打开时用 App 里同版本的组件替换它，上一份备份在 `~/.shifu/computer/backups/`，辅助功能和屏幕录制权限保留。签名不同的组件（自己从源码编的）不会被替换。
- 不会自动更新的：另装在 `~/.cargo/bin/shifu` 的命令行。在终端里用它的话，从同版本下载包里重新装一次（方式 B 第 2 步）。
- 不想让它自己查：在 `~/.shifu/app/settings.json` 里加上 `"update_check": false`；菜单里的「检查更新…」照常可用。

## 快捷键

| 键 | 作用 |
|---|---|
| Cmd+N | 新建任务 |
| Cmd+T | 在当前任务里加一个 agent |
| Cmd+W | 关闭当前面板（agent 不停） |
| Cmd+J | 跳到最急的在等你的 agent（先批准，再提问，再做完的；同类里等得最久的） |
| Cmd+, | 外观 |
| 双击标签 | 给这个 agent 改名，留空就用回话题名 |
| 点标签上的 × | 选择「收起，继续运行」或「结束」；运行中结束另有确认 |

## 隐私

- Shifu 不提供公共托管账号，不收集遥测。状态、任务、布局保存在本机 `~/.shifu/app`（命令行版在 `~/.shifu`）。本机手机入口监听 `127.0.0.1`；新远程模块可连接本人配置的 HTTPS 中继，端到端通道传输终端数据与操作请求，中继网页也应来自自己信任的服务器。[连接说明](REMOTE-RELAY.md)。
- Claude Code、Codex、CodeBuddy 的 hooks 通过启动参数挂在 Shifu 开的那个进程上，**不改**你的 `~/.claude/settings.json`、`~/.codex/config.toml`。
- **Kimi 例外**：Kimi 只认自己的配置文件，Shifu 会把 hooks 写进 `~/.kimi-code/config.toml`，你自己的 hook 原样保留，第一次写之前备份成 `config.toml.before-shifu`。在 Shifu 外面开的 Kimi 也会调用这些 hook，但它们发现不是 Shifu 开的就什么都不做。
- 额度显示：Claude 的额度从它交给 statusLine 的数据里取（你自己的 statusLine 照常显示）；Codex 和 CodeBuddy 的额度读本机会话记录；Kimi 的额度复用 Kimi Code 本机有效登录令牌查询，不另存该令牌；Antigravity 通过当前登录的 `agy` 读取用量。
- **Antigravity** 只从 `~/.gemini/config/hooks.json` 读 hooks，Shifu 在里面加一个 `shifu-status` 分组，其他分组原样保留；文件已存在时，第一次改动前备份成 `hooks.json.before-shifu`。
- Shifu 用自己的 tmux socket（`shifu-app`，命令行版是 `shifu`），不碰你原来的 tmux。

## 已知问题

- 只在 macOS（Apple Silicon）上用过。Linux 和 Windows 没试过，通知、拖放这些用的是 macOS 的接口。
- 终端里折成三行以上的长路径，点后面几行打不开。原因没查清。
- Kimi Code 登录过期时不能刷新额度；会保留上次结果，运行 Kimi 更新登录后再刷新。
- 手机外网连接需要另行配置私有网络。
- 另外接了两个可选的本机数据源：进度页读各项目的 `GOALS.md` 和 WorkBuddy 的本地任务；新回复通知前会读一个叫 Jev 的本机判断日志。没有这些文件时自动跳过。

## 一起做

**多个 agent 同时干活时，人怎么才能只在真正需要自己的时候被叫到？**

| 你喜欢…… | 可以贡献 |
|---|---|
| Rust、并发 | 把读分支名的 git 挪出锁外、加超时，让慢的 git 调用不会卡住整个窗口 |
| 终端、xterm.js | 修长路径折行后点不开；终端里的搜索 |
| 接新的 agent | Gemini CLI、Cursor CLI 等：写 hooks 适配，把状态接进侧栏 |
| 跨平台 | Linux 上的通知、拖放和打包 |
| 交互设计 | 「在等你」怎么排、什么时候该发通知、什么时候不该打扰 |

欢迎提 issue 或 pull request。

## 开发

打开 macOS 应用前可先核对配套命令行。分别运行 `"/path/to/Shifu.app/Contents/MacOS/shifu-app" --source-version` 和 `/path/to/shifu version`。两者的共享 Rust 源码一致时，输出的指纹相同。

```bash
cargo test && cargo clippy -- -D warnings           # 命令行和 hooks
cargo build --release && bash scripts/smoke.sh       # 冒烟：专用 socket、临时目录、假 agent
cd app && npm test && npm run build                  # 前端单测和类型检查
cd app/src-tauri && cargo test                       # 桌面版后端
cd app && npm run tauri dev                          # 开发模式
```

开发时设 `SHIFU_APP_HOME`、`SHIFU_APP_SOCKET`，测试数据就不会和正在用的混在一起。


StepFun 需额外配置 Node/Pi 和订阅密钥，见[StepFun设置](STEPFUN.md)。App与配套CLI应成对更新。后台托管脚本见[AUTOPILOT.md](AUTOPILOT.md)，不会因安装自动启动。

0.4.5 下载包另提供 opencode 与 Pi 两种 Agent（装了 `opencode` / `pi` 就出现在新建列表；用你自己的配置和账号，Shifu 不改它们的配置文件，详见[Agent 描述表](AGENTS.md)），装了就列出的 Aider、Goose、Amp、Cursor CLI、Factory Droid、GitHub Copilot CLI、Crush 和 `~/.shifu/app/agents.json` 里你自己的 CLI，以及[Qwen Code / Token Plan](QWEN.md)、[每日分析定时补跑](DAILY-ANALYSIS.md)和[工作区接续 / 会话恢复](WORKSPACES.md)。这些都已包含在上方下载包中。
