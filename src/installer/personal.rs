//! Public discovery is not authority. These are the only reviewed HMP assets
//! eligible for an independent, explicit local approval.
use super::{approval, model::*};
use crate::catalog::{manual::ManualRelease, model::*};
use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub const HMP_ID: &str = "hyper-media-puller";
pub const VERSION: &str = "0.1.0-beta.1";
const RELEASES: &str = "https://github.com/nakuakaben/hyper-media-puller/releases";
pub const POLICY_ENV: &str = "STUDIO_APP_PERSONAL_INSTALLER_POLICY";

fn package(os: Os, arch: Architecture) -> Option<Package> {
    let (id, format, filename, size_bytes, sha256) = match (&os, &arch) {
        (Os::Macos, Architecture::Aarch64) => (
            "hmp-macos-arm64",
            PackageFormat::Dmg,
            "HMP-V2-0.1.0-beta.1-macos-arm64.dmg",
            8705217,
            "19626af397aa54318b2adc6b03287fc224f9790e6b18eb54233a9d3514ab6097",
        ),
        (Os::Windows, Architecture::X86_64) => (
            "hmp-windows-x64",
            PackageFormat::Exe,
            "HMP-V2-0.1.0-beta.1-windows-x64-setup.exe",
            2881983,
            "d35dcb98366c1bd4b0f44ef1e250159175dbee2e388c215094a173bde1a2255e",
        ),
        _ => return None,
    };
    Some(Package {
        id: id.into(),
        os,
        architecture: arch,
        format,
        url: format!("{RELEASES}/download/v{VERSION}/{filename}"),
        size_bytes,
        sha256: sha256.into(),
        github_asset_id: None,
    })
}

pub(super) fn application() -> Result<AppMetadata> {
    let packages = [
        package(Os::Macos, Architecture::Aarch64),
        package(Os::Windows, Architecture::X86_64),
    ]
    .into_iter()
    .flatten()
    .collect();
    crate::catalog::manual::bundled_hmp(Release {
        version: VERSION.into(),
        published: "2026-11-15".into(),
        notes: vec![],
        packages,
    })
    .map_err(|e| Failure::permanent(e.to_string()))
}

pub(super) fn matches_package(app: &AppMetadata, candidate: &Package) -> bool {
    app.id == HMP_ID
        && app.release.version == VERSION
        && package(candidate.os.clone(), candidate.architecture.clone()).is_some_and(|expected| {
            candidate.id == expected.id
                && candidate.format == expected.format
                && candidate.url == expected.url
                && candidate.sha256 == expected.sha256
                && candidate.size_bytes == expected.size_bytes
                && candidate.github_asset_id.is_none()
        })
}

pub fn check_discovery(
    app: &AppMetadata,
    release: &ManualRelease,
    os: &Os,
    arch: &Architecture,
) -> Result<()> {
    let asset = release.installer(os, arch).ok_or_else(|| {
        Failure::permanent("Aucun installateur publié pour ce système et cette architecture.")
    })?;
    let expected = package(os.clone(), arch.clone());
    if app.id == HMP_ID
        && release.application_id == HMP_ID
        && app.release.version == VERSION
        && release.page == format!("{RELEASES}/tag/v{VERSION}")
        && expected.is_some_and(|p| {
            p.url.rsplit('/').next() == Some(asset.filename.as_str())
                && p.size_bytes == asset.size_bytes
                && p.sha256 == asset.sha256
        })
    {
        Ok(())
    } else {
        Err(Failure::permanent(
            "Cette version ou cet asset ne correspond pas au HMP épinglé pour l’essai personnel.",
        ))
    }
}

/// The reviewed release is pinned in this binary (identity, URL, size, SHA-256).
/// This ephemeral Ed25519 policy records *local user consent*, not publisher
/// identity. It is never used to authorize arbitrary catalogue discoveries.
/// The private key is never written to disk or shipped with Studio App.
fn bundled_session_policy() -> Result<(Policy, PathBuf)> {
    static SESSION_POLICY: std::sync::OnceLock<Result<Vec<u8>>> = std::sync::OnceLock::new();
    let bytes = SESSION_POLICY
        .get_or_init(|| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| Failure::permanent("Horloge système invalide."))?
                .as_secs();
            let key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
                .map_err(|_| Failure::permanent("Impossible de préparer l'autorisation locale."))?;
            let policy = make_policy(key.as_ref(), now, now + 7 * 86400)?;
            Ok(serde_json::to_vec(&policy)?)
        })
        .clone()?;
    let policy = serde_json::from_slice(&bytes)?;
    // The personal profile does not read files relative to policy_root.
    // Keep this path non-privileged and never create any file here.
    Ok((policy, std::env::temp_dir()))
}

