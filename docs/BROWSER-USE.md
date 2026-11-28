# Shifu Browser Use

`shifu browser` 为 Shifu 中的 Agent 提供网页 DOM 快照、元素引用、点击和输入、iframe、标签页、截图及本机持久化登录。执行器是固定版本的微软官方 [Playwright CLI](https://github.com/microsoft/playwright-cli)，不依赖 Orca，也不修改个人 Chrome 配置。

网页优先走此入口；菜单、文件选择框及其他 macOS 窗口使用 [`shifu computer`](COMPUTER-USE.md)。操作入口不扩大当前任务的发送、上传、交易或前台操作授权。

## 安装与诊断

需要 Node.js 20+、npm，以及本机 Chrome。运行时在第一次使用时自动安装（下载包和 App 自带的 CLI 也一样，不需要仓库）；也可以先装好：

```bash
shifu browser install    # 或在 Shifu 菜单「准备情况…」里点浏览器的「设置」
shifu browser doctor
shifu browser guide
```

没有 Node 时，`open` 等命令返回 `runtime_missing`，提示先装 Node.js 20+ 再运行 `shifu browser install`。在仓库里也可以运行 `bash scripts/install-browser.sh`，效果相同。

安装按 `browser/package-lock.json`（编进 CLI 里的同一份）安装依赖，将运行所需的 Node 可执行文件、许可和依赖放在 `~/.shifu/browser/runtime`；后续执行不依赖系统 PATH 中的 Node，不在操作时通过 npx 下载代码。再次安装会备份旧运行时。更新依赖前请先逐个关闭自己的浏览器会话；依赖升级可能令旧 daemon 不兼容。

`doctor` 只检查 Node 版本和运行时，不打开浏览器、不连接已有标签、不读取登录信息。它不证明 Chrome 已安装或某个网页可达；浏览器启动失败会由 `open` 返回。`guide` 和 `help` 在运行时缺失时仍可读取。

## 专用浏览器

```bash
# 默认在后台运行无窗口的专用浏览器并保存专用登录状态；不弹窗、不抢前台，不改用或关闭日常 Chrome。
shifu browser open https://example.com
# 本人要亲自登录或旁观时才显示窗口；临时任务可加 --ephemeral，不保留登录。
shifu browser --session login open https://example.com/login --show
shifu browser --session temporary open https://example.com --ephemeral
shifu browser --session research open https://example.com
shifu browser --session research snapshot
shifu browser --session research click e3
shifu browser --session research fill e5 --text-stdin
shifu browser --session research tab-list
shifu browser --session research tab-new https://example.org
shifu browser --session research tab-select 0
shifu browser --session research screenshot
shifu browser --session research close
```

### 不打断本人

本人和 Agent 共用这台 Mac。默认的无窗口模式不出现窗口、不切换前台、不移动鼠标，快照、截图、点击和输入照常可用；旧脚本里的 `--headless` 仍被接受，效果同默认。无窗口会话闲置一小时后由 Playwright 自动关闭，登录状态保存在配置里，下次 `open` 继续可用。

`--show`（同官方的 `--headed`）只在本人要亲自登录、输入验证码或旁观时使用。Chrome 没有“启动时不激活”的选项，启动和开新标签时它会把自己切到前台；Shifu 在这类会话的每条命令执行期间每 0.1 秒查看前台，发现是自动化启动的 Chrome（带 `--remote-debugging-pipe` 的进程，不会是本人日常 Chrome）抢走了前台，就马上还给原来的应用，窗口留在后面。本人在命令执行期间用过键盘或鼠标时不还，以免把本人刚点开的窗口抢走。返回 JSON 的 `focus` 记录还了几次（`foregroundReturned`）、失败几次、是否留给了本人（`leftToPerson`）。本人要看窗口时从程序坞或 Cmd-Tab 切过去。

可见窗口和无窗口共用同一份专用配置。登录流程：`open 登录页 --show`，本人登录，`close`，之后不加 `--show` 再 `open`，登录状态照旧可用；自己指定的 `--profile 目录` 同样适用。少数网站会识别并拒绝无窗口浏览器，此时只能改用 `--show`。

`e3`、`e5` 必须替换为本会话最新快照中的真实引用。页面变化、导航和切换标签之后重新观察。快照可能是内联的节点数组，也可能是 `snapshot.file` 与新增的绝对 `snapshot.path`；后一种需要调用者先读该文件。`tab-list` 保留官方 CLI 的文本结果，位于 JSON 的 `result.result` 中。

截图不返回 base64；`result.screenshot.path` 是绝对本地路径，Agent 必须使用自己的看图工具打开，拿到文件路径不代表已经看到图片。默认生成的快照和截图在本会话的 `.playwright-cli` 下。

其他命令包括 `find`、`type`、`press`、`select`、`check`、`uncheck`、`hover`、`drag`、`upload`、`go-back`、`go-forward`、`reload`、`console`、`requests`、`eval` 和 `run-code`。运行 `shifu browser help <命令>` 读取固定版本的官方参数，不猜参数或批量盲目执行。

`--text-stdin` 用于 `fill <引用>`、`type`、`eval`、`run-code`，放在命令最后，输入上限 1 MiB。文字会交给浏览器执行器，可能出现在其操作记录和页面内容中，不适合当作秘密存储。相对的上传、配置和输出路径以调用者当前目录解析，而不是内部的会话目录。

## 已有浏览器与登录状态

如需控制日常 Chrome 的现有标签，先由本人通过浏览器调试入口或官方 Playwright 扩展授权，再显式连接：

```bash
shifu browser --session existing attach --cdp=http://127.0.0.1:9222
# 或由本人安装、授权 Playwright 扩展后使用：
shifu browser --session existing attach --extension=chrome
shifu browser --session existing tab-list
shifu browser --session existing detach
```

Shifu 不自动打开 Chrome 的远程调试，不复制个人配置或导出凭据。Chrome 默认配置的调试限制见 [Chrome 官方说明](https://developer.chrome.com/blog/remote-debugging-port)；不要为连接关闭其保护。扩展与浏览器的连接批准由本人完成，不代点。`detach` 只断开控制，外部浏览器继续运行。

默认的专用持久化配置与连接已有浏览器是两种明确的模式。`--ephemeral` 只使用内存配置，关闭浏览器后不保留其登录状态；它不能与 `--persistent` 或 `--profile` 同用。

## 会话、响应和结果不明

- 默认会话名来自 `SHIFU_ID`，外部终端默认 `default`。`--session 名字` 放在命令前；会话名不能是路径。不同 Agent 的会话、配置和输出目录独立，切换项目目录不会丢失当前浏览器。
- 每个会话在 `~/.shifu/browser/sessions/<名字>` 下拥有私有工作区。同一会话命令由文件锁互斥；并发操作返回 `session_busy`，不会换会话或自动重试。不暴露全局 `kill-all`、`close-all`，不清理其他 Playwright 工作区。
- 操作统一输出 JSON，`ok` 同时核对进程退出状态和官方 `isError` 标记。保留原始结果，不把命令成功当作上传、保存、发送或支付完成。
- 等待上限默认 45 秒，可在命令前设置 `--timeout-ms 1000..120000`。超时只停止等待端；页面动作可能已经发生或仍在执行。返回 `result_unknown`、`automaticRetry:false`，先观察再决定下一步。不要重发点击、Enter 或提交。
- `SHIFU_BROWSER_HOME`、`SHIFU_BROWSER_RUNTIME` 和 `SHIFU_BROWSER_NODE` 可显式指向隔离的绝对路径，用于开发或故障检查。Playwright 自己的 daemon 元数据保留在其标准本机缓存中；Shifu 的登录配置与页面产物不放进项目仓库。

## 本机复查

```bash
python3 scripts/verify-browser.py /tmp/shifu-browser-new --cli /absolute/path/to/shifu
```

目录必须尚不存在。脚本只启动自己的无头会话和临时 localhost 页面，核对中文输入、点击回读、失效引用、iframe、标签、截图、会话隔离及专用配置重开；结束时逐个关闭自己的会话。原始响应、截图和 `report.json` 保留在指定目录，不操作日常浏览器。这是脚本驱动的本地功能检查，不代表 Agent 在任意网站上的任务成功率。

前台切换、鼠标移动和剪贴板改动次数用 `scripts/verify-quiet.py` 统计，方法和结果见 [Computer Use 的“不打断本人”](COMPUTER-USE.md#不打断本人)。
