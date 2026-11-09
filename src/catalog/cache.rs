//! Source-scoped, atomically replaced, bounded cache. Revalidate every read.
use super::{Error, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(Error::invalid("Fichier non régulier ou trop volumineux"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(Error::invalid("Fichier trop volumineux"));
    }
    Ok(bytes)
}

#[derive(Clone)]
pub struct Cache {
    pub root: PathBuf,
}
impl Cache {
    pub fn new(base: &Path, source_key: &str) -> Self {
        Self {
            root: base.join(digest(source_key.as_bytes())),
        }
    }
    pub fn read(&self, name: &str, limit: usize) -> Result<Vec<u8>> {
        if fs::symlink_metadata(&self.root)?.file_type().is_symlink() {
            return Err(Error::invalid("Racine de cache symbolique interdite"));
        }
        read_bounded(&self.path(name)?, limit)
    }
    fn path(&self, name: &str) -> Result<PathBuf> {
        if !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            || name.starts_with('.')
            || name.len() > 80
            || name.is_empty()
        {
            return Err(Error::invalid("Nom de cache interne invalide"));
        }
        Ok(self.root.join(name))
    }
    /// Keep only images referenced by the newly accepted snapshot in this source's cache.
    pub fn retain_images(&self, sources: &[String]) -> Result<()> {
        if fs::symlink_metadata(&self.root)?.file_type().is_symlink() {
            return Err(Error::invalid("Racine de cache symbolique interdite"));
        }
        let keep: std::collections::HashSet<_> = sources
            .iter()
            .map(|s| format!("{}.image", digest(s.as_bytes())))
            .collect();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(hash) = name.strip_suffix(".image") {
                if hash.len() == 64
                    && hash.bytes().all(|b| b.is_ascii_hexdigit())
                    && !keep.contains(&name)
                    && entry.file_type()?.is_file()
                {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }
    pub fn write(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let path = self.path(name)?;
        fs::create_dir_all(&self.root)?;
        if fs::symlink_metadata(&self.root)?.file_type().is_symlink() {
            return Err(Error::invalid("Racine de cache symbolique interdite"));
        }
        let mut pending = tempfile::NamedTempFile::new_in(&self.root)?;
        pending.write_all(bytes)?;
        pending.as_file().sync_all()?;
        pending.persist(path).map_err(|e| Error::from(e.error))?;
        Ok(())
    }
}
