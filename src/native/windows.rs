use super::{model::WindowsRule, Installation, Report, Source};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
#[derive(Clone, Debug)]
pub struct Record {
    pub key: String,
    pub publisher: String,
    pub directory: PathBuf,
    pub version: Option<String>,
    pub user: bool,
    pub bits: u8,
}
/// Only local absolute paths. No UNC, URL, environment expansion or device path.
pub fn local_path(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        matches!(path.components().next(),Some(Component::Prefix(p))if matches!(p.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_)))
            && path.is_absolute()
    }
    #[cfg(not(windows))]
    {
        path.is_absolute()
    }
}
pub(crate) fn pe(path: &Path) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut header = [0; 64];
    file.read_exact(&mut header).map_err(|e| e.to_string())?;
    if &header[..2] != b"MZ" {
        return Err("Fichier PE requis".into());
    }
    let offset = u32::from_le_bytes(
        header[60..64]
            .try_into()
            .map_err(|_| "En-tête PE invalide")?,
    ) as u64;
    if !(64..=1024 * 1024).contains(&offset) {
        return Err("En-tête PE hors limite".into());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut coff = [0; 24];
    file.read_exact(&mut coff).map_err(|e| e.to_string())?;
    let flags = u16::from_le_bytes([coff[22], coff[23]]);
    if &coff[..4] != b"PE\0\0" || flags & 0x0002 == 0 || flags & 0x2000 != 0 {
        return Err("Exécutable PE requis (DLL refusée)".into());
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn uninstall_key_exists(rule: &WindowsRule) -> Result<bool, std::io::Error> {
    use winreg::{enums::*, RegKey};
    let path = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        rule.uninstall_key
    );
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            match RegKey::predef(hive).open_subkey_with_flags(&path, KEY_QUERY_VALUE | view) {
                Ok(_) => return Ok(true),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(false)
}
pub fn inspect(rule: &WindowsRule, record: &Record) -> Result<Installation, String> {
    if record.key != rule.uninstall_key || record.publisher != rule.publisher {
        return Err("Identité du registre incorrecte".into());
    }
    if !local_path(&record.directory) {
        return Err("InstallLocation local absolu requis".into());
    }
    let path = record.directory.canonicalize().map_err(|e| e.to_string())?;
    if !local_path(&path) || !path.is_dir() {
        return Err("Dossier d’installation invalide".into());
    }
    if path
        .join(crate::installer::model::PENDING_MARKER)
        .try_exists()
        .map_err(|e| e.to_string())?
    {
        return Err(
            "Installation incomplète ; vérifiez le dossier de test avant toute ouverture".into(),
        );
    }
    let executable = path
        .join(&rule.executable)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !local_path(&executable)
        || !executable.starts_with(&path)
        || executable
            .extension()
            .is_none_or(|s| !s.eq_ignore_ascii_case("exe"))
        || !executable.is_file()
    {
        return Err("Exécutable hors installation".into());
    }
    pe(&executable)?;
    let version = record
        .version
        .clone()
        .filter(|s| !s.is_empty() && s.len() <= 64 && !s.chars().any(char::is_control));
    Ok(Installation {
        path,
        executable,
        version,
        source: Source::WindowsRegistry {
            user: record.user,
            bits: record.bits,
        },
    })
}
pub fn detect(
    rule: &WindowsRule,
    records: impl IntoIterator<Item = Result<Record, String>>,
) -> Report {
    let mut report = Report::default();
    for record in records {
        match record.and_then(|r| inspect(rule, &r)) {
            Ok(i) => report.add(i),
            Err(e) => report.errors.push(e),
        }
    }
    report
}
#[cfg(any(windows, test))]
fn unquote_location(path: &str) -> &str {
    path.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(path)
}

#[cfg(test)]
#[test]
fn nsis_location_strips_only_paired_quotes() {
    assert_eq!(
        unquote_location(r#""C:\Users\User\AppData\Local\HMP""#),
        r"C:\Users\User\AppData\Local\HMP"
    );
    assert_eq!(unquote_location(r#""C:\HMP"#), r#""C:\HMP"#);
}
#[cfg(windows)]
pub fn records(rule: &WindowsRule) -> Vec<Result<Record, String>> {
    use winreg::{enums::*, RegKey};
    let mut out = Vec::new();
    for (user, hive) in [(true, HKEY_CURRENT_USER), (false, HKEY_LOCAL_MACHINE)] {
        for (bits, view) in [(64, KEY_WOW64_64KEY), (32, KEY_WOW64_32KEY)] {
            let parent = RegKey::predef(hive);
            let subkey = format!(
                r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
                rule.uninstall_key
            );
            let key = match parent.open_subkey_with_flags(subkey, KEY_QUERY_VALUE | view) {
                Ok(k) => k,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    out.push(Err(format!("Registre {bits} bits : {e}")));
                    continue;
                }
            };
            let record = (|| -> Result<Record, String> {
                // Values are bounded before decoding; do not read UninstallString or DisplayIcon.
                let read = |name: &str| -> Result<String, String> {
                    let value = key.get_raw_value(name).map_err(|e| e.to_string())?;
                    if value.vtype != REG_SZ || value.bytes.len() > 8192 {
                        return Err("Valeur de registre invalide ou trop longue".into());
                    }
                    use winreg::types::FromRegValue;
                    String::from_reg_value(&value).map_err(|e| e.to_string())
                };
                Ok(Record {
                    key: rule.uninstall_key.clone(),
                    publisher: read("Publisher")?,
                    directory: PathBuf::from(unquote_location(&read("InstallLocation")?)),
                    version: read("DisplayVersion").ok(),
                    user,
                    bits,
                })
            })();
            out.push(record);
        }
    }
    out
}
