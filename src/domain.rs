//! Presentation data, independent of network, filesystem and native operations.
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
#[derive(Clone, Copy)]
pub struct Feature {
    pub title: &'static str,
    pub description: &'static str,
}
pub struct Application {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub color: [u8; 3],
    pub monogram: &'static str,
    pub version: &'static str,
    pub installed_version: Option<&'static str>,
    pub size_mb: u32,
    pub features: Vec<Feature>,
    pub notes: Vec<&'static str>,
    pub state: AppState,
    pub idle_state: AppState,
}
