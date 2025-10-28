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
//! // Load settings from a specific path
//! let settings = settings_loader::load_settings("config/my_settings.json")?;
//!
//! // Load from default location
//! let settings = settings_loader::load_default_settings()?;
//!
//! // Load optional settings (returns None if file doesn't exist)
//! let path = Some(PathBuf::from("settings.json"));
//! let settings = settings_loader::load_optional_settings(path.as_ref())?;
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use models::Settings;

/// Loads settings from a JSON file
pub fn load_settings<P: AsRef<Path>>(path: P) -> Result<Settings> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Reading settings file: {}", path.display()))?;
    let settings: Settings = serde_json::from_str(&raw)
        .with_context(|| format!("Parsing settings JSON in {}", path.display()))?;
    Ok(settings)
}

/// Loads settings from a default location (settings.json in the current directory)
pub fn load_default_settings() -> Result<Settings> {
    load_settings("settings.json")
}

/// Loads settings from an optional path, returning None if no path is provided
pub fn load_optional_settings(path: Option<&PathBuf>) -> Result<Option<Settings>> {
    match path {
        Some(settings_path) => Ok(Some(load_settings(settings_path)?)),
        None => Ok(None),
    }
}

/// Tries to load settings from the provided path, falling back to default location if the path is None
/// or if the file doesn't exist. Returns None only if no settings file is found anywhere.
pub fn load_settings_with_fallback(path: Option<&PathBuf>) -> Result<Option<Settings>> {
    // First try the provided path
    if let Some(settings_path) = path {
        match load_settings(settings_path) {
            Ok(settings) => return Ok(Some(settings)),
            Err(_) => {
                // If the provided path fails, fall back to checking default location
            }
        }
    }

    // Try default location
    match load_default_settings() {
        Ok(settings) => Ok(Some(settings)),
        Err(_) => Ok(None), // No settings file found, return None
    }
}

/// Checks if a settings file exists at the given path
pub fn settings_file_exists<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().exists() && path.as_ref().is_file()
}

/// Checks if the default settings file (settings.json) exists
pub fn default_settings_exist() -> bool {
    settings_file_exists("settings.json")
}
