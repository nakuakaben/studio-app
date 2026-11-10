//! Offline gate: proposals contain no trust keys and never manufacture approvals.
use super::{digest, model::*, sync, Error, Result};
use crate::{
    catalog::{self, model::*},
    installer::{
        approval::{hex, signing_message, unhex},
        model::{ApprovalBody, SignedApproval},
    },
};
use ring::signature::Ed25519KeyPair;
use std::collections::HashSet;
fn check(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::invalid(message))
    }
}
fn find<'a>(registry: &'a Registry, id: &str) -> Result<&'a Registration> {
    registry
        .applications
        .iter()
        .find(|r| r.metadata.id == id)
        .ok_or_else(|| Error::invalid("Application absente du registre explicitement approuvé"))
}
/// Also used before PR creation. Does not imply package approval or production readiness.
pub fn validate_proposal(registry: &Registry, base: &Catalog, proposal: &Proposal) -> Result<()> {
    sync::validate(registry)?;
    catalog::parse(&serde_json::to_vec(base)?)?;
    catalog::parse(&serde_json::to_vec(&proposal.catalog)?)?;
    check(
        proposal.schema_version == 1
            && proposal.registry_sha256 == digest(registry)?
            && proposal.baseline_sha256 == digest(base)?,
        "Proposition périmée ou registre/baseline modifiés",
    )?;
    check(
        proposal.catalog.schema_version == 2 && proposal.catalog.name == base.name,
        "Identité du catalogue modifiée",
    )?;
    check(
        proposal.changed_ids == sync::changes(base, &proposal.catalog)?,
        "Liste des modifications incohérente",
    )?;
    let mut observed = HashSet::new();
    for o in &proposal.observations {
        check(
            o.release_id > 0 && observed.insert(&o.application_id),
            "Observation de release invalide ou dupliquée",
        )?;
        find(registry, &o.application_id)?;
    }
    for app in &proposal.catalog.applications {
        let r = find(registry, &app.id)?;
        let old = base.applications.iter().find(|a| a.id == app.id);
        let expected = r.metadata.application(
            r.github.clone(),
            app.release.clone(),
            sync::history(old, &app.release)?,
        );
        check(
            serde_json::to_vec(&expected)? == serde_json::to_vec(app)?,
            "Métadonnées éditoriales ou historique différents du registre/baseline",
        )?;
        check(
            app.release.packages.len() == r.packages.len(),
            "Packages manquants ou ajoutés hors des bindings",
        )?;
        let tag = format!("{}{}", r.tag_prefix, app.release.version);
        let version = semver::Version::parse(&app.release.version)
            .map_err(|_| Error::invalid("Version invalide"))?;
        check(
            r.channel != Channel::Stable || version.pre.is_empty(),
            "Prerelease refusée dans le canal stable",
        )?;
        for (p, b) in app.release.packages.iter().zip(&r.packages) {
            check(
                p.id == b.package_id
                    && p.os == b.os
                    && p.architecture == b.architecture
                    && p.format == b.format
                    && p.size_bytes <= b.max_size_bytes
                    && p.github_asset_id.is_some(),
                "Binding de package incohérent",
            )?;
            check(
                p.url == sync::expected_url(r, b, &app.release.version, &tag)?,
                "Source de package non configurée",
            )?;
        }
        if proposal.changed_ids.contains(&app.id) {
            let o = proposal
                .observations
                .iter()
                .find(|o| o.application_id == app.id)
                .ok_or_else(|| Error::invalid("Modification sans observation de release"))?;
            check(
                o.tag == tag
                    && o.asset_ids
                        == app
                            .release
                            .packages
                            .iter()
                            .filter_map(|p| p.github_asset_id)
                            .collect::<Vec<_>>(),
                "Provenance de release incohérente",
            )?;
        }
    }
    Ok(())
}
pub fn request(
    registry: &Registry,
    proposal: &Proposal,
    id: &str,
    package_id: &str,
    expires_unix: u64,
) -> Result<ApprovalBody> {
    let app = proposal
        .catalog
        .applications
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| Error::invalid("Application proposée absente"))?;
    let registration = find(registry, id)?;
    let binding = registration
        .packages
        .iter()
        .find(|b| b.package_id == package_id)
        .ok_or_else(|| Error::invalid("Package non configuré"))?;
    check(
        app.release.packages.iter().any(|p| p.id == package_id),
        "Package proposé absent",
    )?;
    Ok(ApprovalBody {
        schema_version: 1,
        application: app.clone(),
        package_id: package_id.into(),
        recipe: binding.recipe.clone(),
        expires_unix,
    })
}
/// Explicit offline operator action. Never called by synchronization or Actions.
pub fn sign(body: ApprovalBody, pkcs8: &[u8], key_id: &str) -> Result<SignedApproval> {
    check(
        crate::catalog::security::identifier(key_id),
        "Identifiant de clé invalide",
    )?;
    let key = Ed25519KeyPair::from_pkcs8(pkcs8)
        .map_err(|_| Error::invalid("Clé Ed25519 PKCS8 invalide (contenu masqué)"))?;
    let signature = hex(key
        .sign(&signing_message(&body).map_err(|e| Error::invalid(&e.message))?)
        .as_ref());
    Ok(SignedApproval {
        key_id: key_id.into(),
        body,
        signature,
    })
}
#[allow(clippy::too_many_arguments)]
pub fn verify(
    registry: &Registry,
    base: &Catalog,
    proposal: &Proposal,
    candidate: &Catalog,
    approvals: &Approvals,
    trust: &Trust,
    environment: Environment,
    now: u64,
) -> Result<()> {
    validate_proposal(registry, base, proposal)?;
    check(
        !proposal.changed_ids.is_empty(),
        "Aucune modification à publier",
    )?;
    check(
        serde_json::to_vec(candidate)? == serde_json::to_vec(&proposal.catalog)?,
        "Catalogue différent de la proposition vérifiée",
    )?;
    check(
        approvals.schema_version == 1
            && trust.schema_version == 1
            && approvals.environment == environment
            && trust.environment == environment,
        "Environnements de production et de test distincts requis",
    )?;
    check(
        !trust.keys.is_empty() && trust.keys.len() <= 16 && approvals.approvals.len() <= 128,
        "Clés de confiance absentes ou limites d’approbation dépassées",
    )?;
    let mut keys = HashSet::new();
    let mut public_keys = HashSet::new();
    for key in &trust.keys {
        check(
            crate::catalog::security::identifier(&key.id)
                && keys.insert(&key.id)
                && public_keys.insert(key.public_key.to_lowercase()),
            "Clé dupliquée ou identifiant invalide",
        )?;
        unhex(&key.public_key, 32).map_err(|e| Error::invalid(&e.message))?;
    }
    let required: usize = candidate
        .applications
        .iter()
        .map(|a| a.release.packages.len())
        .sum();
    check(
        required == approvals.approvals.len(),
        "Une approbation exacte par package courant est obligatoire",
    )?;
    for app in &candidate.applications {
        let r = find(registry, &app.id)?;
        check(r.visibility!=Visibility::Private||r.packages.iter().all(|b|b.public_url.is_some()),"Publication d’URLs de packages privés interdite ; miroir public explicite vérifié requis")?;
        for package in &app.release.packages {
            let expected = request(registry, proposal, &app.id, &package.id, 0)?;
            let matching: Vec<_> = approvals
                .approvals
                .iter()
                .filter(|a| a.body.application.id == app.id && a.body.package_id == package.id)
                .collect();
            check(matching.len() == 1, "Approbation absente ou ambiguë")?;
            let a = matching[0];
            check(
                a.body.schema_version == 1 && a.body.expires_unix > now,
                "Approbation expirée ou version inconnue",
            )?;
            check(
                serde_json::to_vec(&a.body.application)? == serde_json::to_vec(app)?
                    && serde_json::to_vec(&a.body.recipe)? == serde_json::to_vec(&expected.recipe)?,
                "Métadonnées ou recette non approuvées",
            )?;
            let key = trust
                .keys
                .iter()
                .find(|k| k.id == a.key_id)
                .ok_or_else(|| Error::invalid("Clé d’approbation non autorisée"))?;
            ring::signature::UnparsedPublicKey::new(
                &ring::signature::ED25519,
                unhex(&key.public_key, 32).map_err(|e| Error::invalid(&e.message))?,
            )
            .verify(
                &signing_message(&a.body).map_err(|e| Error::invalid(&e.message))?,
                &unhex(&a.signature, 64).map_err(|e| Error::invalid(&e.message))?,
            )
            .map_err(|_| Error::invalid("Signature Ed25519 de publication invalide"))?;
        }
    }
    Ok(())
}
