# Shifu 官网（site/）

Shifu 的第一个官网：一页静态站，介绍产品、给出下载入口和如实的安装说明。目前 Shifu 对外只有 README，这个站把 README 的事实整理成面向新访客的页面。

## 目录

```
site/
├── index.html     整页内容，中英两种语言都写在里面
├── styles.css     全部样式（无预处理）
├── main.js        语言切换、安装页签、复制按钮、滚动淡入；不加载也能正常阅读
├── README.md      本文件
└── assets/        图片与图标
    ├── main-{zh,en}.webp         首屏：主窗口（侧栏 + Claude Code / Gemini CLI 分屏）
    ├── waiting-{zh,en}.webp      侧栏「在等你」特写
    ├── approval-{zh,en}.webp     Codex 请求批准 git push
    ├── split-{zh,en}.webp        分屏特写
    ├── projects-{zh,en}.webp     按项目分组的任务
    ├── orders / research / question-{zh,en}.webp   「不只是写代码」三张：数据分析、调研、提问
    ├── newtask-{zh,en}.webp      新建任务弹窗（项目下拉、Agent 下拉、权限）
    ├── appearance-{zh,en}.webp   外观弹窗（配色、字号）
    ├── og.png                    分享预览图（docs/images/shifu-social-preview.png 原样复制）
    └── logo-96.webp / favicon-32.png / apple-touch-icon.png   来自 app/src-tauri/icons/
```

每张截图都有中文版和英文版，图片尺寸相同；`<img data-l="zh">` / `<img data-l="en">` 成对出现，CSS 只显示当前语言那张（隐藏的那张因 `loading="lazy"` 不会下载），所以切换语言时图片和 alt 一起换，没有布局跳动。

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

1. 顶栏：Logo、锚点导航、语言切换、GitHub、下载
2. 首屏：标题、一句说明、「下载 macOS 预览版」主按钮、GitHub 次按钮、未公证提示、四种会话状态图例、真实界面截图
3. 支持的工具：横向自动滚动的标志带（图标取自 app/src/vendoricons.ts 和 app/src/assets/antigravity.svg，单色内联 SVG，名字是文字）；悬停或聚焦时暂停，`prefers-reduced-motion` 下变成静止的换行排列。Qwen Code / StepFun 标「可选」，Gemini CLI 标「下个版本」
4. 功能：在等你、真终端与分屏、持久会话（tmux 示意图）、「不只是写代码」（数据分析 / 调研 / 提问三张截图）、任务与进度、额度、通知、远程（中继示意图）、电脑与浏览器操作
5. 三步开始：安装 → 选项目和 Agent（说明会按 PATH 检测已装的 CLI）→ 开始工作，附新建任务截图
6. 开源与隐私：无账号无遥测、不改全局设置、Kimi/Antigravity 例外先备份、远程走自己的线路
7. 安装：两个页签（让 AI 装 / 自己装，文本与 README 一致）。长代码块默认只显示前几行，下方「展开全部（共 N 行）」按钮带 `aria-expanded`，可用键盘操作；收起时保持按钮在屏幕上的位置不变。复制按钮始终复制全文。旁边是「装之前要知道」：仅 Apple Silicon、未公证、默认免确认及切换方法、需 tmux 与已登录 Agent、Codex hooks 检查、窗口界面暂为中文
8. FAQ（`<details>`，无需 JS）
9. 结尾下载区与页脚

## 设计决定

