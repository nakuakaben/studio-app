//! In-memory simulator. Never performs real downloads, installs or process launches.
use crate::domain::{AppState, Application};
pub fn start(app: &mut Application) {
    if app.state.busy() || app.state == AppState::Installed {
        return;
    }
    if !matches!(app.state, AppState::Error { .. }) {
        app.idle_state = app.state.clone();
    }
    app.state = AppState::Downloading {
        progress: 0.0,
        paused: false,
    };
}
pub fn cancel(app: &mut Application) {
    app.state = app.idle_state.clone();
}
pub fn tick(app: &mut Application, elapsed: f32) {
    let elapsed = elapsed.clamp(0.0, 0.1);
    match &mut app.state {
        AppState::Downloading {
            progress,
            paused: false,
        } => {
            *progress = (*progress + elapsed * 0.16).min(1.0);
            if *progress >= 1.0 {
                app.state = AppState::Installing { progress: 0.0 };
            }
        }
        AppState::Installing { progress } => {
            *progress = (*progress + elapsed * 0.28).min(1.0);
            if *progress >= 1.0 {
                app.installed_version = Some(app.version);
                app.state = AppState::Installed;
                app.idle_state = AppState::Installed;
            }
        }
        _ => {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_can_pause_cancel_retry_and_finish_without_io() {
        let mut apps = crate::catalog::demonstration();
        let app = &mut apps[0];
        start(app);
        tick(app, 0.1);
        let progress = if let AppState::Downloading { progress, paused } = &mut app.state {
            *paused = true;
            *progress
        } else {
            panic!("expected transfer")
        };
        tick(app, 0.1);
        assert_eq!(
            app.state,
            AppState::Downloading {
                progress,
                paused: true
            }
        );
        cancel(app);
        assert_eq!(app.state, AppState::UpdateAvailable);
        assert_eq!(app.installed_version, Some("1.6.0"));
        start(app);
        for _ in 0..1200 {
            tick(app, 0.1);
        }
        assert_eq!(app.state, AppState::Installed);
        assert_eq!(app.installed_version, Some(app.version));
    }
    #[test]
    fn retry_error_and_cancel_restores_not_installed() {
        let mut apps = crate::catalog::demonstration();
        let app = &mut apps[7];
        start(app);
        cancel(app);
        assert_eq!(app.state, AppState::NotInstalled);
    }
}
