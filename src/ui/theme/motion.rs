//! Best-effort OS preference lookup, once at startup and outside the UI thread.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Clone, Default)]
pub struct Preference(Arc<AtomicBool>);
impl Preference {
    pub fn start(ctx: &eframe::egui::Context) -> Self {
        // No motion until the initial preference lookup completes.
        let preference = Self(Arc::new(AtomicBool::new(true)));
        let result = preference.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let forced = std::env::var("STUDIO_APP_REDUCE_MOTION").is_ok_and(|s| s == "1");
            result.0.store(
                forced || system_reduced().unwrap_or(false),
                Ordering::Relaxed,
            );
            ctx.request_repaint();
        });
        preference
    }
    pub fn reduced(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(target_os = "macos")]
fn system_reduced() -> Option<bool> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    // Fixed, read-only OS preference query. No catalogue data or shell.
    let mut child = Command::new("/usr/bin/defaults")
        .args(["read", "com.apple.universalaccess", "reduceMotion"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut output = String::new();
                child
                    .stdout
                    .take()?
                    .take(64)
                    .read_to_string(&mut output)
                    .ok()?;
                return if status.success() {
                    parse(&output)
                } else {
                    None
                };
            }
            Ok(None) if start.elapsed() < Duration::from_secs(2) => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}
#[cfg(windows)]
fn system_reduced() -> Option<bool> {
    use winreg::{enums::*, RegKey};
    // Honor the user's explicit disabling of window animation; never write it.
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(r"Control Panel\Desktop\WindowMetrics", KEY_QUERY_VALUE)
        .ok()?;
    parse(&key.get_value::<String, _>("MinAnimate").ok()?).map(|enabled| !enabled)
}
#[cfg(not(any(windows, target_os = "macos")))]
fn system_reduced() -> Option<bool> {
    None
}
#[cfg(any(windows, target_os = "macos", test))]
fn parse(value: &str) -> Option<bool> {
    match value.trim() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn preference_values_are_explicit() {
        assert_eq!(super::parse("1\n"), Some(true));
        assert_eq!(super::parse("0"), Some(false));
        assert_eq!(super::parse("unknown"), None);
    }
}
