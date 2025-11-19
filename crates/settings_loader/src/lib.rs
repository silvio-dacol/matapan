//! # Settings Loader
//!
//! This crate provides centralized settings loading functionality for the net-worth application.
//! It handles loading configuration from JSON files, particularly the main `settings.json` file
//! that contains defaults for currency, normalization settings, and other configuration options.
//!
//! ## Features
//!
//! - Load settings from specified file paths
//! - Load settings from default location (`settings.json`)
//! - Handle optional settings gracefully
//! - Provide fallback mechanisms when settings files are missing
//! - Validation and error handling for malformed settings files
//!
//! ## Usage Examples
//!
//! ```rust,no_run
//! use settings_loader;
//! use std::path::PathBuf;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Load settings from a specific path
//!     let settings = settings_loader::load_settings("config/my_settings.json")?;
//!
//!     // Load from default location
//!     let settings = settings_loader::load_default_settings()?;
//!
//!     // Load optional settings (returns None if file doesn't exist)
//!     let path = Some(PathBuf::from("settings.json"));
//!     let settings = settings_loader::load_optional_settings(path.as_ref())?;
//!     Ok(())
//! }
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use models::SettingsFile;

/// Loads settings from a JSON file
pub fn load_settings<P: AsRef<Path>>(path: P) -> Result<SettingsFile> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Reading settings file: {}", path.display()))?;
    let settings: SettingsFile = serde_json::from_str(&raw)
        .with_context(|| format!("Parsing settings JSON in {}", path.display()))?;
    Ok(settings)
}

/// Loads settings from a default location (settings.json in the current directory)
pub fn load_default_settings() -> Result<SettingsFile> {
    load_settings("settings.json")
}

/// Loads settings from an optional path, returning None if no path is provided
pub fn load_optional_settings(path: Option<&PathBuf>) -> Result<Option<SettingsFile>> {
    path.map(load_settings).transpose()
}
