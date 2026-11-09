//! Explicit simulator. Neither catalog metadata nor real local state is modified.
use crate::domain::{AppState, Application, Operation};
pub fn start(app: &mut Application) {
    if app.state().busy() || app.state() == AppState::Installed {
        return;
    }
    app.operation = Operation::SimulatedDownload {
        progress: 0.0,
        paused: false,
    };
}
pub fn cancel(app: &mut Application) {
    app.operation = Operation::Idle;
}
pub fn tick(app: &mut Application, elapsed: f32) {
    let elapsed = elapsed.clamp(0.0, 0.1);
    match &mut app.operation {
        Operation::SimulatedDownload {
            progress,
            paused: false,
        } => {
            *progress = (*progress + elapsed * 0.16).min(1.0);
            if *progress >= 1.0 {
                app.operation = Operation::SimulatedInstall {
                    progress: 0.0,
                    target_version: app.available_version().to_owned(),
                };
            }
        }
        Operation::SimulatedInstall {
            progress,
            target_version,
        } => {
            *progress = (*progress + elapsed * 0.28).min(1.0);
            if *progress >= 1.0 {
                app.simulation.version = Some(target_version.clone());
                app.operation = Operation::Idle;
            }
        }
        _ => {}
    }
}
/// Optional preview scenarios are enabled only by a trusted local config, never a remote manifest.
pub fn seed(apps: &mut [Application]) {
    for app in apps {
        if !app.fictional {
            continue;
        }
        match app.id.as_str() {
            "sequence" => app.simulation.version = Some("1.6.0".into()),
            "prisme" => {
                app.simulation.version = Some("2.3.0".into());
                app.operation = Operation::SimulatedDownload {
                    progress: 0.44,
                    paused: true,
                };
            }
            "onde" => {
                app.simulation.version = Some("1.1.0".into());
                app.operation = Operation::SimulatedInstall {
                    progress: 0.38,
                    target_version: app.available_version().to_owned(),
                };
            }
            "forme" => app.simulation.version = Some(app.available_version().to_owned()),
            "lueur" => {
                app.operation = Operation::SimulationError(
                    "Transfert fictif interrompu. Réessayez la simulation.".into(),
                )
            }
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_pause_cancel_retry_keeps_real_state_and_catalog_unchanged() {
        let mut apps = crate::catalog::demonstration();
        let app = &mut apps[0];
        let release = app.release.version.clone();
        start(app);
        tick(app, 0.1);
        let progress = if let Operation::SimulatedDownload { progress, paused } = &mut app.operation
        {
            *paused = true;
            *progress
        } else {
            panic!("download expected")
        };
        tick(app, 0.1);
        assert_eq!(
            app.operation,
            Operation::SimulatedDownload {
                progress,
                paused: true
            }
        );
        cancel(app);
        assert_eq!(app.state(), AppState::UpdateAvailable);
        assert_eq!(app.simulation.version.as_deref(), Some("1.6.0"));
        start(app);
        for _ in 0..1200 {
            tick(app, 0.1);
        }
        assert_eq!(app.state(), AppState::Installed);
        assert_eq!(app.local.version, None);
        assert_eq!(app.simulation.version.as_deref(), Some(release.as_str()));
        assert_eq!(app.release.version, release);
    }
    #[test]
    fn retry_error_cancel_restores_not_installed() {
        let mut apps = crate::catalog::demonstration();
        let app = &mut apps[7];
        start(app);
        cancel(app);
        assert_eq!(app.state(), AppState::NotInstalled);
    }
}
