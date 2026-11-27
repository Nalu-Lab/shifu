# Agent 描述表：以后加一个 Agent 主要是加一行数据

[回到 Shifu](../README.md)

**目标：** 每个编程 Agent 命令行在 Shifu 里只用一行数据描述：叫什么、怎么找、怎么启动、怎么恢复、怎么回报状态。以前这些散在 `agent.rs`、`launch.rs`、`clis.rs` 的各个 `match` 里，加一个 Agent 要改五六处。这一轮引入描述表，并通过它加入 opencode 和 Pi。

## 表在哪里、有哪些列

`src/agents_spec.rs` 里的 `AGENTS`，用 Rust 常量写成，编译时就做类型检查，不需要额外读文件。顺序就是新建对话下拉菜单的顺序；第一个装了的 Agent 是新用户的默认。

| 列 | 意思 | 例子 |
|---|---|---|
| `id` | 记录里的 kind 名，`shifu hook <id> <事件>` 和 App 都用它 | `opencode` |
| `kind` | 对应的 `Kind`；`Kind::Other` 表示普通命令（没有状态 hook） | `Kind::Pi` |
| `commands` | 选中这个 Agent 的命令名，第一个是实际启动的程序 | `["kimi", "kimi-cli"]` |
| `detect` | 在 PATH 里找哪些程序来判断装没装（只看文件，不运行） | `["agy", "antigravity"]` |
| `label`、`install_url` | 显示名，「未安装」里的安装说明链接 | |
| `install_command`、`login_command` | 启动后马上退出时面板里给的安装、登录命令（没有就只给链接，见 `launch_failure.rs`） | `Some("codex login")` |
| `bypass_args` / `own_args` | 权限开关是「免确认」/「按 Agent 自己的设置」时加的参数 | `["--yolo"]` |
| `bypass_env` | 免确认要靠环境变量而不是参数时用 | 目前没有 Agent 用到；留给只认环境变量的 Agent |
| `model_args` / `effort_args` | 模型、思考档位的参数模板，`{model}`、`{effort}` 会被替换 | `["-m", "{model}"]` |
| `resume_by_id` / `resume_latest` | 按保存的会话 id 恢复；没有 id 时怎么恢复（空 = 重新开） | `["--session", "{id}"]` / `["--continue"]` |
| `hooks` | 状态怎么回报：`ClaudeSettings`、`Codex`、`KimiConfig`、`AntigravityHooksJson`、`PiExtension`、`OpencodePlugin`、`None` | |
| `launch` | `Direct`（`程序 [hook 参数] [权限参数] [模型] [恢复]`）、`Shifu`（经 `shifu stepfun` / `shifu qwen` 启动）、`Bespoke`（自己的代码） | |

读这张表的地方：

- `Kind::of_command`：命令名对应哪个 kind。
- `clis::detect`：哪些 Agent 装了（StepFun 另查 Pi 运行时，Qwen 另认 `SHIFU_QWEN_BIN`）。
- `launch::model_flags`、`agent_command`、`resume_command`：启动、恢复的命令行。

每一种现有 Agent 的启动、恢复命令都有逐字比对的单元测试，迁到表上之后输出一个字都没变。

## 还有哪些是自己的代码（以后可以继续迁）

- **Codex 的恢复**：用 `codex resume <覆盖> <免确认> <模型> <id|--last>` 子命令，参数顺序和 hooks 信任记录有关，留在 `launch.rs`。表里的 Codex 一行只记录了这些值，有测试保证两边一致。
- **各家写 hook 配置**：`commands::write_claude_hooks` 仍按 kind 分支（Claude/CodeBuddy 的 settings 文件、Kimi 和 Antigravity 改用户自己的配置文件、opencode/Pi 写到 `SHIFU_HOME`）。可以改成按 `hooks` 列分支。
- **hook 守卫**：`hook::foreign_caller` 的名单（antigravity、stepfun、qwen、opencode、pi）可以变成表里的一列。
- **状态显示的特例**：`Agent::effective_status`、`initial_tab_status` 里 Antigravity、Codex 的特殊处理。
- **App 一侧**：`app/src/models.ts` 的 `AGENTS`、`vendoricons.ts` 的标记、`sidebar.ts` 的 `KIND_NAMES`/`MARKED` 仍是手写列表；可以改为由 `clis_detect` 的结果提供名字和顺序。模型列表探测在 `app/src-tauri/src/models.rs`。
- **额度、用量、自动驾驶、对话转写搜索、指南注入**：只做了 Claude/Codex/Kimi 等几家，与表无关。

## opencode（`opencode-ai`，按 1.19.6 验证）

