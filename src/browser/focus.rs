//! A visible (`--show`) Shifu Chrome activates itself when it starts and may again when it opens a window.
//! Chrome has no launch option against that, so while a command for a visible session runs, this watches the
//! frontmost app and hands the foreground straight back when the agent's own Chrome took it.
use std::process::Command;

/// The frontmost app's pid, read fresh through LaunchServices (`lsappinfo`), not a cached AppKit value.
pub fn frontmost_pid() -> Option<i32> {
    let front = Command::new("/usr/bin/lsappinfo").arg("front").output().ok()?;
    let asn = String::from_utf8_lossy(&front.stdout).trim().to_string();
    if !asn.starts_with("ASN:") { return None; }
    let info = Command::new("/usr/bin/lsappinfo").args(["info", "-only", "pid", &asn]).output().ok()?;
    parse_pid(&String::from_utf8_lossy(&info.stdout))
}

/// `"pid"=17056` with `-only pid`; some macOS versions ignore `-only` and print `pid = 17056 ...` in a block.
fn parse_pid(info: &str) -> Option<i32> {
    info.match_indices("pid").find_map(|(at, _)| {
        let before = info[..at].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric()) { return None; }
        let rest = info[at + 3..].trim_start_matches('"').trim_start();
        let digits: String = rest.strip_prefix('=')?.trim_start().chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    })
}

/// Whether `pid` is a browser that an automation driver started (Playwright passes `--remote-debugging-pipe`),
/// never the person's own Chrome, which they start without it.
pub fn is_automation_browser(pid: i32) -> bool {
    Command::new("/bin/ps").args(["-o", "args=", "-p", &pid.to_string()]).output().ok()
        .is_some_and(|out| is_automation_args(&String::from_utf8_lossy(&out.stdout)))
}

fn is_automation_args(args: &str) -> bool {
    args.contains("--remote-debugging-pipe") && args.contains("--user-data-dir")
}

/// Activates the app with `pid` without raising all of its windows. Returns whether macOS accepted it.
pub fn activate(pid: i32) -> bool {
    use std::ffi::{c_char, c_void};
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}
    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }
    // SAFETY: Objective-C messages with the exact signatures of +[NSRunningApplication
    // runningApplicationWithProcessIdentifier:] (pid_t -> id) and -[NSRunningApplication activateWithOptions:]
    // (NSUInteger -> BOOL); a nil app is checked before the second message.
    unsafe {
        let class = objc_getClass(c"NSRunningApplication".as_ptr());
        if class.is_null() { return false; }
        let lookup: extern "C" fn(*mut c_void, *mut c_void, i32) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ());
        let app = lookup(class, sel_registerName(c"runningApplicationWithProcessIdentifier:".as_ptr()), pid);
        if app.is_null() { return false; }
        let send: extern "C" fn(*mut c_void, *mut c_void, usize) -> bool = std::mem::transmute(objc_msgSend as *const ());
        send(app, sel_registerName(c"activateWithOptions:".as_ptr()), 0)
    }
}

/// Watches one command of a visible session: `poll` while it runs, `finish` after.
pub struct Guard {
    previous: Option<i32>,
    started: std::time::Instant,
    checked: Option<(i32, bool)>,
    returned: u32,
    failed: u32,
    left_to_person: bool,
}

impl Guard {
    pub fn start() -> Guard {
        Guard { previous: frontmost_pid(), started: std::time::Instant::now(), checked: None, returned: 0, failed: 0, left_to_person: false }
    }

    /// If the agent's Chrome took the foreground from the person's app, give it back. Not when the person
    /// typed or moved the mouse since the command started: they may have clicked into the window to watch.
    pub fn poll(&mut self) {
        let Some(previous) = self.previous else { return };
        let Some(now) = frontmost_pid() else { return };
        if now == previous { return; }
        let automation = match self.checked {
            Some((pid, automation)) if pid == now => automation,
            _ => {
                let automation = is_automation_browser(now);
                self.checked = Some((now, automation));
                automation
            }
        };
        if !automation { return; }
        let person_idle = crate::computer::foreground::seconds_since_person_input();
        if person_idle.is_none_or(|idle| idle < self.started.elapsed().as_secs_f64()) {
            self.left_to_person = true;
            return;
        }
        if activate(previous) { self.returned += 1 } else { self.failed += 1 }
    }

    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"foregroundReturned": self.returned, "foregroundReturnFailed": self.failed, "leftToPerson": self.left_to_person})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_launch_services_pid_and_tells_automation_chrome_from_the_persons() {
        assert_eq!(parse_pid("\"pid\"=17056\n"), Some(17056));
        assert_eq!(parse_pid("    bundleID=[ NULL ] \n    pid = 17056 !cgsConnection ppid = 1"), Some(17056));
        assert_eq!(parse_pid("\n"), None);
        assert!(is_automation_args("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome --remote-debugging-pipe --user-data-dir=/x"));
        assert!(!is_automation_args("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"));
    }
}
