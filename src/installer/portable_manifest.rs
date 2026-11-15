//! Validation of a previously approved and extracted portable release.
//! A self-described manifest is NOT an approval: the full archive must first
//! be pinned by publisher/catalog signature + size + SHA-256 independently.
use super::{archive, model::*};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

const MANIFEST: &str = "studio-portable.json";
const MAX_MANIFEST: u64 = 256 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    app_id: String,
    profile: String,
    version: String,
    platform: String,
    architecture: String,
    executable: String,
    requires_web_view2_evergreen: bool,
    files: Vec<ManifestFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    path: String,
    size: u64,
    sha256: String,
}

fn digest(path: &Path, expected_size: u64, cancel: &Cancellation) -> Result<String> {
    let mut input = File::open(path)?;
    if input.metadata()?.len() != expected_size {
        return Err(Failure::permanent("Taille du fichier portable modifiée."));
    }
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        cancel.check()?;
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .ok_or_else(|| Failure::permanent("Fichier portable trop volumineux."))?;
        if count > expected_size {
            return Err(Failure::permanent("Le fichier portable a changé."));
        }
        hash.update(&buffer[..n]);
    }
    if count != expected_size {
        return Err(Failure::permanent("Fichier portable tronqué."));
    }
    Ok(super::approval::hex(&hash.finalize()))
}

fn walk(root: &Path, current: &Path, names: &mut HashSet<String>) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() || (!meta.is_dir() && !meta.is_file()) {
            return Err(Failure::permanent("Lien ou fichier spécial interdit dans le portable."));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| Failure::permanent("Chemin portable hors racine."))?;
        let name = relative
            .to_str()
            .ok_or_else(|| Failure::permanent("Chemin portable non Unicode."))?
            .replace('\\', "/");
        if name.split('/').count() > 24 {
            return Err(Failure::permanent("Portable trop profond."));
        }
        archive::safe_entry(&format!("HMP/{name}"), "HMP")?;
        if meta.is_dir() {
            walk(root, &path, names)?;
        } else {
            if names.len() >= archive::MAX_ENTRIES {
                return Err(Failure::permanent("Portable avec trop de fichiers."));
            }
            if !names.insert(name.to_ascii_lowercase()) {
                return Err(Failure::permanent("Doublon de fichier portable."));
            }
        }
    }
    Ok(())
}

/// Validate every staged file and reject extra payloads. Call only after the
/// enclosing archive has passed independent approval, TLS and SHA-256 checks.
/// Root is the *extracted HMP/* directory, not the ZIP path.
pub(crate) fn validate(
    root: &Path,
    app_id: &str,
    version: &str,
    executable: &str,
    cancel: &Cancellation,
) -> Result<()> {
    if !fs::symlink_metadata(root)?.is_dir() {
        return Err(Failure::permanent("Dossier portable invalide."));
    }
    let manifest_file = root.join(MANIFEST);
    let meta = fs::symlink_metadata(&manifest_file)?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_MANIFEST {
        return Err(Failure::permanent("Manifeste portable invalide ou excessif."));
    }
    let manifest: Manifest = serde_json::from_reader(File::open(&manifest_file)?)?;
    if manifest.schema_version != 1
        || manifest.app_id != app_id
        || manifest.version != version
        || manifest.platform != "windows"
        || manifest.architecture != "x86_64"
        || manifest.profile.trim().is_empty()
        || manifest.profile.len() > 256
        || manifest.executable != executable
        || !manifest.requires_web_view2_evergreen
        || semver::Version::parse(&manifest.version).is_err()
        || manifest.files.is_empty()
        || manifest.files.len() > archive::MAX_ENTRIES
    {
        return Err(Failure::permanent("Identité ou version du portable non approuvées."));
    }
    archive::safe_entry(&format!("HMP/{}", manifest.executable), "HMP")?;
    let mut observed = HashSet::new();
    walk(root, root, &mut observed)?;
    if !observed.remove(MANIFEST) || observed.len() != manifest.files.len() {
        return Err(Failure::permanent("Arborescence portable différente du manifeste."));
    }
    let mut total = 0u64;
    for file in &manifest.files {
        cancel.check()?;
        let key = file.path.to_ascii_lowercase();
        if file.path == MANIFEST
            || !observed.remove(&key)
            || file.size > archive::MAX_EXPANDED
            || super::approval::unhex(&file.sha256, 32).is_err()
        {
            return Err(Failure::permanent("Fichier portable inconnu ou non vérifiable."));
        }
        total = total
            .checked_add(file.size)
            .ok_or_else(|| Failure::permanent("Portable trop volumineux."))?;
        if total > archive::MAX_EXPANDED {
            return Err(Failure::permanent("Portable décompressé trop volumineux."));
        }
        archive::safe_entry(&format!("HMP/{}", file.path), "HMP")?;
        let actual = digest(&root.join(&file.path), file.size, cancel)?;
        if !actual.eq_ignore_ascii_case(&file.sha256) {
            return Err(Failure::permanent("Empreinte de fichier portable incorrecte."));
        }
    }
    if !observed.is_empty() || !root.join(executable).is_file() {
        return Err(Failure::permanent("Fichier exécutable absent ou fichier supplémentaire."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn stage() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("hmp-v2-preview.exe"), b"MZ test").unwrap();
        let hash = super::super::approval::hex(&Sha256::digest(b"MZ test"));
        let manifest = json!({
            "schemaVersion": 1,
            "appId": "hyper-media-puller",
            "profile": "fr.tktprod.hyperdeck-media-puller.v2.preview",
            "version": "0.1.0-beta.1",
            "platform": "windows",
            "architecture": "x86_64",
            "executable": "hmp-v2-preview.exe",
            "requiresWebView2Evergreen": true,
            "files": [{"path":"hmp-v2-preview.exe","size":7,"sha256":hash}]
        });
        let mut f = File::create(dir.path().join(MANIFEST)).unwrap();
        f.write_all(serde_json::to_string(&manifest).unwrap().as_bytes()).unwrap();
        dir
    }
    fn check(dir: &tempfile::TempDir) -> Result<()> {
        validate(dir.path(), "hyper-media-puller", "0.1.0-beta.1", "hmp-v2-preview.exe", &Cancellation::default())
    }
    #[test]
    fn exact_manifest_is_accepted_but_not_a_signature() {
        let d = stage();
        assert!(check(&d).is_ok());
    }
    #[test]
    fn rejects_modified_extra_or_missing_portable_files() {
        let d = stage();
        fs::write(d.path().join("hmp-v2-preview.exe"), b"MZ changed").unwrap();
        assert!(check(&d).is_err());
        let d = stage();
        fs::write(d.path().join("unexpected.dll"), b"extra").unwrap();
        assert!(check(&d).is_err());
        let d = stage();
        fs::remove_file(d.path().join("hmp-v2-preview.exe")).unwrap();
        assert!(check(&d).is_err());
    }
    #[test]
    fn rejects_unapproved_version_and_identity() {
        let d = stage();
        assert!(validate(d.path(), "another-app", "0.1.0-beta.1", "hmp-v2-preview.exe", &Cancellation::default()).is_err());
        assert!(validate(d.path(), "hyper-media-puller", "0.1.0-beta.2", "hmp-v2-preview.exe", &Cancellation::default()).is_err());
    }
}
