//! Feature-gated, ephemeral development identity. Never a production catalog entry.
#[cfg(any(target_os = "macos", windows))]
use super::platform;
use super::{approval, model::*};
use crate::{
    catalog::model::*,
    native::{
        model::{MacRule, WindowsRule},
        DetectionRules,
    },
};
use ring::{
    rand::{SecureRandom, SystemRandom},
    signature::{Ed25519KeyPair, KeyPair},
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
fn failure(e: impl std::fmt::Display) -> Failure {
    Failure::permanent(e.to_string())
}

pub struct Server {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    pub port: u16,
}
impl Server {
    pub fn start(
        bytes: Vec<u8>,
        path: String,
        certificate: rustls::pki_types::CertificateDer<'static>,
        key: rustls::pki_types::PrivateKeyDer<'static>,
        slow: bool,
    ) -> Result<Self> {
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(failure)?
        .with_no_client_auth()
        .with_single_cert(vec![certificate], key)
        .map_err(failure)?;
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                let Ok((socket, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                };
                let _ = socket.set_nonblocking(false);
                let _ = socket.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = socket.set_write_timeout(Some(Duration::from_secs(2)));
                let Ok(connection) = rustls::ServerConnection::new(Arc::new(config.clone())) else {
                    continue;
                };
                let mut stream = rustls::StreamOwned::new(connection, socket);
                let mut request = Vec::new();
                let mut byte = [0];
                while request.len() < 8192 && !request.ends_with(b"\r\n\r\n") {
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    request.push(byte[0]);
                }
                let request = String::from_utf8_lossy(&request);
                let expected = format!("GET {path} HTTP/1.1\r\n");
                if !request.starts_with(&expected) {
                    let _ = stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                    let _ = stream.flush();
                    continue;
                }
                let offset = request
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("range: bytes=")
                            .and_then(|v| v.strip_suffix('-'))
                            .and_then(|v| v.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if offset >= bytes.len() {
                    continue;
                }
                let range = if offset > 0 {
                    format!(
                        "Content-Range: bytes {offset}-{}/{}\r\n",
                        bytes.len() - 1,
                        bytes.len()
                    )
                } else {
                    String::new()
                };
                let header=format!("HTTP/1.1 {}\r\nContent-Length: {}\r\nETag: \"fixture-v1\"\r\nAccept-Ranges: bytes\r\n{range}Connection: close\r\n\r\n",if offset>0 {"206 Partial Content"}else{"200 OK"},bytes.len()-offset);
                if stream.write_all(header.as_bytes()).is_err() {
                    continue;
                }
                for chunk in bytes[offset..].chunks(16 * 1024) {
                    if stopped.load(Ordering::Relaxed)
                        || stream
                            .write_all(chunk)
                            .and_then(|_| stream.flush())
                            .is_err()
                    {
                        break;
                    }
                    if slow {
                        std::thread::sleep(Duration::from_millis(150));
                    }
                }
                stream.conn.send_close_notify();
                let _ = stream.flush();
            }
        });
        Ok(Self {
            stop,
            thread: Some(thread),
            port,
        })
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub struct Fixture {
    pub workspace: tempfile::TempDir,
    pub root: PathBuf,
    pub token: String,
    pub app: AppMetadata,
    pub policy: Policy,
    pub server: Server,
}
impl Fixture {
    pub fn new(probe: &Path, format: PackageFormat, slow: bool) -> Result<Self> {
        let workspace = tempfile::Builder::new()
            .prefix("studio-installer-source-")
            .tempdir()?;
        let random = SystemRandom::new();
        let mut nonce = [0; 8];
        random.fill(&mut nonce).map_err(failure)?;
        let token = approval::hex(&nonce);
        let (os, architecture) = super::model::platform().ok_or_else(|| {
            failure("Fixtures d’installation disponibles sur Windows et macOS uniquement")
        })?;
        #[cfg(target_os = "macos")]
        let parent = directories::BaseDirs::new()
            .ok_or_else(|| failure("Dossier utilisateur absent"))?
            .home_dir()
            .join("Applications");
        #[cfg(not(target_os = "macos"))]
        let parent = std::env::temp_dir();
        fs::create_dir_all(&parent)?;
        let root = parent.join(format!("StudioAppInstallerTest-{token}"));
        fs::create_dir(&root)?;
        fs::write(root.join(FIXTURE_MARKER), &token)?;
        let result = Self::build(
            workspace,
            root.clone(),
            token.clone(),
            probe,
            format,
            slow,
            os,
            architecture,
        );
        if result.is_err() && approval::fixture_root(&root, &token).is_ok() {
            let _ = fs::remove_dir_all(root);
        }
        result
    }
    #[allow(clippy::too_many_arguments)]
    fn build(
        workspace: tempfile::TempDir,
        root: PathBuf,
        token: String,
        probe: &Path,
        format: PackageFormat,
        slow: bool,
        os: Os,
        architecture: Architecture,
    ) -> Result<Self> {
        let source = workspace.path().join("source");
        fs::create_dir(&source)?;
        let mac = os == Os::Macos;
        let leaf = if mac {
            "StudioInstallerFixture.app"
        } else {
            "StudioInstallerFixture"
        };
        let payload = source.join(leaf);
        let mut rules = DetectionRules {
            macos: Some(MacRule {
                bundle_id: format!("local.studioapp.installer.fixture.{token}"),
                bundle_paths: vec![format!(
                    "StudioAppInstallerTest-{token}/StudioInstallerFixture.app"
                )],
            }),
            windows: Some(WindowsRule {
                uninstall_key: format!("StudioAppInstallerTest-{token}"),
                publisher: "Studio App Installer Test".into(),
                executable: "studio-installer-probe.exe".into(),
            }),
        };
        if mac {
            rules.windows = None;
        } else {
            rules.macos = None;
        }
        if mac {
            fs::create_dir_all(payload.join("Contents/MacOS"))?;
            fs::copy(probe, payload.join("Contents/MacOS/studio-installer-probe"))?;
            let mut dict = plist::Dictionary::new();
            for (key, value) in [
                (
                    "CFBundleIdentifier",
                    rules.macos.as_ref().unwrap().bundle_id.as_str(),
                ),
                ("CFBundlePackageType", "APPL"),
                ("CFBundleExecutable", "studio-installer-probe"),
                ("CFBundleShortVersionString", "0.0.1"),
                ("CFBundleVersion", "1"),
            ] {
                dict.insert(key.into(), value.into());
            }
            plist::Value::Dictionary(dict)
                .to_file_xml(payload.join("Contents/Info.plist"))
                .map_err(failure)?;
            #[cfg(target_os = "macos")]
            {
                let output = platform::command(
                    "/usr/bin/codesign",
                    &[
                        "--force".as_ref(),
                        "--sign".as_ref(),
                        "-".as_ref(),
                        payload.as_os_str(),
                    ],
                    30,
                )?;
                if !output.status.success() {
                    return Err(failure(String::from_utf8_lossy(&output.stderr)));
                }
            }
        } else {
            fs::create_dir(&payload)?;
            fs::copy(probe, payload.join("studio-installer-probe.exe"))?;
        }
        let extension = if format == PackageFormat::Dmg {
            "dmg"
        } else {
            "zip"
        };
        let package_path = workspace.path().join(format!("package.{extension}"));
        match format {
            PackageFormat::Zip => write_zip(&source, &payload, &package_path)?,
            #[cfg(target_os = "macos")]
            PackageFormat::Dmg => {
                let output = platform::command(
                    "/usr/bin/hdiutil",
                    &[
                        "create".as_ref(),
                        "-srcfolder".as_ref(),
                        source.as_os_str(),
                        "-format".as_ref(),
                        "UDZO".as_ref(),
                        "-volname".as_ref(),
                        "StudioInstallerTest".as_ref(),
                        package_path.as_os_str(),
                    ],
                    60,
                )?;
                if !output.status.success() {
                    return Err(failure(String::from_utf8_lossy(&output.stderr)));
                }
            }
            _ => return Err(failure("Format de test indisponible sur ce poste")),
        }
        let bytes = fs::read(&package_path)?;
        let certificate = rcgen::generate_simple_self_signed(vec!["packages.example.com".into()])
            .map_err(failure)?;
        fs::write(workspace.path().join("tls.pem"), certificate.cert.pem())?;
        let id = format!("studio-installer-fixture-{token}");
        let path = format!("/{id}.{extension}");
        let server = Server::start(
            bytes.clone(),
            path.clone(),
            certificate.cert.der().clone(),
            rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der())
                .into(),
            slow,
        )?;
        let app=AppMetadata{id:id.clone(),name:"Studio Installer Test".into(),short_description:"Application minimale de validation native".into(),description:"Package de test isolé, version 0.0.1. Aucun logiciel personnel modifié. L’ouverture crée un témoin dans le dossier de test.".into(),category:"Validation".into(),color:[163,110,235],monogram:"St".into(),publisher:"Studio App Installer Test".into(),fictional:false,logo:None,screenshots:vec![],features:vec![],release:Release{version:"0.0.1".into(),published:"2026-11-09".into(),notes:vec!["Validation du téléchargement, de l’installation et du lancement.".into()],packages:vec![Package{id:"fixture-package".into(),os:os.clone(),architecture:architecture.clone(),format,url:format!("https://packages.example.com{path}"),size_bytes:bytes.len() as u64,sha256:approval::hex(&Sha256::digest(&bytes)),github_asset_id:None}]},history:vec![],links:Links::default(),platforms:vec![Platform{os,architectures:vec![architecture]}],github:None,detection:Some(rules)};
        let body = ApprovalBody {
            schema_version: 1,
            application: app.clone(),
            package_id: "fixture-package".into(),
            recipe: if mac {
                Recipe::MacBundle {
                    bundle_name: leaf.into(),
                    team_id: None,
                }
            } else {
                Recipe::WindowsPortable {
                    directory_name: leaf.into(),
                    authenticode_thumbprint: None,
                }
            },
            expires_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(failure)?
                .as_secs()
                + 3600,
        };
        let key = Ed25519KeyPair::from_pkcs8(
            Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
                .map_err(failure)?
                .as_ref(),
        )
        .map_err(failure)?;
        let signature = approval::hex(key.sign(&approval::signing_message(&body)?).as_ref());
        let policy = Policy {
            schema_version: 1,
            keys: vec![TrustedKey {
                id: "ephemeral-fixture".into(),
                public_key: approval::hex(key.public_key().as_ref()),
            }],
            approvals: vec![SignedApproval {
                key_id: "ephemeral-fixture".into(),
                body,
                signature,
            }],
            deployment: Deployment::LocalFixture {
                root: root.clone(),
                token: token.clone(),
                tls_certificate: "tls.pem".into(),
                port: server.port,
            },
        };
        fs::write(
            workspace.path().join("installer-policy.json"),
            serde_json::to_vec_pretty(&policy)?,
        )?;
        fs::write(
            workspace.path().join("catalog.json"),
            serde_json::to_vec_pretty(&Catalog {
                schema_version: 2,
                name: "Studio App · test natif isolé".into(),
                applications: vec![app.clone()],
            })?,
        )?;
        fs::write(workspace.path().join("config.json"),br#"{"source":{"kind":"local","path":"catalog.json"},"allowed_asset_hosts":[],"demo_scenarios":false}"#)?;
        Ok(Self {
            workspace,
            root,
            token,
            app,
            policy,
            server,
        })
    }
    pub fn plan(&self) -> Result<Plan> {
        let package = &self.app.release.packages[0];
        approval::authorize(
            &self.app,
            &self.policy,
            self.workspace.path(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(failure)?
                .as_secs(),
            &package.os,
            &package.architecture,
        )
    }
    /// Test-only cleanup, confined to the nonce-marked directory and owned HKCU key.
    pub fn cleanup(&self) -> Result<()> {
        let root = approval::fixture_root(&self.root, &self.token)?;
        #[cfg(windows)]
        {
            let package = &self.app.release.packages[0];
            let plan = approval::authorize(
                &self.app,
                &self.policy,
                self.workspace.path(),
                1,
                &package.os,
                &package.architecture,
            )?;
            platform::rollback_registry(&plan, &root.join("StudioInstallerFixture"))?;
        }
        fs::remove_dir_all(root)?;
        Ok(())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.root.exists() {
            if let Err(e) = self.cleanup() {
                eprintln!(
                    "Nettoyage du test incomplet : {e} ({})",
                    self.root.display()
                );
            }
        }
    }
}
fn write_zip(source: &Path, payload: &Path, destination: &Path) -> Result<()> {
    fn add(zip: &mut zip::ZipWriter<fs::File>, source: &Path, path: &Path) -> Result<()> {
        let relative = path
            .strip_prefix(source)
            .map_err(failure)?
            .to_string_lossy()
            .replace('\\', "/");
        let meta = fs::symlink_metadata(path)?;
        if meta.is_dir() {
            zip.add_directory(
                format!("{relative}/"),
                zip::write::SimpleFileOptions::default().unix_permissions(0o755),
            )
            .map_err(failure)?;
            for entry in fs::read_dir(path)? {
                add(zip, source, &entry?.path())?;
            }
        } else {
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                meta.permissions().mode()
            };
            #[cfg(not(unix))]
            let mode = 0o644;
            zip.start_file(
                relative,
                zip::write::SimpleFileOptions::default().unix_permissions(mode),
            )
            .map_err(failure)?;
            std::io::copy(&mut fs::File::open(path)?, zip)?;
        }
        Ok(())
    }
    let mut zip = zip::ZipWriter::new(fs::File::create(destination)?);
    add(&mut zip, source, payload)?;
    zip.finish().map_err(failure)?;
    Ok(())
}

/// Recovery after an interrupted test process, not an application uninstaller.
pub fn cleanup_policy(path: &Path) -> Result<()> {
    let path = path.canonicalize()?;
    let bytes = crate::catalog::cache::read_bounded(&path, 2 * 1024 * 1024).map_err(failure)?;
    let policy: Policy = serde_json::from_slice(&bytes)?;
    if policy.approvals.len() != 1 {
        return Err(failure("Une seule approbation de test attendue"));
    }
    let app = &policy.approvals[0].body.application;
    let package = app
        .release
        .packages
        .first()
        .ok_or_else(|| failure("Package de test absent"))?;
    let plan = approval::authorize(
        app,
        &policy,
        path.parent().ok_or_else(|| failure("Racine absente"))?,
        1,
        &package.os,
        &package.architecture,
    )?;
    let Deployment::LocalFixture { root, token, .. } = &plan.deployment else {
        return Err(failure("Nettoyage réservé aux fixtures"));
    };
    let root = approval::fixture_root(root, token)?;
    let leaf = match &plan.recipe {
        Recipe::MacBundle { bundle_name, .. } => bundle_name,
        Recipe::WindowsPortable { directory_name, .. } => directory_name,
        Recipe::PersonalHmpDmg | Recipe::PersonalHmpNsis => {
            return Err(Failure::permanent("Recette personnelle hors fixture"));
        }
    };
    for entry in fs::read_dir(&root)? {
        let name = entry?.file_name();
        if name != leaf.as_str() && name != FIXTURE_MARKER && name != "probe-launched.txt" {
            return Err(failure(
                "Fichier étranger au test : nettoyage automatique refusé",
            ));
        }
    }
    #[cfg(windows)]
    platform::rollback_registry(&plan, &root.join(leaf))?;
    fs::remove_dir_all(root)?;
    Ok(())
}
