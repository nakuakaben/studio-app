use super::*;
use model::{MacRule, WindowsRule};
use std::path::{Path, PathBuf};
fn mac_rule() -> MacRule {
    MacRule {
        bundle_id: "org.studio.test".into(),
        bundle_paths: vec!["Test.app".into()],
    }
}
fn bundle(root: &Path, relative: &str, id: &str, version: Option<&str>, binary: bool) -> PathBuf {
    let path = root.join(relative);
    std::fs::create_dir_all(path.join("Contents/MacOS")).unwrap();
    std::fs::write(path.join("Contents/MacOS/Test"), b"not executed").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            path.join("Contents/MacOS/Test"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    let mut dict = plist::Dictionary::new();
    for (k, v) in [
        ("CFBundleIdentifier", id),
        ("CFBundlePackageType", "APPL"),
        ("CFBundleExecutable", "Test"),
    ] {
        dict.insert(k.into(), plist::Value::String(v.into()));
    }
    if let Some(v) = version {
        dict.insert(
            "CFBundleShortVersionString".into(),
            plist::Value::String(v.into()),
        );
    }
    let value = plist::Value::Dictionary(dict);
    if binary {
        value
            .to_file_binary(path.join("Contents/Info.plist"))
            .unwrap();
    } else {
        value.to_file_xml(path.join("Contents/Info.plist")).unwrap();
    }
    path
}
fn pe(root: &Path) -> WindowsRule {
    std::fs::create_dir_all(root.join("bin")).unwrap();
    let mut bytes = vec![0; 128];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
    bytes[64..68].copy_from_slice(b"PE\0\0");
    bytes[86..88].copy_from_slice(&2u16.to_le_bytes());
    std::fs::write(root.join("bin/test.exe"), bytes).unwrap();
    WindowsRule {
        uninstall_key: "{12345678-1234-1234-1234-123456789012}".into(),
        publisher: "Studio Test".into(),
        executable: "bin/test.exe".into(),
    }
}
fn record(root: &Path, rule: &WindowsRule) -> windows::Record {
    windows::Record {
        key: rule.uninstall_key.clone(),
        publisher: rule.publisher.clone(),
        directory: root.into(),
        version: Some("1.2.3.0".into()),
        user: false,
        bits: 64,
    }
}
#[test]
fn mac_present_absent_multiple_versions_and_identity_not_name() {
    let temp = tempfile::tempdir().unwrap();
    let one = temp.path().join("one");
    let two = temp.path().join("two");
    std::fs::create_dir_all(&one).unwrap();
    std::fs::create_dir_all(&two).unwrap();
    let rule = mac_rule();
    assert!(macos::detect(&rule, std::slice::from_ref(&one))
        .installations
        .is_empty());
    bundle(&one, "Test.app", "org.wrong.id", Some("1.0.0"), false);
    assert!(macos::detect(&rule, std::slice::from_ref(&one))
        .installations
        .is_empty());
    let path = bundle(&one, "Test.app", "org.studio.test", Some("1.0.0"), false);
    bundle(&two, "Test.app", "org.studio.test", Some("2.0.0"), true);
    let before = std::fs::read(path.join("Contents/Info.plist")).unwrap();
    let report = macos::detect(&rule, &[one.clone(), two, one]);
    assert_eq!(report.installations.len(), 2);
    assert_eq!(report.installations[0].version.as_deref(), Some("1.0.0"));
    assert_eq!(report.installations[1].version.as_deref(), Some("2.0.0"));
    assert_eq!(
        before,
        std::fs::read(path.join("Contents/Info.plist")).unwrap()
    );
}
#[test]
fn mac_unknown_version_missing_executable_and_bounded_plist() {
    let root = tempfile::tempdir().unwrap();
    let path = bundle(root.path(), "Test.app", "org.studio.test", None, true);
    let rule = mac_rule();
    assert_eq!(
        macos::detect(&rule, &[root.path().into()]).installations[0].version,
        None
    );
    std::fs::remove_file(path.join("Contents/MacOS/Test")).unwrap();
    let report = macos::detect(&rule, &[root.path().into()]);
    assert!(report.installations.is_empty());
    assert!(!report.errors.is_empty());
    std::fs::write(path.join("Contents/Info.plist"), vec![b' '; 256 * 1024 + 1]).unwrap();
    assert!(macos::inspect(root.path(), "Test.app", &rule).is_err());
}
#[cfg(unix)]
#[test]
fn mac_permissions_and_symlink_escape_are_rejected() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let path = bundle(root.path(), "Test.app", "org.studio.test", None, false);
    std::fs::set_permissions(
        path.join("Contents/MacOS/Test"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(macos::inspect(root.path(), "Test.app", &mac_rule()).is_err());
    let outside = bundle(other.path(), "Test.app", "org.studio.test", None, false);
    symlink(outside, root.path().join("Escape.app")).unwrap();
    assert!(macos::inspect(root.path(), "Escape.app", &mac_rule()).is_err());
}
#[test]
fn windows_record_exact_identity_missing_path_permissions_and_multiple_views() {
    let root = tempfile::tempdir().unwrap();
    let rule = pe(root.path());
    let good = record(root.path(), &rule);
    let report = windows::detect(&rule, [Ok(good.clone()), Ok(good.clone())]);
    assert_eq!(report.installations.len(), 1);
    assert_eq!(report.installations[0].version.as_deref(), Some("1.2.3.0"));
    let mut wrong = good.clone();
    wrong.key = "Same displayed name".into();
    assert!(windows::inspect(&rule, &wrong).is_err());
    wrong = good.clone();
    wrong.publisher = "Wrong publisher".into();
    assert!(windows::inspect(&rule, &wrong).is_err());
    let second = tempfile::tempdir().unwrap();
    pe(second.path());
    let mut user = record(second.path(), &rule);
    user.user = true;
    user.bits = 32;
    assert_eq!(
        windows::detect(&rule, [Ok(user), Ok(good)])
            .installations
            .len(),
        2
    );
    let report = windows::detect(&rule, [Err("Access denied".into())]);
    assert!(report.installations.is_empty());
    assert_eq!(report.errors, vec!["Access denied"]);
    std::fs::remove_file(root.path().join("bin/test.exe")).unwrap();
    assert!(windows::inspect(&rule, &record(root.path(), &rule)).is_err());
}
#[test]
fn windows_requires_pe_and_handles_unknown_version() {
    let root = tempfile::tempdir().unwrap();
    let rule = pe(root.path());
    let mut entry = record(root.path(), &rule);
    entry.version = None;
    assert_eq!(windows::inspect(&rule, &entry).unwrap().version, None);
    std::fs::write(root.path().join("bin/test.exe"), b"echo not an exe").unwrap();
    assert!(windows::inspect(&rule, &entry).is_err());
}
#[cfg(windows)]
#[test]
fn windows_remote_and_device_paths_rejected() {
    for path in [
        r"\\server\share\app",
        r"\\?\UNC\server\share\app",
        r"\\.\pipe\app",
        r"C:relative",
        r"%LOCALAPPDATA%\app",
    ] {
        assert!(!windows::local_path(Path::new(path)));
    }
}
#[cfg(unix)]
#[test]
fn windows_executable_symlink_escape_rejected() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let rule = pe(root.path());
    pe(other.path());
    std::fs::remove_file(root.path().join("bin/test.exe")).unwrap();
    symlink(
        other.path().join("bin/test.exe"),
        root.path().join("bin/test.exe"),
    )
    .unwrap();
    assert!(windows::inspect(&rule, &record(root.path(), &rule)).is_err());
}
#[test]
fn launch_plan_has_fixed_program_no_shell_arguments_and_propagates_failure() {
    let root = tempfile::tempdir().unwrap();
    let rule = pe(root.path());
    let win = windows::inspect(&rule, &record(root.path(), &rule)).unwrap();
    assert_eq!(
        launch_with(&win, |cmd| {
            assert_eq!(cmd.get_program(), win.executable);
            assert_eq!(cmd.get_args().count(), 0);
            assert_eq!(cmd.get_current_dir(), win.executable.parent());
            Err("permission denied".into())
        }),
        Err("permission denied".into())
    );
    let mac = Installation {
        path: root.path().join("Name ; $(ignored).app"),
        executable: root.path().join("Exec"),
        version: None,
        source: Source::MacBundle,
    };
    launch_with(&mac, |cmd| {
        assert_eq!(cmd.get_program(), "/usr/bin/open");
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, vec![std::ffi::OsStr::new("-a"), mac.path.as_os_str()]);
        Ok(())
    })
    .unwrap();
}
#[test]
fn launch_revalidates_and_never_executes_a_stale_or_unmatched_target() {
    let rules = DetectionRules {
        macos: Some(mac_rule()),
        windows: None,
    };
    let i = Installation {
        path: "/missing/Test.app".into(),
        executable: "/missing/Test.app/Contents/MacOS/Test".into(),
        version: None,
        source: Source::MacBundle,
    };
    assert!(launch(&rules, &i).is_err());
}
#[test]
fn versions_do_not_confuse_available_with_installed() {
    use std::cmp::Ordering::*;
    for (a, b, expected) in [
        ("1.2.3", "1.2.3", Some(Equal)),
        ("1.2.3.0", "1.2.3", Some(Equal)),
        ("1.2", "1.3.0", Some(Less)),
        ("2.0.0", "1.0.0", Some(Greater)),
        ("1.0.0-beta.1", "1.0.0", Some(Less)),
        ("unknown", "1.0.0", None),
    ] {
        assert_eq!(model::compare_versions(a, b), expected);
    }
}
#[test]
fn detection_rules_and_schema_migration_reject_unsafe_data() {
    assert!(
        crate::catalog::parse(include_bytes!("../../tests/fixtures/native-invalid.json")).is_err()
    );
    let mut cat =
        crate::catalog::parse(include_bytes!("../../tests/fixtures/native-v2.json")).unwrap();
    cat.schema_version = 2;
    let rules = DetectionRules {
        macos: Some(mac_rule()),
        windows: None,
    };
    cat.applications[0].detection = Some(rules.clone());
    assert!(crate::catalog::parse(&serde_json::to_vec(&cat).unwrap()).is_ok());
    cat.schema_version = 1;
    assert!(crate::catalog::parse(&serde_json::to_vec(&cat).unwrap()).is_err());
    cat.schema_version = 2;
    for path in [
        "../Escape.app",
        "/Absolute.app",
        "Other\\Path.app",
        "Bad.app/../Escape.app",
    ] {
        let mut bad = rules.clone();
        bad.macos.as_mut().unwrap().bundle_paths = vec![path.into()];
        assert!(model::validate(&bad).is_err());
    }
    let root = tempfile::tempdir().unwrap();
    let win = pe(root.path());
    for path in [
        "../cmd.exe",
        "cmd.exe",
        "bin/cmd.exe",
        "bin/run.cmd",
        "C:/Run.exe",
        "bin/test.exe:stream",
    ] {
        let mut bad = win.clone();
        bad.executable = path.into();
        assert!(model::validate(&DetectionRules {
            macos: None,
            windows: Some(bad)
        })
        .is_err());
    }
    let mut duplicate = cat.applications[0].clone();
    duplicate.id = "other".into();
    cat.applications.push(duplicate);
    assert!(crate::catalog::parse(&serde_json::to_vec(&cat).unwrap()).is_err());
}
#[test]
fn worker_preserves_ids_and_detection_has_no_launch_side_effect() {
    let request = worker::Request {
        id: "explicit-id".into(),
        rules: DetectionRules {
            macos: Some(mac_rule()),
            windows: None,
        },
    };
    let rx = worker::spawn(7, vec![request], || {});
    let snapshot = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    assert_eq!(snapshot.generation, 7);
    assert_eq!(snapshot.entries[0].0.id, "explicit-id");
    assert!(snapshot.entries[0].1.installations.is_empty());
}

