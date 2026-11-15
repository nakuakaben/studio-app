//! Editorial tooling only. No package installation or approval during sync.
use std::path::Path;
use studio_app::{
    catalog::{self, model::Catalog},
    distribution::{self as dist, approval, github::Github, model::*, sync, Error, Result},
};
fn now() -> Result<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::invalid("Horloge invalide"))?
        .as_secs())
}
fn catalog(path: &str) -> Result<Catalog> {
    catalog::parse(&catalog::cache::read_bounded(
        Path::new(path),
        catalog::MAX_CATALOG_BYTES,
    )?)
}
fn run(args: &[String]) -> Result<()> {
    match args {
        [cmd,output] if cmd=="personal-keygen" => {
            studio_app::installer::personal::generate_key(Path::new(output))
                .map_err(|e| Error::invalid(&e.message))?;
            println!("Clé privée générée localement. Conservez-la hors dépôt.");
        }
        [cmd,key,expires,output] if cmd=="personal-hmp" => {
            use studio_app::installer::personal;
            let key = catalog::cache::read_bounded(Path::new(key), 4096)?;
            let expiry = expires.parse::<u64>().map_err(|_| Error::invalid("Expiration Unix requise"))?;
            let policy = personal::make_policy(&key, now()?, expiry).map_err(|e| Error::invalid(&e.message))?;
            let bytes = serde_json::to_vec_pretty(&policy)?;
            personal::write_new(Path::new(output), &bytes).map_err(|e| Error::invalid(&e.message))?;
            println!("Politique HMP personnelle créée, avec la seule clé publique. Aucun paquet téléchargé.");
        }
        [cmd,registry,baseline,output] if cmd=="sync"=> {
            let registry:Registry=dist::read(Path::new(registry))?;sync::validate(&registry)?;
            let api=Github::new(std::env::var("STUDIO_CATALOG_GITHUB_TOKEN").ok().filter(|token|!token.is_empty()))?;
            let proposal=sync::propose(&registry,&catalog(baseline)?,&api)?;
            dist::write(Path::new(output),&proposal)?;
            println!("{} modification(s) proposée(s). Aucune approbation ni installation effectuée.",proposal.changed_ids.len());
        }
        [cmd,registry,baseline,proposal] if cmd=="validate"=> {
            approval::validate_proposal(&dist::read(Path::new(registry))?,&catalog(baseline)?,&dist::read(Path::new(proposal))?)?;println!("Proposition cohérente ; cela ne constitue pas une approbation.");
        }
        [cmd,registry,baseline,proposal,id,package,expires,output] if cmd=="request"=> {
            let registry:Registry=dist::read(Path::new(registry))?;let proposal:Proposal=dist::read(Path::new(proposal))?;
            approval::validate_proposal(&registry,&catalog(baseline)?,&proposal)?;
            let expiry=expires.parse::<u64>().map_err(|_|Error::invalid("Expiration Unix requise"))?;if expiry<=now()? {return Err(Error::invalid("Expiration future requise"));}
            dist::write(Path::new(output),&approval::request(&registry,&proposal,id,package,expiry)?)?;
        }
        [cmd,body,key,key_id,output] if cmd=="sign"=> {
            let body=dist::read(Path::new(body))?;
            let bytes=catalog::cache::read_bounded(Path::new(key),4096)?;
            dist::write(Path::new(output),&approval::sign(body,&bytes,key_id)?)?;println!("Approbation signée hors ligne ; clé privée non copiée.");
        }
        [cmd,registry,baseline,proposal,candidate,approvals,trust] if cmd=="verify"=> verify(registry,baseline,proposal,candidate,approvals,trust,Environment::Production)?,
        [cmd,registry,baseline,proposal,candidate,approvals,trust,flag] if cmd=="verify"&&flag=="--test"=> verify(registry,baseline,proposal,candidate,approvals,trust,Environment::Test)?,
        _=>return Err(Error::invalid("Usage : studio-catalog personal-keygen <private-pkcs8> | personal-hmp <private-pkcs8> <expires-unix> <policy-output> | sync <registry> <baseline> <proposal-output> | validate <registry> <baseline> <proposal> | request <registry> <baseline> <proposal> <app-id> <package-id> <expires-unix> <body-output> | sign <body> <private-pkcs8> <key-id> <approval-output> | verify <registry> <baseline> <proposal> <candidate> <approvals> <trust> [--test]")),
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn verify(
    registry: &str,
    baseline: &str,
    proposal: &str,
    candidate: &str,
    approvals: &str,
    trust: &str,
    environment: Environment,
) -> Result<()> {
    approval::verify(
        &dist::read(Path::new(registry))?,
        &catalog(baseline)?,
        &dist::read(Path::new(proposal))?,
        &catalog(candidate)?,
        &dist::read(Path::new(approvals))?,
        &dist::read(Path::new(trust))?,
        environment,
        now()?,
    )?;
    println!("Publication vérifiée : métadonnées exactes et signatures Ed25519 valides. Aucun package installé.");
    Ok(())
}
fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
