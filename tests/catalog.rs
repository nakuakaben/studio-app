use std::{fs, path::Path, sync::Mutex};
use studio_app::catalog::{
    self, assets, github,
    loader::{self, Config, Origin, Settings, Source, Transport},
    model::GitHubSource,
    security, Error, Result,
};
const VALID: &[u8] = include_bytes!("fixtures/valid.json");
const PNG: &[u8] = include_bytes!("fixtures/sample.png");

struct Stub {
    response: Result<Vec<u8>>,
    calls: Mutex<Vec<String>>,
}
impl Stub {
    fn ok(bytes: &[u8]) -> Self {
        Self {
            response: Ok(bytes.to_vec()),
            calls: Mutex::new(Vec::new()),
        }
    }
    fn offline() -> Self {
        Self {
            response: Err(Error::network("test hors ligne")),
            calls: Mutex::new(Vec::new()),
        }
    }
}
impl Transport for Stub {
    fn get(&self, url: &str, limit: usize) -> Result<Vec<u8>> {
        self.calls.lock().unwrap().push(url.to_owned());
        let bytes = self.response.clone()?;
        if bytes.len() > limit {
            return Err(Error::invalid("Limite de réponse dépassée"));
        }
        Ok(bytes)
    }
}
fn remote(root: &Path, cache: &Path) -> Settings {
    Settings::new(
        Config {
            source: Source::Https {
                url: "https://catalog.example.com/catalog.json".into(),
            },
            allowed_asset_hosts: vec!["images.example.com".into()],
            demo_scenarios: false,
        },
        root.into(),
        cache,
    )
    .unwrap()
}
fn remote_bytes() -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(VALID).unwrap();
    v["applications"][0]["logo"]["source"] = "https://images.example.com/logo.png".into();
    v["applications"][0]["screenshots"][0]["image"]["source"] =
        "https://images.example.com/screen.png".into();
    serde_json::to_vec(&v).unwrap()
}
#[test]
fn owned_metadata_valid_manifest_and_runtime_add_remove() {
    let mut v: serde_json::Value = serde_json::from_slice(VALID).unwrap();
    let first = catalog::parse(VALID).unwrap();
    assert_eq!(first.applications[0].history.len(), 1);
    let mut added = v["applications"][0].clone();
    added["id"] = "added-without-recompile".into();
    v["applications"].as_array_mut().unwrap().push(added);
    assert_eq!(
        catalog::parse(&serde_json::to_vec(&v).unwrap())
            .unwrap()
            .applications
            .len(),
        2
    );
    v["applications"].as_array_mut().unwrap().clear();
    assert!(catalog::parse(&serde_json::to_vec(&v).unwrap())
        .unwrap()
        .applications
        .is_empty());
}
#[test]
fn rejects_fixtures_and_strict_schema_semantics() {
    assert!(catalog::parse(include_bytes!("fixtures/invalid.json")).is_err());
    assert!(catalog::parse(include_bytes!("fixtures/malformed.json")).is_err());
    let mutations: Vec<(&str, serde_json::Value)> = vec![
        ("/schema_version", 0.into()),
        ("/schema_version", 2.into()),
        ("/applications/0/id", "../escape".into()),
        ("/applications/0/release/version", "latest".into()),
        ("/applications/0/release/published", "2026-02-30".into()),
        ("/applications/0/color", serde_json::json!([256, 0, 0])),
        ("/applications/0/platforms/0/os", "plan9".into()),
        ("/applications/0/release/packages/0/sha256", "bad".into()),
        (
            "/applications/0/release/packages/0/url",
            "http://example.com/file".into(),
        ),
        (
            "/applications/0/release/packages/0/architecture",
            "riscv64".into(),
        ),
        ("/applications/0/history/0/version", "9.0.0".into()),
        ("/applications/0/name", "".into()),
    ];
    for (pointer, value) in mutations {
        let mut v: serde_json::Value = serde_json::from_slice(VALID).unwrap();
        *v.pointer_mut(pointer).unwrap() = value;
        assert!(
            catalog::parse(&serde_json::to_vec(&v).unwrap()).is_err(),
            "accepted {pointer}"
        );
    }
    let mut v: serde_json::Value = serde_json::from_slice(VALID).unwrap();
    v["applications"][0]["command"] = "anything".into();
    assert!(catalog::parse(&serde_json::to_vec(&v).unwrap()).is_err());
    let mut v: serde_json::Value = serde_json::from_slice(VALID).unwrap();
    let duplicate = v["applications"][0].clone();
    v["applications"].as_array_mut().unwrap().push(duplicate);
    assert!(catalog::parse(&serde_json::to_vec(&v).unwrap()).is_err());
    assert!(catalog::parse(&vec![b' '; catalog::MAX_CATALOG_BYTES + 1]).is_err());
}
#[test]
fn persistent_cache_offline_recovery_invalid_response_never_overwrites_valid_data() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = remote_bytes();
    let settings = remote(temp.path(), temp.path());
    assert_eq!(
        loader::load(&settings, &Stub::ok(&bytes)).unwrap().origin,
        Origin::Remote
    );
    // Recreate the reader to prove persistence, rather than an in-memory fallback.
    let settings = remote(temp.path(), temp.path());
    let cached = loader::load(&settings, &Stub::offline()).unwrap();
    assert_eq!(cached.origin, Origin::Cache);
    assert!(cached.warning.unwrap().contains("hors ligne"));
    assert_eq!(
        loader::load(&settings, &Stub::ok(b"corrupt"))
            .unwrap()
            .origin,
        Origin::Cache
    );
    assert_eq!(
        settings
            .cache
            .read("catalog.json", catalog::MAX_CATALOG_BYTES)
            .unwrap(),
        bytes
    );
    for message in [
        "Statut HTTP 404",
        "Statut HTTP 429",
        "Délai dépassé",
        "Réponse interrompue",
    ] {
        let failed = Stub {
            response: Err(Error::network(message)),
            calls: Mutex::new(Vec::new()),
        };
        let recovered = loader::load(&settings, &failed).unwrap();
        assert_eq!(recovered.origin, Origin::Cache);
        assert!(recovered.warning.unwrap().contains(message));
    }

    let other = Settings::new(
        Config {
            source: Source::Https {
                url: "https://other.example.com/catalog.json".into(),
            },
            allowed_asset_hosts: vec![],
            demo_scenarios: false,
        },
        temp.path().into(),
        temp.path(),
    )
    .unwrap();
    assert!(
        loader::load(&other, &Stub::offline()).is_err(),
        "must not reuse another source's cache"
    );
}
#[test]
fn local_reload_sees_edits_and_invalid_json_falls_back() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("config.json"),
        br#"{"source":{"kind":"local","path":"manifest.json"}}"#,
    )
    .unwrap();
    fs::write(temp.path().join("manifest.json"), VALID).unwrap();
    let settings =
        Settings::from_path(&temp.path().join("config.json"), &temp.path().join("cache")).unwrap();
    assert_eq!(
        loader::load(&settings, &Stub::offline())
            .unwrap()
            .catalog
            .applications
            .len(),
        1
    );
    fs::write(
        temp.path().join("manifest.json"),
        br#"{"schema_version":1,"name":"Empty","applications":[]}"#,
    )
    .unwrap();
    assert!(loader::load(&settings, &Stub::offline())
        .unwrap()
        .catalog
        .applications
        .is_empty());
    fs::write(temp.path().join("manifest.json"), b"bad").unwrap();
    assert_eq!(
        loader::load(&settings, &Stub::offline()).unwrap().origin,
        Origin::Cache
    );
}
#[test]
fn unavailable_corrupt_cache_and_oversized_network_do_not_claim_success() {
    let temp = tempfile::tempdir().unwrap();
    let settings = remote(temp.path(), temp.path());
    assert!(loader::load(&settings, &Stub::offline()).is_err());
    settings.cache.write("catalog.json", b"not-json").unwrap();
    assert!(loader::load(&settings, &Stub::offline()).is_err());
    assert!(loader::load(
        &settings,
        &Stub::ok(&vec![0; catalog::MAX_CATALOG_BYTES + 1])
    )
    .is_err());
}
#[test]
fn resource_loading_png_cache_and_policy() {
    let temp = tempfile::tempdir().unwrap();
    let settings = remote(temp.path(), temp.path());
    let url = "https://images.example.com/logo.png";
    let raster = assets::load(&settings, url, &Stub::ok(PNG)).unwrap();
    assert_eq!(raster.size, [16, 16]);
    assert_eq!(raster.rgba.len(), 16 * 16 * 4);
    assert_eq!(
        assets::load(&settings, url, &Stub::offline()).unwrap().size,
        [16, 16]
    );
    let http = Stub::ok(PNG);
    assert!(assets::load(&settings, "https://evil.example.com/x.png", &http).is_err());
    assert!(http.calls.lock().unwrap().is_empty());
    assert!(assets::load(&settings, "sample.png", &http).is_err());
    assert!(assets::decode(b"<svg/>").is_err());
    assert!(assets::decode(&vec![0; assets::MAX_ASSET_BYTES + 1]).is_err());
    // Valid PNG header declaring excessive dimensions must be rejected before allocation.
    let mut huge = image::RgbaImage::new(4097, 1);
    huge.put_pixel(0, 0, image::Rgba([1, 2, 3, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    huge.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    assert!(assets::decode(bytes.get_ref()).is_err());
}
#[test]
fn local_assets_are_confined_and_remote_catalog_cannot_reference_disk() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::write(root.join("assets/sample.png"), PNG).unwrap();
    let settings = Settings::new(
        Config {
            source: Source::Local {
                path: "catalog.json".into(),
            },
            allowed_asset_hosts: vec![],
            demo_scenarios: false,
        },
        root,
        temp.path(),
    )
    .unwrap();
    assert_eq!(
        assets::load(&settings, "sample.png", &Stub::offline())
            .unwrap()
            .size,
        [16, 16]
    );
    for path in [
        "../sample.png",
        "/tmp/x",
        "C:/x",
        "a\\b",
        "%2e%2e/x",
        "a//b",
        "./x",
    ] {
        assert!(settings.asset_source(path).is_err(), "{path}");
    }
    assert!(remote(temp.path(), temp.path())
        .validate_bytes(VALID)
        .is_err());
}
#[test]
fn url_and_config_security_blocks_local_network_credentials_schemes_and_unknown_fields() {
    for value in [
        "http://example.com/a",
        "file:///tmp/a",
        "data:image/png,abc",
        "https://user:pass@example.com/a",
        "https://127.0.0.1/a",
        "https://10.1.2.3/a",
        "https://[::1]/a",
        "https://[::ffff:127.0.0.1]/a",
        "https://localhost/a",
        "https://machine.local/a",
        "https://example.com:8080/a",
        "https://example.com/a#fragment",
    ] {
        assert!(security::https_url(value).is_err(), "{value}");
    }
    for ip in [
        "0.0.0.0",
        "127.0.0.1",
        "169.254.169.254",
        "100.64.0.1",
        "198.18.0.1",
        "224.0.0.1",
        "fc00::1",
        "2001:db8::1",
    ] {
        assert!(!security::public_ip(ip.parse().unwrap()));
    }
    assert!(security::https_url("https://github.com/obsproject/obs-studio").is_ok());
    assert!(serde_json::from_str::<Config>(
        r#"{"source":{"kind":"https","url":"https://example.com/","command":"no"}}"#
    )
    .is_err());
}
#[cfg(unix)]
#[test]
fn symlinks_cannot_escape_resource_root_or_poison_cache_reads() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir(&root).unwrap();
    fs::write(temp.path().join("secret"), PNG).unwrap();
    symlink(temp.path().join("secret"), root.join("escape.png")).unwrap();
    assert!(security::confined_file(&root, "escape.png").is_err());
    let cache = catalog::cache::Cache::new(temp.path(), "key");
    symlink(&root, &cache.root).unwrap();
    assert!(cache.read("escape.png", 1000).is_err());
    assert!(cache.write("catalog.json", VALID).is_err());
}
#[test]
fn github_discovery_has_no_catalog_promotion_or_download_side_effect() {
    let temp = tempfile::tempdir().unwrap();
    let settings = remote(temp.path(), temp.path());
    let bytes=br#"[{"tag_name":"v2.0.0","name":"Release","body":"notes","published_at":"2026-11-09T12:00:00Z","html_url":"https://github.com/example/app/releases/tag/v2.0.0","prerelease":false,"draft":false,"assets":[{"id":42,"name":"app.dmg","size":100,"browser_download_url":"https://github.com/example/app/releases/download/v2.0.0/app.dmg"}]}]"#;
    let http = Stub::ok(bytes);
    let source = GitHubSource {
        owner: "example".into(),
        repository: "app".into(),
    };
    let releases = github::discover(&source, &http).unwrap();
    assert_eq!(releases[0].assets[0].id, 42);
    assert_eq!(http.calls.lock().unwrap().len(), 1);
    assert!(!settings.cache.root.exists());
    assert!(github::validate_source(&GitHubSource {
        owner: "../x".into(),
        repository: "app".into()
    })
    .is_err());
    assert!(github::parse_releases(b"{}").is_err());
}
#[test]
fn all_shipped_catalogs_validate() {
    for bytes in [
        include_bytes!("../catalog/catalog.json").as_slice(),
        include_bytes!("../catalog/obs-catalog.json"),
        include_bytes!("../catalog/assets-catalog.json"),
    ] {
        assert!(catalog::parse(bytes).is_ok());
    }
}

#[test]
fn asynchronous_worker_emits_catalog_and_decoded_assets_then_completes() {
    use studio_app::catalog::worker::{self, Event};
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("assets")).unwrap();
    fs::write(temp.path().join("assets/sample.png"), PNG).unwrap();
    fs::write(temp.path().join("catalog.json"), VALID).unwrap();
    fs::write(
        temp.path().join("config.json"),
        br#"{"source":{"kind":"local","path":"catalog.json"}}"#,
    )
    .unwrap();
    let receiver = worker::spawn(
        temp.path().join("config.json"),
        temp.path().join("cache"),
        || {},
    );
    let mut catalog = false;
    let mut asset = false;
    loop {
        match receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
        {
            Event::Catalog { loaded, demo } => {
                catalog = true;
                assert!(!demo);
                assert_eq!(loaded.origin, Origin::Local);
            }
            Event::Asset { result, .. } => {
                asset = true;
                assert_eq!(result.unwrap().size, [16, 16]);
            }
            Event::Failed(error) => panic!("{error}"),
            Event::Done => break,
        }
    }
    assert!(catalog && asset);
}

