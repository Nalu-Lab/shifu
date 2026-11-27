# Shifu 官网（site/）

Shifu 的第一个官网：一页静态站，介绍产品、给出下载入口和如实的安装说明。目前 Shifu 对外只有 README，这个站把 README 的事实整理成面向新访客的页面。

## 目录

```
site/
├── index.html        中文页（生成，勿手改）
├── en/index.html     英文页（生成，勿手改）
├── src/index.html    双语源文件：两种语言都写在这里（data-l="zh" / data-l="en"）
├── tools/build.py    从源文件生成两张页面；版本号等发布信息只在这里改
├── styles.css        全部样式（无预处理）
├── main.js           语言偏好记忆、复制安装指令、长代码折叠、复制按钮、滚动淡入；不加载也能正常阅读（安装指令默认展开）
├── README.md         本文件
└── assets/           图片与图标（每张截图都有 -zh / -en 两版，尺寸相同）
    ├── main-*.webp       首屏：主窗口（桌面）
    ├── approval-*.webp   Codex 请求批准 git push（桌面「在等你」右图；手机首屏）
    ├── waiting-*.webp    侧栏到「在等你」为止（桌面）
    ├── waitlist-*.webp   只有「在等你」列表（手机）
    ├── pane-*.webp       一个 Claude Code 终端（「真终端」）
    ├── newtask-*.webp    新建任务弹窗连同周围（桌面）
    ├── popover-*.webp    只有新建任务弹窗（手机）
    ├── video/            首屏演示视频：hero-{zh,en}.mp4/.webm（1440×924）、hero-m-*（手机竖版裁切 480 宽）、各自的 poster
    ├── og.png            分享预览图（docs/images/shifu-social-preview.png 原样复制）
    └── logo-96.webp / favicon-32.png / apple-touch-icon.png   来自 app/src-tauri/icons/
```

### 改内容、发新版

1. 改 `src/index.html`（文字、结构）或 `tools/build.py` 顶部的 `RELEASE`（版本号、Release 标签）和 `DEMO`（视频、截图是哪个版本录的）。
2. **安装步骤不在这里改。**「让 Agent 帮你装」的安装指令、「手动安装」的步骤和「装之前要知道」里的权限说明，都由 `build.py` 从 `docs/INSTALL.md` / `docs/INSTALL.en.md` 读出来生成（方式 A 的 `text` 代码块、方式 B 的编号列表、`<!-- permission-mode -->` 标记之间的一段）。改安装说明就改这两份文档，再重新生成。安装指令里写明了 Release 标签和网址，必须等于 `RELEASE["tag"]`，否则构建和 `check-docs.py` 都失败；发新版时一起改文档和 README 里的这一句。
3. 运行 `python3 site/tools/build.py`（只用 Python 标准库），重新生成 `index.html` 和 `en/index.html`。
4. 运行 `python3 scripts/release/check-docs.py`，确认版本、下载链接、安装步骤、生成页面和演示说明一致。两张生成的页面一起提交。托管时不需要构建步骤。

演示脚注由 `build.py` 的 `DEMO_NOTE` 生成：`DEMO["version"]` 和 `RELEASE["version"]` 不同时写「录制于 0.4.2 …；当前下载是 0.4.3」；重录后把 `DEMO["version"]` 改成发布版本，自动换成「用 Shifu {版本} 在演示项目里录制。是否出现批准请求，取决于各 Agent 的权限设置。」。`DEMO["overlays"]` 为真时再加一句「通知横幅和按键提示是后期叠加的示意」。

### 发布前的一致性检查

`scripts/release/check-docs.py`（只用标准库）在这些情况下失败：

