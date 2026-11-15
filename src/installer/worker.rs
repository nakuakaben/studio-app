use super::{approval, download, model::*, personal, personal_native, platform};
use crate::{catalog::model::AppMetadata, native};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    time::Duration,
};
pub enum Event {
    Progress(Progress),
    Finished(Result<native::Report>),
}
pub struct Job {
    pub application: AppMetadata,
    pub receiver: Receiver<Event>,
    pub cancel: Cancellation,
    pub consent: mpsc::SyncSender<()>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confirmation {
    pub destination: PathBuf,
    pub personal: bool,
    pub key: Option<String>,
    policy_digest: Option<[u8; 32]>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
pub fn spawn(
    application: AppMetadata,
    confirmed: Confirmation,
    repaint: impl Fn() + Send + 'static,
) -> Job {
    let (sender, receiver) = mpsc::sync_channel(16);
    let (consent, user_confirmation) = mpsc::sync_channel(1);
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
            let authorize = || {
                let (plan, current) = authorized_plan(&app, confirmed.personal)?;
                if current != confirmed {
                    return Err(Failure::permanent(
                        "Politique ou destination modifiée. Rouvrez la fenêtre de confirmation.",
                    ));
                }
                Ok(plan)
            };
            let plan = authorize()?;
            let http = download::Https::new(&plan)?;
            if !confirmed.personal {
                return run(&plan, &http, &cancellation, &mut emit);
            }

            let _lock = personal_native::lock()?;
            personal_native::preflight(&plan, &confirmed.destination)?;
            let package = transfer(&plan, &http, &cancellation, &mut emit)?;
            if matches!(plan.recipe, Recipe::PersonalHmpNsis) {
                native::windows::pe(package.path()).map_err(Failure::permanent)?;
                emit(Progress::phase(Phase::AwaitingConsent));
                wait_for_consent(&user_confirmation, &cancellation)?;
            }
            emit(Progress::phase(Phase::Preparing));
            personal_native::install(
                &plan,
                &package,
                &confirmed.destination,
                &cancellation,
                &mut emit,
                || authorize().map(|_| ()),
            )
        })();
        let _ = sender.send(Event::Finished(result));
        repaint();
    });
    Job {
        application,
        receiver,
        cancel,
        consent,
    }
}
fn wait_for_consent(receiver: &Receiver<()>, cancellation: &Cancellation) -> Result<()> {
    let start = std::time::Instant::now();
    loop {
        cancellation.check()?;
        if start.elapsed() > Duration::from_secs(20 * 60) {
            return Err(Failure::retry(
                "Confirmation de l’ouverture du NSIS expirée.",
            ));
        }
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(()) => return cancellation.check(),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(Failure::retry("Confirmation Windows annulée."))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}
fn authorized_plan(app: &AppMetadata, use_personal: bool) -> Result<(Plan, Confirmation)> {
    let pinned;
    let (app, (policy, root)) = if use_personal {
        pinned = personal::application()?;
        if app.id != pinned.id
            || app.release.version != pinned.release.version
            || app.detection != pinned.detection
            || app.fictional
        {
            return Err(Failure::permanent(
                "L’application affichée ne correspond pas à HMP épinglé.",
            ));
        }
        (&pinned, personal::local_policy()?)
    } else {
        (app, approval::default_policy()?)
    };
    let (os, arch) = platform().ok_or_else(|| {
        Failure::permanent("Installation native non prise en charge sur cet OS/CPU")
    })?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Failure::permanent("Horloge système invalide"))?
        .as_secs();
    let plan = approval::authorize(app, &policy, &root, now, &os, &arch)?;
    let destination = platform::destination(&plan)?;
    let key = if use_personal {
        policy
            .approvals
            .iter()
            .find(|approval| approval.body.package_id == plan.package.id)
            .and_then(|approval| policy.keys.iter().find(|key| key.id == approval.key_id))
            .map(|key| key.public_key.clone())
    } else {
        None
    };
    let policy_digest = if use_personal {
        Some(Sha256::digest(serde_json::to_vec(&policy)?).into())
    } else {
        None
    };
    Ok((
        plan,
        Confirmation {
            destination,
            personal: use_personal,
            key,
            policy_digest,
        },
    ))
}

/// Read-only authorization check for the confirmation dialog. Starting still reauthorizes.
pub fn preview(
    application: AppMetadata,
    personal: bool,
    repaint: impl FnOnce() + Send + 'static,
) -> Receiver<Result<Confirmation>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = authorized_plan(&application, personal).and_then(|(plan, confirmation)| {
            if personal {
                personal_native::preflight(&plan, &confirmation.destination)?;
            }
            Ok(confirmation)
        });
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
    if matches!(plan.deployment, Deployment::PersonalHmp) {
        return Err(Failure::permanent(
            "Le profil HMP personnel exige une confirmation dans Studio App.",
        ));
    }
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
    let package = transfer(plan, http, cancel, emit)?;
    emit(Progress::phase(Phase::Preparing));
    let stage = tempfile::Builder::new()
        .prefix("studio-approved-stage-")
        .tempdir()?;
    let payload = platform::prepare(plan, &package, stage.path(), cancel)?;
    platform::commit(plan, &payload, cancel, emit)
}

fn transfer(
    plan: &Plan,
    http: &dyn download::Transport,
    cancel: &Cancellation,
    emit: &mut impl FnMut(Progress),
) -> Result<download::VerifiedDownload> {
    emit(Progress {
        phase: Phase::Downloading,
        bytes: 0,
        total: plan.package.size_bytes,
        bytes_per_second: None,
        cancellable: true,
    });
    download::download(plan, http, cancel, emit)
}
