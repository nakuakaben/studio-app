//! Versioned, owned catalog metadata. No local state, commands or installation flags.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub name: String,
    pub applications: Vec<AppMetadata>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppMetadata {
    pub id: String,
    pub name: String,
    pub short_description: String,
    pub description: String,
    pub category: String,
    pub color: [u8; 3],
    pub monogram: String,
    pub publisher: String,
    #[serde(default)]
    pub fictional: bool,
    pub logo: Option<Asset>,
    #[serde(default)]
    pub screenshots: Vec<Screenshot>,
    #[serde(default)]
    pub features: Vec<Feature>,
    pub release: Release,
    #[serde(default)]
    pub history: Vec<Release>,
    #[serde(default)]
    pub links: Links,
    pub platforms: Vec<Platform>,
    #[serde(default)]
    pub github: Option<GitHubSource>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub source: String,
    pub alt: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Screenshot {
    pub image: Asset,
    pub caption: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    pub title: String,
    pub description: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub version: String,
    pub published: String,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub packages: Vec<Package>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Links {
    pub website: Option<String>,
    pub documentation: Option<String>,
    pub github: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubSource {
    pub owner: String,
    pub repository: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Platform {
    pub os: Os,
    pub architectures: Vec<Architecture>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Os {
    Macos,
    Windows,
    Linux,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Architecture {
    Aarch64,
    X86_64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PackageFormat {
    Dmg,
    Pkg,
    Msi,
    Exe,
    AppImage,
    Deb,
    Rpm,
    Zip,
    TarGz,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub id: String,
    pub os: Os,
    pub architecture: Architecture,
    pub format: PackageFormat,
    pub url: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub github_asset_id: Option<u64>,
}
