use super::*;
use crate::catalog::model::{Architecture, Os};
use crate::installer::{
    self,
    worker::{Confirmation, Event as InstallEvent},
};

pub(super) use installer::personal::HMP_ID;

enum Authorization {
    Pending(Receiver<installer::Result<Confirmation>>),
    Ready(Confirmation),
    Blocked(installer::Failure),
}

pub(super) struct InstallDialog {
    id: String,
    metadata: Vec<u8>,
    authorization: Authorization,
    cancelling: bool,
    completed: bool,
    acknowledged: bool,
    consent_sent: bool,
}

fn availability(app: &Application, target: Option<&(Os, Architecture)>) -> installer::Result<bool> {
    let Some((os, arch)) = target else {
        return Err(installer::Failure::permanent(
            "Installation native indisponible sur cette plateforme.",
        ));
    };
    if let Some(release) = &app.manual_release {
        return installer::personal::check_discovery(&app.metadata, release, os, arch)
            .map(|_| true);
    }
    installer::model::select(&app.metadata, os, arch).map(|_| false)
}

impl StudioApp {
    pub(super) fn request_installation(&mut self, ctx: &egui::Context, index: usize) {
        let app = &self.apps[index];
        if app.fictional || app.installed() || app.state().busy() {
            return;
        }
        if self.installer_job.is_some() {
            self.notice = Some("Une installation est déjà en cours.".into());
            return;
        }
        let authorization = match availability(app, installer::model::platform().as_ref()) {
            Ok(personal) => {
                let ctx = ctx.clone();
                Authorization::Pending(installer::worker::preview(
                    app.metadata.clone(),
                    personal,
                    move || {
                        ctx.request_repaint();
                    },
                ))
            }
            Err(error) => Authorization::Blocked(error),
        };
        self.install_dialog = Some(InstallDialog {
            id: app.id.clone(),
            metadata: serde_json::to_vec(&app.metadata).unwrap_or_default(),
            authorization,
            cancelling: false,
            completed: false,
            acknowledged: false,
            consent_sent: false,
        });
    }

    fn start_installation(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        confirmation: Confirmation,
    ) {
        let app = &mut self.apps[index];
        app.operation =
            Operation::Native(installer::Progress::phase(installer::Phase::Authorizing));
        let ctx = ctx.clone();
        self.installer_job = Some(installer::worker::spawn(
            app.metadata.clone(),
            confirmation,
            move || ctx.request_repaint(),
        ));
    }