- **状态：** Shifu 把状态插件写到 `SHIFU_HOME/opencode/shifu-status.js`（每次启动重写，里面带当前 `shifu` 的路径），再在启动命令前加 `OPENCODE_CONFIG_CONTENT='{"plugin":["<那个文件>"]}'`。读了已安装包里的源码：opencode 依次加载全局配置、`OPENCODE_CONFIG`、项目配置、`.opencode` 目录，最后加载 `OPENCODE_CONFIG_CONTENT` 并合并进去，`plugin` 列表是追加的。所以你的 provider、模型、插件都照常生效，`~/.config/opencode` 和项目的 `.opencode/` 都不会被 Shifu 改。
- **事件对应：** 插件初始化 → `SessionStart`（空闲）；新建主会话 `session.created` → `SessionStart`（记下会话 id）；`session.status` 变忙 → `UserPromptSubmit`（运行中）；`session.idle` → `Stop`，带最后一条助手回复（回复以问句结尾时显示为「在问你」）；`permission.asked` → `PermissionRequest`（等你批准，显示权限名和命令）；`permission.replied` → `PermissionResult`；`question.asked` / `question.replied` → 提问工具；`session.error` 为中止时发 `Interrupt`，其他错误在 `Stop` 里带 `Error: …`。子代理会话（有 `parentID`）的事件不算。
- **恢复：** 有会话 id 用 `--session <id>`，没有用 `--continue`。**模型：** `-m provider/model`；App 用 `opencode models` 列出可选模型。
- **权限：** 「免确认」加 `--auto`：自动批准所有没被你明确 `deny` 的权限，你写的 `deny` 仍然有效。「按 Agent 自己的设置」什么都不加。opencode 默认本来就不问 bash，想要批准要在自己的配置里写 `"permission": {"bash": "ask"}`。
- **注意：** 如果你自己也设置了 `OPENCODE_CONFIG_CONTENT`，Shifu 启动的会话里它会被 Shifu 的这份替换。opencode 自己会往 `~/.config/opencode` 装 `@opencode-ai/plugin`（`node_modules`、`package.json`）；不经 Shifu 直接开 opencode 也一样，不是 Shifu 写的。`--pure` 会关掉外部插件，状态也就没了。

## Pi（`@mariozechner/pi-coding-agent`，命令 `pi`，按 0.73.1 验证）

- 用你自己装的 `pi`、自己的 provider、扩展和设置。Shifu 不设 `PI_CODING_AGENT_DIR`，不加 `--no-extensions`，只加一个 `--extension SHIFU_HOME/pi/shifu-status.mjs`。
- **状态：** 和 StepFun 的扩展同一套：`session_start` → `SessionStart`，`agent_start` → `UserPromptSubmit`，`agent_end` → `Stop`（带最后的回复，出错时带错误），只有真正退出（`reason: quit`）才发 `SessionEnd`，`/new`、`/resume` 换会话时跟着新的会话文件走。
- **恢复：** 扩展报告的会话 id 就是会话文件路径，恢复用 `--session <文件>`；没有时用 `--continue`。**模型：** `--model provider/model`，思考档位 `--thinking`；App 用 `pi --offline --list-models` 列出有密钥的模型。
- **权限：** Pi 没有批准提示，权限开关对它不起作用（两种设置启动命令完全一样）。

## 两边共同的保护

- 插件和扩展只在 `SHIFU_ID` 存在时回报；在 Shifu 外面用同一个 opencode/Pi 不受影响（它们本来也不会加载这些文件）。
- Agent 在自己的 shell 工具里再开一个 opencode/pi 时，子进程会继承 `SHIFU_ID`。插件第一次加载时设 `SHIFU_STATUS_OWNER=<自己的 pid>`，子进程看到别人的 pid 就不回报，不会把父会话的 id 换掉。
- `shifu hook opencode|pi …` 只改同类的记录（`hook::foreign_caller`），比如 Claude 里跑的 opencode 不会改 Claude 对话的状态。

## 以后：让用户自己声明普通 Agent（hooks: none）

Gemini CLI 已经是表里的一行普通 Agent（`kind: Other`、`hooks: None`）：能被找到、出现在列表、按命令启动，侧栏在它运行期间显示运行中，退出后重新打开。下一步可以让用户在 `SHIFU_HOME/agents.toml` 里写同样的几列（名字、命令、安装链接、模型参数模板、恢复参数），App 启动时读进来接在内置表后面：

- 只允许 `hooks = "none"`，因此不需要运行用户提供的代码，也不需要新的 hook 类型；
- 命令名不能和内置的重名，参数都经过 shell 引号处理；
- `kind` 都存成 `other`，记录里另存 `command`，恢复时按用户写的 `resume_latest` 重开。

## 暂缓：zcode

zcode 先不加：还不知道它有没有能挂状态的 hook / 插件机制，也不知道怎么按 id 恢复会话。等这两点有了文档或源码可查，再按上面的列补一行；在那之前，可以作为用户声明的普通 Agent 使用。

## 测试记录（2026-11-23）

在隔离的 `HOME`、`XDG_*`、`SHIFU_HOME` 和独立 tmux socket 里，用本地模拟的 OpenAI 兼容接口（127.0.0.1，流式回复；一条回复会调用 bash）跑真实的 opencode 1.19.6 和 Pi 0.73.1：

- opencode：启动 → 空闲；发消息 → 运行中 → 回复完成（带最后一句）；以问句结尾 → 在问你；「用自己的设置」+ 用户配置 `bash: ask` → 等你批准（`bash：echo shifu-mock`）→ 批准后完成；退出后 `shifu resume` 用同一个会话 id 恢复，模拟接口收到之前的全部对话；「免确认」下同样的 bash 不再询问；嵌套的 `opencode run` 不改父会话。
- Pi：用户自己的扩展照常加载；启动 → 空闲（记下会话文件）→ 运行中 → 完成 → 在问你；`/quit` → 已退出；`shifu resume` 用 `--session <文件>` 恢复原对话。
- 真实的 `~/.config/opencode`、`~/.local/share/opencode`、`~/.pi`、`~/.shifu` 没有新写入。
