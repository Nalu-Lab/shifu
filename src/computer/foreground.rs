//! When an agent may take the person's foreground: only while they are away from the keyboard and mouse,
//! or never. Foreground means bringing a window forward (`--restore-window`, `--foreground-fallback`) or the
//! system clipboard (`paste-text`). The helper hands back the frontmost app, window and pointer afterwards.
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const DEFAULT_IDLE_SECONDS: f64 = 10.0;
/// Without `--idle-wait-ms` (or `--timeout-ms`) a foreground request waits this long for the person to pause:
/// long enough that a person who is typing is rarely refused, short enough for an agent's two-minute tool call.
pub const DEFAULT_WAIT: Duration = Duration::from_millis(90_000);
/// The most `--idle-wait-ms` may ask for.
pub const MAX_IDLE_WAIT_MS: u64 = 600_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// Foreground only after `idle_seconds` without keyboard or mouse input.
    WhenIdle,
    /// Never; foreground requests are refused before anything is sent.
    Never,
}

pub struct Gate {
    pub mode: Mode,
    pub idle_seconds: f64,
    /// Seconds since the person's last keyboard or mouse input; `None` when it cannot be read.
    pub person_idle: Box<dyn Fn() -> Option<f64>>,
    pub sleep: Box<dyn Fn(Duration)>,
    /// Where the helper's own foreground input is remembered (`foreground-input.json` in the computer home).
    pub clock: Option<PathBuf>,
}

/// Foreground clicks and keys are posted at the HID tap, so the input clock counts them as the person's.
/// Each admitted request is remembered with the person's real last input; a later reading that falls inside
/// that window is the agent's own and is replaced by the remembered one.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OwnInput {
    pub from: f64,
    pub until: f64,
    pub person_last_input: f64,
}

/// Seconds since the person's input, given the raw clock and the agent's last foreground window.
pub fn person_idle_at(now: f64, raw_idle: f64, own: Option<OwnInput>) -> f64 {
    let last_input = now - raw_idle;
    match own {
        Some(own) if last_input >= own.from - 0.05 && last_input <= own.until + 0.5 => now - own.person_last_input,
        _ => raw_idle,
    }
}

fn unix_now() -> f64 { SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64()) }

impl Gate {
    /// Env (`SHIFU_COMPUTER_FOREGROUND`, `SHIFU_COMPUTER_IDLE_SECONDS`) over `<SHIFU_HOME>/settings.json`
    /// (`computer_foreground`, `computer_idle_seconds`) over the defaults (when idle, 10 s).
    pub fn from_environment(computer_home: PathBuf) -> Result<Gate, String> {
        let settings = std::fs::read_to_string(crate::agent::shifu_home().join("settings.json")).ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok()).unwrap_or(Value::Null);
        let (mode, idle_seconds) = policy(
            std::env::var("SHIFU_COMPUTER_FOREGROUND").ok().as_deref(),
            std::env::var("SHIFU_COMPUTER_IDLE_SECONDS").ok().as_deref(),
            &settings,
        )?;
        Ok(Gate { mode, idle_seconds, person_idle: Box::new(seconds_since_person_input), sleep: Box::new(std::thread::sleep),
            clock: Some(computer_home.join("foreground-input.json")) })
    }

    fn own_input(&self) -> Option<OwnInput> {
        serde_json::from_slice(&std::fs::read(self.clock.as_ref()?).ok()?).ok()
    }

    /// Seconds since the person last used the keyboard or mouse, not counting the agent's own foreground input.
    pub fn person_idle(&self) -> Option<f64> {
        let raw = (self.person_idle)()?;
        Some(person_idle_at(unix_now(), raw, self.own_input()))
    }

    /// Called after an admitted request: input until now was the agent's.
    pub fn remember(&self, admitted: &Value) {
        let (Some(path), Some(from), Some(idle)) = (&self.clock, admitted["admittedAt"].as_f64(), admitted["idleSeconds"].as_f64()) else { return };
        let own = OwnInput { from, until: unix_now(), person_last_input: from - idle };
        if let Ok(bytes) = serde_json::to_vec(&own) { let _ = std::fs::write(path, bytes); }
    }
}

