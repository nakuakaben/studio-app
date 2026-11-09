//! Auth stays on api.github.com. CDN and mirror requests never receive the token.
use super::{model::*, sync::Api, Error, Result};
use crate::catalog::{model::GitHubSource, security, MAX_CATALOG_BYTES};
use reqwest::{
    blocking::{Client, Response},
    header::{ACCEPT, ACCEPT_ENCODING, AUTHORIZATION, CONTENT_ENCODING, CONTENT_LENGTH, LOCATION},
};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    time::{Duration, Instant},
};
pub struct Github {
    token: Option<String>,
}
impl Github {
    pub fn new(token: Option<String>) -> Result<Self> {
        if token.as_ref().is_some_and(|s| {
            s.is_empty() || s.len() > 8192 || !s.is_ascii() || s.chars().any(char::is_control)
        }) {
            return Err(Error::invalid("Token GitHub mal formé (valeur masquée)"));
        }
        Ok(Self { token })
    }
    fn request(&self, url: &str, api: bool, binary: bool) -> Result<Response> {
        let parsed = security::https_url(url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| Error::invalid("Hôte absent"))?;
        if api && host != "api.github.com" {
            return Err(Error::invalid("Authentification réservée à api.github.com"));
        }
        let addresses = security::public_addresses(&parsed)?;
        let client = Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(10))
            .user_agent("Studio-App catalog-proposal/0.1")
            .resolve_to_addrs(host, &addresses)
            .build()
            .map_err(|_| Error::network("Client GitHub indisponible"))?;
        let mut request = client
            .get(parsed)
            .header(
                ACCEPT,
                if binary {
                    "application/octet-stream"
                } else {
                    "application/vnd.github+json"
                },
            )
            .header(ACCEPT_ENCODING, "identity");
        if api {
            request = request.header("X-GitHub-Api-Version", "2026-03-10");
            if let Some(token) = &self.token {
                request = request.header(AUTHORIZATION, format!("Bearer {token}"));
            }
        }
        request.send().map_err(|_| {
            Error::network("Requête GitHub/HTTPS échouée ; URL et credentials masqués")
        })
    }
    fn hash(response: Response, size: u64) -> Result<String> {
        if response.status().as_u16() != 200 {
            return Err(Error::network(&format!(
                "Téléchargement du package refusé (HTTP {})",
                response.status().as_u16()
            )));
        }
        if response
            .headers()
            .get(CONTENT_ENCODING)
            .is_some_and(|v| v != "identity")
            || response
                .headers()
                .get(CONTENT_LENGTH)
                .is_some_and(|v| v.to_str().ok().and_then(|s| s.parse::<u64>().ok()) != Some(size))
        {
            return Err(Error::invalid("Taille ou encodage du package incohérent"));
        }
        Self::hash_reader(&mut { response }, size)
    }
    fn visibility(bytes: &[u8], expected: &Visibility) -> Result<()> {
        #[derive(serde::Deserialize)]
        struct Repository {
            private: bool,
        }
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err(Error::invalid("Métadonnées de dépôt excessives"));
        }
        let repository: Repository = serde_json::from_slice(bytes)?;
        if repository.private != (*expected == Visibility::Private) {
            return Err(Error::invalid("Visibilité réelle du dépôt différente du registre ; consentement privé explicite requis"));
        }
        Ok(())
    }
    fn hash_reader(response: &mut dyn Read, size: u64) -> Result<String> {
        let mut total = 0u64;
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        let began = Instant::now();
        loop {
            if began.elapsed() > Duration::from_secs(30 * 60) {
                return Err(Error::network(
                    "Durée maximale du calcul d’empreinte dépassée",
                ));
            }
            let n = response
                .read(&mut buffer)
                .map_err(|_| Error::network("Package interrompu pendant le calcul d’empreinte"))?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > size {
                return Err(Error::invalid("Package plus grand que la taille annoncée"));
            }
            hash.update(&buffer[..n]);
        }
        if total != size {
            return Err(Error::invalid("Package tronqué"));
        }
        Ok(crate::installer::approval::hex(&hash.finalize()))
    }
}
impl Api for Github {
    fn releases(&self, source: &GitHubSource, visibility: &Visibility) -> Result<Vec<ReleaseInfo>> {
        crate::catalog::github::validate_source(source)?;
        let metadata = self.request(
            &format!(
                "https://api.github.com/repos/{}/{}",
                source.owner, source.repository
            ),
            true,
            false,
        )?;
        if metadata.status().as_u16() != 200 {
            return Err(Error::network(
                "Dépôt absent ou privé sans accès autorisé ; détails masqués",
            ));
        }
        let mut bytes = Vec::new();
        metadata
            .take(MAX_CATALOG_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::network("Métadonnées du dépôt interrompues"))?;
        Self::visibility(&bytes, visibility)?;
        crate::catalog::github::validate_source(source)?;
        let response = self.request(
            &format!(
                "https://api.github.com/repos/{}/{}/releases?per_page=20",
                source.owner, source.repository
            ),
            true,
            false,
        )?;
        if response.status().as_u16() != 200 {
            return Err(Error::network(&format!("Lecture des releases refusée (HTTP {}) : dépôt absent, privé sans accès autorisé ou quota API atteint",response.status().as_u16())));
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_CATALOG_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::network("Réponse GitHub interrompue"))?;
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err(Error::invalid("Réponse GitHub supérieure à 2 Mio"));
        }
        let releases: Vec<ReleaseInfo> = serde_json::from_slice(&bytes)?;
        if releases.len() > 20 {
            return Err(Error::invalid("Plus de 20 releases"));
        }
        Ok(releases)
    }
    fn hash_asset(&self, source: &GitHubSource, asset: &AssetInfo) -> Result<String> {
        crate::catalog::github::validate_source(source)?;
        if asset.id == 0 || asset.size == 0 || asset.size > super::sync::MAX_PACKAGE {
            return Err(Error::invalid("Identifiant ou taille d’asset invalide"));
        }
        let response = self.request(
            &format!(
                "https://api.github.com/repos/{}/{}/releases/assets/{}",
                source.owner, source.repository, asset.id
            ),
            true,
            true,
        )?;
        if response.status().as_u16() == 302 {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|h| h.to_str().ok())
                .ok_or_else(|| Error::invalid("Redirection GitHub sans destination"))?;
            let parsed = security::https_url(location)?;
            if !matches!(
                parsed.host_str(),
                Some("release-assets.githubusercontent.com" | "objects.githubusercontent.com")
            ) {
                return Err(Error::invalid("CDN de redirection non autorisé"));
            }
            Self::hash(self.request(location, false, true)?, asset.size)
        } else {
            Self::hash(response, asset.size)
        }
    }
    fn hash_public(&self, url: &str, size: u64) -> Result<String> {
        if size == 0 || size > super::sync::MAX_PACKAGE {
            return Err(Error::invalid("Taille du miroir invalide"));
        }
        Self::hash(self.request(url, false, true)?, size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hashes_actual_bytes_and_rejects_incomplete_or_excessive_streams() {
        let expected = crate::installer::approval::hex(&Sha256::digest(b"fixture bytes"));
        assert_eq!(
            Github::hash_reader(&mut std::io::Cursor::new(b"fixture bytes"), 13).unwrap(),
            expected
        );
        assert!(Github::hash_reader(&mut std::io::Cursor::new(b"fixture"), 13).is_err());
        assert!(
            Github::hash_reader(&mut std::io::Cursor::new(b"fixture bytes extra"), 13).is_err()
        );
    }
    #[test]
    fn interrupted_stream_is_an_error_not_an_approved_digest() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::ConnectionReset.into())
            }
        }
        assert!(Github::hash_reader(&mut Broken, 13).is_err());
    }
    #[test]
    fn actual_private_repository_cannot_masquerade_as_public() {
        assert!(Github::visibility(br#"{"private":true}"#, &Visibility::Public).is_err());
        assert!(Github::visibility(br#"{"private":true}"#, &Visibility::Private).is_ok());
        assert!(Github::visibility(br#"{"private":false}"#, &Visibility::Public).is_ok());
        assert!(Github::visibility(br#"{}"#, &Visibility::Public).is_err());
    }
    #[test]
    fn malformed_token_is_redacted() {
        let secret = "do-not-print-this-secret\n";
        let error = Github::new(Some(secret.into())).err().unwrap().to_string();
        assert!(!error.contains(secret));
        assert!(error.contains("masquée"));
    }
}
