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

## 第 2 阶段：权限用预设还是按 Agent 自己的设置（已完成）

- 加一个设置项：「免确认（全权）」或「按 Agent 自己的设置」。
- 当时默认免确认（11-09 的决定，和 Orca 一样默认全权）；11-27 起全新安装改为先问，见第 5 阶段。
- 选「按 Agent 自己的设置」时，`launch.rs` 不加免确认参数，模型也照旧不传，交给各家的配置。
- 设置存在 `<SHIFU_HOME>/settings.json` 的 `agent_permissions`（`bypass` 或 `own`）。App（`~/.shifu/app`）和终端版（`~/.shifu`）各有一份；App 在新建任务窗口的「权限」里改。
- 每次新开和恢复对话时读取，改了之后对已经在跑的对话不生效。Qwen 选 `own` 时传 `--approval-mode default`。

## 第 3 阶段：首次引导（已完成，分支 `feat/onboarding-guide`）

- 第一次打开时（还没有任务，且 `<SHIFU_HOME>/settings.json` 里没有 `guide_done: true`）盖上一屏引导，见 `app/src/onboarding.ts`：
  1. 默认 Agent：下拉列出找到的 Agent，默认取第一个；下面和空白页一样说明找到了什么、没装的给安装链接、没有 tmux 时提示安装。检测结果晚到或回到窗口时会重画。
  2. 权限：就是第 2 阶段新建任务窗口里那个选择，改了立刻保存。
  3. 通知：「发一条测试通知」走 App 平常发通知的路径，第一次会让 macOS 询问授权；没看到时提示去系统设置里打开。
  4. 第一个项目：选文件夹后「建立任务并开始」，用默认 Agent 建任务，第一条对话直接打开。
- 「跳过」或建好任务后写入 `guide_done`，以后不再出现。
- 默认 Agent 存在 localStorage（`shifu.defaultKind`），新建任务窗口也从它开始、改了也存回去，所以跳过后第一次建任务仍然直接开默认 Agent，重启后也记得。

## 第 4 阶段：启动失败时说清楚（已完成，分支 `feat/explain-launch-failure`）

- Agent 启动后立刻退出时（没找到命令、版本太旧、没登录），在面板里说明原因和对应的安装或登录命令，而不是只留下一个已退出的面板。
- **怎么算「启动后马上退出」**（`src/launch_failure.rs`）：记录还是启动时写的状态（没有任何 hook 改过它），而且 tmux 报的窗格退出时间（`pane_dead_time`）离启动不到 10 秒。退出的窗格因 `remain-on-exit` 还在，从里面读最后的输出。
- **分哪几种：**
  - 退出码 127 或输出里有 `command not found`（zsh、sh、bash 的原话）：没装，给安装命令。
  - 输出里有 not logged in、unauthorized 这类字样：没登录，给登录命令。
  - 输出最后有 Node 自己打的 `Node.js vX`：多半是 Node.js 太旧（在 Node 18 上跑 gemini-cli 实测就是这样结束）。
  - 其他：「启动后马上退出了」，附退出码和最后 8 行输出。
- **命令写在 `src/agents_spec.rs`：** `install_command`（Claude 用官方安装脚本，其余用 npm 包名，2026-11-23 核对过包里的命令名；Kimi、Antigravity、StepFun 没有一条命令，只给安装说明链接）和 `login_command`（只有 `claude auth login`、`codex login` 两条是真有的命令）。
- **实测：** 这台 Mac 上的 Claude、Codex、Antigravity、Qwen Code 在没登录时都不退出，而是自己显示登录界面，所以「没登录」这一类主要留给以后真会退出的 Agent。
- **面板：** 原因、可复制的命令、安装说明链接、最后的输出，按钮从「恢复会话」换成「重试」。重试按第一次的方式重新启动（`retry_agent`，不加 `--continue`，以免接上同一文件夹里别的对话），并先清空窗格，下次失败只读这次的输出。

## 第 5 阶段：第一次开对话前问权限（已完成，分支 `improve/firstrun`）

- **为什么：** 外部评审指出，全新安装一上来就用 `--dangerously-skip-permissions` 这类参数启动 Agent，用户没有机会知情。
- **全新安装：** 第一个 Agent 启动前盖上一个问题（`app/src/permissions.ts`）：「按 Agent 自己的设置」默认选中、标为推荐；「免确认（全权）」要自己选，下面一行写明风险。只有「继续」能关掉它，选择存进 `agent_permissions`，以后在新建任务窗口的「权限」里照旧能改。引导第 2 步显示的权限，点「建立任务并开始」时就算回答过，不再问第二次。
- **怎么拦：** `api.ts` 里所有会启动 Agent 进程的调用（新建任务、加 Agent、项目经理、日报、常驻 Bot、恢复、重试）先过 `setBeforeAgentStart` 装上的关卡；普通终端不问。还没回答时，不经窗口启动的路径（手机遥控、终端版）一律按 `own` 启动，不会默认全权。
- **老用户不受影响**（`src/launch.rs` 的 `settle_permissions_in`）：App 启动时第一件事、在往 `~/.shifu/app` 写任何东西之前判断一次：
  - 已经存了 `agent_permissions`：照用。
  - 没存，但目录里已经有东西（对话记录、`settings.json`、`shell-path` 缓存、任务等，旧版运行过就会有）：说明用过旧版，而旧版没存设置时就是免确认，于是写入 `agent_permissions: "bypass"`，以后也不问。
  - 目录不存在或是空的（隐藏文件不算）：全新安装，写入 `permissions_ask: true`，之后读到它就按 `own` 并等窗口来问；回答后删掉这个标记。标记写好以后，引导、缓存、对话记录再往目录里写东西，也不会被误当成老用户。
  - `settings.json` 读不出来：按以前的免确认，不覆盖那个文件。
