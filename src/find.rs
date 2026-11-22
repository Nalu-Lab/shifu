//! `shifu find`: turn "the file I sent you the other day" into a short, dated list of candidates.
//!
//! The calling agent turns the user's words into keywords, a kind and a time window; this module only looks.
//! It reads the places a file the user mentions usually came from: files dropped into Shifu, images pasted
//! into Codex, files received in WeChat, Downloads, and (with keywords) Spotlight. Nothing is opened, copied
//! or uploaded; results are paths, times and sizes, newest first.

use crate::agent::Res;
use serde::Serialize;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const GUIDE: &str = "shifu find — 按用户的描述找本机文件，列出候选（只读，不打开、不复制）

用法:
  shifu find [关键词 ...] [--kind 类型] [--since 时间] [--from 来源] [--limit N] [--json]

  关键词      文件名里都要出现的词，不分大小写，例如 简版、报价、Screenshot
  --kind      图片 image | pdf | 文档 doc | 表格 sheet | 幻灯片 slide | 视频 video | 音频 audio | 压缩包 archive，
              也可直接写扩展名（docx、png）；多个用逗号分开
  --since     最近多久：30m、36h、7d，或日期 2026-11-20（本机时间零点起）
  --from      只查这些来源，逗号分开：sent（拖进 Shifu 的文件、贴进 Codex 的图）、wechat（微信收到的文件和视频）、
              downloads（下载文件夹）、spotlight（系统索引，需给关键词）。默认全部
  --limit     最多列几条，默认 20

用户说“我发给你的那张图”“之前那份简版 PDF”“微信里这周收到的文件”时，先把描述换成上面的条件再查，
例如：shifu find --from sent --kind image --since 2d、shifu find 简版 --kind pdf,doc、shifu find --from wechat --since 7d。
结果按时间从新到旧；同一文件可能在多个来源各出现一次（比如拖进 Shifu 的副本和原件）。
微信只能看到收到的文件本身，看不出来自哪个聊天；聊天归属仍要在微信里核对。";

const SCAN_LIMIT: usize = 200_000;
const SPOTLIGHT_TIMEOUT: Duration = Duration::from_secs(10);
const SKIP_DIRS: [&str; 6] = ["node_modules", ".git", "target", "Caches", ".Trash", "__pycache__"];