#[test]
fn image_cache_retains_current_snapshot_and_invalid_image_cannot_replace_it() {
    let temp = tempfile::tempdir().unwrap();
    let settings = remote(temp.path(), temp.path());
    let first = "https://images.example.com/first.png";
    let second = "https://images.example.com/second.png";
    assets::load(&settings, first, &Stub::ok(PNG)).unwrap();
    assets::load(&settings, second, &Stub::ok(PNG)).unwrap();
    assert!(assets::load(&settings, first, &Stub::ok(b"corrupt image")).is_ok());
    settings.cache.retain_images(&[second.into()]).unwrap();
    assert!(assets::load_cached(&settings, first).is_err());
    assert!(assets::load_cached(&settings, second).is_ok());
}
#[test]
fn cache_write_failure_keeps_valid_network_result_and_reports_warning() {
    let temp = tempfile::tempdir().unwrap();
    let mut settings = remote(temp.path(), temp.path());
    let blocked = temp.path().join("not-a-directory");
    fs::write(&blocked, b"x").unwrap();
    settings.cache.root = blocked;
    let loaded = loader::load(&settings, &Stub::ok(&remote_bytes())).unwrap();
    assert_eq!(loaded.origin, Origin::Remote);
    assert!(loaded.warning.unwrap().contains("non enregistré"));
}
#[cfg(unix)]
#[test]
fn symbolic_asset_directory_is_not_an_authorized_resource_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sample.png"), PNG).unwrap();
    std::os::unix::fs::symlink(outside, root.join("assets")).unwrap();
    let settings = Settings::new(
        Config {
            source: Source::Local {
                path: "catalog.json".into(),
            },
            allowed_asset_hosts: vec![],
            demo_scenarios: false,
        },
        root,
        temp.path(),
    )
    .unwrap();
    assert!(assets::load(&settings, "sample.png", &Stub::offline()).is_err());
}
