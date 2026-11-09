use super::{
    model::*,
    security::{https_url, identifier, relative_path},
    Error, Result,
};
use std::collections::HashSet;

fn check(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::invalid(message))
    }
}
fn text(s: &str, max: usize) -> Result<()> {
    check(
        !s.trim().is_empty() && s.len() <= max && !s.chars().any(|c| c.is_control() && c != '\n'),
        "Texte vide, trop long ou contenant des caractères de contrôle",
    )
}
fn asset(a: &Asset) -> Result<()> {
    text(&a.alt, 256)?;
    if a.source.starts_with("https://") {
        https_url(&a.source)?;
    } else {
        relative_path(&a.source)?;
    }
    Ok(())
}
fn release(r: &Release, platforms: &[Platform]) -> Result<()> {
    check(r.version.len() <= 64, "Version trop longue")?;
    semver::Version::parse(&r.version).map_err(|_| Error::invalid("Version SemVer requise"))?;
    let date_format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
        .map_err(|_| Error::invalid("Format de date"))?;
    time::Date::parse(&r.published, &date_format)
        .map_err(|_| Error::invalid("Date YYYY-MM-DD invalide"))?;
    check(
        r.notes.len() <= 64 && r.packages.len() <= 16,
        "Trop de notes ou de packages",
    )?;
    for note in &r.notes {
        text(note, 2048)?;
    }
    let mut ids = HashSet::new();
    for p in &r.packages {
        check(
            identifier(&p.id) && ids.insert(&p.id),
            "Identifiant de package invalide ou dupliqué",
        )?;
        https_url(&p.url)?;
        check(
            p.size_bytes > 0 && p.size_bytes <= 100 * 1024 * 1024 * 1024,
            "Taille de package invalide",
        )?;
        check(
            p.sha256.len() == 64 && p.sha256.bytes().all(|c| c.is_ascii_hexdigit()),
            "SHA-256 de package requis",
        )?;
        check(p.github_asset_id != Some(0), "Identifiant GitHub invalide")?;
        check(
            platforms.iter().any(|platform| {
                platform.os == p.os && platform.architectures.contains(&p.architecture)
            }),
            "Package incompatible avec les plateformes déclarées",
        )?;
        let compatible = match p.format {
            PackageFormat::Dmg | PackageFormat::Pkg => p.os == Os::Macos,
            PackageFormat::Msi | PackageFormat::Exe => p.os == Os::Windows,
            PackageFormat::AppImage | PackageFormat::Deb | PackageFormat::Rpm => p.os == Os::Linux,
            PackageFormat::Zip | PackageFormat::TarGz => true,
        };
        check(compatible, "Format de package incompatible")?;
    }
    Ok(())
}

pub fn validate(c: &Catalog) -> Result<()> {
    check(
        matches!(c.schema_version, 1 | 2),
        "Version de schéma non supportée (attendue : 1 ou 2)",
    )?;
    text(&c.name, 128)?;
    check(c.applications.len() <= 128, "Maximum 128 applications")?;
    let mut ids = HashSet::new();
    let mut assets = HashSet::new();
    let mut detection_ids = HashSet::new();
    for a in &c.applications {
        check(
            identifier(&a.id) && ids.insert(&a.id),
            "Identifiant applicatif invalide ou dupliqué",
        )?;
        if let Some(rules) = &a.detection {
            check(
                c.schema_version == 2 && !a.fictional,
                "La détection nécessite le schéma 2 et une application non fictive",
            )?;
            if let Some(mac) = &rules.macos {
                check(
                    detection_ids.insert(format!("mac:{}", mac.bundle_id)),
                    "Identité macOS partagée par plusieurs applications",
                )?;
            }
            if let Some(win) = &rules.windows {
                check(
                    detection_ids.insert(format!("windows:{}", win.uninstall_key.to_lowercase())),
                    "Identité Windows partagée par plusieurs applications",
                )?;
            }
            crate::native::model::validate(rules).map_err(|e| Error::invalid(&e))?;
            check(
                rules.macos.is_none() || a.platforms.iter().any(|p| p.os == Os::Macos),
                "Détection macOS sans plateforme déclarée",
            )?;
            check(
                rules.windows.is_none() || a.platforms.iter().any(|p| p.os == Os::Windows),
                "Détection Windows sans plateforme déclarée",
            )?;
        }
        for value in [&a.name, &a.category, &a.publisher] {
            text(value, 128)?;
        }
        text(&a.short_description, 240)?;
        text(&a.description, 8192)?;
        text(&a.monogram, 8)?;
        check(a.monogram.chars().count() <= 3, "Monogramme trop long")?;
        check(
            a.screenshots.len() <= 6 && a.features.len() <= 16 && a.history.len() <= 32,
            "Limite de captures, fonctionnalités ou historique dépassée",
        )?;
        if let Some(logo) = &a.logo {
            asset(logo)?;
            assets.insert(&logo.source);
        }
        for screenshot in &a.screenshots {
            asset(&screenshot.image)?;
            text(&screenshot.caption, 256)?;
            assets.insert(&screenshot.image.source);
        }
        for f in &a.features {
            text(&f.title, 128)?;
            text(&f.description, 2048)?;
        }
        check(
            !a.platforms.is_empty() && a.platforms.len() <= 3,
            "Plateformes absentes ou trop nombreuses",
        )?;
        let mut systems = HashSet::new();
        for p in &a.platforms {
            check(
                systems.insert(&p.os) && !p.architectures.is_empty() && p.architectures.len() <= 2,
                "Plateforme dupliquée ou architectures invalides",
            )?;
            let unique: HashSet<_> = p.architectures.iter().collect();
            check(
                unique.len() == p.architectures.len(),
                "Architecture dupliquée",
            )?;
        }
        release(&a.release, &a.platforms)?;
        let current = semver::Version::parse(&a.release.version)
            .map_err(|_| Error::invalid("Version invalide"))?;
        let mut versions = HashSet::new();
        for r in &a.history {
            release(r, &a.platforms)?;
            let v = semver::Version::parse(&r.version)
                .map_err(|_| Error::invalid("Version invalide"))?;
            check(
                v.cmp_precedence(&current).is_lt() && versions.insert(&r.version),
                "Historique dupliqué ou version non antérieure",
            )?;
        }
        for link in [&a.links.website, &a.links.documentation, &a.links.github]
            .into_iter()
            .flatten()
        {
            https_url(link)?;
        }
        if let Some(g) = &a.github {
            super::github::validate_source(g)?;
        }
    }
    check(assets.len() <= 64, "Maximum 64 ressources distinctes")
}