- 终端版（`~/.shifu`）同样处理，`shifu new` 在写任何文件之前先判断；`~/.shifu` 里只有 App 的 `app/` 目录不算用过终端版。

## 第 6 阶段：准备情况清单（已完成，分支 `improve/firstrun`）

- **目标：** 新用户不用开终端排查，就知道第一个 Agent 还差什么。见 `app/src/setupcheck.ts`。
- **列什么：** tmux；这台 Mac 上找到的 Agent（沿用第 1 阶段的检测，只看文件）；一个都没找到时，列出全部核心 Agent（Aider 等额外的普通 Agent 和 agents.json 里自己加的只在找到时出现，见 [AGENTS.md](AGENTS.md)），各带可复制的安装命令（`agents_spec` 的 `install_command`）和安装说明链接。装了至少一个以后，其余的只留一行安装链接。
- **登录：** 只看各家自己存登录信息的文件，不运行程序、不读出密钥（`shifu::clis::signed_in`）：
  - Claude Code：`~/.claude.json`（或 `$CLAUDE_CONFIG_DIR/.claude.json`）里有 `oauthAccount`，或有 `ANTHROPIC_API_KEY`；
  - Codex：`~/.codex/auth.json`（或 `$CODEX_HOME/auth.json`）存在；
  - Gemini CLI：`~/.gemini/oauth_creds.json` 存在，或有 `GEMINI_API_KEY`。
  - 其他 Agent 的登录在钥匙串或各自的存储里，没有可靠又便宜的判断办法，就不显示登录状态，不猜。
  - 没找到登录信息只显示提醒（「第一次开对话时按它的提示登录」，有登录命令的附上命令），不算没准备好：这些 Agent 没登录时会在自己界面里请你登录（第 4 阶段实测）。
- **在哪里看到：** 引导第 1 步；空白页（没准备好时显示清单，准备好后只说找到了什么）；没有引导时，启动后若缺 tmux 或一个 Agent 都没有，自动弹出；Shifu 菜单「准备情况…」随时打开。回到窗口或点「重新检查」会重新检测并刷新。
- **还做不到：** tmux 和 Agent 本身仍要在终端里装（清单给出确切命令和链接）；没有 Homebrew 时要先按 tmux 安装说明装。

## 第 7 阶段：一屏扫描、勾选、开始（分支 `feat/first-run-scan`）

- **起因：** 朋友装好后建不了工作区。没有任务、也没有最近文件夹时，新建任务窗口的「项目」下拉只有「选择文件夹…」一项且已选中，`change` 不会触发，文件夹对话框打不开；引导里点「跳过」就走进这个死路。
- **死路修好：** 没有可列的文件夹时，「项目」处换成一个真正的「选择文件夹…」按钮；没选文件夹时点「建立」直接打开对话框，选好就建。空白页（没有任务时）只有一行「还没有项目」和「添加项目」按钮：打开对话框，选好就用默认 Agent 建任务。
- **引导改成一屏**（`app/src/onboarding.ts`），像 Orca：自动查找 → 列出找到的 → 勾选 → 开始。
  - Agent：沿用第 1 阶段检测，找到的每个是一个可选的小块，选中的就是默认 Agent；没找到登录信息的只加小标签「需登录」。没装的收在「更多 Agent」里。一个都没有时一行提示加安装链接；没有 tmux 时一行「需要 tmux」加可复制的 `brew install tmux`。
  - 项目：`app/src-tauri/src/projectscan.rs` 在 ~/Developer、~/Projects、~/code、~/src、~/dev、~/GitHub、~/repos、~/workspace 下找，最多三层、约 2 秒，只看名字（`.git` 或常见项目文件，如 package.json、Cargo.toml、pyproject.toml、go.mod、*.xcodeproj）和修改时间，不读文件内容；跳过隐藏目录、node_modules、Library、target 等。按最近修改排序，最新的 5 个默认勾上；「其他文件夹…」可以手选。
  - 桌面、文稿、下载受 macOS 保护，App 第一次列出其中内容时系统会弹窗请求授权。为了不在打开时突然弹窗，这三个不自动找，只在存在时给一个「查找桌面和文稿」，点一下才找（系统弹窗由这一下引起，用户知道为什么）。
  - 权限：一个「免确认」勾选框，默认不勾（即「按 Agent 自己的设置」，第 2、5 阶段的语义不变）。只在点「开始」时保存；点「跳过」不保存，第一次开对话前仍按第 5 阶段问一次。默认 Agent 也只在「开始」时保存。
  - 通知测试从主流程拿掉：macOS 在第一条真实通知时会自己询问授权。
  - 「开始」给每个勾选的项目建一个任务（同一条 `task_new` 路径），都用默认 Agent，然后打开第一个。
- **文字：** 一般情况下点「开始」前要读的中文约 40 字以内。
- **语言：** 引导、新建任务窗口、空白页、准备情况和权限提问跟随系统语言（`ui_chinese`，与菜单同一规则，`app/src/i18n.ts` 的 `tr(zh, en)`），其余界面仍是中文。
