use super::*;
use crate::installer::{self, worker::Event as InstallEvent};
impl StudioApp {
    pub(super) fn start_installation(&mut self, index: usize) {
        let app = &mut self.apps[index];
        if app.fictional || app.installed() || app.state().busy() {
            return;
        }
        if self.installer_job.is_some() {
            self.notice = Some("Une installation est déjà en cours.".into());
            return;
        }
        if !installer::model::can_request(&app.metadata) {
            self.notice =
                Some("Aucun package compatible pris en charge pour cette application.".into());
            return;
        }
        app.operation =
            Operation::Native(installer::Progress::phase(installer::Phase::Authorizing));
        // repaint callback comes from poll_installer's bounded polling, never I/O on egui.
        self.installer_job = Some(installer::worker::spawn(app.metadata.clone(), || {}));
    }
    pub(super) fn poll_installer(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.installer_job else {
            return;
        };
        let index = self.apps.iter().position(|app| {
            serde_json::to_vec(&app.metadata).ok() == serde_json::to_vec(&job.application).ok()
        });
        if index.is_none() {
            job.cancel.cancel();
        }
        let mut finished = false;
        loop {
            match job.receiver.try_recv() {
                Ok(InstallEvent::Progress(progress)) => {
                    if let Some(index) = index {
                        self.apps[index].operation = Operation::Native(progress);
                    }
                }
                Ok(InstallEvent::Finished(result)) => {
                    if let Some(index) = index {
                        let app = &mut self.apps[index];
                        match result {
                            Ok(report) => {
                                app.local.version =
                                    report.installations.first().and_then(|i| i.version.clone());
                                app.local.installations = report.installations;
                                app.local.errors = report.errors;
                                app.operation = Operation::Idle;
                                self.notice = Some(format!(
                                    "{} installé et confirmé par la détection native.",
                                    app.name
                                ));
                            }
                            Err(error) => {
                                self.notice = Some(error.message.clone());
                                app.operation = Operation::NativeError(error);
                            }
                        }
                    }
                    finished = true;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if let Some(index) = index {
                        self.apps[index].operation=Operation::NativeError(installer::Failure::permanent("Le worker d’installation s’est arrêté. Vérifiez l’état local avant une nouvelle tentative."));
                    }
                    finished = true;
                    break;
                }
            }
        }
        if finished {
            self.installer_job = None;
            self.native_generation = self.native_generation.wrapping_add(1);
            self.native_pending = true;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}