#[derive(Default, Debug, PartialEq)]
struct Query {
    words: Vec<String>,
    exts: Vec<String>,
    since: Option<i64>,
    from: Vec<Source>,
    limit: usize,
    json: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Source {
    Sent,
    Wechat,
    Downloads,
    Spotlight,
}

impl Source {
    const ALL: [Source; 4] = [Source::Sent, Source::Wechat, Source::Downloads, Source::Spotlight];
    fn parse(s: &str) -> Res<Source> {
        match s {
            "sent" => Ok(Source::Sent),
            "wechat" | "微信" => Ok(Source::Wechat),
            "downloads" | "下载" => Ok(Source::Downloads),
            "spotlight" => Ok(Source::Spotlight),
            other => Err(format!("不认识的来源 {other:?}：可用 sent、wechat、downloads、spotlight")),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Source::Sent => "发给Agent",
            Source::Wechat => "微信",
            Source::Downloads => "下载",
            Source::Spotlight => "Spotlight",
        }
    }
}

#[derive(Serialize, Debug, PartialEq)]
struct Found {
    path: String,
    source: Source,
    /// Seconds since the epoch: when the file arrived (drop time for Shifu drops), else last modified.
    time: i64,
    bytes: u64,
}

/// Where each source lives. Built from the environment in `run`, from temporary folders in tests.
struct Roots {
    home: PathBuf,
    shifu_home: PathBuf,
}

fn kind_exts(kind: &str) -> Vec<String> {
    let list: &[&str] = match kind {
        "图片" | "image" | "img" | "截图" => &["png", "jpg", "jpeg", "gif", "heic", "webp", "bmp", "tiff"],
        "pdf" => &["pdf"],
        "文档" | "doc" | "word" => &["doc", "docx", "pages", "rtf", "md", "txt", "odt"],
        "表格" | "sheet" | "excel" => &["xls", "xlsx", "csv", "numbers", "ods"],
        "幻灯片" | "slide" | "ppt" => &["ppt", "pptx", "key", "odp"],
        "视频" | "video" => &["mp4", "mov", "m4v", "avi", "mkv", "webm"],
        "音频" | "audio" => &["mp3", "m4a", "wav", "aac", "amr", "flac", "ogg", "opus", "silk"],
        "压缩包" | "archive" => &["zip", "rar", "7z", "tar", "gz", "tgz"],
        ext => return vec![ext.trim_start_matches('.').to_lowercase()],
    };
    list.iter().map(|s| s.to_string()).collect()
}

/// `30m`, `36h`, `7d`, or a local date `2026-11-20`, as an absolute epoch second.
fn parse_since(text: &str, now: i64) -> Res<i64> {
    let bad = || format!("看不懂时间 {text:?}：用 30m、36h、7d 或 2026-11-20");
    if let Some((num, unit)) = text.char_indices().last().map(|(i, c)| (&text[..i], c)) {
        if let (Ok(n), Some(secs)) = (num.parse::<i64>(), match unit { 'm' => Some(60), 'h' => Some(3600), 'd' => Some(86_400), _ => None }) {
            if n < 0 { return Err(bad()); }
            return Ok(now - n * secs);
        }
    }
    let parts: Vec<&str> = text.split('-').collect();
    if let [y, m, d] = parts[..] {
        let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i32>(), m.parse::<i32>(), d.parse::<i32>()) else { return Err(bad()) };
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) { return Err(bad()); }
        // SAFETY: mktime only reads and normalises the struct we own.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        tm.tm_year = y - 1900;
        tm.tm_mon = m - 1;
        tm.tm_mday = d;
        tm.tm_isdst = -1;
        let t = unsafe { libc::mktime(&mut tm) };
        if t == -1 { return Err(bad()); }
        return Ok(t as i64);
    }
    Err(bad())
}

fn parse(args: &[String], now: i64) -> Res<Query> {
    let mut q = Query { limit: 20, ..Query::default() };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or(format!("{name} 后面要跟一个值"));
        match a.as_str() {
            "--kind" => {
                for k in value("--kind")?.split(',').map(str::trim).filter(|k| !k.is_empty()) {
                    q.exts.extend(kind_exts(&k.to_lowercase()));
                }
            }
            "--since" => q.since = Some(parse_since(&value("--since")?, now)?),
            "--from" => {
                for s in value("--from")?.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    let s = Source::parse(s)?;
                    if !q.from.contains(&s) { q.from.push(s); }
                }
            }
            "--limit" => q.limit = value("--limit")?.parse().ok().filter(|n| *n > 0).ok_or("--limit 要一个正整数")?,
            "--json" => q.json = true,
            flag if flag.starts_with("--") => return Err(format!("不认识的选项 {flag}；运行 shifu find help 看用法")),
            word => q.words.push(word.to_lowercase()),
        }
    }
    if q.from.is_empty() { q.from = Source::ALL.to_vec(); }
    Ok(q)
}

fn matches(q: &Query, name: &str) -> bool {
    let lower = name.to_lowercase();
    if !q.words.iter().all(|w| lower.contains(w.as_str())) { return false; }
    if q.exts.is_empty() { return true; }
    let ext = Path::new(&lower).extension().and_then(|e| e.to_str()).unwrap_or("");
    q.exts.iter().any(|e| e == ext)
}

fn mtime(meta: &fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Shifu names drops `<epoch>-<original name>`; the prefix is when the user sent it.
fn drop_time(name: &str) -> Option<i64> {
    let (head, rest) = name.split_once('-')?;
    (head.len() == 10 && !rest.is_empty()).then(|| head.parse().ok()).flatten()
}

/// WeChat stores a preview image beside each received video: `<id>.jpg` or `<id>_thumb.jpg` next to `<id>.mp4`.
fn is_video_preview(path: &Path) -> bool {
    let (Some(stem), Some(dir)) = (path.file_stem().and_then(|s| s.to_str()), path.parent()) else { return false };
    let is_image = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("jpg"));
    let id = stem.strip_suffix("_thumb").unwrap_or(stem);
    is_image && (stem.ends_with("_thumb") || ["mp4", "mov"].iter().any(|v| dir.join(format!("{id}.{v}")).exists()))
}

