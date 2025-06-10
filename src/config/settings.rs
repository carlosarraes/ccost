use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub currency: CurrencyConfig,
    pub output: OutputConfig,
    pub timezone: TimezoneConfig,
    pub cache: CacheConfig,
    pub sync: SyncConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub claude_projects_path: String,
    pub cost_mode: String, // "auto", "calculate", "display"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyConfig {
    pub default_currency: String,
    pub api_source: String, // "ecb" for European Central Bank
    pub cache_ttl_hours: u32,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub default_format: String, // "table" or "json"
    pub include_project_path: bool,
    pub decimal_places: u8,
    pub colored: bool, // Enable colored table output by default
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimezoneConfig {
    pub timezone: String,
    pub daily_cutoff_hour: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_size_mb: u32,
    pub cleanup_frequency_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub auto_export: bool,
    pub export_format: String, // "json" or "csv"
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                claude_projects_path: "~/.claude/projects".to_string(),
                cost_mode: "auto".to_string(),
            },
            currency: CurrencyConfig {
                default_currency: "USD".to_string(),
                api_source: "ecb".to_string(),
                cache_ttl_hours: 24,
            },
            output: OutputConfig {
                default_format: "table".to_string(),
                include_project_path: false,
                decimal_places: 2,
                colored: false, // No colors by default
            },
            timezone: TimezoneConfig {
                timezone: "America/New_York".to_string(),
                daily_cutoff_hour: 0,
            },
            cache: CacheConfig {
                max_size_mb: 50,
                cleanup_frequency_days: 30,
            },
            sync: SyncConfig {
                auto_export: false,
                export_format: "json".to_string(),
            },
        }
    }
}

impl Config {
    /// Get the default config file path
    pub fn default_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("ccost");
        
