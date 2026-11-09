use super::{approval::simple_name, model::*};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
pub const MAX_EXPANDED: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 10000;
pub fn safe_entry(name: &str, root: &str) -> Result<std::path::PathBuf> {
    let name = name.trim_end_matches('/');
    crate::catalog::security::relative_path(name).map_err(|e| Failure::permanent(e.to_string()))?;
    if name.split('/').count() > 24 || name.split('/').next() != Some(root) {
        return Err(Failure::permanent("Archive hors de la structure approuvée"));
    }
    for part in name.split('/') {
        simple_name(part)?;
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ]
        .contains(&stem.as_str())
        {
            return Err(Failure::permanent(
                "Nom de périphérique interdit dans l’archive",
            ));
        }
    }
    Ok(name.into())
}
// Bound central-directory parsing before the ZIP library allocates its index.
// This first deployment accepts ordinary ZIP only, not ZIP64, SFX or split archives.
fn bounded_zip_index(package: &Path) -> Result<()> {
    let mut file = File::open(package)?;
    let length = file.metadata()?.len();
    let tail_length = length.min(65535 + 22 + 20);
    file.seek(SeekFrom::End(-(tail_length as i64)))?;
    let mut tail = vec![0; tail_length as usize];
    file.read_exact(&mut tail)?;
    let end = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&i| {
            tail.get(i..i + 4) == Some(b"PK\x05\x06")
                && i + 22 + u16::from_le_bytes([tail[i + 20], tail[i + 21]]) as usize == tail.len()
        })
        .ok_or_else(|| Failure::permanent("Index ZIP absent ou tronqué"))?;
    let u16_at = |i| u16::from_le_bytes([tail[end + i], tail[end + i + 1]]);
    let u32_at = |i| u32::from_le_bytes(tail[end + i..end + i + 4].try_into().unwrap()) as u64;
    let count = u16_at(10) as usize;
    let size = u32_at(12);
    let offset = u32_at(16);
    if u16_at(4) != 0
        || u16_at(6) != 0
        || u16_at(8) as usize != count
        || count == 0
        || count > MAX_ENTRIES
        || size > 4 * 1024 * 1024
        || offset.checked_add(size) != Some(length - tail_length + end as u64)
        || (end >= 20 && &tail[end - 20..end - 16] == b"PK\x06\x07")
    {
        return Err(Failure::permanent(
            "Index ZIP hors limite, ZIP64 ou archive multipartie non pris en charge",
        ));
    }
    Ok(())
}

pub fn extract_zip(
    package: &Path,
    destination: &Path,
    root: &str,
    cancel: &Cancellation,
) -> Result<()> {
    bounded_zip_index(package)?;
    let mut archive = zip::ZipArchive::new(File::open(package)?)
        .map_err(|_| Failure::permanent("Archive ZIP corrompue ou non prise en charge"))?;
    if archive.is_empty() || archive.len() > MAX_ENTRIES {
        return Err(Failure::permanent("Nombre de fichiers ZIP hors limite"));
    }
    let mut names = HashSet::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        cancel.check()?;
        let mut entry = archive
            .by_index(index)
            .map_err(|_| Failure::permanent("Entrée ZIP illisible ou chiffrée"))?;
        let relative = safe_entry(entry.name(), root)?;
        if !names.insert(relative.to_string_lossy().to_lowercase()) {
            return Err(Failure::permanent("Chemin dupliqué dans le ZIP"));
        }
        let mode = entry.unix_mode().unwrap_or(0);
        let kind = mode & 0o170000;
        if !matches!(kind, 0 | 0o100000 | 0o040000) {
            return Err(Failure::permanent(
                "Lien symbolique ou fichier spécial interdit dans le ZIP",
            ));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| Failure::permanent("Taille ZIP hors limite"))?;
        if total > MAX_EXPANDED || entry.size() > 1024 * 1024 * 1024 {
            return Err(Failure::permanent("Archive décompressée trop volumineuse"));
        }
        let path = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(path)?;
            continue;
        }
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| Failure::permanent("Chemin ZIP invalide"))?,
        )?;
        let size = entry.size();
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let mut written = 0u64;
        let mut buffer = [0; 64 * 1024];
        loop {
            cancel.check()?;
            let n = entry
                .read(&mut buffer)
                .map_err(|_| Failure::permanent("Données ZIP/CRC corrompues"))?;
            if n == 0 {
                break;
            }
            written += n as u64;
            if written > size {
                return Err(Failure::permanent("Taille ZIP annoncée incorrecte"));
            }
            file.write_all(&buffer[..n])?;
        }
        if written != size {
            return Err(Failure::permanent("Entrée ZIP tronquée"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                path,
                fs::Permissions::from_mode(if mode & 0o111 != 0 { 0o755 } else { 0o644 }),
            )?;
        }
    }
    if !destination.join(root).is_dir() {
        return Err(Failure::permanent("Racine approuvée absente du package"));
    }
    Ok(())
}

/// Used only for validated private staging or the read-only mounted image.
pub fn copy_tree(source: &Path, destination: &Path, cancel: &Cancellation) -> Result<()> {
    fn copy(
        source: &Path,
        destination: &Path,
        cancel: &Cancellation,
        count: &mut usize,
        total: &mut u64,
        depth: usize,
    ) -> Result<()> {
        cancel.check()?;
        *count += 1;
        if *count > MAX_ENTRIES || depth > 24 {
            return Err(Failure::permanent("Arborescence hors limite"));
        }
        let meta = fs::symlink_metadata(source)?;
        if meta.file_type().is_symlink() {
            return Err(Failure::permanent(
                "Lien symbolique interdit dans le package",
            ));
        }
        if meta.is_dir() {
            fs::create_dir(destination)?;
            for entry in fs::read_dir(source)? {
                let entry = entry?;
                let name = entry.file_name();
                simple_name(&name.to_string_lossy())?;
                copy(
                    &entry.path(),
                    &destination.join(name),
                    cancel,
                    count,
                    total,
                    depth + 1,
                )?;
            }
        } else if meta.is_file() {
            *total = total
                .checked_add(meta.len())
                .ok_or_else(|| Failure::permanent("Taille hors limite"))?;
            if *total > MAX_EXPANDED || meta.len() > 1024 * 1024 * 1024 {
                return Err(Failure::permanent("Arborescence trop volumineuse"));
            }
            let mut input = File::open(source)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?;
            let mut copied = 0u64;
            let mut buffer = [0; 64 * 1024];
            loop {
                cancel.check()?;
                let n = input.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                copied += n as u64;
                if copied > meta.len() {
                    return Err(Failure::permanent("Fichier modifié pendant la copie"));
                }
                output.write_all(&buffer[..n])?;
            }
            if copied != meta.len() {
                return Err(Failure::permanent("Fichier modifié pendant la copie"));
            }
            output.sync_all()?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    destination,
                    fs::Permissions::from_mode(if meta.permissions().mode() & 0o111 != 0 {
                        0o755
                    } else {
                        0o644
                    }),
                )?;
            }
        } else {
            return Err(Failure::permanent("Fichier spécial interdit"));
        }
        Ok(())
    }
    copy(source, destination, cancel, &mut 0, &mut 0, 0)
}
