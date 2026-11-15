use super::{approval, download, model::*, platform};
use crate::{catalog::model::AppMetadata, native};
use std::sync::mpsc::{self, Receiver};
pub enum Event {
    Progress(Progress),
    Finished(Result<native::Report>),
}
pub struct Job {
    pub application: AppMetadata,
    pub receiver: Receiver<Event>,
    pub cancel: Cancellation,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
pub fn spawn(
    application: AppMetadata,
    expected_destination: std::path::PathBuf,
    repaint: impl Fn() + Send + 'static,
) -> Job {
    let (sender, receiver) = mpsc::sync_channel(16);
    let cancel = Cancellation::default();
    let cancellation = cancel.clone();
    let app = application.clone();
    std::thread::spawn(move || {
        let mut emit = |progress| {
            let _ = sender.try_send(Event::Progress(progress));
            repaint();
        };
        emit(Progress::phase(Phase::Authorizing));
        let result = (|| {
            let plan = authorized_plan(&app)?;
            if platform::destination(&plan)? != expected_destination {
                return Err(Failure::permanent(
                    "Destination d’installation modifiée. Rouvrez la fenêtre de confirmation.",
                ));
            }
            run(
                &plan,
                &download::Https::new(&plan)?,
                &cancellation,
                &mut emit,
            )
        })();
        let _ = sender.send(Event::Finished(result));
        repaint();
    });
    Job {
        application,
        receiver,
        cancel,
    }
}
fn authorized_plan(app: &AppMetadata) -> Result<Plan> {
    let (policy, root) = approval::default_policy()?;
    let (os, arch) = platform().ok_or_else(|| {
        Failure::permanent("Installation native non prise en charge sur cet OS/CPU")
    })?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Failure::permanent("Horloge système invalide"))?
        .as_secs();
    approval::authorize(app, &policy, &root, now, &os, &arch)
}

/// Read-only authorization check for the confirmation dialog. Starting still reauthorizes.
pub fn preview(
    application: AppMetadata,
    repaint: impl FnOnce() + Send + 'static,
) -> Receiver<Result<std::path::PathBuf>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = authorized_plan(&application).and_then(|plan| platform::destination(&plan));
        let _ = sender.send(result);
        repaint();
    });
    receiver
}
pub fn run(
    plan: &Plan,
    http: &dyn download::Transport,
    cancel: &Cancellation,
    emit: &mut impl FnMut(Progress),
) -> Result<native::Report> {
    cancel.check()?;
    platform::destination(plan)?;
    if plan
        .app
        .detection
        .as_ref()
        .is_some_and(|r| !native::detect(r).installations.is_empty())
    {
        return Err(Failure::permanent(
            "Application déjà installée. Aucune mise à jour ou réinstallation autorisée.",
        ));
    }
    emit(Progress {
        phase: Phase::Downloading,
        bytes: 0,
        total: plan.package.size_bytes,
        bytes_per_second: None,
        cancellable: true,
    });
    let package = download::download(plan, http, cancel, emit)?;
    emit(Progress::phase(Phase::Preparing));
    let stage = tempfile::Builder::new()
        .prefix("studio-approved-stage-")
        .tempdir()?;
    let payload = platform::prepare(plan, &package, stage.path(), cancel)?;
    platform::commit(plan, &payload, cancel, emit)
}
