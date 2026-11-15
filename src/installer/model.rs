use crate::catalog::model::{AppMetadata, Architecture, Os, Package, PackageFormat};
use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Failure>;
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub message: String,
    pub retryable: bool,
}
impl Failure {
    pub fn permanent(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }
    pub fn retry(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for Failure {}
impl From<std::io::Error> for Failure {
    fn from(e: std::io::Error) -> Self {
        Self::retry(format!("Accès au fichier impossible : {e}"))
    }
}
impl From<serde_json::Error> for Failure {
    fn from(e: serde_json::Error) -> Self {
        Self::permanent(format!("Approbation JSON invalide : {e}"))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Authorizing,
    Downloading,
    Verifying,
    Preparing,
    Installing,
    Detecting,
}
impl Phase {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Authorizing => "VÉRIFICATION DE L’APPROBATION",
            Self::Downloading => "TÉLÉCHARGEMENT",
            Self::Verifying => "VÉRIFICATION DU PACKAGE",
            Self::Preparing => "PRÉPARATION",
            Self::Installing => "INSTALLATION NATIVE",
            Self::Detecting => "DÉTECTION APRÈS INSTALLATION",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub phase: Phase,
    pub bytes: u64,
    pub total: u64,
    pub bytes_per_second: Option<u64>,
    pub cancellable: bool,
}
impl Progress {
    pub fn phase(phase: Phase) -> Self {
        Self {
            cancellable: phase != Phase::Installing && phase != Phase::Detecting,
            phase,
            bytes: 0,
            total: 0,
            bytes_per_second: None,
        }
    }
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.bytes as f64 / self.total as f64).clamp(0.0, 1.0) as f32
        }
    }
}
#[derive(Clone, Default)]
pub struct Cancellation(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn check(&self) -> Result<()> {
        if self.0.load(std::sync::atomic::Ordering::Relaxed) {
            Err(Failure::retry("Opération annulée. Aucun package exécuté."))
        } else {
            Ok(())
        }
    }
}

pub fn platform() -> Option<(Os, Architecture)> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::{
            SystemInformation::{IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_ARM64},
            Threading::{GetCurrentProcess, IsWow64Process2},
        };
        let (mut process, mut host) = (0, 0);
        // SAFETY: pseudohandle for the current process and two valid writable USHORTs.
        if unsafe { IsWow64Process2(GetCurrentProcess(), &mut process, &mut host) } == 0 {
            return None;
        }
        let arch = match host {
            IMAGE_FILE_MACHINE_AMD64 => Architecture::X86_64,
            IMAGE_FILE_MACHINE_ARM64 => Architecture::Aarch64,
            _ => return None,
        };
        Some((Os::Windows, arch))
    }
    #[cfg(target_os = "macos")]
    {
        let arch = match std::env::consts::ARCH {
            "aarch64" => Architecture::Aarch64,
            "x86_64" => Architecture::X86_64,
            _ => return None,
        };
        Some((Os::Macos, arch))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    None
}
pub fn select(app: &AppMetadata, os: &Os, arch: &Architecture) -> Result<Package> {
    if app.fictional || app.detection.is_none() {
        return Err(Failure::permanent(
            "Identité native explicite requise pour installer un vrai logiciel.",
        ));
    }
    let supported = |p: &&Package| {
        p.os == *os
            && p.architecture == *arch
            && matches!(
                (&p.os, &p.format),
                (Os::Macos, PackageFormat::Zip | PackageFormat::Dmg)
                    | (Os::Windows, PackageFormat::Zip)
            )
    };
    let mut candidates = app.release.packages.iter().filter(supported);
    let package = candidates.next().ok_or_else(|| {
        Failure::permanent(
            "Aucun package compatible pris en charge. PKG/MSI/EXE restent désactivés.",
        )
    })?;
    if candidates.next().is_some() {
        return Err(Failure::permanent(
            "Sélection ambiguë : publier un seul package compatible par plateforme/architecture.",
        ));
    }
    if package.size_bytes > 2 * 1024 * 1024 * 1024 {
        return Err(Failure::permanent(
            "Limite d’installation : package de 2 Gio maximum.",
        ));
    }
    Ok(package.clone())
}
pub fn can_request(app: &AppMetadata) -> bool {
    platform().is_some_and(|(os, arch)| select(app, &os, &arch).is_ok())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Recipe {
    MacBundle {
        bundle_name: String,
        team_id: Option<String>,
    },
    WindowsPortable {
        directory_name: String,
        authenticode_thumbprint: Option<String>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalBody {
    pub schema_version: u32,
    pub application: AppMetadata,
    pub package_id: String,
    pub recipe: Recipe,
    pub expires_unix: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedApproval {
    pub key_id: String,
    pub body: ApprovalBody,
    pub signature: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedKey {
    pub id: String,
    pub public_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Deployment {
    Production,
    LocalFixture {
        root: std::path::PathBuf,
        token: String,
        tls_certificate: String,
        port: u16,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema_version: u32,
    pub keys: Vec<TrustedKey>,
    pub approvals: Vec<SignedApproval>,
    pub deployment: Deployment,
}

pub struct Plan {
    pub(crate) app: AppMetadata,
    pub(crate) package: Package,
    pub(crate) recipe: Recipe,
    pub(crate) deployment: Deployment,
    pub(crate) policy_root: std::path::PathBuf,
    pub(crate) expires_unix: u64,
}
pub const FIXTURE_MARKER: &str = ".studio-app-installer-fixture";

/// A crash during the final copy must never create a detected installation.
pub const PENDING_MARKER: &str = ".studio-app-installing";
