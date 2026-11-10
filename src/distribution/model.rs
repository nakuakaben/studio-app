//! Editorial registration is distinct from discovered releases and signed approvals.
use crate::{
    catalog::model::*,
    installer::model::{Recipe, SignedApproval, TrustedKey},
    native::DetectionRules,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub short_description: String,
    pub description: String,
    pub category: String,
    pub color: [u8; 3],
    pub monogram: String,
    pub logo: Option<Asset>,
    #[serde(default)]
    pub screenshots: Vec<Screenshot>,
    #[serde(default)]
    pub features: Vec<Feature>,
    #[serde(default)]
    pub links: Links,
    pub platforms: Vec<Platform>,
    pub detection: DetectionRules,
}
impl Definition {
    pub fn application(
        &self,
        github: GitHubSource,
        release: Release,
        history: Vec<Release>,
    ) -> AppMetadata {
        AppMetadata {
            id: self.id.clone(),
            name: self.name.clone(),
            publisher: self.publisher.clone(),
            short_description: self.short_description.clone(),
            description: self.description.clone(),
            category: self.category.clone(),
            color: self.color,
            monogram: self.monogram.clone(),
            fictional: false,
            logo: self.logo.clone(),
            screenshots: self.screenshots.clone(),
            features: self.features.clone(),
            links: self.links.clone(),
            platforms: self.platforms.clone(),
            detection: Some(self.detection.clone()),
            github: Some(github),
            release,
            history,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Private,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Stable,
    Prerelease,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HashPolicy {
    Compute,
    RequireGithubDigest,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub package_id: String,
    pub asset_name: String,
    pub os: Os,
    pub architecture: Architecture,
    pub format: PackageFormat,
    pub max_size_bytes: u64,
    pub hash_policy: HashPolicy,
    pub recipe: Recipe,
    /// Required to publish packages from a private source: public, explicitly configured mirror.
    pub public_url: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub metadata: Definition,
    pub github: GitHubSource,
    pub visibility: Visibility,
    #[serde(default)]
    pub allow_private_metadata_in_proposals: bool,
    pub channel: Channel,
    pub tag_prefix: String,
    pub packages: Vec<Binding>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub schema_version: u32,
    pub applications: Vec<Registration>,
}
// GitHub responses intentionally accept additional API fields. Editorial files do not.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetInfo {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub state: String,
    pub browser_download_url: String,
    pub digest: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub id: u64,
    pub tag_name: String,
    pub body: Option<String>,
    pub published_at: Option<String>,
    pub html_url: String,
    pub draft: bool,
    pub prerelease: bool,
    pub assets: Vec<AssetInfo>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub application_id: String,
    pub release_id: u64,
    pub tag: String,
    pub asset_ids: Vec<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub schema_version: u32,
    pub baseline_sha256: String,
    pub registry_sha256: String,
    pub catalog: Catalog,
    pub observations: Vec<Observation>,
    pub changed_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Production,
    Test,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trust {
    pub schema_version: u32,
    pub environment: Environment,
    pub keys: Vec<TrustedKey>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approvals {
    pub schema_version: u32,
    pub environment: Environment,
    pub approvals: Vec<SignedApproval>,
}
