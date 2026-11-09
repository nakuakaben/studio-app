use super::{
    approval, archive,
    download::{self, Response, Transport},
    model::*,
    platform, worker,
};
use crate::{catalog::model::*, native};
use ring::signature::{Ed25519KeyPair, KeyPair};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::VecDeque,
    io::{Cursor, Write},
};
fn app() -> AppMetadata {
    AppMetadata {
        id: "studio-installer-test".into(),
        name: "Installer test".into(),
        short_description: "Test".into(),
        description: "Test isolated installation".into(),
        category: "Test".into(),
        color: [100, 100, 200],
        monogram: "St".into(),
        publisher: "Studio Test".into(),
        fictional: false,
        logo: None,
        screenshots: vec![],
        features: vec![],
        release: Release {
            version: "0.0.1".into(),
            published: "2026-11-09".into(),
            notes: vec![],
            packages: vec![],
        },
        history: vec![],
        links: Links::default(),
        platforms: vec![Platform {
            os: Os::Macos,
            architectures: vec![Architecture::Aarch64],
        }],
        github: None,
        detection: Some(native::DetectionRules {
            macos: Some(native::model::MacRule {
                bundle_id: "local.studio.test".into(),
                bundle_paths: vec!["Test.app".into()],
            }),
            windows: None,
        }),
    }
}
fn plan(bytes: &[u8]) -> Plan {
    let mut app = app();
    let package = Package {
        id: "approved".into(),
        os: Os::Macos,
        architecture: Architecture::Aarch64,
        format: PackageFormat::Zip,
        url: "https://packages.example.com/test.zip".into(),
        size_bytes: bytes.len() as u64,
        sha256: approval::hex(&Sha256::digest(bytes)),
        github_asset_id: None,
    };
    app.release.packages.push(package.clone());
    Plan {
        app,
        package,
        recipe: Recipe::MacBundle {
            bundle_name: "Test.app".into(),
            team_id: None,
        },
        deployment: Deployment::Production,
        policy_root: "/unused".into(),
        expires_unix: u64::MAX,
    }
}
fn response(bytes: &[u8]) -> Response {
    Response {
        status: 200,
        length: Some(bytes.len() as u64),
        etag: Some("\"stable\"".into()),
        ranges: true,
        content_range: None,
        encoded: false,
        body: Box::new(Cursor::new(bytes.to_vec())),
    }
}
struct Mock {
    responses: RefCell<VecDeque<Result<Response>>>,
    requests: RefCell<Vec<(u64, Option<String>)>>,
}
impl Mock {
    fn new(responses: Vec<Result<Response>>) -> Self {
        Self {
            responses: RefCell::new(responses.into()),
            requests: RefCell::new(vec![]),
        }
    }
}
impl Transport for Mock {
    fn open(&self, offset: u64, etag: Option<&str>) -> Result<Response> {
        self.requests
            .borrow_mut()
            .push((offset, etag.map(str::to_owned)));
        self.responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Err(Failure::retry("offline")))
    }
}
fn transfer(plan: &Plan, http: &Mock) -> Result<download::VerifiedDownload> {
    download::download(plan, http, &Cancellation::default(), &mut |_| {})
}
fn policy(plan: &Plan) -> Policy {
    let key = Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap();
    let body = ApprovalBody {
        schema_version: 1,
        application: plan.app.clone(),
        package_id: plan.package.id.clone(),
        recipe: plan.recipe.clone(),
        expires_unix: 500,
    };
    let signature = approval::hex(
        key.sign(&approval::signing_message(&body).unwrap())
            .as_ref(),
    );
    Policy {
        schema_version: 1,
        keys: vec![TrustedKey {
            id: "trusted".into(),
            public_key: approval::hex(key.public_key().as_ref()),
        }],
        approvals: vec![SignedApproval {
            key_id: "trusted".into(),
            body,
            signature,
        }],
        deployment: Deployment::Production,
    }
}
fn auth(plan: &Plan, policy: &Policy) -> Result<Plan> {
    approval::authorize(
        &plan.app,
        policy,
        std::path::Path::new("/unused"),
        100,
        &Os::Macos,
        &Architecture::Aarch64,
    )
}
#[test]
fn selection_only_current_approved_os_architecture_and_format() {
    let mut p = plan(b"abc");
    assert!(select(&p.app, &Os::Macos, &Architecture::Aarch64).is_ok());
    assert!(select(&p.app, &Os::Windows, &Architecture::Aarch64).is_err());
    assert!(select(&p.app, &Os::Macos, &Architecture::X86_64).is_err());
    p.app.history.push(p.app.release.clone());
    p.app.release.packages.clear();
    assert!(select(&p.app, &Os::Macos, &Architecture::Aarch64).is_err());
    for format in [PackageFormat::Pkg, PackageFormat::Exe, PackageFormat::Msi] {
        p.app.release.packages = vec![p.package.clone()];
        p.app.release.packages[0].format = format;
        assert!(select(&p.app, &Os::Macos, &Architecture::Aarch64).is_err());
    }
}
#[test]
fn selection_rejects_ambiguity_fiction_and_excess() {
    let mut p = plan(b"abc");
    p.app.release.packages.push(p.package.clone());
    assert!(select(&p.app, &Os::Macos, &Architecture::Aarch64).is_err());
    p.app.release.packages.pop();
    p.app.fictional = true;
    assert!(select(&p.app, &Os::Macos, &Architecture::Aarch64).is_err());
    p.app.fictional = false;
    p.app.release.packages[0].size_bytes = 3 * 1024 * 1024 * 1024;
    assert!(select(&p.app, &Os::Macos, &Architecture::Aarch64).is_err());
}
#[test]
fn production_disabled_even_with_valid_signature() {
    let p = plan(b"abc");
    let e = auth(&p, &policy(&p)).err().unwrap();
    assert!(e.message.contains("production désactivé"));
}
#[test]
fn approval_binds_every_metadata_field_and_package() {
    let mut p = plan(b"abc");
    let policy = policy(&p);
    p.app.release.packages[0].url = "https://evil.example.com/x.zip".into();
    assert!(auth(&p, &policy).is_err());
    p.app = policy.approvals[0].body.application.clone();
    p.app
        .detection
        .as_mut()
        .unwrap()
        .macos
        .as_mut()
        .unwrap()
        .bundle_id = "local.evil.identity".into();
    assert!(auth(&p, &policy).is_err());
}
#[test]
fn approval_rejects_unknown_key_expiry_forgery_duplicates_and_schema() {
    let p = plan(b"abc");
    let good = policy(&p);
    for changed in 0..6 {
        let mut policy = good.clone();
        match changed {
            0 => policy.keys.clear(),
            1 => policy.approvals[0].body.expires_unix = 99,
            2 => policy.approvals[0].signature = "00".repeat(64),
            3 => policy.approvals.push(policy.approvals[0].clone()),
            4 => policy.schema_version = 99,
            _ => policy.approvals.clear(),
        }
        assert!(auth(&p, &policy).is_err());
    }
}
#[test]
fn policy_json_is_strict() {
    let p = policy(&plan(b"abc"));
    let mut json = serde_json::to_value(p).unwrap();
    json["command"] = serde_json::json!("arbitrary");
    assert!(serde_json::from_value::<Policy>(json).is_err());
}
#[test]
fn download_verified_and_temporary_file_removed() {
    let p = plan(b"abc");
    let file = transfer(&p, &Mock::new(vec![Ok(response(b"abc"))])).unwrap();
    let path = file.path().to_owned();
    assert_eq!(std::fs::read(&path).unwrap(), b"abc");
    drop(file);
    assert!(!path.exists());
}
#[test]
fn resume_requires_strong_etag_and_exact_range() {
    let p = plan(b"abcdef");
    let mut first = response(b"abc");
    first.length = Some(6);
    let mut second = response(b"def");
    second.status = 206;
    second.content_range = Some("bytes 3-5/6".into());
    let http = Mock::new(vec![Ok(first), Ok(second)]);
    assert!(transfer(&p, &http).is_ok());
    assert_eq!(
        http.requests.borrow().as_slice(),
        &[(0, None), (3, Some("\"stable\"".into()))]
    );
}
#[test]
fn resume_refuses_changed_server_identity_or_representation() {
    for variant in 0..5 {
        let p = plan(b"abcdef");
        let mut first = response(b"abc");
        first.length = Some(6);
        let mut second = response(b"def");
        second.status = 206;
        second.content_range = Some("bytes 3-5/6".into());
        match variant {
            0 => second.etag = Some("\"changed\"".into()),
            1 => second.status = 200,
            2 => second.content_range = Some("bytes 2-5/6".into()),
            3 => second.length = Some(4),
            _ => second.encoded = true,
        }
        assert!(transfer(&p, &Mock::new(vec![Ok(first), Ok(second)])).is_err());
    }
}
#[test]
fn incomplete_without_resume_is_retryable() {
    for etag in [None, Some("W/\"weak\"".into())] {
        let p = plan(b"abcdef");
        let mut first = response(b"abc");
        first.length = Some(6);
        first.etag = etag;
        let http = Mock::new(vec![Ok(first)]);
        assert!(transfer(&p, &http).err().unwrap().retryable);
        assert_eq!(http.requests.borrow().len(), 1);
    }
}
#[test]
fn network_failure_and_unsafe_response_do_not_install() {
    let p = plan(b"abc");
    let offline = Mock::new(vec![]);
    assert!(transfer(&p, &offline).is_err());
    assert_eq!(offline.requests.borrow().len(), 3);
    for variant in 0..4 {
        let mut bad = response(b"abc");
        match variant {
            0 => bad.status = 302,
            1 => bad.length = Some(999),
            2 => bad.encoded = true,
            _ => bad.body = Box::new(Cursor::new(b"abcdef".to_vec())),
        }
        assert!(transfer(&p, &Mock::new(vec![Ok(bad)])).is_err());
    }
}
#[test]
fn hash_mismatch_is_permanent() {
    let p = plan(b"abc");
    assert!(
        !transfer(&p, &Mock::new(vec![Ok(response(b"xyz"))]))
            .err()
            .unwrap()
            .retryable
    );
}
#[test]
fn cancellation_before_network_and_during_download() {
    let p = plan(b"abc");
    let cancel = Cancellation::default();
    cancel.cancel();
    let http = Mock::new(vec![]);
    assert!(download::download(&p, &http, &cancel, &mut |_| {}).is_err());
    assert!(http.requests.borrow().is_empty());
    let cancel = Cancellation::default();
    assert!(download::download(
        &p,
        &Mock::new(vec![Ok(response(b"abc"))]),
        &cancel,
        &mut |_| cancel.cancel()
    )
    .is_err());
}
#[test]
fn malicious_archive_paths_rejected() {
    for name in [
        "../escape",
        "Test.app/../../escape",
        "/Test.app/x",
        "Test.app/C:/x",
        r"Test.app\x",
        "Other.app/x",
        "Test.app/NUL.txt",
        "Test.app/COM1",
        "Test.app/x.",
        "Test.app/x ",
    ] {
        assert!(archive::safe_entry(name, "Test.app").is_err(), "{name}");
    }
    assert!(archive::safe_entry("Test.app/Contents/MacOS/probe", "Test.app").is_ok());
}
fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(vec![]));
    for (name, data) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
