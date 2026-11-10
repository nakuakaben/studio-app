//! All repository names, IDs, versions and bytes here are simulated test data.
use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use std::{cell::Cell, path::Path, process::Command};
use studio_app::{
    catalog::model::*,
    distribution::{
        self as dist, approval,
        model::*,
        sync::{self, Api},
        Error, Result,
    },
    installer::model::TrustedKey,
};
fn registry() -> Registry {
    serde_json::from_str(include_str!("fixtures/distribution-registry.json")).unwrap()
}
fn empty() -> Catalog {
    Catalog {
        schema_version: 2,
        name: "Fixture catalog — not production".into(),
        applications: vec![],
    }
}
fn hash() -> String {
    use sha2::Digest;
    studio_app::installer::approval::hex(&sha2::Sha256::digest(b"fixture bytes"))
}
fn release(version: &str) -> ReleaseInfo {
    let r = registry();
    let source = &r.applications[0].github;
    ReleaseInfo {
        id: 42,
        tag_name: format!("v{version}"),
        body: Some("Release notes\nChanges are simulated.".into()),
        published_at: Some("2026-11-09T12:00:00Z".into()),
        html_url: format!(
            "https://github.com/{}/{}/releases/tag/v{version}",
            source.owner, source.repository
        ),
        draft: false,
        prerelease: version.contains('-'),
        assets: r.applications[0]
            .packages
            .iter()
            .enumerate()
            .map(|(i, b)| AssetInfo {
                id: 100 + i as u64,
                name: sync::expand(&b.asset_name, version, &format!("v{version}")).unwrap(),
                size: 13,
                state: "uploaded".into(),
                browser_download_url: sync::expected_url(
                    &r.applications[0],
                    b,
                    version,
                    &format!("v{version}"),
                )
                .unwrap(),
                digest: Some(format!("sha256:{}", hash())),
            })
            .collect(),
    }
}
struct Mock {
    releases: Vec<ReleaseInfo>,
    queries: Cell<usize>,
    downloads: Cell<usize>,
    error: bool,
    bad_hash: bool,
    bad_mirror: bool,
}
impl Mock {
    fn new(releases: Vec<ReleaseInfo>) -> Self {
        Self {
            releases,
            queries: Cell::new(0),
            downloads: Cell::new(0),
            error: false,
            bad_hash: false,
            bad_mirror: false,
        }
    }
}
impl Api for Mock {
    fn releases(&self, _: &GitHubSource, _: &Visibility) -> Result<Vec<ReleaseInfo>> {
        self.queries.set(self.queries.get() + 1);
        if self.error {
            Err(Error::network("HTTP 404: private repository inaccessible"))
        } else {
            Ok(self.releases.clone())
        }
    }
    fn hash_asset(&self, _: &GitHubSource, _: &AssetInfo) -> Result<String> {
        self.downloads.set(self.downloads.get() + 1);
        Ok(if self.bad_hash {
            "00".repeat(32)
        } else {
            hash()
        })
    }
    fn hash_public(&self, _: &str, _: u64) -> Result<String> {
        Ok(if self.bad_mirror {
            "00".repeat(32)
        } else {
            hash()
        })
    }
}
fn propose() -> Proposal {
    sync::propose(&registry(), &empty(), &Mock::new(vec![release("1.2.0")])).unwrap()
}
fn approvals(p: &Proposal) -> (Approvals, Trust, Vec<u8>) {
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
    let signed = p
        .catalog
        .applications
        .iter()
        .flat_map(|a| {
            a.release
                .packages
                .iter()
                .map(|b| {
                    approval::sign(
                        approval::request(&registry(), p, &a.id, &b.id, 4_000_000_000).unwrap(),
                        pkcs8.as_ref(),
                        "ephemeral-test-key",
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>()
        })
        .collect();
    (
        Approvals {
            schema_version: 1,
            environment: Environment::Test,
            approvals: signed,
        },
        Trust {
            schema_version: 1,
            environment: Environment::Test,
            keys: vec![TrustedKey {
                id: "ephemeral-test-key".into(),
                public_key: studio_app::installer::approval::hex(pair.public_key().as_ref()),
            }],
        },
        pkcs8.as_ref().to_vec(),
    )
}
fn verify(p: &Proposal, a: &Approvals, t: &Trust) -> Result<()> {
    approval::verify(
        &registry(),
        &empty(),
        p,
        &p.catalog,
        a,
        t,
        Environment::Test,
        1_800_000_000,
    )
}
#[test]
fn new_release_is_only_a_proposal_with_explicit_bindings() {
    let api = Mock::new(vec![release("1.2.0")]);
    let p = sync::propose(&registry(), &empty(), &api).unwrap();
    assert_eq!(p.changed_ids, ["api-fixture-only"]);
    let a = &p.catalog.applications[0];
    assert_eq!(a.release.version, "1.2.0");
    assert_eq!(a.release.packages[0].architecture, Architecture::Aarch64);
    assert_eq!(a.release.packages[1].os, Os::Windows);
    assert_eq!(api.downloads.get(), 2);
    assert!(empty().applications.is_empty());
    approval::validate_proposal(&registry(), &empty(), &p).unwrap();
}
#[test]
fn known_release_is_idempotent_and_history_is_preserved() {
    let first = propose();
    let same = sync::propose(
        &registry(),
        &first.catalog,
        &Mock::new(vec![release("1.2.0")]),
    )
    .unwrap();
    assert!(same.changed_ids.is_empty());
    let newer = sync::propose(
        &registry(),
        &first.catalog,
        &Mock::new(vec![release("1.3.0")]),
    )
    .unwrap();
    assert_eq!(newer.catalog.applications[0].history[0].version, "1.2.0");
    approval::validate_proposal(&registry(), &first.catalog, &newer).unwrap();
}
#[test]
fn stable_ignores_prereleases_and_drafts() {
    let mut draft = release("2.0.0");
    draft.draft = true;
    let p = sync::propose(
        &registry(),
        &empty(),
        &Mock::new(vec![draft, release("2.0.0-beta.1"), release("1.2.0")]),
    )
    .unwrap();
    assert_eq!(p.catalog.applications[0].release.version, "1.2.0");
    let p = sync::propose(
        &registry(),
        &empty(),
        &Mock::new(vec![release("2.0.0-beta.1")]),
    )
    .unwrap();
    assert!(p.changed_ids.is_empty());
}
#[test]
fn prerelease_requires_explicit_channel() {
    let mut r = registry();
    r.applications[0].channel = Channel::Prerelease;
    let p = sync::propose(&r, &empty(), &Mock::new(vec![release("2.0.0-beta.1")])).unwrap();
    assert_eq!(p.catalog.applications[0].release.version, "2.0.0-beta.1");
}
#[test]
fn missing_asset_and_incompatible_architecture_refused() {
    let mut rel = release("1.2.0");
    rel.assets.pop();
    assert!(sync::propose(&registry(), &empty(), &Mock::new(vec![rel])).is_err());
    let mut r = registry();
    r.applications[0].packages[0].architecture = Architecture::X86_64;
    let api = Mock::new(vec![]);
    assert!(sync::propose(&r, &empty(), &api).is_err());
    assert_eq!(api.queries.get(), 0);
}
#[test]
fn invalid_version_prefix_and_prerelease_label_refused() {
    for tag in ["vbanana", "1.2.0", "v1.2", "v1.2.0;whoami"] {
        let mut rel = release("1.2.0");
        rel.tag_name = tag.into();
        assert!(sync::propose(&registry(), &empty(), &Mock::new(vec![rel])).is_err());
    }
    let mut rel = release("2.0.0-beta.1");
    rel.prerelease = false;
    assert!(sync::propose(&registry(), &empty(), &Mock::new(vec![rel])).is_err());
}
#[test]
fn api_and_inaccessible_private_repository_fail_without_mutation() {
    let mut api = Mock::new(vec![]);
    api.error = true;
    assert!(sync::propose(&registry(), &empty(), &api).is_err());
    assert_eq!(api.downloads.get(), 0);
    assert!(empty().applications.is_empty());
}
#[test]
fn incorrect_digest_and_missing_required_digest_refused() {
    let mut api = Mock::new(vec![release("1.2.0")]);
    api.bad_hash = true;
    assert!(sync::propose(&registry(), &empty(), &api).is_err());
    let mut rel = release("1.2.0");
    rel.assets[0].digest = None;
    assert!(sync::propose(&registry(), &empty(), &Mock::new(vec![rel])).is_err());
}
#[test]
fn unconfigured_assets_are_never_downloaded() {
    let mut rel = release("1.2.0");
    let mut other = rel.assets[0].clone();
    other.id = 999;
    other.name = "arbitrary-script.sh".into();
    other.browser_download_url = "https://evil.example.com/x".into();
    rel.assets.push(other);
    let api = Mock::new(vec![rel]);
    sync::propose(&registry(), &empty(), &api).unwrap();
    assert_eq!(api.downloads.get(), 2);
}
#[test]
fn source_spoofing_duplicate_ids_and_oversized_assets_fail() {
    for variant in 0..4 {
        let mut rel = release("1.2.0");
        match variant {
            0 => rel.html_url = "https://github.com/other/project/releases/tag/v1.2.0".into(),
            1 => rel.assets[0].browser_download_url = "https://evil.example.com/x.zip".into(),
            2 => rel.assets[1].id = rel.assets[0].id,
            _ => rel.assets[0].size = 2048,
        }
        assert!(sync::propose(&registry(), &empty(), &Mock::new(vec![rel])).is_err());
    }
}
#[test]
fn mutated_known_release_refused_instead_of_silent_republication() {
    let first = propose();
    let mut rel = release("1.2.0");
    rel.assets[0].id = 888;
    assert!(sync::propose(&registry(), &first.catalog, &Mock::new(vec![rel])).is_err());
}
#[test]
fn private_metadata_requires_consent_and_publication_requires_verified_mirror() {
    let mut r = registry();
    r.applications[0].visibility = Visibility::Private;
    assert!(sync::validate(&r).is_err());
    r.applications[0].allow_private_metadata_in_proposals = true;
    let p = sync::propose(&r, &empty(), &Mock::new(vec![release("1.2.0")])).unwrap();
    let (a, t, _) = approvals(&p);
    assert!(approval::verify(&r, &empty(), &p, &p.catalog, &a, &t, Environment::Test, 1).is_err());
    for b in &mut r.applications[0].packages {
        b.public_url = Some(format!(
            "https://downloads.example.com/{}-{{version}}.zip",
            b.package_id
        ));
    }
    let p = sync::propose(&r, &empty(), &Mock::new(vec![release("1.2.0")])).unwrap();
    assert!(p.catalog.applications[0]
        .release
        .packages
        .iter()
        .all(|p| p.url.starts_with("https://downloads.example.com/")));
    let mut bad = Mock::new(vec![release("1.2.0")]);
    bad.bad_mirror = true;
    assert!(sync::propose(&r, &empty(), &bad).is_err());
}
#[test]
fn unsigned_proposal_cannot_be_published() {
    let p = propose();
    let (mut a, t, _) = approvals(&p);
    a.approvals.clear();
    assert!(verify(&p, &a, &t).is_err());
}
#[test]
fn signed_exact_metadata_pass_offline_gate_without_enabling_installer() {
    let p = propose();
    let (a, t, _) = approvals(&p);
    verify(&p, &a, &t).unwrap();
    let policy = studio_app::installer::model::Policy {
        schema_version: 1,
        keys: t.keys,
        approvals: a.approvals,
        deployment: studio_app::installer::model::Deployment::Production,
    };
    let e = studio_app::installer::approval::authorize(
        &p.catalog.applications[0],
        &policy,
        Path::new("."),
        1,
        &Os::Macos,
        &Architecture::Aarch64,
    )
    .err()
    .unwrap();
    assert!(e.message.contains("production désactivé"));
}
#[test]
fn production_never_accepts_test_trust_or_signatures() {
    let p = propose();
    let (a, t, _) = approvals(&p);
    assert!(approval::verify(
        &registry(),
        &empty(),
        &p,
        &p.catalog,
        &a,
        &t,
        Environment::Production,
        1
    )
    .is_err());
}
#[test]
fn expired_bad_unknown_and_duplicate_signatures_refused() {
    let p = propose();
    let (good, t, _) = approvals(&p);
    for i in 0..4 {
        let mut a = good.clone();
        match i {
            0 => a.approvals[0].body.expires_unix = 1,
            1 => a.approvals[0].signature = "00".repeat(64),
            2 => a.approvals[0].key_id = "unknown".into(),
            _ => a.approvals[1] = a.approvals[0].clone(),
        }
        assert!(verify(&p, &a, &t).is_err());
    }
}
#[test]
fn registry_baseline_candidate_or_changed_list_tampering_refused() {
    let p = propose();
    let (a, t, _) = approvals(&p);
    for i in 0..4 {
        let mut altered = p.clone();
        match i {
            0 => altered.registry_sha256 = "00".repeat(32),
            1 => altered.baseline_sha256 = "00".repeat(32),
            2 => altered.catalog.applications[0].publisher = "Other publisher".into(),
            _ => altered.changed_ids.clear(),
        }
        assert!(verify(&altered, &a, &t).is_err());
    }
}
#[test]
fn schema_unknown_fields_templates_and_empty_registry_are_strict() {
    let r: Registry =
        serde_json::from_str(include_str!("../distribution/registry.example.json")).unwrap();
    let api = Mock::new(vec![]);
    assert!(sync::propose(&r, &empty(), &api)
        .unwrap()
        .changed_ids
        .is_empty());
    assert_eq!(api.queries.get(), 0);
    let mut value = serde_json::to_value(registry()).unwrap();
    value["shell"] = serde_json::json!("arbitrary");
    assert!(serde_json::from_value::<Registry>(value).is_err());
    let mut r = registry();
    r.schema_version = 99;
    assert!(sync::validate(&r).is_err());
    assert!(sync::expand("name-{cpu}", "1.0.0", "v1.0.0").is_err());
}
#[test]
fn cli_empty_sync_and_offline_request_sign_verify() {
    let temp = tempfile::tempdir().unwrap();
    let reg = temp.path().join("registry.json");
    let baseline = temp.path().join("baseline.json");
    let proposal = temp.path().join("proposal.json");
    dist::write(
        &reg,
        &Registry {
            schema_version: 1,
            applications: vec![],
        },
    )
    .unwrap();
    dist::write(&baseline, &empty()).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_studio-catalog"))
        .env_remove("STUDIO_CATALOG_GITHUB_TOKEN")
        .args(["sync"])
        .arg(&reg)
        .arg(&baseline)
        .arg(&proposal)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(dist::read::<Proposal>(&proposal)
        .unwrap()
        .changed_ids
        .is_empty());
    let p = propose();
    dist::write(&reg, &registry()).unwrap();
    dist::write(&proposal, &p).unwrap();
    let (a, t, key) = approvals(&p);
    let keyfile = temp.path().join("ephemeral-test.pk8");
    std::fs::write(&keyfile, key).unwrap();
    let body = temp.path().join("body.json");
    let signed = temp.path().join("signed.json");
    let status = Command::new(env!("CARGO_BIN_EXE_studio-catalog"))
        .arg("request")
        .arg(&reg)
        .arg(&baseline)
        .arg(&proposal)
        .args(["api-fixture-only", "fixture-mac-arm", "4000000000"])
        .arg(&body)
        .status()
        .unwrap();
    assert!(status.success());
    let status = Command::new(env!("CARGO_BIN_EXE_studio-catalog"))
        .arg("sign")
        .arg(&body)
        .arg(&keyfile)
        .arg("ephemeral-test-key")
        .arg(&signed)
        .status()
        .unwrap();
    assert!(status.success());
    let signed: studio_app::installer::model::SignedApproval = dist::read(&signed).unwrap();
    assert_eq!(signed.body.application.release.version, "1.2.0");
    verify(&p, &a, &t).unwrap();
    let candidate = temp.path().join("candidate.json");
    let approvals = temp.path().join("approvals.json");
    let trust = temp.path().join("trust.json");
    dist::write(&candidate, &p.catalog).unwrap();
    dist::write(&approvals, &a).unwrap();
    dist::write(&trust, &t).unwrap();
    let args = [
        reg.as_path(),
        baseline.as_path(),
        proposal.as_path(),
        candidate.as_path(),
        approvals.as_path(),
        trust.as_path(),
    ];
    assert!(Command::new(env!("CARGO_BIN_EXE_studio-catalog"))
        .arg("verify")
        .args(args)
        .arg("--test")
        .status()
        .unwrap()
        .success());
    let refused = Command::new(env!("CARGO_BIN_EXE_studio-catalog"))
        .arg("verify")
        .args(args)
        .output()
        .unwrap();
    assert!(!refused.status.success());
}

#[test]
fn invalid_editorial_metadata_fails_before_any_api_request() {
    for variant in 0..4 {
        let mut r = registry();
        match variant {
            0 => r.applications[0].metadata.name.clear(),
            1 => r.applications[0].metadata.platforms[0]
                .architectures
                .clear(),
            2 => {
                r.applications[0].metadata.logo = Some(Asset {
                    source: "local-file.png".into(),
                    alt: "Local".into(),
                })
            }
            _ => {
                r.applications[0].packages[0].public_url =
                    Some("https://downloads.example.com/x.zip?token=forbidden".into())
            }
        };
        let api = Mock::new(vec![]);
        assert!(sync::propose(&r, &empty(), &api).is_err());
        assert_eq!(api.queries.get(), 0);
    }
}

#[test]
fn shipped_distribution_registration_validates_without_contacting_github() {
    let r: Registry = serde_json::from_str(include_str!("../distribution/registry.json")).unwrap();
    sync::validate(&r).unwrap();
    studio_app::catalog::parse(include_bytes!("../distribution/catalog.json")).unwrap();
    let trust: Trust = serde_json::from_str(include_str!("../distribution/trust.json")).unwrap();
    assert_eq!(trust.environment, Environment::Production);
}
