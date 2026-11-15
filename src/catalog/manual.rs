//! Optional, operator-configured handoff to a public release page. No installer authority.
use super::{
    cache::digest,
    github,
    loader::{Settings, Transport},
    model::*,
    security, Error, Result, MAX_CATALOG_BYTES,
};
use crate::distribution::{
    model::{Definition, ReleaseInfo},
    sync,
};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    github: GitHubSource,
    metadata: Definition,
    installers: Vec<Installer>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Installer {
    os: Os,
    architecture: Architecture,
    filename: String,
}
#[derive(Clone, Debug)]
pub struct ManualRelease {
    pub application_id: String,
    /// Exact validated GitHub release page, not an executable URL.
    pub page: String,
    targets: Vec<DiscoveredInstaller>,
    macos_beta_cores: HashSet<(u64, u64, u64)>,
}
/// Display metadata from GitHub. Only the approved catalogue can supply a Package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredInstaller {
    pub os: Os,
    pub architecture: Architecture,
    pub filename: String,
    pub size_bytes: u64,
    pub sha256: String,
}
impl ManualRelease {
    pub fn version_is_ambiguous(&self, os: &Os, installed: &str) -> bool {
        *os == Os::Macos
            && semver::Version::parse(installed).is_ok_and(|v| {
                v.pre.is_empty() && self.macos_beta_cores.contains(&(v.major, v.minor, v.patch))
            })
    }
    pub fn supports(&self, os: &Os, arch: &Architecture) -> bool {
        self.installer(os, arch).is_some()
    }
    pub fn installer(&self, os: &Os, arch: &Architecture) -> Option<&DiscoveredInstaller> {
        self.targets
            .iter()
            .find(|asset| asset.os == *os && asset.architecture == *arch)
    }
}

fn inspect(source: &Source, bytes: &[u8]) -> Result<Option<(AppMetadata, ManualRelease)>> {
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(Error::invalid("Réponse Releases trop volumineuse"));
    }
    let releases: Vec<ReleaseInfo> = serde_json::from_slice(bytes)?;
    if releases.len() > 20 {
        return Err(Error::invalid("Réponse Releases trop volumineuse"));
    }
    let repository = format!(
        "https://github.com/{}/{}",
        source.github.owner, source.github.repository
    );
    let mut best: Option<(semver::Version, AppMetadata, ManualRelease)> = None;
    let mut macos_beta_cores = HashSet::new();
    for release in releases {
        if release.draft || release.published_at.is_none() {
            continue;
        }
        let Some(v) = release
            .tag_name
            .strip_prefix('v')
            .and_then(|s| semver::Version::parse(s).ok())
        else {
            continue;
        };
        if !v.build.is_empty()
            || (!v.pre.is_empty() && !release.prerelease)
            || release.assets.len() > 128
        {
            continue;
        }
        let page = format!("{repository}/releases/tag/v{v}");
        if release.id == 0 || release.html_url != page {
            continue;
        }
        let targets: Vec<_> = source
            .installers
            .iter()
            .filter_map(|installer| {
                let name =
                    sync::expand(&installer.filename, &v.to_string(), &release.tag_name).ok()?;
                let mut matches = release.assets.iter().filter(|asset| asset.name == name);
                let asset = matches.next()?;
                let sha256 = asset.digest.as_deref()?.strip_prefix("sha256:")?;
                if matches.next().is_some()
                    || asset.state != "uploaded"
                    || asset.id == 0
                    || asset.size == 0
                    || asset.size > sync::MAX_PACKAGE
                    || asset.browser_download_url
                        != format!("{repository}/releases/download/v{v}/{name}")
                    || crate::installer::approval::unhex(sha256, 32).is_err()
                {
                    return None;
                }
                Some(DiscoveredInstaller {
                    os: installer.os.clone(),
                    architecture: installer.architecture.clone(),
                    filename: name,
                    size_bytes: asset.size,
                    sha256: sha256.to_ascii_lowercase(),
                })
            })
            .collect();
        if targets.len() != source.installers.len() {
            // Wait for the complete configured release; do not announce an update unavailable on a target.
            continue;
        }
        if !v.pre.is_empty() && targets.iter().any(|asset| asset.os == Os::Macos) {
            macos_beta_cores.insert((v.major, v.minor, v.patch));
        }
        let published = time::OffsetDateTime::parse(
            release.published_at.as_deref().unwrap_or_default(),
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(|_| Error::invalid("Date de release publique invalide"))?;
        let notes = release
            .body
            .unwrap_or_default()
            .lines()
            .filter(|s| !s.trim().is_empty() && s.len() <= 2048 && !s.chars().any(char::is_control))
            .take(16)
            .map(str::to_owned)
            .collect();
        let mut app = source.metadata.application(
            source.github.clone(),
            Release {
                version: v.to_string(),
                published: published.date().to_string(),
                notes,
                packages: vec![],
            },
            vec![],
        );
        app.links.github = Some(repository.clone());
        let handoff = ManualRelease {
            application_id: app.id.clone(),
            page,
            targets,
            macos_beta_cores: HashSet::new(),
        };
        if best
            .as_ref()
            .is_some_and(|(old, _, _)| old.cmp_precedence(&v).is_eq())
        {
            return Err(Error::invalid("Version publique ambiguë"));
        }
        if best
            .as_ref()
            .is_none_or(|(old, _, _)| v.cmp_precedence(old).is_gt())
        {
            best = Some((v, app, handoff));
        }
    }
    Ok(best.map(|(_, app, mut handoff)| {
        handoff.macos_beta_cores = macos_beta_cores;
        (app, handoff)
    }))
}