pub(super) fn local_policy() -> Result<(Policy, PathBuf)> {
    // Advanced operator provisioning remains supported, but is never required
    // by a user running the packaged launcher.
    let (policy, root) = if let Some(path) = std::env::var_os(POLICY_ENV).map(PathBuf::from) {
        if !path.is_absolute() {
            return Err(Failure::permanent(
                "Le chemin de la politique HMP doit être absolu.",
            ));
        }
        approval::read_policy(path)?
    } else {
        bundled_session_policy()?
    };
    if !matches!(policy.deployment, Deployment::PersonalHmp) {
        return Err(Failure::permanent(
            "Politique locale du profil personal_hmp requise.",
        ));
    }
    Ok((policy, root))
}

pub fn make_policy(pkcs8: &[u8], now: u64, expires_unix: u64) -> Result<Policy> {
    if expires_unix <= now || expires_unix - now > 30 * 86400 {
        return Err(Failure::permanent(
            "Expiration future de 30 jours maximum requise.",
        ));
    }
    let key = Ed25519KeyPair::from_pkcs8(pkcs8)
        .map_err(|_| Failure::permanent("Clé personnelle Ed25519 PKCS#8 invalide."))?;
    let app = application()?;
    let key_id = "hmp-personal".to_owned();
    let approvals = app
        .release
        .packages
        .iter()
        .map(|package| {
            let body = ApprovalBody {
                schema_version: 1,
                application: app.clone(),
                package_id: package.id.clone(),
                recipe: if package.os == Os::Macos {
                    Recipe::PersonalHmpDmg
                } else {
                    Recipe::PersonalHmpNsis
                },
                expires_unix,
            };
            let signature = approval::hex(key.sign(&approval::signing_message(&body)?).as_ref());
            Ok(SignedApproval {
                key_id: key_id.clone(),
                body,
                signature,
            })
        })
        .collect::<Result<_>>()?;
    Ok(Policy {
        schema_version: 1,
        keys: vec![TrustedKey {
            id: key_id,
            public_key: approval::hex(key.public_key().as_ref()),
        }],
        approvals,
        deployment: Deployment::PersonalHmp,
    })
}

pub fn generate_key(path: &Path) -> Result<()> {
    let key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
        .map_err(|_| Failure::permanent("Génération Ed25519 impossible."))?;
    write_new(path, key.as_ref())
}

pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(root)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist_noclobber(path)
        .map_err(|e| Failure::from(e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_current_locally_signed_pinned_package_is_eligible() {
        let key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        let app = application().unwrap();
        let root = tempfile::tempdir().unwrap();
        for (os, arch) in [
            (Os::Macos, Architecture::Aarch64),
            (Os::Windows, Architecture::X86_64),
        ] {
            let authorize = |app: &AppMetadata, policy: &Policy, time| {
                approval::authorize(app, policy, root.path(), time, &os, &arch)
            };
            let policy = make_policy(key.as_ref(), 100, 200).unwrap();
            assert!(authorize(&app, &policy, 199).is_ok());
            assert!(authorize(&app, &policy, 200).is_err());
            for change in 0..6 {
                let mut app = app.clone();
                let mut policy = policy.clone();
                let package = app
                    .release
                    .packages
                    .iter_mut()
                    .find(|p| p.os == os)
                    .unwrap();
                match change {
                    0 => package.url = "https://github.com/somewhere/else/setup.exe".into(),
                    1 => package.sha256 = "0".repeat(64),
                    2 => package.size_bytes += 1,
                    3 => app.release.version = "0.1.0-beta.2".into(),
                    4 => policy.approvals.clear(),
                    _ => policy.deployment = Deployment::Production,
                }
                assert!(authorize(&app, &policy, 101).is_err(), "mutation {change}");
            }
        }
        assert!(select(&app, &Os::Macos, &Architecture::X86_64).is_err());
        assert!(select(&app, &Os::Windows, &Architecture::Aarch64).is_err());
        assert!(make_policy(key.as_ref(), 100, 99).is_err());
        assert!(make_policy(key.as_ref(), 100, 100 + 31 * 86400).is_err());
    }

    #[test]
    fn bundled_session_policy_needs_no_cargo_or_external_key_file() {
        let (policy, root) = bundled_session_policy().unwrap();
        assert!(matches!(policy.deployment, Deployment::PersonalHmp));
        assert_eq!(policy.keys.len(), 1);
        assert_eq!(policy.approvals.len(), 2);
        let (same_policy, _) = bundled_session_policy().unwrap();
        assert_eq!(
            serde_json::to_vec(&policy).unwrap(),
            serde_json::to_vec(&same_policy).unwrap()
        );
        let app = application().unwrap();
        let plan = approval::authorize(
            &app,
            &policy,
            &root,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            &Os::Macos,
            &Architecture::Aarch64,
        )
        .unwrap();
        assert!(matches!(plan.recipe, Recipe::PersonalHmpDmg));
        assert!(matches_package(&app, &plan.package));
    }

    #[test]
    fn key_and_policy_writes_never_replace_existing_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("private.pkcs8");
        generate_key(&path).unwrap();
        let key = std::fs::read(&path).unwrap();
        assert!(make_policy(&key, 1, 2).is_ok());
        assert!(generate_key(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), key);
    }
}
