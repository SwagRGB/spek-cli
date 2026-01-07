use serde::Deserialize;
use std::path::PathBuf;
use directories::ProjectDirs;
use std::fs;
use anyhow::{Result, Context};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub colors: ColorConfig,
    pub font_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ColorConfig {
    pub stops: Vec<ColorStop>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ColorStop {
    pub position: f32, // 0.0 to 1.0
    pub color: String, // Hex code "#RRGGBB"
}

impl Default for Config {
    fn default() -> Self {
        Config {
            colors: ColorConfig {
                stops: default_audacity_palette(),
            },
            font_path: None,
        }
    }
}

fn default_audacity_palette() -> Vec<ColorStop> {
    vec![
        ColorStop { position: 0.0, color: "#000000".to_string() },   // Black
        ColorStop { position: 0.4, color: "#0000FF".to_string() },   // Blue
        ColorStop { position: 0.7, color: "#FF0000".to_string() },   // Red
        ColorStop { position: 1.0, color: "#FFFFFF".to_string() },   // White
    ]
}

pub fn load_config() -> Result<Config> {
    if let Some(proj_dirs) = ProjectDirs::from("", "", "spek") {
        let config_path = proj_dirs.config_dir().join("config.toml");
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file at {:?}", config_path))?;
            let config: Config = toml::from_str(&content)
                .context("Failed to parse config file")?;
            return Ok(config);
        }
    }
    
    // If no config or no directories, return default
    Ok(Config::default())
}
