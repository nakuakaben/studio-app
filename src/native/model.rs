use serde::{Deserialize, Serialize};
use std::path::PathBuf;
/// Identity hints only. No absolute path, command, argument or executable URL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionRules {
    pub macos: Option<MacRule>,
    pub windows: Option<WindowsRule>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacRule {
    pub bundle_id: String,
    pub bundle_paths: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsRule {
    pub uninstall_key: String,
    pub publisher: String,
    pub executable: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    MacBundle,
    WindowsRegistry { user: bool, bits: u8 },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installation {
    pub path: PathBuf,
    pub executable: PathBuf,
    pub version: Option<String>,
    pub source: Source,
}
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub installations: Vec<Installation>,
    pub errors: Vec<String>,
}
impl Report {
    pub(crate) fn add(&mut self, installation: Installation) {
        if !self
            .installations
            .iter()
            .any(|i| i.path == installation.path)
        {
            self.installations.push(installation);
        }
    }
}
fn relative(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && !value
            .chars()
            .any(|c| c.is_control() || "\\:%\"<>|?*".contains(c))
        && value
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != ".." && !s.ends_with(['.', ' ']))
}
pub fn validate(rules: &DetectionRules) -> Result<(), String> {
    if rules.macos.is_none() && rules.windows.is_none() {
        return Err("Identifiant de détection absent".into());
    }
    if let Some(rule) = &rules.macos {
        if rule.bundle_id.len() > 200
            || rule.bundle_id.split('.').count() < 2
            || !rule.bundle_id.split('.').all(|part| {
                !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
            || rule.bundle_paths.is_empty()
            || rule.bundle_paths.len() > 4
            || rule
                .bundle_paths
                .iter()
                .any(|p| !relative(p) || !p.ends_with(".app"))
        {
            return Err("Règle de bundle macOS invalide".into());
        }
    }
    if let Some(rule) = &rules.windows {
        if rule.uninstall_key.is_empty()
            || rule.uninstall_key.len() > 128
            || !rule
                .uninstall_key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b" {}._-".contains(&b))
            || rule.publisher.trim().is_empty()
            || rule.publisher.len() > 128
            || rule.publisher.chars().any(char::is_control)
            || !relative(&rule.executable)
            || !rule.executable.to_ascii_lowercase().ends_with(".exe")
        {
            return Err("Règle Windows invalide".into());
        }
        let name = rule
            .executable
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if [
            "cmd.exe",
            "powershell.exe",
            "pwsh.exe",
            "wscript.exe",
            "cscript.exe",
            "mshta.exe",
            "rundll32.exe",
            "regsvr32.exe",
            "msiexec.exe",
        ]
        .contains(&name.as_str())
        {
            return Err("Interpréteur ou installateur interdit".into());
        }
    }
    Ok(())
}
/// SemVer first; numeric Windows versions (1–4 components) otherwise. No guesses.
pub fn compare_versions(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    if let (Ok(a), Ok(b)) = (semver::Version::parse(a), semver::Version::parse(b)) {
        return Some(a.cmp_precedence(&b));
    }
    fn numeric(s: &str) -> Option<[u64; 4]> {
        let parts: Vec<_> = s.split('.').collect();
        if parts.is_empty() || parts.len() > 4 {
            return None;
        }
        let mut out = [0; 4];
        for (i, p) in parts.iter().enumerate() {
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            out[i] = p.parse().ok()?;
        }
        Some(out)
    }
    Some(numeric(a)?.cmp(&numeric(b)?))
}