/// Walk `dir` (bounded depth), keeping regular files that match. `budget` caps entries across one search.
fn walk(dir: &Path, depth: usize, q: &Query, source: Source, budget: &mut usize, out: &mut Vec<Found>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if *budget == 0 { return; }
        *budget -= 1;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') { continue; }
        let Ok(meta) = entry.metadata() else { continue };
        let path = entry.path();
        if meta.is_dir() {
            if depth > 0 && !SKIP_DIRS.contains(&name.as_str()) { walk(&path, depth - 1, q, source, budget, out); }
            continue;
        }
        if !meta.is_file() || !matches(q, &name) || (source == Source::Wechat && is_video_preview(&path)) { continue; }
        let time = if source == Source::Sent { drop_time(&name).unwrap_or_else(|| mtime(&meta)) } else { mtime(&meta) };
        if q.since.is_some_and(|s| time < s) { continue; }
        out.push(Found { path: path.to_string_lossy().to_string(), source, time, bytes: meta.len() });
    }
}

/// Every WeChat account folder's received files and videos (WeChat 4 layout, then the older 3.x one).
fn wechat_dirs(home: &Path) -> Vec<PathBuf> {
    let container = home.join("Library/Containers/com.tencent.xinWeChat/Data");
    let mut dirs = Vec::new();
    if let Ok(accounts) = fs::read_dir(container.join("Documents/xwechat_files")) {
        for a in accounts.flatten() {
            for sub in ["msg/file", "msg/video"] {
                let d = a.path().join(sub);
                if d.is_dir() { dirs.push(d); }
            }
        }
    }
    let old = container.join("Library/Application Support/com.tencent.xinWeChat");
    if let Ok(versions) = fs::read_dir(&old) {
        for v in versions.flatten() {
            let Ok(accounts) = fs::read_dir(v.path()) else { continue };
            for a in accounts.flatten() {
                let temp = a.path().join("Message/MessageTemp");
                let Ok(chats) = fs::read_dir(&temp) else { continue };
                for c in chats.flatten() {
                    for sub in ["File", "Video"] {
                        let d = c.path().join(sub);
                        if d.is_dir() { dirs.push(d); }
                    }
                }
            }
        }
    }
    dirs
}

fn walked(roots: &Roots, q: &Query, source: Source, out: &mut Vec<Found>) {
    let mut budget = SCAN_LIMIT;
    let dirs: Vec<(PathBuf, usize)> = match source {
        Source::Sent => vec![(roots.shifu_home.join("drops"), 1), (roots.home.join(".codex/attachments"), 3)],
        Source::Wechat => wechat_dirs(&roots.home).into_iter().map(|d| (d, 2)).collect(),
        Source::Downloads => vec![(roots.home.join("Downloads"), 2)],
        Source::Spotlight => return,
    };
    for (dir, depth) in dirs {
        walk(&dir, depth, q, source, &mut budget, out);
    }
}

fn spotlight_query(q: &Query) -> String {
    let esc = |w: &str| w.replace('\\', "\\\\").replace('"', "\\\"").replace('*', "\\*");
    let mut parts: Vec<String> = q.words.iter().map(|w| format!("kMDItemFSName == \"*{}*\"cd", esc(w))).collect();
    if let Some(s) = q.since {
        parts.push(format!("(kMDItemFSContentChangeDate >= $time.iso({0}) || kMDItemLastUsedDate >= $time.iso({0}))", iso_utc(s)));
    }
    parts.join(" && ")
}