#[test]
fn corrupt_and_case_duplicate_archives_rejected() {
    for bytes in [
        b"corrupted".to_vec(),
        zip(&[("Test.app/a", b"a"), ("Test.app/A", b"b")]),
        zip(&[("../escape", b"x")]),
    ] {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), bytes).unwrap();
        let dest = tempfile::tempdir().unwrap();
        assert!(archive::extract_zip(
            file.path(),
            dest.path(),
            "Test.app",
            &Cancellation::default()
        )
        .is_err());
    }
}
#[test]
fn symlink_archive_rejected() {
    let mut zip = zip::ZipWriter::new(Cursor::new(vec![]));
    zip.add_symlink(
        "Test.app/link",
        "/etc/passwd",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), bytes).unwrap();
    let dest = tempfile::tempdir().unwrap();
    assert!(archive::extract_zip(
        file.path(),
        dest.path(),
        "Test.app",
        &Cancellation::default()
    )
    .is_err());
}
#[test]
fn invalid_package_never_reaches_native_installation() {
    let p = plan(b"not a ZIP");
    let http = Mock::new(vec![Ok(response(b"not a ZIP"))]);
    assert!(worker::run(&p, &http, &Cancellation::default(), &mut |_| {}).is_err());
}
#[test]
fn verified_temp_removed_after_preparation_failure() {
    let p = plan(b"badzip");
    let file = transfer(&p, &Mock::new(vec![Ok(response(b"badzip"))])).unwrap();
    let path = file.path().to_owned();
    let stage = tempfile::tempdir().unwrap();
    assert!(platform::prepare(&p, &file, stage.path(), &Cancellation::default()).is_err());
    drop(file);
    assert!(!path.exists());
}

