use super::{approval, model::*};
use crate::catalog::security;
use reqwest::header::{
    ACCEPT_ENCODING, ACCEPT_RANGES, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE, ETAG,
    IF_RANGE, RANGE,
};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    time::{Duration, Instant},
};

pub struct Response {
    pub status: u16,
    pub length: Option<u64>,
    pub etag: Option<String>,
    pub ranges: bool,
    pub content_range: Option<String>,
    pub encoded: bool,
    pub body: Box<dyn Read + Send>,
}
pub trait Transport {
    fn open(&self, offset: u64, etag: Option<&str>) -> Result<Response>;
}
pub struct Https {
    client: reqwest::blocking::Client,
    url: url::Url,
}
impl Https {
    pub fn new(plan: &Plan) -> Result<Self> {
        let url = security::https_url(&plan.package.url)
            .map_err(|e| Failure::permanent(e.to_string()))?;
        let host = url
            .host_str()
            .ok_or_else(|| Failure::permanent("Hôte HTTPS absent"))?;
        let mut builder = reqwest::blocking::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(10))
            .user_agent("Studio-App approved-package-installer/0.1");
        match &plan.deployment {
            Deployment::Production => {
                let addresses = security::public_addresses(&url)
                    .map_err(|e| Failure::permanent(e.to_string()))?;
                builder = builder.resolve_to_addrs(host, &addresses);
            }
            Deployment::LocalFixture {
                tls_certificate,
                port,
                ..
            } => {
                if !cfg!(feature = "installer-fixtures")
                    || host != "packages.example.com"
                    || url.path()
                        != format!(
                            "/{}.{}",
                            plan.app.id,
                            if plan.package.format == crate::catalog::model::PackageFormat::Dmg {
                                "dmg"
                            } else {
                                "zip"
                            }
                        )
                    || url.query().is_some()
                {
                    return Err(Failure::permanent("Source du test isolé incorrecte"));
                }
                let cert = security::confined_file(&plan.policy_root, tls_certificate)
                    .map_err(|e| Failure::permanent(e.to_string()))?;
                let bytes = crate::catalog::cache::read_bounded(&cert, 16 * 1024)
                    .map_err(|e| Failure::permanent(e.to_string()))?;
                let certificate = reqwest::Certificate::from_pem(&bytes)
                    .map_err(|_| Failure::permanent("Certificat TLS du test invalide"))?;
                // Explicit local test CA + loopback mapping, never accept invalid TLS or names.
                builder = builder
                    .add_root_certificate(certificate)
                    .resolve(host, std::net::SocketAddr::from(([127, 0, 0, 1], *port)));
            }
        }
        Ok(Self {
            client: builder
                .build()
                .map_err(|_| Failure::retry("Client HTTPS indisponible"))?,
            url,
        })
    }
}
impl Transport for Https {
    fn open(&self, offset: u64, etag: Option<&str>) -> Result<Response> {
        let mut request = self
            .client
            .get(self.url.clone())
            .header(ACCEPT_ENCODING, "identity");
        if offset > 0 {
            request = request.header(RANGE, format!("bytes={offset}-")).header(
                IF_RANGE,
                etag.ok_or_else(|| Failure::permanent("Reprise sans ETag fort interdite"))?,
            );
        }
        let response = request.send().map_err(|_| {
            Failure::retry(
                "Connexion HTTPS échouée ou délai dépassé : vérifiez le serveur et son certificat.",
            )
        })?;
        let headers = response.headers();
        let get = |key| {
            headers
                .get(key)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let data = Response {
            status: response.status().as_u16(),
            length: get(CONTENT_LENGTH).and_then(|v| v.parse().ok()),
            etag: get(ETAG),
            ranges: get(ACCEPT_RANGES).is_some_and(|s| s.eq_ignore_ascii_case("bytes")),
            content_range: get(CONTENT_RANGE),
            encoded: get(CONTENT_ENCODING).is_some_and(|s| !s.eq_ignore_ascii_case("identity")),
            body: Box::new(response),
        };
        Ok(data)
    }
}
pub struct VerifiedDownload {
    file: tempfile::NamedTempFile,
}
impl VerifiedDownload {
    pub(crate) fn path(&self) -> &std::path::Path {
        self.file.path()
    }
}
fn strong(etag: Option<String>) -> Option<String> {
    etag.filter(|s| {
        s.len() >= 2
            && s.len() <= 256
            && s.starts_with('"')
            && s.ends_with('"')
            && s.is_ascii()
            && !s.chars().any(char::is_control)
    })
}
fn range_matches(value: Option<&str>, start: u64, total: u64) -> bool {
    let Some(value) = value.and_then(|s| s.strip_prefix("bytes ")) else {
        return false;
    };
    let Some((range, size)) = value.split_once('/') else {
        return false;
    };
    let Some((a, b)) = range.split_once('-') else {
        return false;
    };
    a.parse::<u64>().ok() == Some(start)
        && b.parse::<u64>().ok() == total.checked_sub(1)
        && size.parse::<u64>().ok() == Some(total)
}
pub fn download(
    plan: &Plan,
    http: &dyn Transport,
    cancel: &Cancellation,
    emit: &mut impl FnMut(Progress),
) -> Result<VerifiedDownload> {
    let mut file = tempfile::Builder::new()
        .prefix("studio-approved-package-")
        .tempfile()?;
    let mut offset = 0;
    let total = plan.package.size_bytes;
    let mut etag = None;
    let mut resumable = false;
    let began = Instant::now();
    let mut last = Instant::now() - Duration::from_secs(1);
    for attempt in 0..3 {
        cancel.check()?;
        let mut response = match http.open(offset, etag.as_deref()) {
            Ok(r) => r,
            Err(e) if e.retryable && attempt < 2 && offset == 0 => continue,
            Err(e) => return Err(e),
        };
        if response.encoded || response.length.is_some_and(|n| n != total - offset) {
            return Err(Failure::permanent(
                "Taille/encodage HTTP incohérent avec le package approuvé",
            ));
        }
        if offset == 0 {
            if response.status != 200 {
                return Err(Failure::retry(format!(
                    "Le serveur a refusé le téléchargement (HTTP {}).",
                    response.status
                )));
            }
            etag = strong(response.etag);
            resumable = response.ranges && etag.is_some();
        } else if response.status != 206
            || response.etag != etag
            || !range_matches(response.content_range.as_deref(), offset, total)
        {
            return Err(Failure::permanent("Reprise refusée : ETag, Content-Range ou taille modifiés. Le fichier partiel est supprimé."));
        }
        let mut interrupted = false;
        let mut buffer = [0; 64 * 1024];
        loop {
            cancel.check()?;
            if began.elapsed() > Duration::from_secs(30 * 60) {
                return Err(Failure::retry("Durée maximale de téléchargement dépassée"));
            }
            let count = match response.body.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => {
                    interrupted = true;
                    break;
                }
            };
            offset += count as u64;
            if offset > total {
                return Err(Failure::permanent(
                    "Le serveur a envoyé plus d’octets que la taille approuvée",
                ));
            }
            file.write_all(&buffer[..count])?;
            if last.elapsed() >= Duration::from_millis(100) {
                emit(Progress {
                    phase: Phase::Downloading,
                    bytes: offset,
                    total,
                    bytes_per_second: Some(
                        (offset as f64 / began.elapsed().as_secs_f64().max(0.001)) as u64,
                    ),
                    cancellable: true,
                });
                last = Instant::now();
            }
        }
        if offset == total && !interrupted {
            break;
        }
        if attempt == 2 || !resumable || offset == 0 {
            return Err(Failure::retry("Transfert interrompu. Aucun fichier incomplet ne sera installé ; réessayez le téléchargement."));
        }
    }
    if offset != total {
        return Err(Failure::retry("Téléchargement incomplet"));
    }
    emit(Progress {
        phase: Phase::Downloading,
        bytes: offset,
        total,
        bytes_per_second: None,
        cancellable: true,
    });
    emit(Progress::phase(Phase::Verifying));
    cancel.check()?;
    file.flush()?;
    file.as_file().sync_all()?;
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        cancel.check()?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    if !approval::hex(&hash.finalize()).eq_ignore_ascii_case(&plan.package.sha256) {
        return Err(Failure::permanent("Empreinte SHA-256 incorrecte. Package rejeté et supprimé ; aucune installation exécutée."));
    }
    Ok(VerifiedDownload { file })
}