fn spotlight(roots: &Roots, q: &Query, out: &mut Vec<Found>) -> Option<String> {
    if q.words.is_empty() { return None; }
    let mut child = Command::new("/usr/bin/mdfind")
        .args(["-onlyin", &roots.home.to_string_lossy(), &spotlight_query(q)])
        .stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = thread::spawn(move || { let mut s = String::new(); let _ = stdout.read_to_string(&mut s); s });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < SPOTLIGHT_TIMEOUT => thread::sleep(Duration::from_millis(50)),
            _ => { let _ = child.kill(); let _ = child.wait(); return Some("Spotlight 超过 10 秒没有返回，已跳过".into()); }
        }
    }
    let text = reader.join().unwrap_or_default();
    for line in text.lines() {
        let path = Path::new(line);
        if path.components().any(|c| SKIP_DIRS.contains(&c.as_os_str().to_string_lossy().as_ref())) { continue; }
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else { continue };
        let Ok(meta) = fs::metadata(path) else { continue };
        if !meta.is_file() || !matches(q, &name) { continue; }
        out.push(Found { path: line.to_string(), source: Source::Spotlight, time: mtime(&meta), bytes: meta.len() });
    }
    None
}

fn search(roots: &Roots, q: &Query) -> (Vec<Found>, Vec<String>) {
    let mut out = Vec::new();
    let mut notes = Vec::new();
    for &s in &q.from {
        if s == Source::Spotlight {
            if q.words.is_empty() {
                if q.from.len() == 1 { notes.push("Spotlight 需要至少一个关键词".into()); }
            } else if let Some(n) = spotlight(roots, q, &mut out) {
                notes.push(n);
            }
        } else {
            walked(roots, q, s, &mut out);
        }
    }
    // The same path can come back from Spotlight and a walked folder; keep the walked (more specific) one.
    let mut seen = std::collections::HashSet::new();
    out.sort_by_key(|f| f.source == Source::Spotlight);
    out.retain(|f| seen.insert(f.path.clone()));
    out.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| a.path.cmp(&b.path)));
    (out, notes)
}

fn iso_utc(t: i64) -> String {
    let tm = broken_down(t, false);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
}

fn local_time(t: i64) -> String {
    let tm = broken_down(t, true);
    format!("{:04}-{:02}-{:02} {:02}:{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min)
}

fn broken_down(t: i64, local: bool) -> libc::tm {
    let secs = t as libc::time_t;
    // SAFETY: both functions fill the struct we own from a value we own.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { if local { libc::localtime_r(&secs, &mut tm) } else { libc::gmtime_r(&secs, &mut tm) } };
    tm
}

fn size(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 30 => format!("{:.1}GB", b as f64 / (1u64 << 30) as f64),
        b if b >= 1 << 20 => format!("{:.1}MB", b as f64 / (1u64 << 20) as f64),
        b if b >= 1 << 10 => format!("{}KB", b >> 10),
        b => format!("{b}B"),
    }
}

