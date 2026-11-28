//! 查找我的项目: the first-run guide's look for project folders (onboarding.ts). Only names and modification times
//! are read, never file contents: a folder counts as a project when it holds `.git` or a common project file. The
//! usual places for code under the home folder are looked through, at most three levels down, for at most about
//! two seconds. Desktop, Documents and Downloads are protected by macOS, which asks the person before an app may
//! list them, so they are looked through only when the guide asks for them (one tap, `protected`), never on
//! their own.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Places that macOS does not guard: listing them never brings up a permission prompt.
const OPEN_ROOTS: &[&str] = &["Developer", "Projects", "code", "src", "dev", "GitHub", "repos", "workspace", "Code", "Workspace"];
/// Places macOS guards (Files and Folders privacy): the first listing makes it ask the person.
const PROTECTED_ROOTS: &[&str] = &["Desktop", "Documents", "Downloads"];
/// Files whose presence marks a project. Only their names are checked.
const MARKERS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "setup.py",
    "requirements.txt",
    "go.mod",
    "Gemfile",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "Package.swift",
    "composer.json",
    "CMakeLists.txt",
    "mix.exs",
    "deno.json",
    "pubspec.yaml",
    "CLAUDE.md",
    "AGENTS.md",
];
/// Folders that are never a project of their own and are often huge.
const SKIP: &[&str] = &["node_modules", "Library", "target", "build", "dist", "vendor", "Pods", "venv", "__pycache__"];
const MAX_DEPTH: usize = 3;
const MAX_FOUND: usize = 60;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct FoundProject {
    pub path: String,
    pub name: String,
    pub git: bool,
    /// Seconds since 1970 of the latest change seen (the folder, or its `.git/index`/`HEAD`).
    pub modified: u64,
}

#[derive(Serialize, Debug, Default)]
pub struct ScanResult {
    pub projects: Vec<FoundProject>,
    /// The guarded places that exist here but were not looked through (offered as one tap).
    pub protected_left: Vec<String>,
    /// The time box ran out before everything was looked through.
    pub partial: bool,
}

fn mtime(path: &Path) -> u64 {
    fs::symlink_metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// Whether `entries` (names in a folder) mark a project, and whether it is a git repository.
fn project_kind(names: &[String]) -> Option<bool> {
    let git = names.iter().any(|n| n == ".git");
    let marked = git || names.iter().any(|n| MARKERS.contains(&n.as_str()) || n.ends_with(".xcodeproj"));
    marked.then_some(git)
}

fn skipped(name: &str) -> bool {
    name.starts_with('.') || SKIP.contains(&name) || name.ends_with(".app")
}

/// Looks through `roots` (not following symlinks) until `deadline`. A project folder is not looked into further.
pub fn scan_roots(roots: &[PathBuf], deadline: Instant) -> (Vec<FoundProject>, bool) {
    let mut found = Vec::new();
    let mut partial = false;
    let mut queue: Vec<(PathBuf, usize)> = roots.iter().filter(|r| r.is_dir()).map(|r| (r.clone(), 0)).collect();
    // Breadth first, so a time-out leaves the shallow (likelier) projects found.
    let mut at = 0;
    while at < queue.len() {
        if Instant::now() >= deadline || found.len() >= MAX_FOUND {
            partial = at < queue.len();
            break;
        }
        let (dir, depth) = queue[at].clone();
        at += 1;
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        let mut names = Vec::new();
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().is_ok_and(|t| t.is_dir()) && !skipped(&name) {
                subdirs.push(entry.path());
            }
            names.push(name);
        }
        if let Some(git) = project_kind(&names) {
            let mut modified = mtime(&dir);
            if git {
                modified = modified.max(mtime(&dir.join(".git/index"))).max(mtime(&dir.join(".git/HEAD")));
            }
            found.push(FoundProject {
                path: dir.to_string_lossy().into_owned(),
                name: dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                git,
                modified,
            });
            continue;
        }
        if depth < MAX_DEPTH {
            subdirs.sort();
            queue.extend(subdirs.into_iter().map(|d| (d, depth + 1)));
        }
    }
    found.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.path.cmp(&b.path)));
    found.dedup_by(|a, b| a.path == b.path);
    (found, partial)
}

