# Shifu 官网（site/）

Shifu 的第一个官网：一页静态站，介绍产品、给出下载入口和如实的安装说明。目前 Shifu 对外只有 README，这个站把 README 的事实整理成面向新访客的页面。

## 目录

```
site/
├── index.html     整页内容，中英两种语言都写在里面
├── styles.css     全部样式（无预处理）
├── main.js        语言切换、安装页签、复制按钮、滚动淡入；不加载也能正常阅读
├── README.md      本文件
└── assets/        图片与图标（都从仓库现有素材生成，原图未改动）
    ├── workspace-1600.webp / workspace-960.webp   首屏截图（docs/images/shifu-workspace.png 缩放）
    ├── crop-waiting.webp     侧栏「在等你」特写（同上裁切）
    ├── crop-approval.webp    Codex 等你批准的终端特写（同上裁切）
    ├── crop-split.webp       两个 Agent 分屏（同上裁切，去掉侧栏）
    ├── crop-projects.webp    按项目分组的任务列表（docs/images/shifu-main.png 裁切）
    ├── crop-quota.webp       额度区域（shifu-main.png 裁切）
    ├── new-task.webp         新建任务表单（docs/images/shifu-new-task.png）
    ├── og.png                分享预览图（docs/images/shifu-social-preview.png 原样复制）
    ├── logo-96.webp / favicon-32.png / apple-touch-icon.png   来自 app/src-tauri/icons/
```

图片全部转成 WebP，`assets/` 合计约 420 KB；首屏实际传输约 330 KB（含 HTML/CSS/JS）。

## 页面结构

1. 顶栏：Logo、锚点导航、语言切换、GitHub、下载
2. 首屏：标题、一句说明、「下载 macOS 预览版」主按钮、GitHub 次按钮、未公证提示、四种会话状态图例、真实界面截图
3. 支持的 Agent：只列 Shifu 实际支持的；Qwen Code / StepFun 标「可选」，Gemini CLI 标「当前源码」并说明侧栏暂无等待状态
4. 功能：在等你、真终端与分屏、持久会话（tmux 示意图）、任务与进度、额度、通知、远程（中继示意图）、电脑与浏览器操作
5. 三步开始：安装 → 选项目和 Agent（说明会按 PATH 检测已装的 CLI）→ 开始工作，附新建任务截图
6. 开源与隐私：无账号无遥测、不改全局设置、Kimi/Antigravity 例外先备份、远程走自己的线路
7. 安装：两个页签（让 AI 装 / 自己装，文本与 README 一致，可一键复制）+「装之前要知道」：仅 Apple Silicon、未公证、默认免确认及切换方法、需 tmux 与已登录 Agent、Codex hooks 检查、窗口界面暂为中文
8. FAQ（`<details>`，无需 JS）
9. 结尾下载区与页脚

## 设计决定

- **深色优先。** Shifu 默认是墨黑主题，所有真实截图都是深色；页面用同一色调，截图和页面融为一体，不会出现白底里嵌一块黑图的割裂感。目标用户长时间在终端里工作，深色也更贴近他们的环境。不做浅色模式，减少一半的视觉验证成本。
- **用产品自己的状态色当强调色。** 琥珀色（等你批准）是唯一的品牌强调色，用于小标题、步骤编号、注意事项框；绿色（运行中）、蓝色（在问你）只出现在状态图例里。这样页面的颜色语言和 App 一致。
- **字体用系统字体栈。** 英文走 SF Pro，中文走苹方，等宽走 SF Mono；不需要自托管字体，零字体请求，没有字体加载导致的跳动。
- **字号与间距。** 字号约 1.25 倍阶梯（13/15/17/20/24/30/38/48/64，标题用 clamp 随视口缩放）；间距以 4px 为基数（4/8/12/16/24/32/48/64/96/128），定义在 `:root` 变量里。
- **对比度。** 正文 #ecedf0、次要文字 #b4b8c2、弱化文字 #8e94a0，在背景 #0b0c0f 上分别约 16:1、9.6:1、6.3:1，在卡片底色上也高于 WCAG AA 的 4.5:1。
- **双语。** 中英文都直接写在 HTML 里（`data-l="zh"` / `data-l="en"`），按 `<html lang>` 用 CSS 隐藏另一种。`<head>` 里的内联脚本在首次绘制前决定语言：先看 localStorage 里保存的选择，没有就看浏览器语言（zh 开头用中文，其他用英文）。切换时同步更新 `<title>`、meta description、图片 alt 和示意图的 aria-label。没有 JS 时显示中文。
- **如实。** 截图都标注了「演示数据」；新建任务截图来自较早版本，图注写明了 0.4.2 的变化；通知卡片是示意图，也标了「非真实截图」。没有编造数字、用户评价或价格。
- **无障碍。** 跳到正文链接、所有交互元素有可见焦点框、安装页签按 WAI-ARIA tabs 模式实现（左右方向键切换）、装饰元素 `aria-hidden`、示意图有文字描述。`prefers-reduced-motion` 下关闭所有动画和淡入。
- **性能。** 无框架、无外部请求、无统计脚本。首屏图 `fetchpriority="high"` 并 preload，其余图片 `loading="lazy"`；所有图片写了宽高，实测 CLS 为 0。
- **没用 shifu-workflow.gif。** 录屏里的标签右键菜单还有「移到右边」，0.4.2 已经去掉；分辨率也只有 1000px。改用静态截图裁切展示同样的功能。

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
- 没有真实的通知、进度页、远程连接、电脑操作截图；这些功能用文字、示意图或命令示例表达。补上真实截图会更有说服力。
- 截图里的界面是中文；英文版页面也用这些截图，并在图注里说明窗口界面暂为中文。
- 没有浅色模式、没有站内搜索、没有独立的文档页（文档链接都指向 GitHub 上的 docs/）。
- 没有部署，没有域名，没有 sitemap / robots.txt。
- 「Shifu 与 Anthropic、OpenAI 等厂商无关联」这句页脚声明是常规写法，发布前请作者确认。
