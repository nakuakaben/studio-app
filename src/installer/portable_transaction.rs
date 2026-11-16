//! Transactional on-disk commit of an already independently approved
//! Windows portable release. This module is deliberately not yet wired to the
//! public installer: production activation needs trusted release publication.
use super::{archive, model::*, portable_manifest};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

/// The caller has already verified the full archive against an independently
/// signed catalog approval. We reject an unverified payload again before and
/// after its installation; these checks do not establish publisher trust.
pub(crate) fn commit(
    verified_payload: &Path,
    application_id: &str,
    version: &str,
    executable: &str,
    owned_root: &Path,
    cancel: &Cancellation,
) -> Result<PathBuf> {
    super::approval::simple_name(application_id)?;
    super::approval::simple_name(version)?;
    super::approval::simple_name(executable)?;
    portable_manifest::validate(
        verified_payload,
        application_id,
        version,
        executable,
        cancel,
    )?;
    cancel.check()?;
    let root = owned_root.canonicalize()?;
    if !root.is_dir() || fs::symlink_metadata(&root)?.file_type().is_symlink() {
        return Err(Failure::permanent("Racine privée d'installation invalide."));
    }
    let app_root = root.join(application_id);
    match fs::create_dir(&app_root) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let actual = fs::symlink_metadata(&app_root)?;
            if !actual.is_dir() || actual.file_type().is_symlink() {
                return Err(Failure::permanent("Dossier de l'application non sûr."));
            }
        }
        Err(e) => return Err(e.into()),
    }
    if app_root.canonicalize()? != app_root {
        return Err(Failure::permanent("Dossier de l'application non canonique."));
    }
    let target = app_root.join(version);
    // Atomic reservation: no pre-existing destination is modified or removed.
    fs::create_dir(&target).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Failure::permanent("Cette version existe déjà ; aucun écrasement autorisé.")
        } else {
            error.into()
        }
    })?;
    let result = (|| {
        let marker = target.join(PENDING_MARKER);
        let mut file = OpenOptions::new().write(true).create_new(true).open(&marker)?;
        file.write_all(application_id.as_bytes())?;
        file.sync_all()?;
        // Once reserved, copy without observing cancellation: an interruption
        // must leave either a complete verified installation or no installation.
        let uninterrupted = Cancellation::default();
        for entry in fs::read_dir(verified_payload)? {
            let entry = entry?;
            let name = entry.file_name();
            super::approval::simple_name(
                name.to_str()
                    .ok_or_else(|| Failure::permanent("Nom de fichier non Unicode."))?,
            )?;
            archive::copy_tree(
                &entry.path(),
                &target.join(name),
                &uninterrupted,
            )?;
        }
        portable_manifest::validate(
            &target,
            application_id,
            version,
            executable,
            &uninterrupted,
        )?;
        // The identity marker is removed last. Readers must reject a directory
        // while the marker exists.
        fs::remove_file(marker)?;
        Ok(target.clone())
    })();
    if let Err(error) = result {
        // Only the destination reserved by this transaction may be removed.
        if let Err(cleanup) = fs::remove_dir_all(&target) {
            return Err(Failure::permanent(format!(
                "{error}. Nettoyage incomplet de notre nouvelle version : {cleanup}. Vérifiez {}.",
                target.display()
            )));
        }
        return Err(error);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    fn payload(root: &Path, version: &str) -> PathBuf {
        let source = root.join("approved");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("hmp-v2-preview.exe"), b"portable binary").unwrap();
        let manifest = json!({
            "schemaVersion": 1,
            "appId": "hyper-media-puller",
            "profile": "fr.tktprod.hyperdeck-media-puller.v2.preview",
            "version": version,
            "platform": "windows",
            "architecture": "x86_64",
            "executable": "hmp-v2-preview.exe",
            "requiresWebView2Evergreen": true,
            "files": [{
                "path": "hmp-v2-preview.exe",
                "size": 15,
                "sha256": super::super::approval::hex(&Sha256::digest(b"portable binary"))
            }]
        });
        fs::write(source.join("studio-portable.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
        source
    }
    #[test]
    fn installs_new_version_and_preserves_existing_data() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("library");
        fs::create_dir(&root).unwrap();
        let source = payload(d.path(), "0.1.0-beta.1");
        let target = commit(
            &source, "hyper-media-puller", "0.1.0-beta.1", "hmp-v2-preview.exe",
            &root, &Cancellation::default(),
        ).unwrap();
        assert!(target.join("hmp-v2-preview.exe").is_file());
        assert!(!target.join(PENDING_MARKER).exists());
        fs::write(target.join("user-content.txt"), b"keep").unwrap();
        assert!(commit(
            &source, "hyper-media-puller", "0.1.0-beta.1", "hmp-v2-preview.exe",
            &root, &Cancellation::default(),
        ).is_err());
        assert_eq!(fs::read(target.join("user-content.txt")).unwrap(), b"keep");
    }
    #[test]
    fn altered_payload_never_creates_destination() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("library");
        fs::create_dir(&root).unwrap();
        let source = payload(d.path(), "0.1.0-beta.1");
        fs::write(source.join("hmp-v2-preview.exe"), b"changed").unwrap();
        assert!(commit(
            &source, "hyper-media-puller", "0.1.0-beta.1", "hmp-v2-preview.exe",
            &root, &Cancellation::default(),
        ).is_err());
        assert!(!root.join("hyper-media-puller/0.1.0-beta.1").exists());
    }
}
