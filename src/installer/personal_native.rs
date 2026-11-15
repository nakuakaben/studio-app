use super::{download::VerifiedDownload, model::*};
use crate::native;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn destination(recipe: &Recipe) -> Result<PathBuf> {
    let dirs = directories::BaseDirs::new()
        .ok_or_else(|| Failure::permanent("Dossier utilisateur indisponible."))?;
    match recipe {
        Recipe::PersonalHmpDmg if cfg!(target_os = "macos") => {
            Ok(dirs.home_dir().join("Applications/HMP.app"))
        }
        Recipe::PersonalHmpNsis if cfg!(windows) => Ok(dirs.data_local_dir().join("HMP")),
        _ => Err(Failure::permanent(
            "Recette HMP indisponible sur ce système.",
        )),
    }
}

fn absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(Failure::permanent(
            "Destination HMP déjà présente. Aucun remplacement autorisé.",
        )),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn preflight(plan: &Plan, destination: &Path) -> Result<()> {
    absent(destination)?;
    let rules = plan
        .app
        .detection
        .as_ref()
        .ok_or_else(|| Failure::permanent("Identité HMP absente."))?;
    let report = native::detect(rules);
    if !report.installations.is_empty() || !report.errors.is_empty() {
        return Err(Failure::permanent("Une installation HMP existe ou sa présence n’a pas pu être exclue. Aucun remplacement autorisé."));
    }
    #[cfg(windows)]
    if let Some(rule) = &rules.windows {
        if native::windows::uninstall_key_exists(rule)? {
            return Err(Failure::permanent(
                "Une clé d’installation HMP existe déjà. Aucun remplacement autorisé.",
            ));
        }
    }
    Ok(())
}

pub(super) fn lock() -> Result<fs::File> {
    let dirs = directories::ProjectDirs::from("app", "StudioApp", "StudioApp")
        .ok_or_else(|| Failure::permanent("Dossier d’état utilisateur indisponible."))?;
    let directory = dirs.cache_dir();
    fs::create_dir_all(directory)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("installer-personal-hmp.lock"))?;
    file.try_lock().map_err(|e| {
        Failure::retry(format!(
            "Une autre installation personnelle HMP est active ou le verrou est inaccessible : {e}"
        ))
    })?;
    Ok(file)
}

pub(super) fn install(
    plan: &Plan,
    package: &VerifiedDownload,
    destination: &Path,
    cancel: &Cancellation,
    emit: &mut impl FnMut(Progress),
    reauthorize: impl Fn() -> Result<()>,
) -> Result<native::Report> {
    let before_install = || {
        cancel.check()?;
        reauthorize()?;
        preflight(plan, destination)
    };
    #[cfg(target_os = "macos")]
    if matches!(plan.recipe, Recipe::PersonalHmpDmg) {
        let rule = plan
            .app
            .detection
            .as_ref()
            .and_then(|r| r.macos.as_ref())
            .ok_or_else(|| Failure::permanent("Identité du bundle absente."))?;
        return install_mac(
            package.path(),
            destination,
            rule,
            cancel,
            emit,
            before_install,
        );
    }
    #[cfg(windows)]
    if matches!(plan.recipe, Recipe::PersonalHmpNsis) {
        return install_windows(plan, package.path(), emit, before_install);
    }
    let _ = (package, emit, before_install);
    Err(Failure::permanent("Recette HMP incompatible."))
}

