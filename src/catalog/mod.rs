pub mod assets;
pub mod cache;
pub mod github;
pub mod loader;
pub mod model;
pub mod security;
mod validation;
pub mod worker;

pub use model::Catalog;
use std::fmt;
pub const MAX_CATALOG_BYTES: usize = 2 * 1024 * 1024;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Error(pub String);
impl Error {
    pub fn invalid(message: &str) -> Self {
        Self(format!("Catalogue invalide : {message}"))
    }
    pub fn network(message: &str) -> Self {
        Self(format!("Réseau : {message}"))
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self(format!("E/S : {e}"))
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self(format!("JSON : {e}"))
    }
}

pub fn parse(bytes: &[u8]) -> Result<Catalog> {
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(Error::invalid("Limite de 2 MiB dépassée"));
    }
    let catalog: Catalog = serde_json::from_slice(bytes)?;
    validation::validate(&catalog)?;
    Ok(catalog)
}

/// Tests use the same JSON file as the runtime example, never a separate Rust catalog.
#[cfg(test)]
pub fn demonstration() -> Vec<crate::domain::Application> {
    let catalog = parse(include_bytes!("../../catalog/catalog.json")).unwrap();
    let mut apps: Vec<_> = catalog
        .applications
        .into_iter()
        .map(crate::domain::Application::new)
        .collect();
    crate::demo::seed(&mut apps);
    apps
}
