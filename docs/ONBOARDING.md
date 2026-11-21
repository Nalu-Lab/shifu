# 首次使用：照用户这台 Mac 的实际情况来

[回到 Shifu](../README.md)

**目标：** Shifu 第一次打开时，按用户自己的终端环境找出已经装好的 Agent，而不是摆出一份写死的清单和预设参数。这一做法参考了 Orca 1.4.258 的首次引导（2026-11-21 读过它的 app.asar）。

## Orca 怎么做

- **找 PATH：** 用 `$SHELL -ilc` 向用户的登录 shell 要 PATH，再补上常见的安装目录：Homebrew、nvm（每个 Node 版本一个）、volta、asdf、mise、pnpm、yarn、bun、nix、`~/.local/bin` 等。
- **判断装没装：** 在 PATH 里逐个查程序文件是否存在、是否可执行，不运行它们，不查版本，也不查登录状态。版本过低等问题等到真正启动 Agent 时才报。
- **分两组显示：** 找到的放在「Detected on your system」，没找到的收在「Show N more」里；每个都有安装说明链接。默认 Agent 取找到的第一个，顺序是 claude、codex、…。
- **什么时候查：** 首次引导时查一次，打开选择器时也查（有缓存）；状态栏另有手动刷新。
- **引导步骤：** Agent 打开方式 → 默认 Agent，并附带一个「Yolo 全权」开关（默认开，关掉就用 Agent 自己的设置）→ 主题（检测到 Ghostty 配置时可一键导入）→ GitHub 登录 → 通知（带测试通知）→ 添加第一个项目，加好后自动用默认 Agent 开第一条对话。
- **Agent 自己的设置：** Orca 不读也不覆盖 Agent 自己选的模型（不加 `--model`），只把状态 hooks 写进各家的配置。

## Shifu 原来的做法

- 新建对话的选择器和空白页总是列出全部 7 个 Agent，不管装没装。
- App 从 Finder 启动时只有系统最小的 PATH，代码里补的是一份固定目录，用 nvm、volta、bun 等装的 CLI 找不到。
- 所有 Agent 一律用免确认参数启动（`--dangerously-skip-permissions` 等），见 `src/launch.rs`。
- 第一次打开没有引导，只有一个「新建任务」按钮。

## 第 1 阶段：找出本机已装的 Agent（已完成，分支 `feat/detect-local-clis`）

- `src/clis.rs`（共用库）：
  - 向登录 shell 要 PATH，10 秒超时，超时就结束那个进程。
  - 列出常见安装目录，nvm 的按版本新到旧排。
  - 只看程序文件是否存在、可执行，什么都不运行。StepFun 要同时有 node 和 Shifu 的 Pi 运行时；Qwen 认 `SHIFU_QWEN_BIN`。
- `app/src-tauri/src/clis.rs`（App）：
  - **启动时的 PATH：** 依次是上次缓存的登录 shell PATH（`~/.shifu/app/shell-path`）、App 继承到的 PATH、常见安装目录。
  - **读新的 PATH：** 后台线程重新问一次登录 shell，拿到后更新缓存，并发 `clis` 事件给页面。
  - **App 开的会话：** tmux 会话现在带上 `-e PATH=…`，所以找得到的 Agent 也一定启动得了。
- 页面：
  - 选择器和「+」菜单只列已装的 Agent。
  - 没装的列在「未安装：…」里，点名字打开安装说明。
  - 记住的 Agent 没装时，默认改用第一个已装的。
  - 空白页说明这台 Mac 上找到了哪些 Agent；没有 tmux 时先提示 `brew install tmux`。
  - 回到窗口时重新检查一次，所以刚装的 CLI 能直接出现。

## 第 2 阶段：权限用预设还是用 Agent 自己的设置

- 加一个设置项：「免确认（全权）」或「用 Agent 自己的设置」。
- 默认保持免确认，这是 11-09 的决定：和 Orca 一样默认全权。
- 选「用 Agent 自己的设置」时，`launch.rs` 不加免确认参数，模型也照旧不传，交给各家的配置。
- 设置放在 App 的偏好里，CLI 的 `shifu new` 读同一个文件。

## 第 3 阶段：首次引导

- 第一次打开时显示一屏引导：
  1. 找到的 Agent，选一个默认（默认取第一个找到的）。
  2. 权限开关（第 2 阶段）。
  3. 通知：发一条测试通知，顺便触发 macOS 的授权。
  4. 选第一个项目文件夹，建好任务后直接用默认 Agent 开第一条对话。
- 可以跳过；跳过后，第一次建任务仍然自动开默认 Agent。

## 第 4 阶段：启动失败时说清楚

- Agent 启动后立刻退出时（没找到命令、版本太旧、没登录），在面板里说明原因和对应的安装或登录命令，而不是只留下一个已退出的面板。