#[cfg(target_os = "macos")]
fn install_mac(
    package: &Path,
    destination: &Path,
    rule: &native::model::MacRule,
    cancel: &Cancellation,
    emit: &mut impl FnMut(Progress),
    before_install: impl FnOnce() -> Result<()>,
) -> Result<native::Report> {
    use super::{
        archive,
        platform::{command, require_success, with_mounted},
    };
    use std::ffi::OsStr;
    use std::time::{SystemTime, UNIX_EPOCH};

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Failure::permanent("Horloge système invalide."))?
        .as_secs();
    let quarantine = format!("0081;{stamp:x};Studio App;");
    let mark = |path: &Path| {
        let existing = command(
            "/usr/bin/xattr",
            &[
                OsStr::new("-p"),
                OsStr::new("com.apple.quarantine"),
                path.as_os_str(),
            ],
            10,
        )?;
        if !existing.status.success() || existing.stdout.is_empty() {
            require_success(
                command(
                    "/usr/bin/xattr",
                    &[
                        OsStr::new("-w"),
                        OsStr::new("com.apple.quarantine"),
                        OsStr::new(&quarantine),
                        path.as_os_str(),
                    ],
                    10,
                )?,
                "Marquage de quarantaine",
            )?;
        }
        Ok::<_, Failure>(())
    };
    let copy = |source: &Path, target: &Path| {
        require_success(
            command(
                "/usr/bin/ditto",
                &[
                    OsStr::new("--extattr"),
                    OsStr::new("--qtn"),
                    source.as_os_str(),
                    target.as_os_str(),
                ],
                60,
            )?,
            "Copie du bundle avec ses attributs",
        )?;
        Ok::<_, Failure>(())
    };
    let verify = |root: &Path| {
        let installed =
            native::macos::inspect(root, "HMP.app", rule).map_err(Failure::permanent)?;
        if !matches!(
            installed.version.as_deref(),
            Some(super::personal::VERSION | "0.1.0")
        ) {
            return Err(Failure::permanent("Version du bundle HMP inattendue."));
        }
        require_success(
            command(
                "/usr/bin/codesign",
                &[
                    OsStr::new("--verify"),
                    OsStr::new("--deep"),
                    OsStr::new("--strict"),
                    installed.path.as_os_str(),
                ],
                30,
            )?,
            "Signature de code HMP",
        )?;
        Ok::<_, Failure>(installed)
    };

    mark(package)?;
    let stage = tempfile::tempdir()?;
    let bundle = stage.path().join("HMP.app");
    with_mounted(package, |mount| {
        // Bound and validate the tree (ordinary files only) before copying attributes.
        let validation = tempfile::tempdir()?;
        let source = mount.join("HMP.app");
        archive::copy_tree(&source, &validation.path().join("HMP.app"), cancel)?;
        cancel.check()?;
        copy(&source, &bundle)
    })?;
    mark(&bundle)?;
    verify(stage.path())?;
    before_install()?;

    let root = destination
        .parent()
        .ok_or_else(|| Failure::permanent("Dossier Applications absent."))?;
    match fs::create_dir(root) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    if !fs::symlink_metadata(root)?.is_dir() || root.canonicalize()? != root {
        return Err(Failure::permanent(
            "Le dossier Applications n’est pas un dossier local canonique.",
        ));
    }
    cancel.check()?;
    fs::create_dir(destination)?;
    emit(Progress::phase(Phase::Installing));
    let result = (|| {
        let pending = destination.join(PENDING_MARKER);
        fs::write(&pending, super::personal::HMP_ID)?;
        copy(&bundle, destination)?;
        mark(destination)?;
        fs::remove_file(pending)?;
        emit(Progress::phase(Phase::Detecting));
        let installed = verify(root)?;
        let report = native::macos::detect(rule, &[root.to_owned()]);
        if report.installations.contains(&installed) {
            Ok(report)
        } else {
            Err(Failure::permanent(
                "HMP n’a pas été confirmé par la redétection macOS.",
            ))
        }
    })();
    if result.is_err() {
        fs::remove_dir_all(destination).map_err(|e| {
            Failure::permanent(format!(
                "Installation incomplète, nettoyage impossible dans {} : {e}",
                destination.display()
            ))
        })?;
    }
    result
}

