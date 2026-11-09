use super::model::*;
use crate::catalog::{cache::read_bounded, model::*, security};
use std::path::{Path, PathBuf};

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn unhex(value: &str, length: usize) -> Result<Vec<u8>> {
    if value.len() != length * 2 || !value.is_ascii() {
        return Err(Failure::permanent("Encodage cryptographique invalide"));
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| {
            std::str::from_utf8(c)
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
                .ok_or_else(|| Failure::permanent("Encodage hexadécimal invalide"))
        })
        .collect()
}
pub fn signing_message(body: &ApprovalBody) -> Result<Vec<u8>> {
    let mut message = b"Studio App approved installation v1\0".to_vec();
    message.extend(serde_json::to_vec(body)?);
    Ok(message)
}
pub fn default_policy() -> Result<(Policy, PathBuf)> {
    let path = std::env::var_os("STUDIO_APP_INSTALLER_POLICY")
        .map(PathBuf::from)
        .or_else(|| {
            directories::ProjectDirs::from("app", "StudioApp", "StudioApp")
                .map(|d| d.config_dir().join("installer-policy.json"))
        })
        .ok_or_else(|| Failure::permanent("Configuration d’installation indisponible"))?;
    let path = path.canonicalize().map_err(|_| {
        Failure::permanent(
            "Installation non activée : installer-policy.json local de confiance manquant.",
        )
    })?;
    let bytes =
        read_bounded(&path, 2 * 1024 * 1024).map_err(|e| Failure::permanent(e.to_string()))?;
    Ok((
        serde_json::from_slice(&bytes)?,
        path.parent()
            .ok_or_else(|| Failure::permanent("Dossier de confiance absent"))?
            .to_owned(),
    ))
}
pub fn authorize(
    app: &AppMetadata,
    policy: &Policy,
    policy_root: &Path,
    now: u64,
    os: &Os,
    arch: &Architecture,
) -> Result<Plan> {
    if policy.schema_version != 1
        || policy.keys.is_empty()
        || policy.keys.len() > 16
        || policy.approvals.len() > 128
    {
        return Err(Failure::permanent("Politique d’installation invalide"));
    }
    // Revalidate the entire immutable application, including URLs and native identity.
    crate::catalog::parse(&serde_json::to_vec(&Catalog {
        schema_version: 2,
        name: "Installation approval".into(),
        applications: vec![app.clone()],
    })?)
    .map_err(|e| Failure::permanent(e.to_string()))?;
    let package = select(app, os, arch)?;
    let mut accepted = None;
    for approval in &policy.approvals {
        if approval.body.application.id != app.id || approval.body.package_id != package.id {
            continue;
        }
        if approval.body.schema_version != 1 || approval.body.expires_unix <= now {
            return Err(Failure::permanent("Approbation expirée ou schéma inconnu"));
        }
        let keys: Vec<_> = policy
            .keys
            .iter()
            .filter(|k| k.id == approval.key_id)
            .collect();
        if keys.len() != 1 {
            return Err(Failure::permanent("Clé d’approbation absente ou ambiguë"));
        }
        let public = unhex(&keys[0].public_key, 32)?;
        let signature = unhex(&approval.signature, 64)?;
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public)
            .verify(&signing_message(&approval.body)?, &signature)
            .map_err(|_| Failure::permanent("Signature Ed25519 de l’approbation invalide"))?;
        if serde_json::to_vec(&approval.body.application)? != serde_json::to_vec(app)? {
            return Err(Failure::permanent(
                "Le catalogue ne correspond plus aux métadonnées approuvées.",
            ));
        }
        if accepted.is_some() {
            return Err(Failure::permanent("Approbation de package ambiguë"));
        }
        accepted = Some((approval.body.recipe.clone(), approval.body.expires_unix));
    }
    let (recipe,expires_unix)=accepted.ok_or_else(||Failure::permanent("Package sans approbation signée par une clé locale de confiance. Aucun téléchargement effectué."))?;
    match (&recipe, os) {
        (Recipe::MacBundle { bundle_name, .. }, Os::Macos) => {
            simple_name(bundle_name)?;
            if !bundle_name.ends_with(".app")
                || app
                    .detection
                    .as_ref()
                    .and_then(|r| r.macos.as_ref())
                    .is_none()
            {
                return Err(Failure::permanent("Bundle approuvé incompatible"));
            }
        }
        (Recipe::WindowsPortable { directory_name, .. }, Os::Windows) => {
            simple_name(directory_name)?;
            if app
                .detection
                .as_ref()
                .and_then(|r| r.windows.as_ref())
                .is_none()
            {
                return Err(Failure::permanent("Identité Windows manquante"));
            }
        }
        _ => {
            return Err(Failure::permanent(
                "Recette incompatible avec la plateforme",
            ))
        }
    }
    validate_deployment(&policy.deployment, app, &recipe, policy_root)?;
    Ok(Plan {
        app: app.clone(),
        package,
        recipe,
        deployment: policy.deployment.clone(),
        policy_root: policy_root.to_owned(),
        expires_unix,
    })
}
pub fn simple_name(name: &str) -> Result<()> {
    security::relative_path(name).map_err(|e| Failure::permanent(e.to_string()))?;
    if name.contains('/')
        || name.len() > 128
        || name.ends_with(['.', ' '])
        || name.chars().any(|c| "<>\"|?*".contains(c))
    {
        return Err(Failure::permanent("Nom de destination non sûr"));
    }
    Ok(())
}
fn validate_deployment(
    deployment: &Deployment,
    app: &AppMetadata,
    recipe: &Recipe,
    policy_root: &Path,
) -> Result<()> {
    match deployment {
        Deployment::Production => Err(Failure::permanent("Déploiement de production désactivé : une publication signée/notarisée de notre gamme doit d’abord être validée. Seul le profil de test isolé est activé dans cette étape.")),
        Deployment::LocalFixture{root,token,tls_certificate,port}=> {
            if !cfg!(feature="installer-fixtures") {return Err(Failure::permanent("Profil de test absent de ce binaire"));}
            if token.len()!=16 || !token.bytes().all(|b|b.is_ascii_hexdigit()) || app.id!=format!("studio-installer-fixture-{token}") || app.release.version!="0.0.1" || *port==0 {return Err(Failure::permanent("Profil réservé au logiciel de test isolé 0.0.1"));}
            fixture_root(root,token)?;
            let cert=security::confined_file(policy_root,tls_certificate).map_err(|e|Failure::permanent(e.to_string()))?;
            if std::fs::metadata(cert)?.len()>16*1024 {return Err(Failure::permanent("Certificat de test trop volumineux"));}
            match recipe {
                Recipe::MacBundle{bundle_name,team_id} => {
                    let mac=app.detection.as_ref().and_then(|r|r.macos.as_ref()).ok_or_else(||Failure::permanent("Identité de test manquante"))?;
                    if bundle_name!="StudioInstallerFixture.app" || team_id.is_some() || mac.bundle_id!=format!("local.studioapp.installer.fixture.{token}") || mac.bundle_paths!=[format!("StudioAppInstallerTest-{token}/{bundle_name}")] {return Err(Failure::permanent("Identité du bundle de test incorrecte"));}
                }
                Recipe::WindowsPortable{directory_name,authenticode_thumbprint}=> {
                    let win=app.detection.as_ref().and_then(|r|r.windows.as_ref()).ok_or_else(||Failure::permanent("Identité de test manquante"))?;
                    if directory_name!="StudioInstallerFixture" || authenticode_thumbprint.is_some() || win.uninstall_key!=format!("StudioAppInstallerTest-{token}") || win.publisher!="Studio App Installer Test" || win.executable!="studio-installer-probe.exe" {return Err(Failure::permanent("Identité Windows de test incorrecte"));}
                }
            }
            Ok(())
        }
    }
}
pub fn fixture_root(root: &Path, token: &str) -> Result<PathBuf> {
    if !root.is_absolute() || std::fs::symlink_metadata(root)?.file_type().is_symlink() {
        return Err(Failure::permanent(
            "Racine de test absolue, sans lien symbolique, requise",
        ));
    }
    let root = root.canonicalize()?;
    if root
        .file_name()
        .is_none_or(|s| s != format!("StudioAppInstallerTest-{token}").as_str())
    {
        return Err(Failure::permanent("Nom de racine de test incorrect"));
    }
    #[cfg(target_os = "macos")]
    if root.parent()
        != directories::BaseDirs::new()
            .and_then(|d| d.home_dir().join("Applications").canonicalize().ok())
            .as_deref()
    {
        return Err(Failure::permanent(
            "Test macOS confiné au sous-dossier dédié de ~/Applications",
        ));
    }
    #[cfg(not(target_os = "macos"))]
    if !root.starts_with(std::env::temp_dir().canonicalize()?) {
        return Err(Failure::permanent("Test confiné au dossier temporaire"));
    }
    let marker = security::confined_file(&root, FIXTURE_MARKER)
        .map_err(|e| Failure::permanent(e.to_string()))?;
    if read_bounded(&marker, 128).map_err(|e| Failure::permanent(e.to_string()))?
        != token.as_bytes()
    {
        return Err(Failure::permanent(
            "Marqueur de propriété du test absent ou incorrect",
        ));
    }
    Ok(root)
}
