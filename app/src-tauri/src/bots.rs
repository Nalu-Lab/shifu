//! Personal bots keep their own folders and return to the same task on every open.
use crate::config::{write_atomic, Config};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

const JOINED: &str = ".joined";
const PROFILE: &str = ".shifu-cat.json";
/// Set when the user removes a bot from 我的 Bot; its folder and conversation stay for when it is added back.
const LEFT: &str = ".left";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Coat { Orange, Calico, Tuxedo, Silver, Cream, Black }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Ears { Pointed, Round, Folded }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tail { Short, Medium, Long }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Eyes { Amber, Green, Blue, Copper }

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Appearance {
    pub coat: Coat,
    pub ears: Ears,
    pub tail: Tail,
    pub eyes: Eyes,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Profile {
    pub name: String,
    pub purpose: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Appearance>,
}

struct Template {
    id: &'static str,
    name: &'static str,
    purpose: &'static str,
    appearance: Option<Appearance>,
}

const ANIMAL_PURPOSE: &str = "陪你处理日常任务。";

const TEMPLATES: [Template; 7] = [
    Template { id: "tabby", name: "金橘", purpose: "帮你梳理待办，拆解下一步。", appearance: Some(Appearance { coat: Coat::Orange, ears: Ears::Pointed, tail: Tail::Long, eyes: Eyes::Green }) },
    Template { id: "calico", name: "三花", purpose: "查找资料，核对来源。", appearance: Some(Appearance { coat: Coat::Calico, ears: Ears::Round, tail: Tail::Medium, eyes: Eyes::Amber }) },
    Template { id: "tuxedo", name: "踏雪", purpose: "陪你起草和修改文稿。", appearance: Some(Appearance { coat: Coat::Tuxedo, ears: Ears::Pointed, tail: Tail::Short, eyes: Eyes::Amber }) },
    Template { id: "rabbit", name: "月牙", purpose: ANIMAL_PURPOSE, appearance: None },
    Template { id: "puppy", name: "可可", purpose: ANIMAL_PURPOSE, appearance: None },
    Template { id: "red-panda", name: "枫糖", purpose: ANIMAL_PURPOSE, appearance: None },
    Template { id: "hedgehog", name: "松果", purpose: ANIMAL_PURPOSE, appearance: None },
];

#[derive(Clone, Debug, Serialize)]
pub struct ResidentBot {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub appearance: Option<Appearance>,
    pub dir: String,
    pub joined: bool,
}

fn template(id: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.id == id)
}

fn validate_id(id: &str) -> Result<(), String> {
    if template(id).is_some() { return Ok(()); }
    let uuid = id.strip_prefix("cat-").ok_or("找不到这只 Bot")?;
    if uuid.len() != 36 || !uuid.bytes().enumerate().all(|(i, byte)| {
        if [8, 13, 18, 23].contains(&i) { byte == b'-' }
        else { byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) }
    }) {
        return Err("猫咪编号无效".into());
    }
    Ok(())
}

fn validate_profile(id: &str, mut profile: Profile) -> Result<Profile, String> {
    profile.name = profile.name.trim().to_string();
    profile.purpose = profile.purpose.trim().to_string();
    if profile.name.is_empty() || profile.name.chars().count() > 24 || profile.name.chars().any(char::is_control) {
        return Err("名字请填 1–24 个字，使用单行文字".into());
    }
    if profile.purpose.is_empty() || profile.purpose.chars().count() > 160 {
        return Err("职责请填 1–160 个字".into());
    }
    let has_cat_traits = template(id).is_none_or(|t| t.appearance.is_some());
    if profile.appearance.is_some() != has_cat_traits {
        return Err(if has_cat_traits { "猫咪需要外观设置" } else { "这只小动物使用固定头像" }.into());
    }
    Ok(profile)
}

