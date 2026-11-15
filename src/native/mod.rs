//! Read-only native identity detection and explicit launch. No installer or shell.
pub mod macos;
pub mod model;
pub mod windows;
pub mod worker;
pub use model::{DetectionRules, Installation, Report, Source};
use std::process::{Command, Stdio};

pub fn detect(rules: &DetectionRules) -> Report {
    if let Err(error) = model::validate(rules) {
        return Report {
            errors: vec![error],
            ..Default::default()
        };
    }
    #[cfg(target_os = "macos")]
    if let Some(rule) = &rules.macos {
        return macos::detect(rule, &macos::roots());
    }
    #[cfg(windows)]
    if let Some(rule) = &rules.windows {
        return windows::detect(rule, windows::records(rule));
    }
    Report::default()
}
/// Re-detect immediately before launching. Cached paths alone are never authority.
pub fn launch(rules: &DetectionRules, installation: &Installation) -> Result<(), String> {
    let fresh=detect(rules).installations.into_iter().find(|i|i.path==installation.path&&i.executable==installation.executable).ok_or("Application disparue, déplacée, inaccessible ou identité modifiée. Actualisez le catalogue.")?;
    launch_with(&fresh, |command| {
        #[cfg(target_os = "macos")]
        {
            let mut child = command
                .spawn()
                .map_err(|e| format!("Lancement impossible : {e}"))?;
            let start = std::time::Instant::now();
            loop {
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    if !status.success() {
                        return Err(format!("LaunchServices a refusé le lancement ({status}). Si macOS affiche une alerte, consultez la procédure Apple : https://support.apple.com/fr-fr/102445"));
                    }
                    break;
                }
                if start.elapsed() > std::time::Duration::from_secs(10) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Délai de réponse de LaunchServices dépassé".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        #[cfg(windows)]
        {
            let _child = command
                .spawn()
                .map_err(|e| format!("Lancement impossible : {e}"))?;
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = command;
            return Err("Lancement natif non pris en charge sur cet OS".into());
        }
        #[allow(unreachable_code)]
        Ok(())
    })
}
fn launch_with(
    installation: &Installation,
    run: impl FnOnce(&mut Command) -> Result<(), String>,
) -> Result<(), String> {
    let mut command = match installation.source {
        Source::MacBundle => {
            let mut c = Command::new("/usr/bin/open");
            c.arg("-a").arg(&installation.path);
            c
        }
        Source::WindowsRegistry { .. } => {
            let mut c = Command::new(&installation.executable);
            if let Some(parent) = installation.executable.parent() {
                c.current_dir(parent);
            }
            c
        }
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    run(&mut command)
}

#[cfg(test)]
mod tests;