- 版本文件（`Cargo.toml`、`app/package.json`、`app/package-lock.json`、`tauri.conf.json`、`app/src-tauri/Cargo.toml`、`remote/Cargo.toml` 和三个 `Cargo.lock` 里的 shifu 包）与发布版本不同；
- README、`docs/INSTALL*.md`、两张网站页面的下载链接指向别的 Release 标签；`build.py` 的 `RELEASE` 不是这个标签；
- README 的「方式 A / 方式 B」和 `docs/INSTALL*.md` 不是同一段文字；
- 安装说明对「是否必须另装 CLI」的说法和这个版本的 App 不符：App 自带 CLI（`tauri.conf.json` 打包了 `cli-bundle/`）时只能说可选，否则必须说要装。文档用 `<!-- cli-step: required|optional -->` 标明，网站安装区带同样的 `data-cli-step`；包内说明（`scripts/release/INSTALL.*.md`）跟随当前源码；
- 网站页面不是用当前源文件和文档生成的；
- `RELEASE["permission"]` / `own_label` 和 `docs/INSTALL*.md` 权限段对这个版本的说法不一致，或生成页面的首屏权限说明没写这个版本；
- 演示脚注没写发布版本，或写了 `RELEASE` / `DEMO` 以外的版本号。

本地：`python3 scripts/release/check-docs.py`（按 `build.py` 的 `RELEASE` 检查；判断 CLI 时读该标签的 `tauri.conf.json`，本地没有标签就用当前源码并提示）。发布工作流应在推标签后、构建前运行 `--tag "$GITHUB_REF_NAME"`，PR 检查也运行一次（这两处工作流改动需要有 `workflow` 权限的人提交，内容见引入本脚本的 PR 描述）。所以发版顺序是：改版本号、`RELEASE`、README / INSTALL 下载链接 → 生成网站 → 提交 → 推标签。

### 语言与网址

- `/` 是中文，`/en/` 是英文，各自是完整的静态页面：自己的 `<title>`、description、og 标签、`lang`、canonical，并用 hreflang 互相指向（x-default 指中文）。
- 右上角的切换是普通链接，跳到另一种语言的页面，同时把选择记在 localStorage（`shifu-lang`）。页面语言以网址为准，不根据记录或浏览器语言自动跳转，分享出去的链接打开的就是那种语言。
- 源文件里 `data-zh-alt` / `data-en-alt`、`data-zh-aria-label` 之类的属性，生成时变成对应语言的 `alt` / `aria-label`；图片路径里的 `-{L}` 换成 `-zh` / `-en`。

### 首屏视频怎么做的

15.7 秒、无声、首尾交叉淡化成无缝循环。故事：三个 Agent 分屏工作 → Codex 请求批准 `git push`，侧栏把它挪到「在等你」并出现通知横幅 → ⌘J 跳过去 → 回车批准，Codex 继续，侧栏回到运行中 → Claude Code 做完，侧栏显示 ✓ 做完了。

