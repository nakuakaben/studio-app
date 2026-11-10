//! Release metadata proposals and an offline publication gate. Never installs or executes assets.
pub mod approval;
pub mod github;
pub mod model;
pub mod sync;
pub use crate::catalog::{Error, Result};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
pub fn digest<T: Serialize>(value: &T) -> Result<String> {
    Ok(crate::installer::approval::hex(&Sha256::digest(
        serde_json::to_vec(value)?,
    )))
}
pub fn read<T: DeserializeOwned>(path: &std::path::Path) -> Result<T> {
    let bytes = crate::catalog::cache::read_bounded(path, crate::catalog::MAX_CATALOG_BYTES)?;
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn write<T: Serialize>(path: &std::path::Path, value: &T) -> Result<()> {
    use std::io::Write;
    let bytes = serde_json::to_vec_pretty(value)?;
    if bytes.len() > crate::catalog::MAX_CATALOG_BYTES {
        return Err(Error::invalid("Sortie de distribution supérieure à 2 Mio"));
    }
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().unwrap_or(std::path::Path::new(".")))?;
    file.write_all(&bytes)?;
    file.as_file().sync_all()?;
    file.persist(path)
        .map_err(|e| Error::invalid(&format!("Écriture atomique impossible : {}", e.error)))?;
    Ok(())
}