pub fn policy(mode: Option<&str>, idle: Option<&str>, settings: &Value) -> Result<(Mode, f64), String> {
    let mode_text = mode.map(str::to_string).or_else(|| settings["computer_foreground"].as_str().map(str::to_string));
    let mode = match mode_text.as_deref().map(str::trim) {
        None | Some("" | "idle" | "when-idle" | "when_idle") => Mode::WhenIdle,
        Some("never" | "off" | "0" | "false") => Mode::Never,
        Some(other) => return Err(format!("SHIFU_COMPUTER_FOREGROUND / computer_foreground 只接受 idle 或 never，不认识 {other}")),
    };
    let idle_seconds = match idle {
        Some(text) => text.trim().parse::<f64>().map_err(|_| "SHIFU_COMPUTER_IDLE_SECONDS 需要秒数".to_string())?,
        None => settings["computer_idle_seconds"].as_f64().unwrap_or(DEFAULT_IDLE_SECONDS),
    };
    if !idle_seconds.is_finite() || !(0.0..=86_400.0).contains(&idle_seconds) {
        return Err("空闲秒数需在 0 到 86400 之间".into());
    }
    Ok((mode, idle_seconds))
}

/// Why a request takes the foreground or the clipboard, if it does at the start.
pub fn reason(method: &str, params: &Value) -> Option<&'static str> {
    if params["restoreWindow"] == true { Some("restore_window") }
    else if method == "pasteText" { Some("clipboard") }
    else { None }
}

/// Waits up to `budget` for the person to pause. `Ok(details)` to go ahead, `Err(response)` to refuse.
pub fn admit(gate: &Gate, reason: &str, budget: Duration) -> Result<Value, Value> {
    let required = gate.idle_seconds;
    if gate.mode == Mode::Never {
        return Err(refusal("foreground_disabled", reason, None, required, 0,
            "本机设置禁止 Agent 使用前台或剪贴板（SHIFU_COMPUTER_FOREGROUND / settings.json 的 computer_foreground 为 never）；请改用后台读取、后台点击或 set-value，不要绕过设置"));
    }
    let start = Instant::now();
    loop {
        let idle = gate.person_idle();
        if let Some(seconds) = idle.filter(|&seconds| seconds >= required) {
            return Ok(json!({"reason":reason,"idleSeconds":round(seconds),"requiredIdleSeconds":required,
                "waitedMs":start.elapsed().as_millis() as u64,"admittedAt":unix_now()}));
        }
        let elapsed = start.elapsed();
        if elapsed >= budget {
            return Err(refusal("user_active", reason, idle, required, elapsed.as_millis() as u64,
                "本人正在使用这台 Mac，未切换前台、未动剪贴板；请改用后台路径（get-app-state、后台 click、set-value、type-text），或稍后再试，不要去掉门槛或换工具绕过"));
        }
        let wait = idle.map_or(Duration::from_millis(250), |s| Duration::from_secs_f64((required - s).max(0.05)));
        (gate.sleep)(wait.min(Duration::from_millis(250)).min(budget - elapsed));
    }
}

fn refusal(code: &str, reason: &str, idle: Option<f64>, required: f64, waited: u64, message: &str) -> Value {
    json!({"ok":false,"error":{"code":code,"message":message},"result":{
        "foregroundGate":{"reason":reason,"idleSeconds":idle.map(round),"requiredIdleSeconds":required,"waitedMs":waited},
        "recovery":{"automaticRetry":false,"actionReported":false,"nextStep":if code == "user_active" {"use_background_or_wait"} else {"use_background"},"message":message}}})
}

fn round(seconds: f64) -> f64 { (seconds * 10.0).round() / 10.0 }