/// The places to look through under `home`; the guarded ones only when `protected`. Case-insensitive file
/// systems would list ~/code and ~/Code twice, so a folder already listed (same canonical path) is dropped.
pub fn roots_in(home: &Path, protected: bool) -> (Vec<PathBuf>, Vec<String>) {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf, roots: &mut Vec<PathBuf>| {
        let canon = fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
        if p.is_dir() && !seen.contains(&canon) {
            seen.push(canon);
            roots.push(p);
        }
    };
    for name in OPEN_ROOTS {
        push(home.join(name), &mut roots);
    }
    // Whether a guarded folder exists can be told without listing it (no prompt).
    let guarded: Vec<PathBuf> = PROTECTED_ROOTS.iter().map(|n| home.join(n)).filter(|p| p.is_dir()).collect();
    if protected {
        for p in guarded {
            push(p, &mut roots);
        }
        (roots, Vec::new())
    } else {
        (roots, guarded.iter().filter_map(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).collect())
    }
}

pub fn scan(home: &Path, protected: bool, budget: Duration) -> ScanResult {
    let (roots, protected_left) = roots_in(home, protected);
    let (projects, partial) = scan_roots(&roots, Instant::now() + budget);
    ScanResult { projects, protected_left, partial }
}

/// The guide's scan, off the main thread. `protected` adds Desktop, Documents and Downloads.
#[tauri::command]
pub async fn projects_scan(protected: bool) -> Result<ScanResult, String> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    tauri::async_runtime::spawn_blocking(move || scan(&home, protected, Duration::from_secs(2)))
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("shifu-projectscan-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }

    #[test]
    fn finds_git_repos_and_marked_folders_but_skips_hidden_deep_and_heavy_ones() {
        let home = tmp("find");
        fs::create_dir_all(home.join("code/alpha/.git")).unwrap();
        touch(&home.join("code/group/beta/package.json"));
        fs::create_dir_all(home.join("code/group/beta/node_modules/inner/.git")).unwrap();
        fs::create_dir_all(home.join("code/a/b/c/too-deep/.git")).unwrap();
        fs::create_dir_all(home.join("code/.hidden/.git")).unwrap();
        fs::create_dir_all(home.join("code/node_modules/pkg/.git")).unwrap();
        fs::create_dir_all(home.join("code/plain/notes")).unwrap();
        fs::create_dir_all(home.join("Projects/App.xcodeproj")).unwrap();
        fs::create_dir_all(home.join("Desktop/desk/.git")).unwrap();

        let open = scan(&home, false, Duration::from_secs(5));
        let mut names: Vec<_> = open.projects.iter().map(|p| p.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Projects", "alpha", "beta"]);
        assert!(open.projects.iter().find(|p| p.name == "alpha").unwrap().git);
        assert!(!open.projects.iter().find(|p| p.name == "beta").unwrap().git);
        assert_eq!(open.protected_left, ["Desktop"]);
        assert!(!open.partial);

        let all = scan(&home, true, Duration::from_secs(5));
        assert!(all.projects.iter().any(|p| p.name == "desk"));
        assert!(all.protected_left.is_empty());
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn newest_first_and_stops_at_the_deadline() {
        let home = tmp("order");
        fs::create_dir_all(home.join("dev/old/.git")).unwrap();
        fs::create_dir_all(home.join("dev/new/.git")).unwrap();
        let old = fs::File::create(home.join("dev/old/.git/index")).unwrap();
        old.set_modified(UNIX_EPOCH + Duration::from_secs(1_000)).unwrap();
        let (found, _) = scan_roots(&[home.join("dev")], Instant::now() + Duration::from_secs(5));
        // "old"'s folder itself is new too; its index is not newer than the folder, so both compare by folder time.
        assert_eq!(found.len(), 2);
        assert!(found[0].modified >= found[1].modified);

        let (none, partial) = scan_roots(&[home.join("dev")], Instant::now());
        assert!(none.is_empty() && partial);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn missing_home_finds_nothing() {
        let r = scan(Path::new("/nonexistent-shifu-home"), true, Duration::from_secs(1));
        assert!(r.projects.is_empty() && r.protected_left.is_empty());
    }
}