- **真实录制**：侧栏、分屏、状态变化、终端输出都是隔离演示实例的真实画面（`screencapture -l <窗口 id>` 后台连续截帧，约 20 帧/秒，重采成 30 帧）。状态变化靠改演示目录里的 agent 记录；终端是 tmux 里的小脚本按提示文件逐行打印；「批准」是向演示 socket 的 Codex 窗格 `tmux send-keys Enter`。没有任何输入进入桌面，演示窗口也没有抢焦点。
- **后期合成**：macOS 通知横幅、「⌘J 跳到在等你」「↵ 批准」按键提示、橙色焦点框，都是 Playwright 渲染的透明 PNG，用 ffmpeg 叠上去。演示实例本身不会发系统通知：它用一个未注册的 bundle id（通知调用直接失败），而且启动前把两条将要出现的等待状态预先标成「已通知过」。
- **左上角品牌名**：非正式 bundle 的实例会自称「Shifu 测试」，合成时用同一版本早先截图里的「Shifu」小块盖住，除此之外画面未改动。
- 底部用量栏（本机真实额度）被裁掉。
- 重做：`site/tools/video/` 里的脚本（需要先按「截图怎么来的」编出 App 和 CLI）：`run4.py zh|en` 布置并录制，`overlays.cjs` 画叠加层，`compose4.py zh|en` 合成，`encode4.sh` 输出 MP4（H.264、yuv420p、faststart）/ WebM / poster。脚本里的临时路径按自己的环境改。
- **用 0.4.3 重录还没做**，原因：演示实例是一个真实的 macOS 窗口，WebView 只在窗口服务器上有可见窗口时才渲染，`screencapture -l` 也只截屏幕上的窗口；启动时还会短暂抢焦点（脚本随后切回）。所以录制要在一个可以被占用的图形会话里做：另一个 macOS 用户的登录会话、虚拟机或备用 Mac，或者本人不用电脑的时候。另外需要：给运行脚本的终端「屏幕录制」权限；`demo4.py` 用 `~/.cargo/bin/shifu computer list-windows` 找窗口 id（要电脑操作组件和权限）；把 `stage4.py` 里写死的两条复制路径改成 0.4.3 下载包里的 `Shifu.app/Contents/MacOS/shifu-app` 和 `bin/shifu`（保留独立的 `SHIFU_APP_HOME`、`shifu-app-demo` socket 和 `invalid.shifu-site-demo` bundle id）；ffmpeg 和 Playwright（叠加层、编码）；同版本的截图也要重拍（见下一节）。重录后把 `tools/build.py` 的 `DEMO["version"]` 改成 0.4.3。
- 页面：`<video autoplay muted loop playsinline preload="metadata">`，桌面和手机各有 `media` 限定的源；`prefers-reduced-motion` 时不自动播放，停在 poster 上并显示「播放演示」按钮。

### 截图怎么来的

截图来自本分支（0.4.2 + Gemini CLI，提交 4620d85）编出来的 App，在一个与真实 Shifu 完全隔离的演示环境里拍：

- `SHIFU_APP_HOME=/tmp/demo/home`、`SHIFU_APP_SOCKET=shifu-app-demo`、`SHIFU_APP_SHIFU` 指向同一分支编出的 CLI；不碰 `~/.shifu/app`、真实 tmux socket、`~/.cargo/bin/shifu` 或 `~/Applications/Shifu.app`。
- 可执行文件复制到 bundle 之外运行：这样窗口名仍是「Shifu」，WebView 的 localStorage 也落在独立目录（`~/Library/WebKit/shifu-demo`），不会读写真实 App 的置顶、已读等状态。
- 各个「Agent」都是 tmux 里 `cat` 一段事先写好的 ANSI 文本，没有运行任何 Agent CLI，也没有调用模型；状态直接写在演示目录的 agent 记录里。
- 为避免以真实 App 的身份发系统通知，演示实例先启动一次记下「已看过」，退出后再把三条回复标成新的，第二次启动时它们显示为「做完了」，不触发通知。
- 每次启动后立即把焦点还给原来的前台应用；之后的点击、按键都在后台发送，截图用 `screencapture -l <窗口 id>`。
- 拍完即退出演示实例、`tmux -L shifu-app-demo kill-server`，并删除 `/tmp/demo` 和演示用的 WebKit / Caches 目录。
- 底部状态栏显示的是本机真实账号的额度，所以截图都裁掉了这一栏。

## 页面结构

顺序按访客的判断过程：看懂 → 支持我的工具 → 可信 → 安装 → 会用。