        Ok(config_dir.join("config.toml"))
    }

    /// Load config from file, creating default if it doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::default_path()?;
        Self::load_from_path(&config_path)
    }

    /// Load config from specific path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        
        if !path.exists() {
            // Create default config
            let config = Self::default();
            config.save_to_path(path)?;
            return Ok(config);
        }

        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse TOML config: {}", path.display()))?;

        config.validate()?;
        Ok(config)
    }

    /// Save config to default path
    pub fn save(&self) -> Result<()> {
        let config_path = Self::default_path()?;
        self.save_to_path(&config_path)
    }

    /// Save config to specific path
    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        
        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }

        let toml_string = toml::to_string_pretty(self)
            .context("Failed to serialize config to TOML")?;

        fs::write(path, toml_string)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Validate cost mode
        match self.general.cost_mode.as_str() {
            "auto" | "calculate" | "display" => {},
            _ => anyhow::bail!("Invalid cost_mode: must be 'auto', 'calculate', or 'display'"),
        }

        // Validate currency (basic check for 3-letter codes)
        if self.currency.default_currency.len() != 3 {
            anyhow::bail!("Invalid currency: must be 3-letter code (e.g., USD, EUR)");
        }

        // Validate output format
        match self.output.default_format.as_str() {
            "table" | "json" => {},
            _ => anyhow::bail!("Invalid output format: must be 'table' or 'json'"),
        }

        // Validate sync export format
        match self.sync.export_format.as_str() {
            "json" | "csv" => {},
            _ => anyhow::bail!("Invalid export format: must be 'json' or 'csv'"),
        }

        // Validate daily cutoff hour
        if self.timezone.daily_cutoff_hour > 23 {
            anyhow::bail!("Invalid daily_cutoff_hour: must be 0-23");
        }

        // Validate decimal places
        if self.output.decimal_places > 10 {
            anyhow::bail!("Invalid decimal_places: must be 0-10");
        }

        Ok(())
    }

    /// Set a configuration value by key path
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "general.claude_projects_path" => self.general.claude_projects_path = value.to_string(),
            "general.cost_mode" => {
                match value {
                    "auto" | "calculate" | "display" => self.general.cost_mode = value.to_string(),
                    _ => anyhow::bail!("Invalid cost_mode: must be 'auto', 'calculate', or 'display'"),
                }
            },
            "currency.default_currency" => {
                if value.len() != 3 {
                    anyhow::bail!("Invalid currency: must be 3-letter code");
                }
                self.currency.default_currency = value.to_uppercase();
            },
            "currency.cache_ttl_hours" => {
                let hours: u32 = value.parse()
                    .context("Invalid cache_ttl_hours: must be a number")?;
                self.currency.cache_ttl_hours = hours;
            },
            "output.default_format" => {
                match value {
                    "table" | "json" => self.output.default_format = value.to_string(),
                    _ => anyhow::bail!("Invalid output format: must be 'table' or 'json'"),
                }
            },
            "output.decimal_places" => {
                let places: u8 = value.parse()
                    .context("Invalid decimal_places: must be a number")?;
                if places > 10 {
                    anyhow::bail!("Invalid decimal_places: must be 0-10");
                }
                self.output.decimal_places = places;
            },
            "timezone.timezone" => self.timezone.timezone = value.to_string(),
            "timezone.daily_cutoff_hour" => {
                let hour: u8 = value.parse()
                    .context("Invalid daily_cutoff_hour: must be a number")?;
                if hour > 23 {
                    anyhow::bail!("Invalid daily_cutoff_hour: must be 0-23");
                }
                self.timezone.daily_cutoff_hour = hour;
            },
            _ => anyhow::bail!("Unknown configuration key: {}", key),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_config_is_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_string = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_string).unwrap();
        assert_eq!(config.general.cost_mode, parsed.general.cost_mode);
        assert_eq!(config.currency.default_currency, parsed.currency.default_currency);
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        
        let original_config = Config::default();
        original_config.save_to_path(&config_path).unwrap();
        
        let loaded_config = Config::load_from_path(&config_path).unwrap();
        assert_eq!(original_config.general.cost_mode, loaded_config.general.cost_mode);
        assert_eq!(original_config.currency.default_currency, loaded_config.currency.default_currency);
    }

    #[test]
    fn test_config_load_creates_default_if_missing() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("missing_config.toml");
        
        assert!(!config_path.exists());
        
        let config = Config::load_from_path(&config_path).unwrap();
        assert!(config_path.exists());
        assert_eq!(config.general.cost_mode, "auto");
    }

    #[test]
    fn test_invalid_toml_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid_config.toml");
        
        fs::write(&config_path, "invalid toml [").unwrap();
        
        let result = Config::load_from_path(&config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        
        // Valid config should pass
        assert!(config.validate().is_ok());
        
        // Invalid cost mode
        config.general.cost_mode = "invalid".to_string();
        assert!(config.validate().is_err());
        
        // Reset and test invalid currency
        config = Config::default();
        config.currency.default_currency = "INVALID".to_string();
        assert!(config.validate().is_err());
        
        // Reset and test invalid output format
        config = Config::default();
        config.output.default_format = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_set_value() {
        let mut config = Config::default();
        
        // Test setting valid values
        config.set_value("general.cost_mode", "calculate").unwrap();
        assert_eq!(config.general.cost_mode, "calculate");
        
        config.set_value("currency.default_currency", "eur").unwrap();
        assert_eq!(config.currency.default_currency, "EUR");
        
        config.set_value("output.decimal_places", "4").unwrap();
        assert_eq!(config.output.decimal_places, 4);
        
        // Test setting invalid values
        assert!(config.set_value("general.cost_mode", "invalid").is_err());
        assert!(config.set_value("currency.default_currency", "TOOLONG").is_err());
        assert!(config.set_value("output.decimal_places", "20").is_err());
        assert!(config.set_value("unknown.key", "value").is_err());
    }

    #[test]
    fn test_config_directory_creation() {
        let temp_dir = TempDir::new().unwrap();
        let nested_config_path = temp_dir.path().join("nested").join("config.toml");
        
        assert!(!nested_config_path.parent().unwrap().exists());
        
        let config = Config::default();
        config.save_to_path(&nested_config_path).unwrap();
        
        assert!(nested_config_path.exists());
        assert!(nested_config_path.parent().unwrap().exists());
    }
}