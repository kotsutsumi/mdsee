//! Config（design.md §63〜§65）。
//!
//! `defaults < config file < CLI` の優先順位でmergeする。

use std::path::PathBuf;

use serde::Deserialize;

/// config.toml全体（§64）。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfigFile {
    pub theme: String,
    pub layout: LayoutSection,
    pub reader: ReaderSection,
    pub images: ImagesSection,
    pub network: NetworkSection,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            theme: "auto".to_string(),
            layout: LayoutSection::default(),
            reader: ReaderSection::default(),
            images: ImagesSection::default(),
            network: NetworkSection::default(),
        }
    }
}

/// §64 `[layout]`。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LayoutSection {
    pub max_width: u16,
    pub margin: u16,
}

impl Default for LayoutSection {
    fn default() -> Self {
        Self {
            max_width: 100,
            margin: 2,
        }
    }
}

/// §64 `[reader]`。Sprint 6（S6-7）で使用する。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReaderSection {
    pub mouse: bool,
}

impl Default for ReaderSection {
    fn default() -> Self {
        Self { mouse: true }
    }
}

/// §64 `[images]`。Sprint 4以降で使用する。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImagesSection {
    pub enabled: bool,
    pub backend: String,
    pub max_height: u16,
}

impl Default for ImagesSection {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: "auto".to_string(),
            max_height: 40,
        }
    }
}

/// §64 `[network]`。Sprint 7以降で使用する。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkSection {
    pub remote_images: bool,
    pub timeout_seconds: u64,
    pub max_download_mb: u64,
}

impl Default for NetworkSection {
    fn default() -> Self {
        Self {
            remote_images: true,
            timeout_seconds: 5,
            max_download_mb: 20,
        }
    }
}

/// config.tomlのpath（§63）。directoriesでOS規定の位置を解決する。
pub fn config_path() -> Option<PathBuf> {
    let project = directories::ProjectDirs::from("", "", "mdsee")?;
    Some(project.config_dir().join("config.toml"))
}

/// config.tomlを読む。不存在ならdefaults。不正ならエラーを返す。
pub fn load_config_file(path: &std::path::Path) -> Result<ConfigFile, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    toml::from_str(&content).map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_design() {
        let config = ConfigFile::default();
        assert_eq!(config.theme, "auto");
        assert_eq!(config.layout.max_width, 100);
        assert_eq!(config.layout.margin, 2);
        assert!(config.reader.mouse);
        assert!(config.images.enabled);
        assert_eq!(config.images.backend, "auto");
        assert_eq!(config.images.max_height, 40);
        assert!(config.network.remote_images);
        assert_eq!(config.network.timeout_seconds, 5);
        assert_eq!(config.network.max_download_mb, 20);
    }

    #[test]
    fn parses_full_config() {
        let raw = r#"
theme = "dark"

[layout]
max_width = 80
margin = 1

[reader]
mouse = false

[images]
enabled = false
backend = "kitty"
max_height = 20

[network]
remote_images = false
timeout_seconds = 3
max_download_mb = 8
"#;
        let config: ConfigFile = toml::from_str(raw).unwrap();
        assert_eq!(config.theme, "dark");
        assert_eq!(config.layout.max_width, 80);
        assert_eq!(config.layout.margin, 1);
        assert!(!config.reader.mouse);
        assert!(!config.images.enabled);
        assert_eq!(config.images.backend, "kitty");
        assert_eq!(config.images.max_height, 20);
        assert!(!config.network.remote_images);
        assert_eq!(config.network.timeout_seconds, 3);
        assert_eq!(config.network.max_download_mb, 8);
    }

    #[test]
    fn partial_config_fills_defaults() {
        let config: ConfigFile = toml::from_str("[layout]\nmargin = 4\n").unwrap();
        assert_eq!(config.theme, "auto");
        assert_eq!(config.layout.margin, 4);
        assert_eq!(config.layout.max_width, 100);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let result: Result<ConfigFile, _> = toml::from_str("no_such_key = 1\n");
        assert!(result.is_err());
    }
}