1. 顶栏：Logo、锚点导航（功能、兼容性、隐私、安装、FAQ）、「反馈」、语言切换、GitHub、下载（跳到 #install）。760px 以下锚点导航收进一个 `<details>` 菜单（不需要 JS）；英文链接更长，960px 以下就收进菜单（761–960px 之间实测会挤到按钮、横向溢出）。480px 以下隐藏 GitHub 图标（菜单里有）
2. 首屏：标题、品类说明、「免费开源 · 沿用你已有的 Agent 账号」、下载主按钮（跳到 #install）、GitHub 次按钮（480px 以下变成文字链接）、「仅支持 Apple Silicon …」，以及一行**随版本变化的权限说明**。480px 以下隐藏顶部小标签（开源和版本号紧接着就会出现）并收紧间距，390×844 首屏内能看到演示视频的顶部
3. 演示：状态图例 → 视频 → 版本脚注（`DEMO_NOTE`）→ Agent 名单
4. 问题：「不用再挨个检查终端。」
5. 功能：「为多个 Agent 并行工作而设计」，三块核心卖点（在等你、真终端、关掉窗口照样干活）
6. 兼容性：能力矩阵（依据 `src/agents_spec.rs`、`src/launch.rs`、`docs/AGENTS.md`）
7. 开源与隐私：四条事实。只有 Kimi Code（`~/.kimi-code/config.toml`）和 Antigravity（`~/.gemini/config/hooks.json`）要改全局配置，文件已存在时首次改动前备份成 `*.before-shifu`（`src/commands.rs` 的 `install_kimi_hooks` / `install_antigravity_hooks`）；其余靠启动参数或 Shifu 自己目录里的文件
8. 安装 + 快速开始：标题、版本 · Apple Silicon · 免费、安装要求、「装之前要知道」（在两种装法之上），然后两张并列的卡片：「让 Agent 帮你装」（主按钮复制安装指令，`<details>` 里是完整指令；没有 JS 时指令直接展开）和「手动安装」（按钮在新标签打开 Release，同时展开安装步骤），完成标准；之后是快速开始两步 + 新建任务弹窗
9. 还有这些：六格功能网格
10. FAQ → 结尾下载按钮 → 页脚

### 首屏的权限说明

`tools/build.py` 的 `RELEASE["permission"]` 决定首屏、「装之前要知道」第一条和安装指令第 4 步怎么写：`"no-confirmation"`（0.4.3：默认免确认，告诉用户在「权限」里改成 `RELEASE["own_label"]`）或 `"asks"`（之后的版本：第一次开对话前问一次）。文案模板在 `PERMISSION_LINE`。生成时会和 `docs/INSTALL*.md` 对照：`"no-confirmation"` 要求 permission-mode 段和方式 A 安装指令都逐字包含这句（中文「{版本} 默认以免确认模式启动支持该模式的 Agent。…」，英文 "In {版本}, agents that support it skip individual approvals by default. …"）；`"asks"` 要求两处都没有这句。网站的「装之前要知道」里这句单独加粗放在第一条，所以生成时从 permission-mode 段里去掉，不重复。对不上时构建失败，`check-docs.py` 也会列出来。发新版时改 `RELEASE` 的同时改这两个字段。

## 设计决定

