//! Catalog metadata, actual local installation and in-memory simulation stay separate.
use crate::catalog::model::AppMetadata;
use std::ops::Deref;

#[derive(Clone, Debug, PartialEq)]
pub enum AppState {
    NotInstalled,
    Installed,
    UpdateAvailable,
    Downloading { progress: f32, paused: bool },
    Installing { progress: f32 },
    Error { message: String },
}
impl AppState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NotInstalled => "DISPONIBLE À L’INSTALLATION",
            Self::Installed => "INSTALLÉ · À JOUR",
            Self::UpdateAvailable => "MISE À JOUR DISPONIBLE",
            Self::Downloading { paused: true, .. } => "TÉLÉCHARGEMENT EN PAUSE",
            Self::Downloading { .. } => "TÉLÉCHARGEMENT",
            Self::Installing { .. } => "INSTALLATION",
            Self::Error { .. } => "OPÉRATION INTERROMPUE",
        }
    }
    pub fn action(&self) -> &'static str {
        match self {
            Self::NotInstalled => "SIMULER L’INSTALLATION",
            Self::Installed => "SIMULER L’OUVERTURE",
            Self::UpdateAvailable => "SIMULER LA MISE À JOUR",
            Self::Error { .. } => "RÉESSAYER LA SIMULATION",
            _ => "SIMULATION EN COURS",
        }
    }
    pub fn busy(&self) -> bool {
        matches!(self, Self::Downloading { .. } | Self::Installing { .. })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Operation {
    #[default]
    Idle,
    SimulatedDownload {
        progress: f32,
        paused: bool,
    },
    SimulatedInstall {
        progress: f32,
        target_version: String,
    },
    SimulationError(String),
    Native(crate::installer::Progress),
    NativeError(crate::installer::Failure),
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocalInstallation {
    /// Populated only by the native detector, NEVER by catalog metadata or simulator.
    pub version: Option<String>,
    pub installations: Vec<crate::native::Installation>,
    pub errors: Vec<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Simulation {
    pub version: Option<String>,
}

pub struct Application {
    pub metadata: AppMetadata,
    pub local: LocalInstallation,
    pub operation: Operation,
    pub simulation: Simulation,
}
impl Deref for Application {
    type Target = AppMetadata;
    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}
impl Application {
    pub fn new(metadata: AppMetadata) -> Self {
        Self {
            metadata,
            local: LocalInstallation::default(),
            operation: Operation::Idle,
            simulation: Simulation::default(),
        }
    }
    pub fn available_version(&self) -> &str {
        &self.release.version
    }
    pub fn displayed_installed_version(&self) -> Option<&str> {
        if self.fictional {
            self.simulation
                .version
                .as_deref()
                .or(self.local.version.as_deref())
        } else {
            self.local.version.as_deref()
        }
    }
    pub fn installed(&self) -> bool {
        !self.local.installations.is_empty() || self.displayed_installed_version().is_some()
    }
    pub fn state(&self) -> AppState {
        match &self.operation {
            Operation::Native(progress) => {
                if progress.phase == crate::installer::Phase::Downloading {
                    AppState::Downloading {
                        progress: progress.fraction(),
                        paused: false,
                    }
                } else {
                    AppState::Installing { progress: 0.0 }
                }
            }
            Operation::NativeError(error) => AppState::Error {
                message: error.message.clone(),
            },
            Operation::SimulatedDownload { progress, paused } => AppState::Downloading {
                progress: *progress,
                paused: *paused,
            },
            Operation::SimulatedInstall { progress, .. } => AppState::Installing {
                progress: *progress,
            },
            Operation::SimulationError(message) => AppState::Error {
                message: message.clone(),
            },
            Operation::Idle => match self.displayed_installed_version() {
                None if !self.local.installations.is_empty() => AppState::Installed,
                None => AppState::NotInstalled,
                Some(v) => {
                    if crate::native::model::compare_versions(v, self.available_version())
                        == Some(std::cmp::Ordering::Less)
                    {
                        AppState::UpdateAvailable
                    } else {
                        AppState::Installed
                    }
                }
            },
        }
    }
    pub fn size_mb(&self) -> f64 {
        let selected = crate::installer::model::platform().and_then(|(os, arch)| {
            crate::installer::model::select(&self.metadata, &os, &arch).ok()
        });
        selected
            .as_ref()
            .or_else(|| self.release.packages.first())
            .map_or(0.0, |p| p.size_bytes as f64 / 1_000_000.0)
    }
}