- **深色优先。** Shifu 默认是墨黑主题，所有真实截图都是深色；页面用同一色调，截图和页面融为一体，不会出现白底里嵌一块黑图的割裂感。目标用户长时间在终端里工作，深色也更贴近他们的环境。不做浅色模式，减少一半的视觉验证成本。
- **用产品自己的状态色当强调色。** 琥珀色（等你批准）是唯一的品牌强调色，用于小标题、步骤编号、注意事项框；绿色（运行中）、蓝色（在问你）只出现在状态图例里。这样页面的颜色语言和 App 一致。
- **字体用系统字体栈。** 英文走 SF Pro，中文走苹方，等宽走 SF Mono；不需要自托管字体，零字体请求，没有字体加载导致的跳动。
- **字号与间距。** 字号约 1.25 倍阶梯（13/15/17/20/24/30/38/48/64，标题用 clamp 随视口缩放）；间距以 4px 为基数（4/8/12/16/24/32/48/64/96/128），定义在 `:root` 变量里。
- **对比度。** 正文 #ecedf0、次要文字 #b4b8c2、弱化文字 #8e94a0，在背景 #0b0c0f 上分别约 16:1、9.6:1、6.3:1，在卡片底色上也高于 WCAG AA 的 4.5:1。
- **双语。** 中英文都直接写在 HTML 里（`data-l="zh"` / `data-l="en"`），按 `<html lang>` 用 CSS 隐藏另一种。`<head>` 里的内联脚本在首次绘制前决定语言：先看 localStorage 里保存的选择，没有就看浏览器语言（zh 开头用中文，其他用英文）。切换时同步更新 `<title>`、meta description、图片 alt 和示意图的 aria-label。没有 JS 时显示中文。
- **如实。** 截图都标注了演示数据；通知卡片是示意图，也标了「非真实截图」。没有编造数字、用户评价或价格。App 自己的界面文字是中文，英文页的截图只翻译了演示内容，图注里说明了这一点。
- **无障碍。** 跳到正文链接、所有交互元素有可见焦点框、安装页签按 WAI-ARIA tabs 模式实现（左右方向键切换）、装饰元素 `aria-hidden`、示意图有文字描述。`prefers-reduced-motion` 下关闭所有动画和淡入。
- **性能。** 无框架、无外部请求、无统计脚本。首屏图 `fetchpriority="high"` 并 preload，其余图片 `loading="lazy"`；所有图片写了宽高，实测 CLS 为 0。
- **没用 docs/images 里的旧截图和 shifu-workflow.gif。** 它们来自较早版本；全部改用当前版本的新截图。

## 本地预览

```bash
cd site && python3 -m http.server 62200 --bind 127.0.0.1
# 打开 http://127.0.0.1:62200/
```

强制看英文：在浏览器控制台运行 `localStorage.setItem('shifu-lang','en')` 后刷新，或点右上角「EN」。

## 以后部署

整站是纯静态文件，任何静态托管都可以，不需要构建：

- **GitHub Pages：** 仓库设置 → Pages → 从分支部署，目录选 `/site`（或用 Actions 把 `site/` 作为 artifact 发布）。
- **Vercel / Netlify / Cloudflare Pages：** 新建项目，根目录设为 `site`，构建命令留空，输出目录 `.`。

部署前要改的：

1. `og:image` 和 `twitter:image` 现在是相对路径 `assets/og.png`。多数社交平台要求绝对地址，拿到域名后改成 `https://<域名>/assets/og.png`，并加上 `<link rel="canonical">` 和 `og:url`。
2. 新版本发布时，全局替换 Release 链接 `remote-v0.4.2-preview.1` 以及页面上的「0.4.2」字样。

## 还没做的

- 分享预览图 `og.png` 是仓库原图，左下角写的是旧地址 `github.com/Longado/shifu`（目前会跳转到 Nalu-Lab/shifu）。需要时重新导出一张。
- 没有真实的通知、进度页、远程连接、电脑操作截图；这些功能用文字、示意图或命令示例表达。
- 截图是 1x 分辨率（拍摄机器是 1080p 外接屏，窗口 1200×800），在 Retina 屏上略软；换 Retina 屏重拍会更清晰。
- Gemini CLI 在演示里作为普通命令显示，侧栏没有它的图标，这和当前源码的实际表现一致。
- 没有浅色模式、没有站内搜索、没有独立的文档页（文档链接都指向 GitHub 上的 docs/）。
- 没有部署，没有域名，没有 sitemap / robots.txt。
- 「Shifu 与 Anthropic、OpenAI 等厂商无关联」这句页脚声明是常规写法，发布前请作者确认。