/// Hardware keyboard and mouse event types only: the catch-all counter also resets on the helper's own
/// background (pid-posted) clicks, which would make the agent look like a busy person.
#[cfg(target_os = "macos")]
pub fn seconds_since_person_input() -> Option<f64> {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }
    const HID_SYSTEM_STATE: i32 = 1;
    // keyDown, flagsChanged, mouseMoved, left/right/other mouse down, leftMouseDragged, scrollWheel.
    let seconds = [10u32, 12, 5, 1, 3, 25, 6, 22].iter()
        // SAFETY: a pure query of the window server's input clock; no pointers are passed.
        .map(|&kind| unsafe { CGEventSourceSecondsSinceLastEventType(HID_SYSTEM_STATE, kind) })
        .fold(f64::INFINITY, f64::min);
    seconds.is_finite().then_some(seconds)
}

#[cfg(not(target_os = "macos"))]
pub fn seconds_since_person_input() -> Option<f64> { None }

#[cfg(test)]
pub fn test_gate(mode: Mode, idle: Vec<f64>) -> Gate {
    let samples = std::cell::RefCell::new(idle.into_iter());
    let last = std::cell::Cell::new(0.0);
    Gate {
        mode, idle_seconds: DEFAULT_IDLE_SECONDS,
        person_idle: Box::new(move || { if let Some(v) = samples.borrow_mut().next() { last.set(v); } Some(last.get()) }),
        sleep: Box::new(|_| {}),
        clock: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_overrides_settings_and_bad_values_are_errors_not_silent_defaults() {
        assert_eq!(policy(None, None, &Value::Null).unwrap(), (Mode::WhenIdle, 10.0));
        let settings = json!({"computer_foreground":"never","computer_idle_seconds":30});
        assert_eq!(policy(None, None, &settings).unwrap(), (Mode::Never, 30.0));
        assert_eq!(policy(Some("idle"), Some("5"), &settings).unwrap(), (Mode::WhenIdle, 5.0));
        assert!(policy(Some("sometimes"), None, &Value::Null).is_err());
        assert!(policy(None, Some("-1"), &Value::Null).is_err());
        assert!(policy(None, Some("soon"), &Value::Null).is_err());
    }

    #[test]
    fn the_agents_own_foreground_input_does_not_count_as_the_person() {
        let own = OwnInput { from: 100.0, until: 101.0, person_last_input: 40.0 };
        // Clock reset at 100.8 by the agent's click: the person was last seen at 40.
        assert_eq!(person_idle_at(102.0, 1.2, Some(own)), 62.0);
        // Input after the window is the person's.
        assert_eq!(person_idle_at(105.0, 2.0, Some(own)), 2.0);
        assert_eq!(person_idle_at(105.0, 2.0, None), 2.0);
    }

    #[test]
    fn only_foreground_and_clipboard_requests_are_gated() {
        assert_eq!(reason("click", &json!({"restoreWindow":true})), Some("restore_window"));
        assert_eq!(reason("pasteText", &json!({})), Some("clipboard"));
        for method in ["click", "typeText", "pressKey", "getAppState", "setValue", "scroll"] {
            assert_eq!(reason(method, &json!({})), None, "{method}");
        }
    }

    #[test]
    fn waits_for_a_pause_then_refuses_with_a_next_step() {
        let gate = test_gate(Mode::WhenIdle, vec![2.0, 4.0, 12.0]);
        let ok = admit(&gate, "restore_window", Duration::from_secs(60)).unwrap();
        assert_eq!(ok["idleSeconds"], 12.0);
        let busy = test_gate(Mode::WhenIdle, vec![0.5]);
        let refused = admit(&busy, "clipboard", Duration::ZERO).unwrap_err();
        assert_eq!(refused["error"]["code"], "user_active");
        assert_eq!(refused["result"]["recovery"]["automaticRetry"], false);
        assert_eq!(refused["result"]["foregroundGate"]["reason"], "clipboard");
        let never = test_gate(Mode::Never, vec![1000.0]);
        assert_eq!(admit(&never, "restore_window", Duration::from_secs(5)).unwrap_err()["error"]["code"], "foreground_disabled");
    }
}