- **深色优先。** Shifu 默认是墨黑主题，所有真实截图都是深色；页面用同一色调，截图和页面融为一体，不会出现白底里嵌一块黑图的割裂感。目标用户长时间在终端里工作，深色也更贴近他们的环境。不做浅色模式，减少一半的视觉验证成本。
- **用产品自己的状态色当强调色。** 琥珀色（等你批准）是唯一的品牌强调色，用于小标题、步骤编号、注意事项框；绿色（运行中）、蓝色（在问你）只出现在状态图例里。这样页面的颜色语言和 App 一致。
- **字体用系统字体栈。** 英文走 SF Pro，中文走苹方，等宽走 SF Mono；不需要自托管字体，零字体请求，没有字体加载导致的跳动。
- **字号与间距。** 字号约 1.25 倍阶梯（13/15/17/20/24/30/38/48/64，标题用 clamp 随视口缩放）；间距以 4px 为基数（4/8/12/16/24/32/48/64/96/128），定义在 `:root` 变量里。
- **对比度。** 正文 #ecedf0、次要文字 #b4b8c2、弱化文字 #8e94a0，在背景 #0b0c0f 上分别约 16:1、9.6:1、6.3:1，在卡片底色上也高于 WCAG AA 的 4.5:1。
- **统一缩放。** 所有产品截图都按首屏截图的缩放比例显示：一张宽 N 像素的裁图显示为 N ×（首屏宽度 / 1200）。CSS 里是 `.scaled { width: calc(var(--w) / 1200 * min(1120px, 100vw - 2 * var(--pad))) }`，`--w` 写在每张图上。桌面上实测所有截图都是 0.933 倍。
- **手机用紧凑裁图。** 390px 宽时不再缩小整个窗口，而是用 `<picture>` + `media="(max-width: 600px)"` 换成只含一个元素的裁图：首屏是 Codex 请求批准的终端，「在等你」只有等待列表，「真终端」是一个终端，新建任务只有弹窗。各张显示宽度不超过原图，文字可读。
- **页面长度。** 第三轮把功能收成三大块 + 一个紧凑网格，去掉两张框图和结尾下载区，FAQ 留 6 条。高度：桌面中文 10134 → 6295 px，英文 10633 → 6607 px；手机中文 13851 → 7840 px，英文 15363 → 8755 px。
- **如实。** 首屏脚注说明全站截图都是演示数据，英文页还说明 App 自己的界面文字是中文。没有编造数字、用户评价或价格。
- **无障碍。** 跳到正文链接、所有交互元素有可见焦点框、安装页签按 WAI-ARIA tabs 模式实现（左右方向键切换）、装饰元素 `aria-hidden`、示意图有文字描述。`prefers-reduced-motion` 下关闭所有动画和淡入。
- **性能。** 无框架、无外部请求、无统计脚本。首屏图 `fetchpriority="high"` 并 preload，其余图片 `loading="lazy"`；所有图片写了宽高，实测 CLS 为 0。
- **没用 docs/images 里的旧截图和 shifu-workflow.gif。** 它们来自较早版本；全部改用当前版本的新截图。

## 本地预览

```bash
cd site && python3 -m http.server 62200 --bind 127.0.0.1
# 打开 http://127.0.0.1:62200/
```

英文页在 http://127.0.0.1:62200/en/ 。改了 `src/index.html` 后先运行 `python3 tools/build.py` 再刷新。

## 以后部署

整站是纯静态文件，任何静态托管都可以，不需要构建：

- **GitHub Pages：** 仓库设置 → Pages → 从分支部署，目录选 `/site`（或用 Actions 把 `site/` 作为 artifact 发布）。
- **Vercel / Netlify / Cloudflare Pages：** 新建项目，根目录设为 `site`，构建命令留空，输出目录 `.`。

现在的部署（2026-11-25）：https://shifu.yutailin.com ，DNS 在 Vercel（`vercel dns`，A 记录指向 shifu-relay 136.109.124.132），文件在那台机器的 `/opt/shifu-site/current`（`releases/<提交>` 的链接），由已有的 Caddy 直接当静态文件发出。`tools/build.py` 里的 `SITE_URL` 让 canonical、hreflang、`og:url`、`og:image` 都是绝对地址。

新版本发布时只改 `tools/build.py` 顶部的 `RELEASE`，重新生成，再把 `site/` 传到新的 `releases/<提交>` 并切换 `current`。

## 还没做的

- 分享预览图 `og.png` 是仓库原图，左下角写的是旧地址 `github.com/Longado/shifu`（目前会跳转到 Nalu-Lab/shifu）。需要时重新导出一张。
- 「还有这些」网格里的功能（通知、进度页、远程、电脑操作等）没有截图，只有一两行说明和文档链接。
- 截图是 1x 分辨率（拍摄机器是 1080p 外接屏，窗口 1200×800），在 Retina 屏上略软；换 Retina 屏重拍会更清晰。
- Gemini CLI 在演示里作为普通命令显示，侧栏没有它的图标，这和当前源码的实际表现一致。
- 没有浅色模式、没有站内搜索、没有独立的文档页（文档链接都指向 GitHub 上的 docs/）。
- 没有 sitemap / robots.txt。
- 「Shifu 与 Anthropic、OpenAI 等厂商无关联」这句页脚声明是常规写法，发布前请作者确认。
