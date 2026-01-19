use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use directories::ProjectDirs;
use std::fs;
use anyhow::{Result, Context};
use crate::Palette;

/// Main configuration struct
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    /// Default settings for CLI flags
    #[serde(default)]
    pub defaults: DefaultSettings,
    
    /// Color palette configuration
    #[serde(default)]
    pub colors: ColorConfig,
    
    /// Path to custom font (optional)
    pub font_path: Option<PathBuf>,
}

/// Default values for CLI flags (can be overridden by command line)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DefaultSettings {
    /// Default image width in pixels
    #[serde(default = "default_width")]
    pub width: u32,
    
    /// Default image height in pixels
    #[serde(default = "default_height")]
    pub height: u32,
    
    /// Use logarithmic frequency scale by default
    #[serde(default)]
    pub log_scale: bool,
    
    /// Default color palette
    #[serde(default = "default_palette")]
    pub palette: String,
    
    /// Show spectral rolloff indicator by default
    #[serde(default)]
    pub rolloff: bool,
    
    /// Verbose mode by default
    #[serde(default)]
    pub verbose: bool,
}

fn default_width() -> u32 { 2048 }
fn default_height() -> u32 { 1024 }
fn default_palette() -> String { "audacity".to_string() }

impl Default for DefaultSettings {
    fn default() -> Self {
        DefaultSettings {
            width: default_width(),
            height: default_height(),
            log_scale: false,
            palette: default_palette(),
            rolloff: false,
            verbose: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ColorConfig {
    #[serde(default = "default_color_stops")]
    pub stops: Vec<ColorStop>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ColorStop {
    pub position: f32, // 0.0 to 1.0
    pub color: String, // Hex code "#RRGGBB"
}

fn default_color_stops() -> Vec<ColorStop> {
    get_palette_stops_by_name("audacity")
}

impl Default for Config {
    fn default() -> Self {
        Config {
            defaults: DefaultSettings::default(),
            colors: ColorConfig::default(),
            font_path: None,
        }
    }
}

/// Get color stops for a given palette enum
pub fn get_palette_stops(palette: Palette) -> Vec<ColorStop> {
    match palette {
        Palette::Audacity => audacity_palette(),
        Palette::Magma => magma_palette(),
        Palette::Viridis => viridis_palette(),
        Palette::Inferno => inferno_palette(),
        Palette::Grayscale => grayscale_palette(),
    }
}

/// Get color stops by palette name string
pub fn get_palette_stops_by_name(name: &str) -> Vec<ColorStop> {
    match name.to_lowercase().as_str() {
        "audacity" => audacity_palette(),
        "magma" => magma_palette(),
        "viridis" => viridis_palette(),
        "inferno" => inferno_palette(),
        "grayscale" => grayscale_palette(),
        _ => audacity_palette(),
    }
}

fn audacity_palette() -> Vec<ColorStop> {
    vec![
        ColorStop { position: 0.0, color: "#000000".to_string() },
        ColorStop { position: 0.4, color: "#0000FF".to_string() },
        ColorStop { position: 0.7, color: "#FF0000".to_string() },
        ColorStop { position: 1.0, color: "#FFFFFF".to_string() },
    ]
}

fn magma_palette() -> Vec<ColorStop> {
    vec![
        ColorStop { position: 0.00, color: "#000004".to_string() },
        ColorStop { position: 0.25, color: "#3B0F70".to_string() },
        ColorStop { position: 0.50, color: "#8C2981".to_string() },
        ColorStop { position: 0.75, color: "#DE4968".to_string() },
        ColorStop { position: 0.90, color: "#FE9F6D".to_string() },
        ColorStop { position: 1.00, color: "#FCFDBF".to_string() },
    ]
}

fn viridis_palette() -> Vec<ColorStop> {
    vec![
        ColorStop { position: 0.00, color: "#440154".to_string() },
        ColorStop { position: 0.25, color: "#3B528B".to_string() },
        ColorStop { position: 0.50, color: "#21908C".to_string() },
        ColorStop { position: 0.75, color: "#5DC863".to_string() },
        ColorStop { position: 1.00, color: "#FDE725".to_string() },
    ]
}

fn inferno_palette() -> Vec<ColorStop> {
    vec![
        ColorStop { position: 0.00, color: "#000004".to_string() },
        ColorStop { position: 0.25, color: "#420A68".to_string() },
        ColorStop { position: 0.50, color: "#932667".to_string() },
        ColorStop { position: 0.75, color: "#DD513A".to_string() },
        ColorStop { position: 0.90, color: "#FCA50A".to_string() },
        ColorStop { position: 1.00, color: "#FCFFA4".to_string() },
    ]
}

fn grayscale_palette() -> Vec<ColorStop> {
    vec![
        ColorStop { position: 0.0, color: "#000000".to_string() },
        ColorStop { position: 1.0, color: "#FFFFFF".to_string() },
    ]
}

/// Get the config directory path
pub fn get_config_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "spek").map(|p| p.config_dir().to_path_buf())
}

/// Get the config file path
pub fn get_config_path() -> Option<PathBuf> {
    get_config_dir().map(|p| p.join("config.toml"))
}

/// Load config, creating default if it doesn't exist
pub fn load_config() -> Result<Config> {
    let config_path = match get_config_path() {
        Some(p) => p,
        None => return Ok(Config::default()),
    };
    
    // Create config directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }
    }
    
    // If config doesn't exist, create default one
    if !config_path.exists() {
        create_default_config(&config_path)?;
    }
    
    // Read and parse config
    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {:?}", config_path))?;
    
    let config: Config = toml::from_str(&content)
        .with_context(|| "Failed to parse config file")?;
    
    Ok(config)
}

/// Create a default config file with helpful comments
fn create_default_config(path: &PathBuf) -> Result<()> {
    let default_config = r##"# ╔═══════════════════════════════════════════════════════════════════════════╗
# ║                        Spek-CLI Configuration                             ║
# ╚═══════════════════════════════════════════════════════════════════════════╝
#
# This file configures default settings for spek-cli.
# Command-line flags always override these settings.
#
# Location: ~/.config/spek/config.toml

# ─────────────────────────────────────────────────────────────────────────────
# DEFAULT SETTINGS
# ─────────────────────────────────────────────────────────────────────────────
# These are the default values used when you run spek-cli without flags.
# Any flag you pass on the command line will override these.

[defaults]
# Image dimensions (in pixels)
width = 2048
height = 1024

# Frequency scale: false = linear (default), true = logarithmic
# Use --log on command line to override
log_scale = false

# Default color palette
# Options: "audacity", "magma", "viridis", "inferno", "grayscale"
palette = "audacity"

# Show spectral rolloff indicator line
# The rolloff shows where 85% of the audio energy is concentrated.
# Useful for detecting lossy compression (MP3s typically cut off around 16kHz)
rolloff = false

# Show timing statistics after processing
verbose = false

# ─────────────────────────────────────────────────────────────────────────────
# CUSTOM FONT (optional)
# ─────────────────────────────────────────────────────────────────────────────
# Uncomment and set to use a specific font for axis labels.
# If not set, spek-cli will auto-detect a system font.
#
# font_path = "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf"

# ─────────────────────────────────────────────────────────────────────────────
# CUSTOM COLOR PALETTE (optional)
# ─────────────────────────────────────────────────────────────────────────────
# You can define your own color gradient here.
# Each stop has a position (0.0 to 1.0) and a hex color.
# Uncomment and modify to use a custom palette.
#
# [colors]
# stops = [
#     { position = 0.0, color = "#000000" },  # Silence (black)
#     { position = 0.3, color = "#1a0a3e" },  # Deep purple
#     { position = 0.5, color = "#4a1c7a" },  # Purple
#     { position = 0.7, color = "#c43c6e" },  # Pink
#     { position = 0.9, color = "#f9a03f" },  # Orange
#     { position = 1.0, color = "#fcffc0" },  # Light yellow (max)
# ]
"##;
    
    fs::write(path, default_config)
        .with_context(|| format!("Failed to write default config to {:?}", path))?;
    
    Ok(())
}

/// Parse palette name to enum
pub fn parse_palette(name: &str) -> Palette {
    match name.to_lowercase().as_str() {
        "audacity" => Palette::Audacity,
        "magma" => Palette::Magma,
        "viridis" => Palette::Viridis,
        "inferno" => Palette::Inferno,
        "grayscale" => Palette::Grayscale,
        _ => Palette::Audacity,
    }
}