fn definition(bytes: &[u8]) -> Result<Source> {
    let source: Source = serde_json::from_slice(bytes)?;
    github::validate_source(&source.github)?;
    if !security::identifier(&source.metadata.id)
        || source.installers.is_empty()
        || source.installers.len() > 8
    {
        return Err(Error::invalid(
            "Configuration des installeurs publics invalide",
        ));
    }
    let mut targets = HashSet::new();
    for rule in &source.installers {
        let filename = sync::expand(&rule.filename, "0.0.0", "v0.0.0")?;
        if !filename
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
            || filename.is_empty()
            || filename.len() > 128
            || !targets.insert((&rule.os, &rule.architecture))
            || !source
                .metadata
                .platforms
                .iter()
                .any(|p| p.os == rule.os && p.architectures.contains(&rule.architecture))
        {
            return Err(Error::invalid("Cible ou nom d’installeur public invalide"));
        }
    }
    Ok(source)
}

pub(super) fn enrich(
    settings: &Settings,
    loaded: &mut super::loader::Loaded,
    http: &dyn Transport,
) {
    let Some(path) = &settings.config.manual_release else {
        return;
    };
    let result = (|| {
        let bytes =
            super::cache::read_bounded(&security::confined_file(&settings.root, path)?, 32 * 1024)?;
        let source = definition(&bytes)?;
        // An explicit entry in the actual catalog has precedence, including native approvals.
        if loaded
            .catalog
            .applications
            .iter()
            .any(|a| a.id == source.metadata.id)
        {
            return Ok(None);
        }
        let key = format!("manual-{}.json", digest(&bytes));
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases?per_page=20",
            source.github.owner, source.github.repository
        );
        let accept = |bytes: &[u8]| -> Result<Option<(Catalog, ManualRelease)>> {
            let Some((app, handoff)) = inspect(&source, bytes)? else {
                return Ok(None);
            };
            let mut next = loaded.catalog.clone();
            next.schema_version = 2;
            next.applications.push(app);
            let next = settings.validate_bytes(&serde_json::to_vec(&next)?)?;
            Ok(Some((next, handoff)))
        };
        let fresh = http.get(&url, MAX_CATALOG_BYTES).and_then(|bytes| {
            let value = accept(&bytes)?;
            let _ = settings.cache.write(&key, &bytes);
            Ok(value.map(|(catalog, handoff)| (catalog, handoff, false)))
        });
        match fresh {
            Ok(value) => Ok(value),
            Err(error) => settings
                .cache
                .read(&key, MAX_CATALOG_BYTES)
                .and_then(|bytes| accept(&bytes))
                .map(|cached| cached.map(|(catalog, handoff)| (catalog, handoff, true)))
                .map_err(|_| error),
        }
    })();
    match result {
        Ok(Some((catalog, handoff, cached))) => {
            loaded.catalog = catalog;
            loaded.manual_release = Some(handoff);
            if cached {
                append_warning(loaded, "Release GitHub en cache");
            }
        }
        Ok(None) => {}
        Err(_) => append_warning(loaded, "Release GitHub indisponible"),
    }
}
fn append_warning(loaded: &mut super::loader::Loaded, text: &str) {
    loaded.warning = Some(match loaded.warning.take() {
        Some(previous) => format!("{previous} · {text}"),
        None => text.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::loader::{self, Config, Source as CatalogSource};
    use serde_json::{json, Value};

    fn hmp() -> Source {
        definition(include_bytes!("../../catalog/hyper-media-puller.json")).unwrap()
    }

    fn release(version: &str) -> Value {
        let root = "https://github.com/nakuakaben/hyper-media-puller/releases";
        json!({
            "id": 1, "tag_name": format!("v{version}"), "draft": false, "prerelease": version.contains("beta"),
            "html_url": format!("{root}/tag/v{version}"), "published_at": "2026-11-15T16:26:32Z", "body": "Notes",
            "assets":hmp().installers.iter().enumerate().map(|(i, a)| {
                let name = a.filename.replace("{version}", version);
                json!({"name":name,"id":i+1,"size":1000,"state":"uploaded",
                    "digest":format!("sha256:{}", "ab".repeat(32)),
                    "browser_download_url":format!("{root}/download/v{version}/{name}")})
            }).collect::<Vec<_>>()
        })
    }

    #[test]
    fn latest_complete_public_release_is_manual_only() {
        let bytes =
            serde_json::to_vec(&vec![release("0.1.0-beta.1"), release("0.1.0-beta.2")]).unwrap();
        let (app, action) = inspect(&hmp(), &bytes).unwrap().unwrap();
        assert_eq!(app.release.version, "0.1.0-beta.2");
        assert!(app.release.packages.is_empty());
        assert!(action.supports(&Os::Macos, &Architecture::Aarch64));
        let mac = action
            .installer(&Os::Macos, &Architecture::Aarch64)
            .unwrap();
        let windows = action
            .installer(&Os::Windows, &Architecture::X86_64)
            .unwrap();
        assert_eq!(mac.filename, "HMP-V2-0.1.0-beta.2-macos-arm64.dmg");
        assert_eq!(
            windows.filename,
            "HMP-V2-0.1.0-beta.2-windows-x64-setup.exe"
        );
        assert_eq!(windows.sha256, "ab".repeat(32));
        assert_eq!(mac.size_bytes, 1000);
        assert!(action
            .installer(&Os::Windows, &Architecture::Aarch64)
            .is_none());
        assert!(action.version_is_ambiguous(&Os::Macos, "0.1.0"));
        assert!(!action.version_is_ambiguous(&Os::Macos, "0.1.0-beta.1"));
        assert!(!action.version_is_ambiguous(&Os::Windows, "0.1.0"));
        assert!(!action.version_is_ambiguous(&Os::Macos, "0.0.9"));
        assert!(action.supports(&Os::Windows, &Architecture::X86_64));
        assert!(!action.supports(&Os::Macos, &Architecture::X86_64));
        assert!(action.page.ends_with("/releases/tag/v0.1.0-beta.2"));
        let mut application = crate::domain::Application::new(app);
        application.local.version = Some("0.1.0-beta.1".into());
        assert_eq!(
            application.state(),
            crate::domain::AppState::UpdateAvailable
        );
    }

    #[test]
    fn refuses_draft_and_missing_or_untrusted_installer_metadata() {
        for (path, value) in [
            ("/draft", json!(true)),
            ("/published_at", Value::Null),
            ("/html_url", json!("https://example.com/not-a-release")),
            (
                "/assets/0/browser_download_url",
                json!("https://example.com/setup.dmg"),
            ),
            ("/assets/0/state", json!("new")),
            ("/assets/0/digest", json!("sha256:invalid")),
            ("/assets", json!([])),
        ] {
            let mut r = release("0.1.0-beta.1");
            *r.pointer_mut(path).unwrap() = value;
            assert!(
                inspect(&hmp(), &serde_json::to_vec(&[r]).unwrap())
                    .unwrap()
                    .is_none(),
                "{path}"
            );
        }
    }

    struct Network(Option<Vec<u8>>);
    impl Transport for Network {
        fn get(&self, url: &str, _: usize) -> Result<Vec<u8>> {
            assert_eq!(
                url,
                "https://api.github.com/repos/nakuakaben/hyper-media-puller/releases?per_page=20"
            );
            self.0.clone().ok_or_else(|| Error::network("hors ligne"))
        }
    }

    #[test]
    fn cached_public_release_survives_network_error_but_not_removal() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("manual.json"),
            include_bytes!("../../catalog/hyper-media-puller.json"),
        )
        .unwrap();
        std::fs::write(
            root.join("base.json"),
            br#"{"name":"test","schema_version":2,"applications":[]}"#,
        )
        .unwrap();
        let settings = Settings::new(
            Config {
                source: CatalogSource::Local {
                    path: "base.json".into(),
                },
                allowed_asset_hosts: vec![],
                demo_scenarios: false,
                manual_release: Some("manual.json".into()),
            },
            root.to_path_buf(),
            &root.join("cache"),
        )
        .unwrap();
        let bytes = serde_json::to_vec(&[release("0.1.0-beta.1")]).unwrap();
        assert!(loader::load(&settings, &Network(Some(bytes)))
            .unwrap()
            .manual_release
            .is_some());
        assert!(loader::load(&settings, &Network(None))
            .unwrap()
            .manual_release
            .is_some());
        assert!(loader::load(&settings, &Network(Some(b"[]".to_vec())))
            .unwrap()
            .manual_release
            .is_none());
        assert!(loader::load(&settings, &Network(None))
            .unwrap()
            .manual_release
            .is_none());
    }
}
