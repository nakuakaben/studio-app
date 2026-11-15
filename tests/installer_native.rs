#![cfg(all(feature = "installer-fixtures", any(target_os = "macos", windows)))]
use studio_app::{
    catalog::model::PackageFormat,
    installer::{approval, download::Https, fixture::Fixture, worker, Cancellation, Phase},
    native,
};
fn fixture(format: PackageFormat) -> Fixture {
    Fixture::new(
        std::path::Path::new(env!("CARGO_BIN_EXE_studio-installer-probe")),
        format,
        false,
    )
    .unwrap()
}
fn install_launch_cleanup(format: PackageFormat) {
    let fixture = fixture(format);
    let plan = fixture.plan().unwrap();
    let rules = fixture.app.detection.as_ref().unwrap();
    assert!(native::detect(rules).installations.is_empty());
    let mut phases = vec![];
    let report = worker::run(
        &plan,
        &Https::new(&plan).unwrap(),
        &Cancellation::default(),
        &mut |p| phases.push(p.phase),
    )
    .unwrap();
    assert_eq!(report.installations.len(), 1);
    assert_eq!(report.installations[0].version.as_deref(), Some("0.0.1"));
    assert!(phases.contains(&Phase::Downloading));
    assert!(phases.contains(&Phase::Verifying));
    assert!(phases.contains(&Phase::Installing));
    assert!(phases.contains(&Phase::Detecting));
    native::launch(rules, &report.installations[0]).unwrap();
    let launched = fixture.root.join("probe-launched.txt");
    for _ in 0..100 {
        if launched.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(std::fs::read_to_string(launched)
        .unwrap()
        .contains("0.0.1 launched"));
    let error = worker::run(
        &plan,
        &Https::new(&plan).unwrap(),
        &Cancellation::default(),
        &mut |_| {},
    )
    .err()
    .unwrap();
    assert!(error.message.contains("déjà installée"));
    assert_eq!(native::detect(rules).installations.len(), 1);
    fixture.cleanup().unwrap();
    assert!(!fixture.root.exists());
    assert!(native::detect(rules).installations.is_empty());
}
#[test]
fn real_https_zip_install_detect_launch_cleanup() {
    install_launch_cleanup(PackageFormat::Zip);
}
#[test]
fn cancelled_download_can_be_retried_without_an_installation_left_behind() {
    let fixture = fixture(PackageFormat::Zip);
    let plan = fixture.plan().unwrap();
    let rules = fixture.app.detection.as_ref().unwrap();
    let cancellation = Cancellation::default();
    let error = worker::run(
        &plan,
        &Https::new(&plan).unwrap(),
        &cancellation,
        &mut |progress| {
            if progress.phase == Phase::Downloading {
                cancellation.cancel();
            }
        },
    )
    .unwrap_err();
    assert!(error.retryable);
    assert!(native::detect(rules).installations.is_empty());
    let report = worker::run(
        &plan,
        &Https::new(&plan).unwrap(),
        &Cancellation::default(),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(report.installations.len(), 1);
    fixture.cleanup().unwrap();
}
#[cfg(target_os = "macos")]
#[test]
fn real_readonly_dmg_install_detect_launch_cleanup() {
    install_launch_cleanup(PackageFormat::Dmg);
}
#[test]
fn unapproved_metadata_rejected_before_transfer() {
    let fixture = fixture(PackageFormat::Zip);
    let package = &fixture.app.release.packages[0];
    let mut app = fixture.app.clone();
    app.release.packages[0].url = "https://unapproved.example.com/asset.zip".into();
    assert!(approval::authorize(
        &app,
        &fixture.policy,
        fixture.workspace.path(),
        1,
        &package.os,
        &package.architecture
    )
    .is_err());
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 1);
}
#[test]
fn existing_destination_never_replaced() {
    let fixture = fixture(PackageFormat::Zip);
    let plan = fixture.plan().unwrap();
    let leaf = if cfg!(target_os = "macos") {
        "StudioInstallerFixture.app"
    } else {
        "StudioInstallerFixture"
    };
    let dest = fixture.root.join(leaf);
    std::fs::create_dir(&dest).unwrap();
    std::fs::write(dest.join("personal-data"), b"untouched").unwrap();
    assert!(worker::run(
        &plan,
        &Https::new(&plan).unwrap(),
        &Cancellation::default(),
        &mut |_| {}
    )
    .is_err());
    assert_eq!(
        std::fs::read(dest.join("personal-data")).unwrap(),
        b"untouched"
    );
}
#[test]
fn actual_tls_connection_failure_leaves_no_installation() {
    let fixture = fixture(PackageFormat::Zip);
    let mut policy = fixture.policy.clone();
    if let studio_app::installer::model::Deployment::LocalFixture { port, .. } =
        &mut policy.deployment
    {
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        *port = socket.local_addr().unwrap().port();
        drop(socket);
    }
    let package = &fixture.app.release.packages[0];
    let plan = approval::authorize(
        &fixture.app,
        &policy,
        fixture.workspace.path(),
        1,
        &package.os,
        &package.architecture,
    )
    .unwrap();
    assert!(worker::run(
        &plan,
        &Https::new(&plan).unwrap(),
        &Cancellation::default(),
        &mut |_| {}
    )
    .is_err());
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 1);
}
#[cfg(unix)]
#[test]
fn permission_denied_does_not_change_destination() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = fixture(PackageFormat::Zip);
    let plan = fixture.plan().unwrap();
    std::fs::set_permissions(&fixture.root, std::fs::Permissions::from_mode(0o555)).unwrap();
    let result = worker::run(
        &plan,
        &Https::new(&plan).unwrap(),
        &Cancellation::default(),
        &mut |_| {},
    );
    std::fs::set_permissions(&fixture.root, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(result.is_err());
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 1);
}

#[test]
fn tls_certificate_mismatch_is_rejected() {
    let fixture = fixture(PackageFormat::Zip);
    let plan = fixture.plan().unwrap();
    let other = rcgen::generate_simple_self_signed(vec!["packages.example.com".into()]).unwrap();
    std::fs::write(fixture.workspace.path().join("tls.pem"), other.cert.pem()).unwrap();
    assert!(worker::run(
        &plan,
        &Https::new(&plan).unwrap(),
        &Cancellation::default(),
        &mut |_| {}
    )
    .is_err());
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 1);
}
#[test]
fn recovery_cleanup_refuses_foreign_data_or_wrong_marker() {
    let fixture = fixture(PackageFormat::Zip);
    let policy = fixture.workspace.path().join("installer-policy.json");
    let foreign = fixture.root.join("foreign-data");
    std::fs::write(&foreign, b"preserve").unwrap();
    assert!(studio_app::installer::fixture::cleanup_policy(&policy).is_err());
    assert!(foreign.exists());
    std::fs::remove_file(foreign).unwrap();
    let marker = fixture
        .root
        .join(studio_app::installer::model::FIXTURE_MARKER);
    std::fs::write(&marker, b"wrong owner").unwrap();
    assert!(studio_app::installer::fixture::cleanup_policy(&policy).is_err());
    assert!(fixture.root.exists());
    std::fs::write(marker, &fixture.token).unwrap();
    studio_app::installer::fixture::cleanup_policy(&policy).unwrap();
    assert!(!fixture.root.exists());
}
