use super::{model::MacRule, Installation, Report, Source};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};
const MAX_PLIST: u64 = 256 * 1024;
pub fn inspect(root: &Path, relative: &str, rule: &MacRule) -> Result<Installation, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !path.starts_with(&root) || !path.is_dir() || path.extension().is_none_or(|e| e != "app") {
        return Err("Bundle hors du dossier autorisé".into());
    }
    let info = path
        .join("Contents/Info.plist")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !info.starts_with(&path) {
        return Err("Info.plist hors du bundle".into());
    }
    let mut bytes = Vec::new();
    File::open(info)
        .map_err(|e| e.to_string())?
        .take(MAX_PLIST + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_PLIST {
        return Err("Info.plist trop volumineux".into());
    }
    let value = plist::Value::from_reader(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let dict = value.as_dictionary().ok_or("Info.plist invalide")?;
    let get = |name: &str| dict.get(name).and_then(plist::Value::as_string);
    if get("CFBundleIdentifier") != Some(rule.bundle_id.as_str())
        || get("CFBundlePackageType") != Some("APPL")
    {
        return Err("Identité ou type du bundle incorrect".into());
    }
    let name = get("CFBundleExecutable").ok_or("Exécutable absent du bundle")?;
    if name.is_empty()
        || name.len() > 200
        || name == "."
        || name == ".."
        || name.chars().any(|c| c.is_control() || "/\\:".contains(c))
    {
        return Err("Nom d’exécutable invalide".into());
    }
    let executable = path
        .join("Contents/MacOS")
        .join(name)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !executable.starts_with(path.join("Contents/MacOS")) || !executable.is_file() {
        return Err("Exécutable hors du bundle".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(&executable)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err("Permission d’exécution absente".into());
        }
    }
    let version = get("CFBundleShortVersionString")
        .or_else(|| get("CFBundleVersion"))
        .filter(|v| !v.is_empty() && v.len() <= 64 && !v.chars().any(char::is_control))
        .map(str::to_owned);
    Ok(Installation {
        path,
        executable,
        version,
        source: Source::MacBundle,
    })
}
pub fn detect(rule: &MacRule, roots: &[PathBuf]) -> Report {
    let mut report = Report::default();
    for root in roots {
        for relative in &rule.bundle_paths {
            let candidate = root.join(relative);
            match std::fs::symlink_metadata(&candidate) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    report.errors.push(format!("{} : {e}", candidate.display()));
                    continue;
                }
                Ok(_) => {}
            }
            match inspect(root, relative, rule) {
                Ok(i) => report.add(i),
                Err(e) => report.errors.push(format!("{} : {e}", candidate.display())),
            }
        }
    }
    report
}
#[cfg(target_os = "macos")]
pub fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(base) = directories::BaseDirs::new() {
        roots.push(base.home_dir().join("Applications"));
    }
    roots.push("/Applications".into());
    roots.push("/System/Applications".into());
    roots
}
