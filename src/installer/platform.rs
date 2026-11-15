use super::{approval, archive, download::VerifiedDownload, model::*};
use crate::{catalog::model::PackageFormat, native};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
pub(crate) fn command(
    program: &str,
    args: &[&std::ffi::OsStr],
    seconds: u64,
) -> Result<std::process::Output> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Failure::retry(format!("Outil système indisponible : {e}")))?;
    // Bound output while draining both pipes so a verbose helper cannot deadlock.
    fn drain(pipe: impl std::io::Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut pipe = pipe;
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            while let Ok(n) = pipe.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                if bytes.len() < 64 * 1024 {
                    bytes.extend_from_slice(&buffer[..n.min(64 * 1024 - bytes.len())]);
                }
            }
            bytes
        })
    }
    let out = drain(
        child
            .stdout
            .take()
            .ok_or_else(|| Failure::permanent("Sortie système absente"))?,
    );
    let err = drain(
        child
            .stderr
            .take()
            .ok_or_else(|| Failure::permanent("Sortie système absente"))?,
    );
    let began = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if began.elapsed() > Duration::from_secs(seconds) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = out.join();
            let _ = err.join();
            return Err(Failure::retry(
                "Délai de réponse de l’outil système dépassé",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Ok(std::process::Output {
        status,
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    })
}
#[cfg(target_os = "macos")]
fn require_success(output: std::process::Output, what: &str) -> Result<std::process::Output> {
    if output.status.success() {
        Ok(output)
    } else {
        Err(Failure::permanent(format!(
            "{what} refusé : {}",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(1000)
                .collect::<String>()
        )))
    }
}
fn leaf(plan: &Plan) -> &str {
    match &plan.recipe {
        Recipe::MacBundle { bundle_name, .. } => bundle_name,
        Recipe::WindowsPortable { directory_name, .. } => directory_name,
    }
}

pub(crate) fn destination(plan: &Plan) -> Result<PathBuf> {
    let Deployment::LocalFixture { root, token, .. } = &plan.deployment else {
        return Err(Failure::permanent("Installation de production désactivée"));
    };
    Ok(approval::fixture_root(root, token)?.join(leaf(plan)))
}

pub(crate) fn prepare(
    plan: &Plan,
    package: &VerifiedDownload,
    stage: &Path,
    cancel: &Cancellation,
) -> Result<PathBuf> {
    match plan.package.format {
        PackageFormat::Zip => archive::extract_zip(package.path(), stage, leaf(plan), cancel)?,
        PackageFormat::Dmg => {
            #[cfg(target_os = "macos")]
            mounted_copy(package.path(), stage, leaf(plan), cancel)?;
            #[cfg(not(target_os = "macos"))]
            return Err(Failure::permanent(
                "Montage DMG disponible seulement sur macOS",
            ));
        }
        _ => {
            return Err(Failure::permanent(
                "Format d’installation désactivé : aucun installateur exécuté",
            ))
        }
    }
    cancel.check()?;
    verify_payload(plan, stage)?;
    Ok(stage.join(leaf(plan)))
}
fn verify_payload(plan: &Plan, root: &Path) -> Result<()> {
    match &plan.recipe {
        Recipe::MacBundle { bundle_name, .. } => {
            let rule = plan
                .app
                .detection
                .as_ref()
                .and_then(|r| r.macos.as_ref())
                .ok_or_else(|| Failure::permanent("Identité macOS absente"))?;
            let app =
                native::macos::inspect(root, bundle_name, rule).map_err(Failure::permanent)?;
            if app
                .version
                .as_deref()
                .and_then(|v| native::model::compare_versions(v, &plan.app.release.version))
                != Some(std::cmp::Ordering::Equal)
            {
                return Err(Failure::permanent(
                    "Version réelle du bundle différente de la version approuvée",
                ));
            }
            #[cfg(target_os = "macos")]
            {
                require_success(
                    command(
                        "/usr/bin/codesign",
                        &[
                            "--verify".as_ref(),
                            "--deep".as_ref(),
                            "--strict".as_ref(),
                            app.path.as_os_str(),
                        ],
                        30,
                    )?,
                    "Signature du bundle",
                )?;
                let details = require_success(
                    command(
                        "/usr/bin/codesign",
                        &["-d".as_ref(), "--verbose=4".as_ref(), app.path.as_os_str()],
                        10,
                    )?,
                    "Lecture de signature",
                )?;
                if matches!(plan.deployment, Deployment::LocalFixture { .. })
                    && !String::from_utf8_lossy(&details.stderr).contains("Signature=adhoc")
                {
                    return Err(Failure::permanent(
                        "Signature ad-hoc du binaire de test local attendue",
                    ));
                }
            }
        }
        Recipe::WindowsPortable { directory_name, .. } => {
            let rule = plan
                .app
                .detection
                .as_ref()
                .and_then(|r| r.windows.as_ref())
                .ok_or_else(|| Failure::permanent("Identité Windows absente"))?;
            native::windows::inspect(
                rule,
                &native::windows::Record {
                    key: rule.uninstall_key.clone(),
                    publisher: rule.publisher.clone(),
                    directory: root.join(directory_name),
                    version: Some(plan.app.release.version.clone()),
                    user: true,
                    bits: 64,
                },
            )
            .map_err(Failure::permanent)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn mounted_copy(package: &Path, stage: &Path, bundle: &str, cancel: &Cancellation) -> Result<()> {
    let mount = tempfile::Builder::new()
        .prefix("studio-readonly-dmg-")
        .tempdir()?;
    let output = command(
        "/usr/bin/hdiutil",
        &[
            "attach".as_ref(),
            package.as_os_str(),
            "-readonly".as_ref(),
            "-nobrowse".as_ref(),
            "-noautoopen".as_ref(),
            "-mountpoint".as_ref(),
            mount.path().as_os_str(),
        ],
        30,
    )?;
    if !output.status.success() {
        // hdiutil may have mounted before a late error; detach only our private mountpoint.
        // A failed attach can leave a mount. Retain the private mountpoint rather than
        // recursively deleting a possibly mounted filesystem. Never force detach.
        let detached = command(
            "/usr/bin/hdiutil",
            &["detach".as_ref(), mount.path().as_os_str()],
            20,
        );
        if !detached.is_ok_and(|o| o.status.success()) {
            let path = mount.keep();
            return Err(Failure::permanent(format!(
                "Montage DMG refusé. Vérifiez le montage privé avant nettoyage : {}",
                path.display()
            )));
        }
        return Err(Failure::permanent(
            "DMG corrompu, chiffré ou montage refusé",
        ));
    }
    let copied = archive::copy_tree(&mount.path().join(bundle), &stage.join(bundle), cancel);
    let detached = command(
        "/usr/bin/hdiutil",
        &["detach".as_ref(), mount.path().as_os_str()],
        20,
    )
    .and_then(|o| require_success(o, "Démontage DMG"));
    if let Err(e) = detached {
        let path = mount.keep();
        return Err(Failure::permanent(format!(
            "{e}. Montage privé conservé : {}",
            path.display()
        )));
    }
    copied
}

pub(crate) fn commit(
    plan: &Plan,
    source: &Path,
    cancel: &Cancellation,
    emit: &mut impl FnMut(Progress),
) -> Result<native::Report> {
    cancel.check()?;
    if std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Failure::permanent("Horloge système invalide"))?
        .as_secs()
        >= plan.expires_unix
    {
        return Err(Failure::permanent(
            "Approbation expirée pendant le transfert : aucune installation effectuée",
        ));
    }
    let Deployment::LocalFixture { root, token, .. } = &plan.deployment else {
        return Err(Failure::permanent("Installation de production désactivée"));
    };
    let root = approval::fixture_root(root, token)?;
    let rules = plan
        .app
        .detection
        .as_ref()
        .ok_or_else(|| Failure::permanent("Identité native absente"))?;
    if !native::detect(rules).installations.is_empty() {
        return Err(Failure::permanent(
            "Application déjà installée : aucun remplacement autorisé dans cette étape.",
        ));
    }
    let destination = root.join(leaf(plan));
    match fs::symlink_metadata(&destination) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        _ => {
            return Err(Failure::permanent(
                "Destination existante ou inaccessible. Aucun remplacement autorisé.",
            ))
        }
    }
    #[cfg(windows)]
    preflight_registry(plan)?;
    cancel.check()?;
    // Atomic reservation refuses ANY existing destination. No rename-overwrite semantics.
    fs::create_dir(&destination).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            Failure::permanent("Destination créée par une autre opération ; installation refusée.")
        } else {
            e.into()
        }
    })?;
    emit(Progress::phase(Phase::Installing));
    let result = (|| {
        fs::write(
            destination.join(super::model::PENDING_MARKER),
            plan.app.id.as_bytes(),
        )?;
        let uninterrupted = Cancellation::default();
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            archive::copy_tree(
                &entry.path(),
                &destination.join(entry.file_name()),
                &uninterrupted,
            )?;
        }
        // Validate using the private stage before making the final copy visible to detection.
        // The copied files are subsequently checked after removing the pending marker.
        fs::remove_file(destination.join(super::model::PENDING_MARKER))?;
        verify_payload(plan, &root)?;
        #[cfg(windows)]
        register(plan, &destination)?;
        emit(Progress::phase(Phase::Detecting));
        let report = native::detect(rules);
        let canonical = destination.canonicalize()?;
        if !report.installations.iter().any(|i| {
            i.path == canonical
                && i.version
                    .as_deref()
                    .and_then(|v| native::model::compare_versions(v, &plan.app.release.version))
                    == Some(std::cmp::Ordering::Equal)
        }) {
            return Err(Failure::permanent(
                "Installation non confirmée par le détecteur natif. Aucun succès déclaré.",
            ));
        }
        Ok(report)
    })();
    if let Err(error) = result {
        #[cfg(windows)]
        let registry_cleanup = rollback_registry(plan, &destination);
        if let Err(cleanup) = fs::remove_dir_all(&destination) {
            return Err(Failure::permanent(format!("{error}. Nettoyage de la nouvelle destination incomplet : {cleanup} ({}). Vérifiez avant de réessayer.",destination.display())));
        }
        #[cfg(windows)]
        if let Err(cleanup) = registry_cleanup {
            return Err(Failure::permanent(format!(
                "{error}. Nettoyage de la clé de test Windows incomplet : {cleanup}"
            )));
        }
        return Err(error);
    }
    result
}
#[cfg(windows)]
fn registry_path(plan: &Plan) -> Result<String> {
    let key = &plan
        .app
        .detection
        .as_ref()
        .and_then(|r| r.windows.as_ref())
        .ok_or_else(|| Failure::permanent("Clé Windows absente"))?
        .uninstall_key;
    Ok(format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{key}"
    ))
}
#[cfg(windows)]
fn preflight_registry(plan: &Plan) -> Result<()> {
    use winreg::{enums::*, RegKey};
    match RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(registry_path(plan)?, KEY_QUERY_VALUE | KEY_WOW64_64KEY)
    {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(Failure::permanent(
            "Clé d’installation Windows existante ou inaccessible. Aucun remplacement autorisé.",
        )),
    }
}
#[cfg(windows)]
fn register(plan: &Plan, destination: &Path) -> Result<()> {
    use winreg::{enums::*, RegKey};
    let (key, disposition) = RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(
        registry_path(plan)?,
        KEY_QUERY_VALUE | KEY_SET_VALUE | KEY_WOW64_64KEY,
    )?;
    if disposition != REG_CREATED_NEW_KEY {
        return Err(Failure::permanent(
            "Une autre installation a créé la clé Windows. Aucune valeur modifiée.",
        ));
    }
    // The fixture owner is written first. Cleanup can only remove this newly owned key.
    if let Err(error) = key.set_value(
        "StudioAppInstallerOwner",
        &format!("{}:{}", plan.app.id, destination.display()),
    ) {
        drop(key);
        // We just created this key, and have not written any values. Remove only an empty key.
        if let Err(cleanup) = RegKey::predef(HKEY_CURRENT_USER).delete_subkey(registry_path(plan)?)
        {
            return Err(Failure::permanent(format!(
                "{error}. Nettoyage de la clé vide impossible : {cleanup}"
            )));
        }
        return Err(error.into());
    }
    key.set_value("Publisher", &plan.app.publisher)?;
    key.set_value("DisplayName", &plan.app.name)?;
    key.set_value("DisplayVersion", &plan.app.release.version)?;
    key.set_value("InstallLocation", &destination.to_string_lossy().as_ref())?;
    Ok(())
}
#[cfg(windows)]
pub(crate) fn rollback_registry(plan: &Plan, destination: &Path) -> Result<()> {
    use winreg::{enums::*, RegKey};
    if let Ok(path) = registry_path(plan) {
        let parent = RegKey::predef(HKEY_CURRENT_USER);
        if parent
            .open_subkey_with_flags(&path, KEY_QUERY_VALUE | KEY_WOW64_64KEY)
            .ok()
            .and_then(|key| key.get_value::<String, _>("StudioAppInstallerOwner").ok())
            .as_deref()
            == Some(format!("{}:{}", plan.app.id, destination.display()).as_str())
        {
            parent.delete_subkey_all(path)?;
        }
    }
    Ok(())
}
