//! Local UI preferences, independent of catalogue and installation state.
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    #[default]
    Grid,
    List,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    #[default]
    System,
    Dark,
    Light,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    Auto,
    French,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    Name,
    Category,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub schema_version: u32,
    pub view: View,
    pub appearance: Appearance,
    pub language: Language,
    pub sort: Sort,
    pub community_website: Option<String>,
    pub community_discord: Option<String>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema_version: 1,
            view: View::Grid,
            appearance: Appearance::System,
            language: Language::Auto,
            sort: Sort::Name,
            community_website: None,
            community_discord: None,
        }
    }
}
impl Preferences {
    pub fn path() -> Option<PathBuf> {
        if let Some(dir) = std::env::var_os("STUDIO_APP_PREFERENCES_DIR") {
            return Some(PathBuf::from(dir).join("ui-preferences.json"));
        }
        directories::ProjectDirs::from("app", "Studio App", "Studio App")
            .map(|d| d.config_dir().join("ui-preferences.json"))
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("Version des préférences non prise en charge".into());
        }
        for value in [&self.community_website, &self.community_discord]
            .into_iter()
            .flatten()
        {
            let url = url::Url::parse(value).map_err(|_| "Lien communautaire invalide")?;
            if value.len() > 2048
                || url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(
                    "Les liens communautaires doivent être des URLs HTTPS sans identifiants".into(),
                );
            }
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.take(16385)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 16384 {
            return Err("Préférences trop volumineuses".into());
        }
        let prefs: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        prefs.validate()?;
        Ok(prefs)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let parent = path.parent().ok_or("Chemin de préférences invalide")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_round_trip_and_invalid_write_preserves_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.json");
        let mut p = Preferences {
            view: View::List,
            appearance: Appearance::Light,
            ..Default::default()
        };
        p.save(&path).unwrap();
        assert_eq!(Preferences::load(&path).unwrap().view, View::List);
        p.community_website = Some("javascript:alert(1)".into());
        assert!(p.save(&path).is_err());
        assert_eq!(
            Preferences::load(&path).unwrap().appearance,
            Appearance::Light
        );
    }
    #[test]
    fn invalid_schema_unknown_fields_and_large_files_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.json");
        for bytes in [
            b"{}".to_vec(),
            vec![b' '; 16385],
            b"{\"schema_version\":2}".to_vec(),
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(Preferences::load(&path).is_err());
        }
        let p = Preferences {
            schema_version: 2,
            ..Default::default()
        };
        assert!(p.validate().is_err());
        let mut value = serde_json::to_value(Preferences::default()).unwrap();
        value["unknown"] = true.into();
        assert!(serde_json::from_value::<Preferences>(value).is_err());
    }
}
