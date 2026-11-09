use super::{Error, Result};
use std::{
    net::{IpAddr, ToSocketAddrs},
    path::{Component, Path, PathBuf},
};
use url::{Host, Url};

pub fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && value.as_bytes()[0].is_ascii_alphanumeric()
}

pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let [a, b, _, _] = v.octets();
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_broadcast()
                || v.is_documentation()
                || v.is_unspecified()
                || v.is_multicast()
                || a == 0
                || a >= 240
                || (a == 100 && (64..128).contains(&b))
                || (a == 198 && (18..20).contains(&b))
                || (a == 192 && b == 0)
                || (a == 192 && b == 88 && v.octets()[2] == 99))
        }
        IpAddr::V6(v) => {
            // Only global unicast; exclude mapped IPv4, documentation and transition ranges.
            let s = v.segments();
            (s[0] & 0xe000 == 0x2000)
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}

pub fn https_url(value: &str) -> Result<Url> {
    if value.trim() != value
        || value.len() > 2048
        || value.chars().any(|c| c.is_control() || c == '\\')
    {
        return Err(Error::invalid("URL trop longue ou mal formée"));
    }
    let url = Url::parse(value).map_err(|_| Error::invalid("URL invalide"))?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port().is_some_and(|p| p != 443)
    {
        return Err(Error::invalid(
            "HTTPS sans identifiants, fragment ni port alternatif requis",
        ));
    }
    match url.host() {
        Some(Host::Domain(host))
            if host.contains('.')
                && !["localhost", "local", "internal", "test", "invalid"]
                    .iter()
                    .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
                && !host.ends_with('.') => {}
        Some(Host::Ipv4(ip)) if public_ip(ip.into()) => {}
        Some(Host::Ipv6(ip)) if public_ip(ip.into()) => {}
        _ => return Err(Error::invalid("Hôte non public interdit")),
    }
    Ok(url)
}

/// Resolve once, validate every address and pin that set into the HTTP client.
/// Redirects and proxies are disabled by the transport, so validation cannot be bypassed.
pub fn public_addresses(url: &Url) -> Result<Vec<std::net::SocketAddr>> {
    let addresses: Vec<_> = match url.host() {
        Some(Host::Ipv4(ip)) => vec![std::net::SocketAddr::new(ip.into(), 443)],
        Some(Host::Ipv6(ip)) => vec![std::net::SocketAddr::new(ip.into(), 443)],
        Some(Host::Domain(host)) => (host, 443)
            .to_socket_addrs()
            .map_err(|_| Error::network("Résolution DNS impossible"))?
            .collect(),
        None => return Err(Error::invalid("Hôte absent")),
    };
    if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
        return Err(Error::invalid("DNS vers une adresse non publique interdit"));
    }
    Ok(addresses)
}

pub fn relative_path(value: &str) -> Result<&Path> {
    if value.is_empty()
        || value.len() > 512
        || value.contains(['\\', ':', '%'])
        || value.chars().any(char::is_control)
        || value.starts_with('/')
        || value
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(Error::invalid("Chemin relatif non sûr"));
    }
    let p = Path::new(value);
    if !p.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(Error::invalid("Traversée de chemin interdite"));
    }
    Ok(p)
}

pub fn confined_file(root: &Path, value: &str) -> Result<PathBuf> {
    let relative = relative_path(value)?;
    let root = root
        .canonicalize()
        .map_err(|_| Error::invalid("Racine locale inaccessible"))?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|_| Error::invalid("Ressource locale inaccessible"))?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(Error::invalid("Ressource hors de la racine autorisée"));
    }
    Ok(path)
}
