//! Blocking I/O lives on the catalog worker, never on egui's render thread.
use super::{
    assets,
    cache::{read_bounded, Cache},
    model::Catalog,
    security, Error, Result, MAX_CATALOG_BYTES,
};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

pub trait Transport: Send + Sync {
    fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>>;
}
pub struct HttpsTransport;
impl Transport for HttpsTransport {
    fn get(&self, value: &str, limit: usize) -> Result<Vec<u8>> {
        let url = security::https_url(value)?;
        let addresses = security::public_addresses(&url)?;
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(10))
            .user_agent("Studio-App/0.1 catalog-reader")
            .resolve_to_addrs(
                url.host_str()
                    .ok_or_else(|| Error::invalid("Hôte absent"))?,
                &addresses,
            )
            .build()
            .map_err(|_| Error::network("Client HTTPS indisponible"))?;
        let response = client
            .get(url)
            .header("Accept", "application/json, image/png, image/jpeg")
            .send()
            .map_err(|_| Error::network("Requête HTTPS échouée ou délai dépassé"))?;
        if !response.status().is_success() {
            return Err(Error::network(&format!(
                "Statut HTTP {}",
                response.status()
            )));
        }
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err(Error::invalid("Réponse HTTPS trop volumineuse"));
        }
        let mut bytes = Vec::new();
        response
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::network("Réponse HTTPS interrompue"))?;
        if bytes.len() > limit {
            return Err(Error::invalid("Réponse HTTPS trop volumineuse"));
        }
        Ok(bytes)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub source: Source,
    #[serde(default)]
    pub allowed_asset_hosts: Vec<String>,
    #[serde(default)]
    pub demo_scenarios: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Local { path: String },
    Https { url: String },
}
#[derive(Clone)]
pub struct Settings {
    pub config: Config,
    pub root: PathBuf,
    pub cache: Cache,
}
impl Settings {
    pub fn from_path(path: &Path, cache_base: &Path) -> Result<Self> {
        let path = path.canonicalize()?;
        let bytes = read_bounded(&path, 16 * 1024)?;
        let config: Config = serde_json::from_slice(&bytes)?;
        let root = path
            .parent()
            .ok_or_else(|| Error::invalid("Configuration sans dossier parent"))?
            .to_owned();
        Self::new(config, root, cache_base)
    }
    pub fn new(config: Config, root: PathBuf, cache_base: &Path) -> Result<Self> {
        match &config.source {
            Source::Local { path } => {
                security::relative_path(path)?;
            }
            Source::Https { url } => {
                security::https_url(url)?;
            }
        }
        if config.allowed_asset_hosts.len() > 16 {
            return Err(Error::invalid("Maximum 16 hôtes de ressources"));
        }
        for host in &config.allowed_asset_hosts {
            let url = security::https_url(&format!("https://{host}/"))?;
            if url.host_str() != Some(host) || host.contains(['/', ':', '?', '#']) {
                return Err(Error::invalid("Hôte de ressources invalide"));
            }
        }
        let key = format!("{}\n{}", root.display(), serde_json::to_string(&config)?);
        Ok(Self {
            config,
            root,
            cache: Cache::new(cache_base, &key),
        })
    }
    pub fn asset_source(&self, source: &str) -> Result<AssetSource> {
        if source.starts_with("https://") {
            let url = security::https_url(source)?;
            if !self
                .config
                .allowed_asset_hosts
                .iter()
                .any(|h| Some(h.as_str()) == url.host_str())
            {
                return Err(Error::invalid(
                    "Hôte de ressource non autorisé par la configuration locale",
                ));
            }
            Ok(AssetSource::Https(source.to_owned()))
        } else {
            security::relative_path(source)?;
            if !matches!(self.config.source, Source::Local { .. }) {
                return Err(Error::invalid(
                    "Un catalogue distant ne peut lire aucune ressource locale",
                ));
            }
            Ok(AssetSource::Local(source.to_owned()))
        }
    }
    pub fn validate_bytes(&self, bytes: &[u8]) -> Result<Catalog> {
        let catalog = super::parse(bytes)?;
        for asset in assets::references(&catalog) {
            self.asset_source(&asset.source)?;
        }
        Ok(catalog)
    }
}
#[derive(Debug)]
pub enum AssetSource {
    Local(String),
    Https(String),
}
#[derive(Clone, Debug, PartialEq)]
pub enum Origin {
    Local,
    Remote,
    Cache,
}
pub struct Loaded {
    pub catalog: Catalog,
    pub origin: Origin,
    pub warning: Option<String>,
}

pub fn load(settings: &Settings, http: &dyn Transport) -> Result<Loaded> {
    let result = (|| {
        let bytes = match &settings.config.source {
            Source::Local { path } => read_bounded(
                &security::confined_file(&settings.root, path)?,
                MAX_CATALOG_BYTES,
            )?,
            Source::Https { url } => http.get(url, MAX_CATALOG_BYTES)?,
        };
        let catalog = settings.validate_bytes(&bytes)?;
        let warning = settings
            .cache
            .write("catalog.json", &bytes)
            .err()
            .map(|e| format!("Catalogue chargé ; cache non enregistré : {e}"));
        Ok(Loaded {
            catalog,
            origin: match settings.config.source {
                Source::Local { .. } => Origin::Local,
                Source::Https { .. } => Origin::Remote,
            },
            warning,
        })
    })();
    match result {
        Ok(loaded) => Ok(loaded),
        Err(error) => {
            let fallback = settings
                .cache
                .read("catalog.json", MAX_CATALOG_BYTES)
                .and_then(|bytes| settings.validate_bytes(&bytes));
            match fallback {
                Ok(catalog) => Ok(Loaded {
                    catalog,
                    origin: Origin::Cache,
                    warning: Some(format!("{error} · données du cache")),
                }),
                Err(_) => Err(error),
            }
        }
    }
}

pub fn default_paths() -> Result<(PathBuf, PathBuf)> {
    let dirs = directories::ProjectDirs::from("app", "StudioApp", "StudioApp")
        .ok_or_else(|| Error::invalid("Dossiers utilisateur indisponibles"))?;
    let config = if let Some(path) = std::env::var_os("STUDIO_APP_CONFIG") {
        PathBuf::from(path)
    } else {
        let personal = dirs.config_dir().join("catalog-config.json");
        let checkout = std::env::current_exe().ok().and_then(|exe| {
            exe.parent().and_then(|p| {
                p.ancestors()
                    .take(6)
                    .map(|p| p.join("catalog/config.json"))
                    .find(|p| p.is_file())
            })
        });
        if personal.is_file() {
            personal
        } else if Path::new("catalog/config.json").is_file() {
            PathBuf::from("catalog/config.json")
        } else {
            checkout.unwrap_or(personal)
        }
    };
    Ok((
        config,
        std::env::var_os("STUDIO_APP_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs.cache_dir().join("catalog-v1")),
    ))
}