#[test]
fn interrupted_io_resumes_but_never_executes_partial_download() {
    struct Broken {
        cursor: Cursor<Vec<u8>>,
    }
    impl std::io::Read for Broken {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.cursor.position() >= 3 {
                Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "interrupted",
                ))
            } else {
                std::io::Read::read(&mut self.cursor, buffer)
            }
        }
    }
    let p = plan(b"abcdef");
    let mut first = response(b"abc");
    first.length = Some(6);
    first.body = Box::new(Broken {
        cursor: Cursor::new(b"abc".to_vec()),
    });
    let mut next = response(b"def");
    next.status = 206;
    next.content_range = Some("bytes 3-5/6".into());
    assert!(transfer(&p, &Mock::new(vec![Ok(first), Ok(next)])).is_ok());
}
#[test]
fn incomplete_installation_marker_blocks_native_detection() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("Test.app");
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join(PENDING_MARKER), "pending").unwrap();
    let error = native::macos::inspect(
        dir.path(),
        "Test.app",
        app().detection.unwrap().macos.as_ref().unwrap(),
    )
    .unwrap_err();
    assert!(error.contains("incomplète"));
}
#[test]
fn oversized_central_directory_rejected_before_allocation() {
    let mut bytes = zip(&[("Test.app/a", b"a")]);
    let end = bytes.len() - 22;
    bytes[end + 10..end + 12].copy_from_slice(&10001u16.to_le_bytes());
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), bytes).unwrap();
    let stage = tempfile::tempdir().unwrap();
    assert!(archive::extract_zip(
        file.path(),
        stage.path(),
        "Test.app",
        &Cancellation::default()
    )
    .is_err());
    assert_eq!(std::fs::read_dir(stage.path()).unwrap().count(), 0);
}
