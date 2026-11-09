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
pub fn spawn(application: AppMetadata, repaint: impl Fn() + Send + 'static) -> Job {
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
            let (policy, root) = approval::default_policy()?;
            let (os, arch) = platform().ok_or_else(|| {
                Failure::permanent("Installation native non prise en charge sur cet OS/CPU")
            })?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| Failure::permanent("Horloge système invalide"))?
                .as_secs();
            let plan = approval::authorize(&app, &policy, &root, now, &os, &arch)?;
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
pub fn run(
    plan: &Plan,
    http: &dyn download::Transport,
    cancel: &Cancellation,
    emit: &mut impl FnMut(Progress),
) -> Result<native::Report> {
    cancel.check()?;
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