#[cfg(windows)]
#[test]
fn native_registry_is_read_only_and_bad_executable_launch_reports_failure() {
    use winreg::{enums::*, RegKey};
    struct Cleanup(String);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.0);
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut rule = pe(root.path());
    rule.uninstall_key = format!("StudioAppNativeTest{}", std::process::id());
    let path = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        rule.uninstall_key
    );
    let cleanup = Cleanup(path.clone());
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(&path)
        .unwrap();
    key.set_value("Publisher", &rule.publisher).unwrap();
    key.set_value("InstallLocation", &root.path().to_string_lossy().as_ref())
        .unwrap();
    key.set_value("DisplayVersion", &"1.2.3.0").unwrap();
    key.set_value("Sentinel", &"unchanged").unwrap();
    let rules = DetectionRules {
        macos: None,
        windows: Some(rule),
    };
    let report = detect(&rules);
    assert_eq!(report.installations.len(), 1);
    assert_eq!(key.get_value::<String, _>("Sentinel").unwrap(), "unchanged");
    // Minimal PE fixture passes identity detection, but is deliberately not a runnable program.
    assert!(launch(&rules, &report.installations[0]).is_err());
    // Positive native CreateProcess check: a Windows-supplied PE, no arguments or shell.
    let system = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32");
    key.set_value("InstallLocation", &system.to_string_lossy().as_ref())
        .unwrap();
    let mut positive = rules.clone();
    positive.windows.as_mut().unwrap().executable = "where.exe".into();
    let present = detect(&positive);
    assert_eq!(present.installations.len(), 1);
    launch(&positive, &present.installations[0]).unwrap();
    drop(key);
    drop(cleanup);
}
