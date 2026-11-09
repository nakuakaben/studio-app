use super::{digest, model::*, Error, Result};
use crate::catalog::{self, model::*, security};
use std::collections::HashSet;
pub const MAX_PACKAGE: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_TOTAL: u64 = 4 * 1024 * 1024 * 1024;
pub trait Api {
    fn releases(&self, source: &GitHubSource, visibility: &Visibility) -> Result<Vec<ReleaseInfo>>;
    fn hash_asset(&self, source: &GitHubSource, asset: &AssetInfo) -> Result<String>;
    fn hash_public(&self, url: &str, size: u64) -> Result<String>;
}
pub fn expand(template: &str, version: &str, tag: &str) -> Result<String> {
    let value = template.replace("{version}", version).replace("{tag}", tag);
    if value.contains(['{', '}']) {
        return Err(Error::invalid("Variable de modèle inconnue"));
    }
    Ok(value)
}
fn check(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::invalid(message))
    }
}
fn validate_definition(d: &Definition) -> Result<()> {
    fn text(s: &str, max: usize) -> Result<()> {
        check(
            !s.trim().is_empty()
                && s.len() <= max
                && !s.chars().any(|c| c.is_control() && c != '\n'),
            "Métadonnée éditoriale vide, excessive ou avec contrôle",
        )
    }
    for s in [&d.name, &d.publisher, &d.category] {
        text(s, 128)?;
    }
    text(&d.short_description, 240)?;
    text(&d.description, 8192)?;
    text(&d.monogram, 8)?;
    check(
        d.monogram.chars().count() <= 3 && d.screenshots.len() <= 6 && d.features.len() <= 16,
        "Limites de ressources éditoriales dépassées",
    )?;
    for a in d.logo.iter().chain(d.screenshots.iter().map(|s| &s.image)) {
        security::https_url(&a.source)?;
        text(&a.alt, 256)?;
    }
    for s in &d.screenshots {
        text(&s.caption, 256)?;
    }
    for f in &d.features {
        text(&f.title, 128)?;
        text(&f.description, 2048)?;
    }
    for link in [&d.links.website, &d.links.documentation, &d.links.github]
        .into_iter()
        .flatten()
    {
        security::https_url(link)?;
    }
    check(
        !d.platforms.is_empty() && d.platforms.len() <= 3,
        "Plateformes requises",
    )?;
    let mut systems = HashSet::new();
    for p in &d.platforms {
        let unique: HashSet<_> = p.architectures.iter().collect();
        check(
            systems.insert(&p.os)
                && !p.architectures.is_empty()
                && p.architectures.len() <= 2
                && unique.len() == p.architectures.len(),
            "OS/architectures invalides ou dupliqués",
        )?;
    }
    check(
        (d.detection.macos.is_none() || d.platforms.iter().any(|p| p.os == Os::Macos))
            && (d.detection.windows.is_none() || d.platforms.iter().any(|p| p.os == Os::Windows)),
        "Identité native sans plateforme déclarée",
    )?;
    Ok(())
}
pub fn validate(registry: &Registry) -> Result<()> {
    check(
        registry.schema_version == 1 && registry.applications.len() <= 32,
        "Registre de distribution v1, 32 applications maximum",
    )?;
    let mut ids = HashSet::new();
    let mut identities = HashSet::new();
    for r in &registry.applications {
        validate_definition(&r.metadata)?;
        if let Some(rule) = &r.metadata.detection.macos {
            check(
                identities.insert(format!("mac:{}", rule.bundle_id)),
                "Identité macOS partagée",
            )?;
        }
        if let Some(rule) = &r.metadata.detection.windows {
            check(
                identities.insert(format!("win:{}", rule.uninstall_key.to_lowercase())),
                "Identité Windows partagée",
            )?;
        }
        check(
            security::identifier(&r.metadata.id) && ids.insert(&r.metadata.id),
            "Identifiant applicatif invalide ou dupliqué",
        )?;
        catalog::github::validate_source(&r.github)?;
        crate::native::model::validate(&r.metadata.detection).map_err(|e| Error::invalid(&e))?;
        check(r.visibility!=Visibility::Private||r.allow_private_metadata_in_proposals,"Consentement explicite requis pour inclure des métadonnées privées dans une proposition")?;
        check(
            r.tag_prefix.len() <= 32
                && r.tag_prefix
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
            "Préfixe de tag non sûr",
        )?;
        check(
            !r.packages.is_empty() && r.packages.len() <= 16,
            "Entre 1 et 16 bindings de packages requis",
        )?;
        let mut packages = HashSet::new();
        let mut targets = HashSet::new();
        let mut names = HashSet::new();
        for b in &r.packages {
            check(
                security::identifier(&b.package_id) && packages.insert(&b.package_id),
                "ID de package invalide ou dupliqué",
            )?;
            check(
                targets.insert((&b.os, &b.architecture)),
                "Une seule structure approuvée par OS/architecture",
            )?;
            check(
                r.metadata
                    .platforms
                    .iter()
                    .any(|p| p.os == b.os && p.architectures.contains(&b.architecture)),
                "Architecture incompatible avec la définition",
            )?;
            check(
                b.max_size_bytes > 0 && b.max_size_bytes <= MAX_PACKAGE,
                "Limite de package invalide",
            )?;
            let name = expand(&b.asset_name, "0.0.0", "v0.0.0")?;
            crate::installer::approval::simple_name(&name)
                .map_err(|e| Error::invalid(&e.message))?;
            check(
                match b.format {
                    PackageFormat::Zip => name.ends_with(".zip"),
                    PackageFormat::Dmg => name.ends_with(".dmg"),
                    _ => false,
                },
                "Extension différente du format configuré",
            )?;
            check(
                !name.is_empty()
                    && name.len() <= 240
                    && name
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_.+".contains(&c))
                    && names.insert(name),
                "Nom exact d’asset invalide ou ambigu",
            )?;
            let supported = match (&b.os, &b.format, &b.recipe) {
                (
                    Os::Macos,
                    PackageFormat::Zip | PackageFormat::Dmg,
                    crate::installer::model::Recipe::MacBundle { bundle_name, .. },
                ) => {
                    crate::installer::approval::simple_name(bundle_name).is_ok()
                        && bundle_name.ends_with(".app")
                        && r.metadata.detection.macos.is_some()
                }
                (
                    Os::Windows,
                    PackageFormat::Zip,
                    crate::installer::model::Recipe::WindowsPortable { directory_name, .. },
                ) => {
                    crate::installer::approval::simple_name(directory_name).is_ok()
                        && r.metadata.detection.windows.is_some()
                }
                _ => false,
            };
            check(
                supported,
                "Recette/format désactivé ou identité native manquante",
            )?;
            if let Some(url) = &b.public_url {
                check(
                    security::https_url(&expand(url, "0.0.0", "v0.0.0")?)?
                        .query()
                        .is_none(),
                    "URL de miroir sans paramètres d’authentification requise",
                )?;
            }
        }
        for asset in r
            .metadata
            .logo
            .iter()
            .chain(r.metadata.screenshots.iter().map(|s| &s.image))
        {
            security::https_url(&asset.source)?;
        }
    }
    Ok(())
}
pub fn version(r: &Registration, release: &ReleaseInfo) -> Result<semver::Version> {
    let raw = release
        .tag_name
        .strip_prefix(&r.tag_prefix)
        .ok_or_else(|| Error::invalid("Tag sans le préfixe explicitement configuré"))?;
    let v = semver::Version::parse(raw)
        .map_err(|_| Error::invalid("Tag sans version SemVer valide"))?;
    check(
        v.pre.is_empty() || release.prerelease,
        "Tag prerelease présenté comme release stable",
    )?;
    Ok(v)
}
fn source_urls(r: &Registration, release: &ReleaseInfo) -> Result<()> {
    let expected = format!(
        "https://github.com/{}/{}/releases/tag/{}",
        r.github.owner, r.github.repository, release.tag_name
    );
    check(
        release.html_url == expected,
        "Release hors du dépôt configuré",
    )?;
    check(
        release.id > 0 && release.tag_name.len() <= 128 && release.assets.len() <= 128,
        "Métadonnées de release excessives ou invalides",
    )?;
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for a in &release.assets {
        check(
            a.id > 0 && ids.insert(a.id) && names.insert(&a.name),
            "Assets GitHub dupliqués ou sans identifiant",
        )?;
    }
    Ok(())
}
pub fn expected_url(r: &Registration, b: &Binding, version: &str, tag: &str) -> Result<String> {
    if let Some(url) = &b.public_url {
        let value = expand(url, version, tag)?;
        security::https_url(&value)?;
        Ok(value)
    } else {
        Ok(format!(
            "https://github.com/{}/{}/releases/download/{}/{}",
            r.github.owner,
            r.github.repository,
            tag,
            expand(&b.asset_name, version, tag)?
        ))
    }
}
fn release_metadata(
    r: &Registration,
    release: &ReleaseInfo,
    v: &semver::Version,
    api: &dyn Api,
    budget: &mut u64,
) -> Result<Release> {
    source_urls(r, release)?;
    let published = release
        .published_at
        .as_deref()
        .ok_or_else(|| Error::invalid("Date de publication absente"))?;
    time::OffsetDateTime::parse(published, &time::format_description::well_known::Rfc3339)
        .map_err(|_| Error::invalid("Date GitHub RFC3339 invalide"))?;
    let notes = release.body.as_deref().unwrap_or("");
    check(notes.len() <= 64 * 2048, "Notes de version excessives")?;
    let notes: Vec<_> = notes
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .collect();
    check(
        notes.len() <= 64 && notes.iter().all(|s| s.len() <= 2048),
        "Notes limitées à 64 lignes de 2048 octets",
    )?;
    let mut packages = vec![];
    for b in &r.packages {
        let name = expand(&b.asset_name, &v.to_string(), &release.tag_name)?;
        let a = release
            .assets
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| Error::invalid("Asset explicitement configuré manquant"))?;
        check(
            a.state == "uploaded" && a.size > 0 && a.size <= b.max_size_bytes,
            "Asset incomplet ou taille hors limite",
        )?;
        let original = format!(
            "https://github.com/{}/{}/releases/download/{}/{}",
            r.github.owner, r.github.repository, release.tag_name, a.name
        );
        check(
            a.browser_download_url == original,
            "Asset hors du dépôt/tag configuré",
        )?;
        *budget = budget
            .checked_add(a.size)
            .ok_or_else(|| Error::invalid("Budget de transfert dépassé"))?;
        check(
            *budget <= MAX_TOTAL,
            "Maximum 4 Gio de packages par synchronisation",
        )?;
        let hash = api.hash_asset(&r.github, a)?;
        crate::installer::approval::unhex(&hash, 32).map_err(|e| Error::invalid(&e.message))?;
        match &a.digest {
            Some(d) => check(
                d.strip_prefix("sha256:")
                    .is_some_and(|expected| expected.eq_ignore_ascii_case(&hash)),
                "Empreinte GitHub différente des octets téléchargés",
            )?,
            None => check(
                b.hash_policy != HashPolicy::RequireGithubDigest,
                "Digest SHA-256 GitHub obligatoire pour ce binding",
            )?,
        }
        let url = expected_url(r, b, &v.to_string(), &release.tag_name)?;
        if b.public_url.is_some() {
            *budget = budget
                .checked_add(a.size)
                .ok_or_else(|| Error::invalid("Budget dépassé"))?;
            check(*budget <= MAX_TOTAL, "Budget de transfert dépassé")?;
            check(
                api.hash_public(&url, a.size)?.eq_ignore_ascii_case(&hash),
                "Empreinte du miroir public incorrecte",
            )?;
        }
        packages.push(Package {
            id: b.package_id.clone(),
            os: b.os.clone(),
            architecture: b.architecture.clone(),
            format: b.format.clone(),
            url,
            size_bytes: a.size,
            sha256: hash.to_lowercase(),
            github_asset_id: Some(a.id),
        });
    }
    Ok(Release {
        version: v.to_string(),
        published: published[..10].into(),
        notes,
        packages,
    })
}
pub fn history(old: Option<&AppMetadata>, release: &Release) -> Result<Vec<Release>> {
    let Some(old) = old else { return Ok(vec![]) };
    let previous = semver::Version::parse(&old.release.version)
        .map_err(|_| Error::invalid("Version précédente invalide"))?;
    let next = semver::Version::parse(&release.version)
        .map_err(|_| Error::invalid("Version proposée invalide"))?;
    match next.cmp_precedence(&previous) {
        std::cmp::Ordering::Less => Err(Error::invalid("Régression de version interdite")),
        std::cmp::Ordering::Equal => {
            check(
                serde_json::to_vec(&old.release)? == serde_json::to_vec(release)?,
                "Une release déjà connue a changé ; audit manuel requis",
            )?;
            Ok(old.history.clone())
        }
        std::cmp::Ordering::Greater => {
            let mut h = vec![old.release.clone()];
            h.extend(old.history.iter().take(31).cloned());
            Ok(h)
        }
    }
}
pub fn changes(base: &Catalog, next: &Catalog) -> Result<Vec<String>> {
    check(
        base.applications
            .iter()
            .all(|a| next.applications.iter().any(|n| n.id == a.id)),
        "Suppression d’application hors du périmètre de synchronisation",
    )?;
    let mut changed = vec![];
    for app in &next.applications {
        let old = base.applications.iter().find(|a| a.id == app.id);
        if old.map(serde_json::to_vec).transpose()?.as_deref()
            != Some(serde_json::to_vec(app)?.as_slice())
        {
            changed.push(app.id.clone());
        }
    }
    changed.sort();
    Ok(changed)
}
pub fn propose(registry: &Registry, base: &Catalog, api: &dyn Api) -> Result<Proposal> {
    validate(registry)?;
    catalog::parse(&serde_json::to_vec(base)?)?;
    let mut next = base.clone();
    let mut observations = vec![];
    let mut budget = 0;
    for r in &registry.applications {
        let releases = api.releases(&r.github, &r.visibility)?;
        check(releases.len() <= 20, "Maximum 20 releases par dépôt")?;
        let mut candidates = vec![];
        for release in releases {
            if release.draft || (r.channel == Channel::Stable && release.prerelease) {
                continue;
            }
            let v = version(r, &release)?;
            candidates.push((v, release));
        }
        candidates.sort_by(|(a, _), (b, _)| b.cmp_precedence(a));
        let Some((v, release)) = candidates.first() else {
            continue;
        };
        check(
            !candidates
                .iter()
                .skip(1)
                .any(|(other, _)| other.cmp_precedence(v).is_eq()),
            "Plusieurs releases de même version",
        )?;
        let old = base.applications.iter().find(|a| a.id == r.metadata.id);
        if old.is_some_and(|a| {
            semver::Version::parse(&a.release.version)
                .is_ok_and(|old| old.cmp_precedence(v).is_gt())
        }) {
            continue;
        }
        let metadata = release_metadata(r, release, v, api, &mut budget)?;
        let app =
            r.metadata
                .application(r.github.clone(), metadata.clone(), history(old, &metadata)?);
        if let Some(existing) = next.applications.iter_mut().find(|a| a.id == app.id) {
            *existing = app;
        } else {
            next.applications.push(app);
        }
        observations.push(Observation {
            application_id: r.metadata.id.clone(),
            release_id: release.id,
            tag: release.tag_name.clone(),
            asset_ids: r
                .packages
                .iter()
                .map(|b| {
                    release
                        .assets
                        .iter()
                        .find(|a| {
                            expand(&b.asset_name, &v.to_string(), &release.tag_name)
                                .is_ok_and(|n| n == a.name)
                        })
                        .unwrap()
                        .id
                })
                .collect(),
        });
    }
    catalog::parse(&serde_json::to_vec(&next)?)?;
    let changed_ids = changes(base, &next)?;
    Ok(Proposal {
        schema_version: 1,
        baseline_sha256: digest(base)?,
        registry_sha256: digest(registry)?,
        catalog: next,
        observations,
        changed_ids,
    })
}