pub fn run(args: &[String]) -> Res<()> {
    if matches!(args.first().map(String::as_str), Some("help" | "-h" | "--help" | "guide")) {
        println!("{GUIDE}");
        return Ok(());
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let q = parse(args, now)?;
    let home = PathBuf::from(std::env::var("HOME").map_err(|_| "没有 HOME 环境变量")?);
    let shifu_home = std::env::var("SHIFU_HOME").ok().filter(|v| !v.is_empty()).map(PathBuf::from)
        .unwrap_or_else(|| home.join(".shifu/app"));
    let (found, notes) = search(&Roots { home, shifu_home }, &q);
    let total = found.len();
    let shown: Vec<&Found> = found.iter().take(q.limit).collect();
    if q.json {
        println!("{}", serde_json::json!({"total": total, "results": shown, "notes": notes}));
        return Ok(());
    }
    for n in &notes { eprintln!("{n}"); }
    if shown.is_empty() {
        println!("没有找到。可以少给一个关键词、放宽 --since，或换 --from 来源。");
        return Ok(());
    }
    for f in &shown {
        println!("{}  {:<9} {:>7}  {}", local_time(f.time), f.source.label(), size(f.bytes), f.path);
    }
    if total > shown.len() { println!("另有 {} 个，加关键词或 --limit 看更多。", total - shown.len()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> { s.split_whitespace().map(String::from).collect() }

    fn touch(path: &Path, age_secs: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
        let t = SystemTime::now() - Duration::from_secs(age_secs);
        fs::File::options().write(true).open(path).unwrap().set_modified(t).unwrap();
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("shifu-find-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn since_accepts_relative_and_dates() {
        assert_eq!(parse_since("2d", 1_000_000).unwrap(), 1_000_000 - 172_800);
        assert_eq!(parse_since("90m", 10_000).unwrap(), 10_000 - 5_400);
        let day = parse_since("2026-11-20", 0).unwrap();
        assert_eq!(parse_since("2026-11-21", 0).unwrap() - day, 86_400);
        assert!(parse_since("yesterday", 0).is_err());
        assert!(parse_since("2026-13-01", 0).is_err());
    }

    #[test]
    fn kinds_words_and_sources_parse() {
        let q = parse(&args("简版 --kind pdf,文档 --from wechat,sent --limit 5"), 0).unwrap();
        assert_eq!(q.words, vec!["简版"]);
        assert!(q.exts.contains(&"pdf".into()) && q.exts.contains(&"docx".into()));
        assert_eq!(q.from, vec![Source::Wechat, Source::Sent]);
        assert_eq!(q.limit, 5);
        assert_eq!(parse(&args(""), 0).unwrap().from, Source::ALL.to_vec());
        assert!(parse(&args("--from email"), 0).is_err());
        assert!(parse(&args("--limit 0"), 0).is_err());
    }

    #[test]
    fn matching_needs_every_word_and_the_kind() {
        let q = parse(&args("PLTR 简版 --kind pdf"), 0).unwrap();
        assert!(matches(&q, "1794993140-PLTR-中国企业对标简版-新.pdf"));
        assert!(!matches(&q, "PLTR-简版.docx"));
        assert!(!matches(&q, "PLTR-完整版.pdf"));
    }

    #[test]
    fn drops_use_the_send_time_and_wechat_skips_thumbnails() {
        let home = temp("home");
        let shifu = home.join(".shifu/app");
        let sent_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64 - 3_600;
        touch(&shifu.join(format!("drops/{sent_at}-报价单.pdf")), 30 * 86_400); // old original, sent an hour ago
        touch(&shifu.join("drops/1000000000-很久以前.png"), 10);
        let wx = home.join("Library/Containers/com.tencent.xinWeChat/Data/Documents/xwechat_files/wxid_a/msg");
        touch(&wx.join("file/2026-11/修改意见.docx"), 2 * 86_400);
        touch(&wx.join("video/2026-11/abc.mp4"), 86_400);
        touch(&wx.join("video/2026-11/abc_thumb.jpg"), 86_400);
        touch(&wx.join("video/2026-11/abc.jpg"), 86_400);
        touch(&wx.join("video/2026-11/photo.jpg"), 3 * 86_400);
        touch(&wx.join("file/2026-10/旧文件.pdf"), 40 * 86_400);
        let roots = Roots { home: home.clone(), shifu_home: shifu };
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

        let (sent, _) = search(&roots, &parse(&args("--from sent --since 1d"), now).unwrap());
        assert_eq!(sent.len(), 1);
        assert!(sent[0].path.ends_with("报价单.pdf") && sent[0].time == sent_at);

        let (wechat, _) = search(&roots, &parse(&args("--from wechat --since 7d"), now).unwrap());
        let names: Vec<&str> = wechat.iter().map(|f| f.path.rsplit('/').next().unwrap()).collect();
        assert_eq!(names, vec!["abc.mp4", "修改意见.docx", "photo.jpg"], "newest first; video previews and old files left out");
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn spotlight_query_escapes_and_bounds_time() {
        let q = Query { words: vec!["a\"b*".into()], since: Some(0), ..Query::default() };
        let text = spotlight_query(&q);
        assert!(text.contains("kMDItemFSName == \"*a\\\"b\\**\"cd"), "{text}");
        assert!(text.contains("$time.iso(1970-01-01T00:00:00Z)"));
    }
}