fn joined(path: &Path, id: &str) -> Result<bool, String> {
    match std::fs::read_to_string(path.join(JOINED)) {
        Ok(saved) if saved.trim() == id => Ok(true),
        Ok(_) => Err(format!("Bot 登记内容不匹配：{}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("读取 Bot 登记失败：{error}")),
    }
}

fn saved_profile(dir: &Path, id: &str) -> Result<Option<Profile>, String> {
    match std::fs::read_to_string(dir.join(PROFILE)) {
        Ok(text) => {
            let profile = serde_json::from_str(&text).map_err(|e| format!("读取 Bot 设置失败：{e}"))?;
            validate_profile(id, profile).map(Some)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("读取 Bot 设置失败：{e}")),
    }
}

fn description(cfg: &Config, id: &str) -> Result<ResidentBot, String> {
    let dir = cfg.home.join("bots").join(id);
    let saved = saved_profile(&dir, id)?;
    let is_joined = !dir.join(LEFT).exists() && (saved.is_some() || joined(&dir, id)?);
    let profile = saved.or_else(|| template(id).map(|t| Profile {
        name: t.name.into(), purpose: t.purpose.into(), appearance: t.appearance.clone(),
    })).ok_or("找不到这只 Bot")?;
    // A removed bot keeps its folder; its canonical path lets the sidebar keep that conversation out of the projects.
    let dir = if is_joined || dir.join(LEFT).exists() { dir.canonicalize().map_err(|e| format!("读取 Bot 目录失败：{e}"))? } else { dir };
    Ok(ResidentBot { id: id.into(), name: profile.name, purpose: profile.purpose, appearance: profile.appearance,
        dir: dir.to_string_lossy().into_owned(), joined: is_joined })
}

fn clear_left(dir: &Path) -> Result<(), String> {
    match std::fs::remove_file(dir.join(LEFT)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("加入 Bot 失败：{e}")),
        _ => Ok(()),
    }
}

fn instructions(name: &str, purpose: &str) -> String {
    format!("# {}\n\n你是用户在 Shifu 中的常驻 Bot「{}」。{}\n\n这个目录是你的独立工作区。按用户当次的话开始工作，只读取相关材料。\n", name, name, purpose)
}

/// A single app-owned gate covers joining and the load/create sequence of opening a resident task.
#[derive(Default)]
pub struct Bots {
    lock: Mutex<()>,
}

impl Bots {
    fn guard(&self) -> std::sync::MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn descriptions(cfg: &Config) -> Result<Vec<ResidentBot>, String> {
        let mut listed: Vec<_> = TEMPLATES.iter().map(|t| description(cfg, t.id)).collect::<Result<_, _>>()?;
        let root = cfg.home.join("bots");
        let entries = match std::fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(listed),
            Err(e) => return Err(format!("读取猫咪名单失败：{e}")),
        };
        let mut custom = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| format!("读取猫咪名单失败：{e}"))?;
            let id = entry.file_name().to_string_lossy().into_owned();
            if template(&id).is_none() && validate_id(&id).is_ok() && entry.path().join(PROFILE).is_file() {
                custom.push(id);
            }
        }
        custom.sort();
        for id in custom { listed.push(description(cfg, &id)?); }
        Ok(listed)
    }

    pub fn list(&self, cfg: &Config) -> Result<Vec<ResidentBot>, String> {
        let _held = self.guard();
        Self::descriptions(cfg)
    }

    pub fn join(&self, cfg: &Config, id: &str) -> Result<Vec<ResidentBot>, String> {
        validate_id(id)?;
        let _held = self.guard();
        let bot = description(cfg, id)?;
        let folder = format!("bots/{id}");
        let dir = crate::pm::ensure_role_dir(cfg, &folder, &instructions(&bot.name, &bot.purpose))?;
        if !bot.joined { write_atomic(&Path::new(&dir).join(JOINED), id)?; }
        clear_left(Path::new(&dir))?;
        Self::descriptions(cfg)
    }

    /// Off the 我的 Bot list; nothing in its folder is deleted.
    pub fn leave(&self, cfg: &Config, id: &str) -> Result<Vec<ResidentBot>, String> {
        validate_id(id)?;
        let _held = self.guard();
        let dir = cfg.home.join("bots").join(id);
        if dir.is_dir() { write_atomic(&dir.join(LEFT), id)?; }
        Self::descriptions(cfg)
    }

    pub fn save(&self, cfg: &Config, id: &str, profile: Profile) -> Result<Vec<ResidentBot>, String> {
        validate_id(id)?;
        let profile = validate_profile(id, profile)?;
        let _held = self.guard();
        // The profile is the commit marker for customization. An interrupted first save can retry with the same id.
        let folder = format!("bots/{id}");
        let dir = crate::pm::ensure_role_dir(cfg, &folder, &instructions(&profile.name, &profile.purpose))?;
        let text = serde_json::to_string_pretty(&profile).map_err(|e| format!("保存 Bot 设置失败：{e}"))?;
        write_atomic(&Path::new(&dir).join(PROFILE), &text)?;
        clear_left(Path::new(&dir))?;
        Self::descriptions(cfg)
    }

    pub fn open<T>(&self, cfg: &Config, id: &str, open: impl FnOnce(&ResidentBot) -> Result<T, String>) -> Result<T, String> {
        validate_id(id)?;
        let _held = self.guard();
        let bot = description(cfg, id)?;
        if !bot.joined { return Err("先把这只 Bot 加入我的 Bot".into()); }
        // Serialize the entire task lookup/start operation, including calls arriving from separate windows.
        open(&bot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

    fn config(tag: &str) -> Config {
        let home = std::env::temp_dir().join(format!("shifu-resident-bot-{tag}-{}", std::process::id()));
        assert!(!home.exists());
        Config { home, socket: "unused-bot-test".into(), shifu_bin: "unused-shifu".into(), user_statusline: None }
    }

    #[test]
    fn listing_and_invalid_ids_do_not_create_anything() {
        let cfg = config("list");
        let bots = Bots::default();
        let listed = bots.list(&cfg).unwrap();
        assert_eq!(listed.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
                   ["tabby", "calico", "tuxedo", "rabbit", "puppy", "red-panda", "hedgehog"]);
        assert_eq!(listed[3..].iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
                   ["月牙", "可可", "枫糖", "松果"]);
        assert!(listed[3..].iter().all(|b| b.appearance.is_none()));
        assert!(listed.iter().all(|b| !b.joined));
        assert!(bots.join(&cfg, "../../pm").is_err());
        assert!(bots.open(&cfg, "tabby", |_| Ok(())).is_err());
        assert!(!cfg.home.exists());
    }

    #[test]
    fn join_is_persistent_without_creating_a_session_and_preserves_user_instructions() {
        let cfg = config("join");
        let bots = Bots::default();
        let first = bots.join(&cfg, "calico").unwrap();
        let cat = first.iter().find(|b| b.id == "calico").unwrap();
        assert!(cat.joined);
        assert!(!cfg.home.join("tasks.json").exists());
        assert!(!cfg.home.join("agents").exists());
        let instructions = Path::new(&cat.dir).join("AGENTS.md");
        std::fs::write(&instructions, "my custom instructions").unwrap();
        bots.join(&cfg, "calico").unwrap();
        assert_eq!(std::fs::read_to_string(&instructions).unwrap(), "my custom instructions");
        let restarted = Bots::default();
        assert!(restarted.list(&cfg).unwrap().iter().find(|b| b.id == "calico").unwrap().joined);
        assert_eq!(restarted.open(&cfg, "calico", |b| Ok(b.dir.clone())).unwrap(), cat.dir);
        std::fs::remove_dir_all(cfg.home).unwrap();
    }

    #[test]
    fn concurrent_opens_serialize_the_lookup_and_start_sequence() {
        let cfg = Arc::new(config("concurrent"));
        let bots = Arc::new(Bots::default());
        bots.join(&cfg, "tabby").unwrap();
        let active = Arc::new(AtomicUsize::new(0));
        let max = Arc::new(AtomicUsize::new(0));
        let workers: Vec<_> = (0..6).map(|_| {
            let (bots, cfg, active, max) = (bots.clone(), cfg.clone(), active.clone(), max.clone());
            std::thread::spawn(move || bots.open(&cfg, "tabby", |_| {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                max.fetch_max(count, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(3));
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            }).unwrap())
        }).collect();
        for worker in workers { worker.join().unwrap(); }
        assert_eq!(max.load(Ordering::SeqCst), 1);
        std::fs::remove_dir_all(&cfg.home).unwrap();
    }

    #[test]
    fn leaving_keeps_the_folder_and_joining_again_returns_to_it() {
        let cfg = config("leave");
        let bots = Bots::default();
        let custom = "cat-52a016db-f14c-4b89-a47e-dd56868d2236";
        bots.join(&cfg, "tabby").unwrap();
        bots.save(&cfg, custom, profile("雪团")).unwrap();
        let before: Vec<_> = bots.list(&cfg).unwrap().into_iter().filter(|b| b.joined).map(|b| b.dir).collect();
        for dir in &before { std::fs::write(Path::new(dir).join("conversation.txt"), "keep").unwrap(); }
        for id in ["tabby", custom] {
            let listed = bots.leave(&cfg, id).unwrap();
            assert!(!listed.iter().find(|b| b.id == id).unwrap().joined);
        }
        let restarted = Bots::default();
        assert!(restarted.list(&cfg).unwrap().iter().all(|b| !b.joined));
        assert_eq!(restarted.list(&cfg).unwrap().iter().filter(|b| b.id == custom).count(), 1);
        assert!(restarted.open(&cfg, custom, |_| Ok(())).is_err());
        restarted.join(&cfg, "tabby").unwrap();
        // Saving a left cat's profile (its edit form says 加入) joins it again too.
        let after = restarted.save(&cfg, custom, profile("月白")).unwrap();
        let dirs: Vec<_> = after.into_iter().filter(|b| b.joined).map(|b| b.dir).collect();
        assert_eq!(dirs, before);
        for dir in &dirs { assert_eq!(std::fs::read_to_string(Path::new(dir).join("conversation.txt")).unwrap(), "keep"); }
        assert!(bots.leave(&cfg, "../../pm").is_err());
        std::fs::remove_dir_all(cfg.home).unwrap();
    }

    fn profile(name: &str) -> Profile {
        Profile { name: name.into(), purpose: "一起写作".into(), appearance: Some(Appearance {
            coat: Coat::Cream, ears: Ears::Folded, tail: Tail::Short, eyes: Eyes::Blue,
        }) }
    }

    #[test]
    fn animal_settings_keep_the_fixed_portrait_and_reopen_the_same_folder() {
        let cfg = config("animal");
        let bots = Bots::default();
        let id = "rabbit";
        assert!(bots.save(&cfg, id, profile("灰耳兔")).is_err());
        assert!(!cfg.home.exists());
        bots.join(&cfg, id).unwrap();
        let dir = bots.open(&cfg, id, |b| Ok(b.dir.clone())).unwrap();
        let settings = serde_json::from_str(r#"{"name":"小灰","purpose":"一起读书"}"#).unwrap();
        bots.save(&cfg, id, settings).unwrap();
        bots.leave(&cfg, id).unwrap();
        let restarted = Bots::default();
        restarted.join(&cfg, id).unwrap();
        restarted.open(&cfg, id, |bot| {
            assert_eq!(bot.dir, dir);
            assert_eq!(bot.name, "小灰");
            assert_eq!(bot.purpose, "一起读书");
            assert_eq!(serde_json::to_value(bot).unwrap()["appearance"], serde_json::Value::Null);
            Ok(())
        }).unwrap();
        std::fs::remove_dir_all(cfg.home).unwrap();
    }

    #[test]
    fn legacy_cat_edit_keeps_folder_and_user_instructions() {
        let cfg = config("edit");
        let bots = Bots::default();
        let initial = bots.join(&cfg, "tabby").unwrap();
        let original = &initial[0];
        assert_eq!(original.appearance.as_ref().unwrap().coat, Coat::Orange);
        let dir = Path::new(&original.dir);
        std::fs::write(dir.join("AGENTS.md"), "user notes").unwrap();
        std::fs::write(dir.join("conversation.txt"), "keep this conversation").unwrap();
        std::fs::write(dir.join("profile.json"), "existing project artifact").unwrap();
        assert_eq!(bots.list(&cfg).unwrap()[0].name, "金橘");
        let edited = bots.save(&cfg, "tabby", profile("雪团")).unwrap();
        assert_eq!(edited[0].id, original.id);
        assert_eq!(edited[0].dir, original.dir);
        assert_eq!(edited[0].name, "雪团");
        assert_eq!(edited[0].appearance.as_ref().unwrap().eyes, Eyes::Blue);
        assert_eq!(std::fs::read_to_string(dir.join("AGENTS.md")).unwrap(), "user notes");
        assert!(std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap().contains("雪团"));
        assert_eq!(std::fs::read_to_string(dir.join("conversation.txt")).unwrap(), "keep this conversation");
        assert_eq!(std::fs::read_to_string(dir.join("profile.json")).unwrap(), "existing project artifact");
        assert!(dir.join(PROFILE).is_file());
        assert_eq!(Bots::default().open(&cfg, "tabby", |b| Ok(b.name.clone())).unwrap(), "雪团");
        std::fs::remove_dir_all(cfg.home).unwrap();
    }

    #[test]
    fn custom_cats_persist_after_every_template_is_joined_and_saving_is_repeatable() {
        let cfg = config("custom");
        let bots = Bots::default();
        for template in &TEMPLATES { bots.join(&cfg, template.id).unwrap(); }
        let ids = ["cat-52a016db-f14c-4b89-a47e-dd56868d2236", "cat-a11d3279-5abb-448c-a82b-2cc248905798"];
        for id in ids {
            bots.save(&cfg, id, profile("雪团")).unwrap();
            bots.save(&cfg, id, profile("雪团")).unwrap();
        }
        let listed = Bots::default().list(&cfg).unwrap();
        assert_eq!(listed.len(), TEMPLATES.len() + ids.len());
        assert!(listed.iter().all(|b| b.joined));
        assert!(!cfg.home.join("tasks.json").exists());
        assert!(!cfg.home.join("agents").exists());
        let first = bots.open(&cfg, ids[0], |b| Ok(b.dir.clone())).unwrap();
        bots.save(&cfg, ids[0], profile("月白")).unwrap();
        assert_eq!(Bots::default().open(&cfg, ids[0], |b| Ok(b.dir.clone())).unwrap(), first);
        assert_ne!(first, bots.open(&cfg, ids[1], |b| Ok(b.dir.clone())).unwrap());
        std::fs::remove_dir_all(cfg.home).unwrap();
    }

    #[test]
    fn invalid_profiles_and_paths_leave_storage_untouched() {
        let cfg = config("invalid-save");
        let bots = Bots::default();
        assert!(bots.save(&cfg, "../../pm", profile("猫咪")).is_err());
        assert!(bots.save(&cfg, "cat-../pm", profile("猫咪")).is_err());
        assert!(bots.save(&cfg, "cat-52A016DB-f14c-4b89-a47e-dd56868d2236", profile("猫咪")).is_err());
        assert!(bots.save(&cfg, "tabby", profile("  ")).is_err());
        assert!(bots.save(&cfg, "tabby", profile(&"猫".repeat(25))).is_err());
        assert!(bots.save(&cfg, "tabby", profile("猫\n咪")).is_err());
        let missing_appearance = serde_json::from_str(r#"{"name":"金橘","purpose":"写作"}"#).unwrap();
        assert!(bots.save(&cfg, "tabby", missing_appearance).is_err());
        let mut invalid = profile("猫咪");
        invalid.purpose = "写".repeat(161);
        assert!(bots.save(&cfg, "tabby", invalid).is_err());
        let bad = r#"{"name":"猫咪","purpose":"写作","appearance":{"coat":"purple","ears":"pointed","tail":"long","eyes":"green"}}"#;
        assert!(serde_json::from_str::<Profile>(bad).is_err());
        assert!(!cfg.home.exists());
    }

}