    pub(super) fn installation_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.install_dialog.take() else {
            return;
        };
        let Some(index) = self.apps.iter().position(|app| {
            app.id == dialog.id
                && serde_json::to_vec(&app.metadata).is_ok_and(|bytes| bytes == dialog.metadata)
        }) else {
            self.notice = Some("Catalogue modifié. Rouvrez la fenêtre d’installation.".into());
            return;
        };
        if let Authorization::Pending(receiver) = &dialog.authorization {
            match receiver.try_recv() {
                Ok(Ok(path)) => dialog.authorization = Authorization::Ready(path),
                Ok(Err(error)) => dialog.authorization = Authorization::Blocked(error),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    dialog.authorization = Authorization::Blocked(installer::Failure::retry(
                        "Le contrôle d’approbation a été interrompu.",
                    ));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
            }
        }
        let app = &self.apps[index];
        let busy = self.installer_job.is_some();
        let awaiting = matches!(&app.operation, Operation::Native(progress)
            if progress.phase == installer::Phase::AwaitingConsent);
        let (bg, border, muted, blue) = theme::palette(ctx);
        let mut close = false;
        let mut start = None;
        let mut recheck = false;
        let mut open = false;
        let modal = egui::Modal::new(egui::Id::new("installation"))
            .frame(egui::Frame::new().fill(bg).stroke(Stroke::new(1.0_f32, border)))
            .show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 48.0).clamp(300.0, 600.0));
                egui::Frame::new()
                    .fill(theme::tinted(ctx, app.color))
                    .inner_margin(20)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let title = if app.id == HMP_ID { "HMP" } else { &app.name };
                        ui.heading(theme::title(format!("Installer {title}"), 28.0));
                        ui.label(mono(format!("VERSION {}", app.available_version())).color(muted));
                    });
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 180.0).max(120.0))
                    .show(ui, |ui| {
                        egui::Frame::new().inner_margin(20).show(ui, |ui| {
                            installation_summary(ui, app, &dialog.authorization);
                            if matches!(&dialog.authorization, Authorization::Ready(c) if c.personal)
                                && !busy && !dialog.completed {
                                ui.checkbox(&mut dialog.acknowledged, "J’autorise l’installation personnelle de HMP v0.1.0-beta.1, non certifié par Apple ou Microsoft.");
                            }
                            if let Operation::Native(progress) = &app.operation {
                                ui.label(mono(progress.phase.label()).color(blue));
                                if awaiting {
                                    ui.label("L’installateur NSIS va s’ouvrir avec les contrôles de Windows. Choisissez un dossier vide.");
                                } else if progress.total > 0 {
                                    let counter = format!(
                                        "{:.1} / {:.1} Mo",
                                        progress.bytes as f64 / 1_000_000.0,
                                        progress.total as f64 / 1_000_000.0
                                    );
                                    ui.add(egui::ProgressBar::new(progress.fraction()).text(counter));
                                } else {
                                    ui.spinner();
                                }
                            }
                            if let Operation::NativeError(error) = &app.operation {
                                ui.colored_label(Color32::LIGHT_RED, &error.message);
                            }
                            if dialog.completed {
                                ui.label("Installation confirmée par la détection native.");
                            }
                            if dialog.cancelling && busy {
                                ui.label("Annulation demandée…");
                            }
                            if !busy && !dialog.completed {
                                if let Some(page) = app.manual_installer_page() {
                                    ui.hyperlink_to("Ouvrir la release officielle ↗", page);
                                }
                            }
                        });
                    });
                ui.separator();
                egui::Frame::new().inner_margin(16).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let cancellable = matches!(&app.operation, Operation::Native(p) if p.cancellable);
                        if ui.add_enabled(
                            !busy || (cancellable && !dialog.cancelling),
                            egui::Button::new(if dialog.completed { "FERMER" } else { "ANNULER" }),
                        ).clicked() {
                            if busy {
                                if let Some(job) = &self.installer_job {
                                    job.cancel.cancel();
                                    dialog.cancelling = true;
                                }
                            } else {
                                close = true;
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if dialog.completed {
                                open = primary(ui, "OUVRIR", 128.0).clicked();
                            } else if awaiting {
                                if ui.add_enabled_ui(!dialog.cancelling && !dialog.consent_sent, |ui| {
                                    primary(ui, "OUVRIR NSIS", 150.0)
                                }).inner.clicked() {
                                    if let Some(job) = &self.installer_job {
                                        dialog.consent_sent = job.consent.try_send(()).is_ok();
                                    }
                                }
                            } else if matches!(&dialog.authorization, Authorization::Blocked(e) if e.retryable) {
                                recheck = primary(ui, "RÉESSAYER", 128.0).clicked();
                            } else {
                                let retry = matches!(&app.operation, Operation::NativeError(e) if e.retryable);
                                let terminal = matches!(&app.operation, Operation::NativeError(e) if !e.retryable);
                                let ready = matches!(&dialog.authorization, Authorization::Ready(c) if !c.personal || dialog.acknowledged);
                                let personal = matches!(&dialog.authorization, Authorization::Ready(c) if c.personal);
                                let clicked = ui.add_enabled_ui(ready && !busy && !terminal && !app.installed(), |ui| {
                                    primary(ui, if retry { "RÉESSAYER" } else if personal { "TÉLÉCHARGER" } else { "INSTALLER" }, 150.0)
                                }).inner.clicked();
                                if clicked {
                                    if let Authorization::Ready(path) = &dialog.authorization {
                                        start = Some(path.clone());
                                    }
                                }
                            }
                        });
                    });
                });
            });
        if open {
            self.open_application(index);
        } else if recheck {
            self.request_installation(ctx, index);
        } else if !close && (busy || !modal.should_close()) {
            if let Some(destination) = start {
                dialog.cancelling = false;
                dialog.consent_sent = false;
                dialog.acknowledged = false;
                self.start_installation(ctx, index, destination);
            }
            self.install_dialog = Some(dialog);
        }
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
                                if let Some(dialog) = &mut self.install_dialog {
                                    if dialog.id == app.id {
                                        dialog.completed = true;
                                    }
                                }
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

fn installation_summary(ui: &mut egui::Ui, app: &Application, authorization: &Authorization) {
    let target = installer::model::platform();
    if let Some((os, arch)) = &target {
        let os_label = match os {
            Os::Macos => "macOS",
            Os::Windows => "Windows",
            Os::Linux => "Linux",
        };
        let cpu = match arch {
            Architecture::Aarch64 => "ARM64",
            Architecture::X86_64 => "x64",
        };
        ui.label(format!("{os_label} · {cpu}"));
        if let Some(asset) = app
            .manual_release
            .as_ref()
            .and_then(|release| release.installer(os, arch))
        {
            ui.label(format!(
                "{} · {:.1} Mo",
                asset.filename,
                asset.size_bytes as f64 / 1_000_000.0
            ))
            .on_hover_text(format!("SHA-256 déclaré par GitHub\n{}", asset.sha256));
            ui.separator();
            ui.label(mono("DOSSIER D’INSTALLATION"));
            match os {
                Os::Macos => ui.label("~/Applications/HMP.app"),
                Os::Windows => ui.label("%LOCALAPPDATA% · choix dans l’installateur NSIS"),
                Os::Linux => ui.label("Indisponible"),
            };
            ui.small("Le dossier ne peut pas être modifié depuis Studio App pour ce package.");
        }
    }
    match authorization {
        Authorization::Pending(_) => {
            ui.spinner();
            ui.label("Vérification de HMP et des empreintes approuvées…");
        }
        Authorization::Ready(confirmation) => {
            if confirmation.personal {
                ui.label(mono("INSTALLATION PERSONNELLE — ACCORD REQUIS"));
                ui.small("HMP est épinglé dans Studio App (URL, taille, SHA-256). La politique Ed25519 de session ne certifie pas l’éditeur. macOS/Windows peuvent refuser l’ouverture.");
                if let Some(key) = &confirmation.key {
                    ui.label("Clé publique temporaire de cette session").on_hover_text(key);
                }
            }
            ui.label(mono("DOSSIER D’INSTALLATION APPROUVÉ"));
            ui.label(confirmation.destination.display().to_string());
        }
        Authorization::Blocked(error) => {
            ui.colored_label(Color32::LIGHT_RED, &error.message);
        }
    }
}