#[cfg(windows)]
fn install_windows(
    plan: &Plan,
    package: &Path,
    emit: &mut impl FnMut(Progress),
    before_install: impl FnOnce() -> Result<()>,
) -> Result<native::Report> {
    use std::io::Write;
    let rules = plan
        .app
        .detection
        .as_ref()
        .ok_or_else(|| Failure::permanent("Identité NSIS absente."))?;
    native::windows::pe(package).map_err(Failure::permanent)?;
    let mut zone = package.as_os_str().to_os_string();
    zone.push(":Zone.Identifier");
    let mut mark = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(PathBuf::from(zone))
        .map_err(|e| {
            Failure::permanent(format!(
                "Impossible de conserver la marque Internet du fichier : {e}"
            ))
        })?;
    write!(
        mark,
        "[ZoneTransfer]\r\nZoneId=3\r\nHostUrl={}\r\n",
        plan.package.url
    )?;
    mark.sync_all()?;
    drop(mark);
    before_install()?;
    emit(Progress::phase(Phase::Installing));
    let outcome = studio_windows_host::execute_attachment(package, &plan.package.url);
    emit(Progress::phase(Phase::Detecting));
    let report = native::detect(rules);
    let status = outcome.map_err(|e| Failure::permanent(format!(
        "Windows n’a pas autorisé ou confirmé l’exécution de NSIS : {e}. Consultez la release officielle."
    )))?;
    if status != 0 {
        return Err(Failure::permanent(format!(
            "NSIS a terminé avec le code {status}. Vérifiez le résultat dans Windows et la disponibilité de WebView2 Evergreen."
        )));
    }
    if report.installations.len() != 1
        || !matches!(
            report.installations.first(),
            Some(native::Installation {
                source: native::Source::WindowsRegistry { user: true, .. }, version: Some(version), ..
            }) if version == super::personal::VERSION
        )
    {
        return Err(Failure::permanent("NSIS terminé, mais l’identité, la version exacte et la clé HKCU de HMP n’ont pas été confirmées. Vérifiez aussi WebView2 Evergreen."));
    }
    Ok(report)
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
#[test]
#[ignore = "contacts GitHub and copies the real HMP bundle to a private temporary directory"]
fn approved_public_hmp_dmg_installs_and_detects_without_touching_applications() {
    use super::{approval, download, personal};
    use ring::{rand::SystemRandom, signature::Ed25519KeyPair};

    let outer = tempfile::tempdir().unwrap();
    let root = outer.path().canonicalize().unwrap().join("Applications");
    let destination = root.join("HMP.app");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
    let policy = personal::make_policy(key.as_ref(), now, now + 60 * 60).unwrap();
    let app = personal::application().unwrap();
    let authorize = || {
        approval::authorize(
            &app,
            &policy,
            outer.path(),
            now,
            &crate::catalog::model::Os::Macos,
            &crate::catalog::model::Architecture::Aarch64,
        )
    };
    let plan = authorize().unwrap();
    let cancel = Cancellation::default();
    let download = download::download(
        &plan,
        &download::Https::new(&plan).unwrap(),
        &cancel,
        &mut |_| {},
    )
    .unwrap();
    let rule = app.detection.as_ref().unwrap().macos.as_ref().unwrap();
    let mut phases = vec![];
    let report = install_mac(
        download.path(),
        &destination,
        rule,
        &cancel,
        &mut |p| phases.push(p.phase),
        || {
            authorize()?;
            absent(&destination)
        },
    )
    .unwrap();
    assert_eq!(report.installations.len(), 1);
    assert_eq!(report.installations[0].path, destination);
    assert_eq!(report.installations[0].version.as_deref(), Some("0.1.0"));
    assert_eq!(phases, [Phase::Installing, Phase::Detecting]);
    for path in [download.path(), destination.as_path()] {
        let output = std::process::Command::new("/usr/bin/xattr")
            .args(["-p", "com.apple.quarantine"])
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success() && !output.stdout.is_empty());
    }
    // Foreign data in the existing destination survive any subsequent reservation.
    let witness = destination.join("already-exists.txt");
    fs::write(&witness, "original").unwrap();
    assert!(fs::create_dir(&destination).is_err());
    assert_eq!(fs::read_to_string(witness).unwrap(), "original");
}
