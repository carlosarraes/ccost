use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub currency: CurrencyConfig,
    pub pricing: PricingConfig,
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
    pub cache_ttl_hours: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    pub update_source: String, // "manual", "github"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub format: String, // "table" or "json"
    pub colored: bool,
    pub decimal_places: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimezoneConfig {
    pub timezone: String, // e.g., "UTC", "America/New_York"
    pub daily_cutoff_hour: u8, // 0-23, hour when new day starts
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub ttl_hours: u32,
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
                cache_ttl_hours: 24,
            },
            pricing: PricingConfig {
                update_source: "manual".to_string(),
            },
            output: OutputConfig {
                format: "table".to_string(),
                colored: false,
                decimal_places: 2,
            },
            timezone: TimezoneConfig {
                timezone: "UTC".to_string(),
                daily_cutoff_hour: 0,
            },
            cache: CacheConfig {
                ttl_hours: 24,
            },
            sync: SyncConfig {
                auto_export: false,
                export_format: "json".to_string(),
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::default_path()?;
        
        if !config_path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }
        
        let contents = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
        
        let config: Self = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;
        
        Ok(config)
    }
    
    pub fn save(&self) -> Result<()> {
        let config_path = Self::default_path()?;
        
        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }
        
        let contents = self.to_commented_toml()?;
        
        fs::write(&config_path, contents)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;
        
        Ok(())
    }
    
    /// Generate TOML configuration with comprehensive comments explaining all options
    pub fn to_commented_toml(&self) -> Result<String> {
        let mut output = String::new();
        
        output.push_str("# ccost Configuration File\n");
        output.push_str("# Claude Cost Tracking Tool - Configuration Options\n");
        output.push_str("#\n");
        output.push_str("# This file contains all configuration options for ccost.\n");
        output.push_str("# All settings have sensible defaults and can be overridden via CLI flags.\n");
        output.push_str("\n");
        
        // General settings
        output.push_str("# =============================================================================\n");
        output.push_str("# GENERAL SETTINGS\n");
        output.push_str("# =============================================================================\n");
        output.push_str("\n");
        output.push_str("[general]\n");
        output.push_str("# Path to Claude projects directory (where JSONL conversation files are stored)\n");
        output.push_str("# Default: ~/.claude/projects\n");
        output.push_str("# The tool will look for JSONL files in subdirectories of this path\n");
        output.push_str(&format!("claude_projects_path = \"{}\"\n", self.general.claude_projects_path));
        output.push_str("\n");
        output.push_str("# Cost calculation mode - controls how costs are determined:\n");
        output.push_str("#   \"auto\"      - Use embedded costUSD if available, calculate if missing (recommended)\n");
        output.push_str("#   \"calculate\" - Always calculate cost from tokens × pricing (ignores embedded costs)\n");
        output.push_str("#   \"display\"   - Only show embedded costUSD, $0.00 if missing\n");
        output.push_str("# Most users should use \"auto\" which provides the most accurate results\n");
        output.push_str(&format!("cost_mode = \"{}\"\n", self.general.cost_mode));
        output.push_str("\n");
        
        // Currency settings
        output.push_str("# =============================================================================\n");
        output.push_str("# CURRENCY SETTINGS\n");
        output.push_str("# =============================================================================\n");
        output.push_str("\n");
        output.push_str("[currency]\n");
        output.push_str("# Default currency for displaying costs\n");
        output.push_str("# Supported: USD, EUR, GBP, JPY, CAD, AUD, CHF, CNY, and other major currencies\n");
        output.push_str("# Exchange rates are fetched from the European Central Bank (ECB) API\n");
        output.push_str("# USD costs from Claude are converted to your preferred currency\n");
        output.push_str(&format!("default_currency = \"{}\"\n", self.currency.default_currency));
        output.push_str("\n");
        output.push_str("# How long to cache exchange rates (in hours)\n");
        output.push_str("# Exchange rates are cached locally to reduce API calls\n");
        output.push_str("# Default: 24 hours (rates typically update daily)\n");
        output.push_str("# Set to 0 to always fetch fresh rates (not recommended)\n");
        output.push_str(&format!("cache_ttl_hours = {}\n", self.currency.cache_ttl_hours));
        output.push_str("\n");
        
        // Pricing settings
        output.push_str("# =============================================================================\n");
        output.push_str("# PRICING SETTINGS\n");
        output.push_str("# =============================================================================\n");
        output.push_str("\n");
        output.push_str("[pricing]\n");
        output.push_str("# Source for model pricing updates:\n");
        output.push_str("#   \"manual\" - Only use manually configured pricing (default)\n");
        output.push_str("#   \"github\" - Fetch pricing from GitHub repository (not implemented)\n");
        output.push_str("# Note: Anthropic updates pricing directly in Claude Code, so external\n");
        output.push_str("# pricing sources are typically unnecessary\n");
        output.push_str(&format!("update_source = \"{}\"\n", self.pricing.update_source));
        output.push_str("\n");
        
        // Output settings
        output.push_str("# =============================================================================\n");
        output.push_str("# OUTPUT SETTINGS\n");
        output.push_str("# =============================================================================\n");
        output.push_str("\n");
        output.push_str("[output]\n");
        output.push_str("# Default output format:\n");
        output.push_str("#   \"table\" - Human-readable tables (recommended for terminal use)\n");
        output.push_str("#   \"json\"  - Machine-readable JSON (good for scripting)\n");
        output.push_str("# Can be overridden with --json flag\n");
        output.push_str(&format!("format = \"{}\"\n", self.output.format));
        output.push_str("\n");
        output.push_str("# Enable colored table output by default\n");
        output.push_str("# true  - Use colors and enhanced styling for tables\n");
        output.push_str("# false - Plain ASCII tables without colors\n");
        output.push_str("# Can be overridden with --colored flag\n");
        output.push_str(&format!("colored = {}\n", self.output.colored));
        output.push_str("\n");
        output.push_str("# Number of decimal places for currency display\n");
        output.push_str("# Default: 2 (e.g., $12.34)\n");
        output.push_str("# Increase for more precision, decrease for cleaner display\n");
        output.push_str(&format!("decimal_places = {}\n", self.output.decimal_places));
        output.push_str("\n");
        
        // Timezone settings
        output.push_str("# =============================================================================\n");
        output.push_str("# TIMEZONE SETTINGS\n");
        output.push_str("# =============================================================================\n");
        output.push_str("\n");
        output.push_str("[timezone]\n");
        output.push_str("# Your timezone for date filtering and daily cutoffs\n");
        output.push_str("# Examples: \"UTC\", \"America/New_York\", \"Europe/London\", \"Asia/Tokyo\"\n");
        output.push_str("# Use `timedatectl list-timezones` on Linux to see available timezones\n");
        output.push_str("# Affects when \"today\", \"yesterday\" etc. start and end\n");
        output.push_str(&format!("timezone = \"{}\"\n", self.timezone.timezone));
        output.push_str("\n");
        output.push_str("# Hour of day when a new \"day\" begins (0-23)\n");
        output.push_str("# Default: 0 (midnight)\n");
        output.push_str("# Useful if you work late nights and want \"today\" to start at e.g. 6 AM\n");
        output.push_str("# Example: Set to 6 to make \"today\" start at 6 AM instead of midnight\n");
        output.push_str(&format!("daily_cutoff_hour = {}\n", self.timezone.daily_cutoff_hour));
        output.push_str("\n");
        
        // Cache settings
        output.push_str("# =============================================================================\n");
        output.push_str("# CACHE SETTINGS\n");
        output.push_str("# =============================================================================\n");
        output.push_str("\n");
        output.push_str("[cache]\n");
        output.push_str("# Time-to-live for cached data (in hours)\n");
        output.push_str("# Used for caching exchange rates and other data to reduce API calls\n");
        output.push_str("# Default: 24 hours\n");
        output.push_str("# Set to 0 to always fetch fresh data (not recommended)\n");
        output.push_str(&format!("ttl_hours = {}\n", self.cache.ttl_hours));
        output.push_str("\n");
        
        // Sync settings
        output.push_str("# =============================================================================\n");
        output.push_str("# SYNC SETTINGS\n");
        output.push_str("# =============================================================================\n");
        output.push_str("\n");
        output.push_str("[sync]\n");
        output.push_str("# Whether to automatically export data after processing\n");
        output.push_str("# true  - Automatically create export files after commands\n");
        output.push_str("# false - Only export when explicitly requested (default)\n");
        output.push_str(&format!("auto_export = {}\n", self.sync.auto_export));
        output.push_str("\n");
        output.push_str("# Default format for export operations\n");
        output.push_str("#   \"json\" - Export to JSON format (recommended for data interchange)\n");
        output.push_str("#   \"csv\"  - Export to CSV format (good for spreadsheets)\n");
        output.push_str(&format!("export_format = \"{}\"\n", self.sync.export_format));
        output.push_str("\n");
        
        // Final notes
        output.push_str("# =============================================================================\n");
        output.push_str("# USAGE NOTES\n");
        output.push_str("# =============================================================================\n");
        output.push_str("#\n");
        output.push_str("# Command-line flags override these configuration values:\n");
        output.push_str("#   --currency EUR        Override default_currency\n");
        output.push_str("#   --timezone UTC         Override timezone\n");
        output.push_str("#   --json                 Override output format\n");
        output.push_str("#   --config /path/file    Use different config file\n");
        output.push_str("#\n");
        output.push_str("# To reset to defaults: ccost config --init\n");
        output.push_str("# To modify values:     ccost config --set currency.default_currency EUR\n");
        output.push_str("# To view current:      ccost config --show\n");
        output.push_str("#\n");
        output.push_str("# For more information: https://github.com/anthropics/ccost\n");
        
        Ok(output)
    }
    
    pub fn default_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .context("Failed to determine home directory")?;
        Ok(home.join(".config").join("ccost").join("config.toml"))
    }
    
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "general.claude_projects_path" => self.general.claude_projects_path = value.to_string(),
            "general.cost_mode" => {
                if !["auto", "calculate", "display"].contains(&value) {
                    anyhow::bail!("Invalid cost_mode: {}. Must be 'auto', 'calculate', or 'display'", value);
                }
                self.general.cost_mode = value.to_string();
            }
            "currency.default_currency" => self.currency.default_currency = value.to_string(),
            "currency.cache_ttl_hours" => {
                self.currency.cache_ttl_hours = value.parse()
                    .with_context(|| format!("Invalid TTL value: {}", value))?;
            }
            "pricing.update_source" => {
                if !["manual", "github"].contains(&value) {
                    anyhow::bail!("Invalid update_source: {}. Must be 'manual' or 'github'", value);
                }
                self.pricing.update_source = value.to_string();
            }
            "output.format" => {
                if !["table", "json"].contains(&value) {
                    anyhow::bail!("Invalid output format: {}. Must be 'table' or 'json'", value);
                }
                self.output.format = value.to_string();
            }
            "output.colored" => {
                self.output.colored = value.parse()
                    .with_context(|| format!("Invalid boolean value: {}", value))?;
            }
            "output.decimal_places" => {
                let places: u8 = value.parse()
                    .with_context(|| format!("Invalid decimal places value: {}", value))?;
                if places > 10 {
                    anyhow::bail!("Decimal places must be between 0 and 10");
                }
                self.output.decimal_places = places;
            }
            "timezone.timezone" => self.timezone.timezone = value.to_string(),
            "timezone.daily_cutoff_hour" => {
                let hour: u8 = value.parse()
                    .with_context(|| format!("Invalid hour value: {}", value))?;
                if hour > 23 {
                    anyhow::bail!("Hour must be between 0 and 23");
                }
                self.timezone.daily_cutoff_hour = hour;
            }
            "cache.ttl_hours" => {
                self.cache.ttl_hours = value.parse()
                    .with_context(|| format!("Invalid TTL value: {}", value))?;
            }
            "sync.auto_export" => {
                self.sync.auto_export = value.parse()
                    .with_context(|| format!("Invalid boolean value: {}", value))?;
            }
            "sync.export_format" => {
                if !["json", "csv"].contains(&value) {
                    anyhow::bail!("Invalid export format: {}. Must be 'json' or 'csv'", value);
                }
                self.sync.export_format = value.to_string();
            }
            _ => anyhow::bail!("Unknown configuration key: {}", key),
        }
        Ok(())
    }
}
