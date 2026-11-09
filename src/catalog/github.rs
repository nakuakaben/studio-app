//! Discovery is deliberately a different type from the catalog's approved Package.
//! No discovery result can be installed or silently promoted into the catalog.
use super::{loader::Transport, model::GitHubSource, security, Error, Result};
use serde::{Deserialize, Serialize};

pub fn validate_source(source: &GitHubSource) -> Result<()> {
    fn segment(s: &str) -> bool {
        !s.is_empty()
            && s.len() <= 100
            && s != "."
            && s != ".."
            && s.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    }
    if !segment(&source.owner) || !segment(&source.repository) {
        return Err(Error::invalid("Identifiant GitHub invalide"));
    }
    Ok(())
}
#[derive(Debug, Deserialize, Serialize)]
pub struct DiscoveredRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub published_at: Option<String>,
    pub html_url: String,
    pub prerelease: bool,
    pub draft: bool,
    pub assets: Vec<DiscoveredAsset>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct DiscoveredAsset {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}
pub fn parse_releases(bytes: &[u8]) -> Result<Vec<DiscoveredRelease>> {
    if bytes.len() > super::MAX_CATALOG_BYTES {
        return Err(Error::invalid("Réponse GitHub trop volumineuse"));
    }
    let releases: Vec<DiscoveredRelease> = serde_json::from_slice(bytes)?;
    if releases.len() > 20 {
        return Err(Error::invalid("Maximum 20 publications découvertes"));
    }
    for release in &releases {
        if release.tag_name.len() > 256 || release.assets.len() > 128 {
            return Err(Error::invalid("Métadonnées GitHub excessives"));
        }
        let page = security::https_url(&release.html_url)?;
        if page.host_str() != Some("github.com") {
            return Err(Error::invalid("Publication GitHub hors de github.com"));
        }
        for asset in &release.assets {
            let url = security::https_url(&asset.browser_download_url)?;
            if url.host_str() != Some("github.com") || asset.name.len() > 256 {
                return Err(Error::invalid("Asset de découverte GitHub invalide"));
            }
        }
    }
    Ok(releases.into_iter().filter(|r| !r.draft).collect())
}
pub fn discover(source: &GitHubSource, http: &dyn Transport) -> Result<Vec<DiscoveredRelease>> {
    validate_source(source)?;
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=20",
        source.owner, source.repository
    );
    parse_releases(&http.get(&url, super::MAX_CATALOG_BYTES)?)
}
