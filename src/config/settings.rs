use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub currency: CurrencyConfig,
    pub output: OutputConfig,
    pub timezone: TimezoneConfig,
    pub pricing: PricingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub claude_projects_path: String,
    pub cost_mode: String, // "auto", "calculate", "display"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyConfig {
    pub default_currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub format: String, // "table" or "json"
    pub colored: bool,
    pub decimal_places: u8,
    pub date_format: String, // Date display format: "yyyy-mm-dd", "dd-mm-yyyy", "mm-dd-yyyy"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimezoneConfig {
    pub timezone: String,      // e.g., "UTC", "America/New_York"
    pub daily_cutoff_hour: u8, // 0-23, hour when new day starts
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    pub source: String,         // "static", "live", "auto"
    pub cache_ttl_minutes: u32, // Cache time-to-live in minutes
    pub offline_fallback: bool, // Whether to fallback to static pricing offline
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
            },
            output: OutputConfig {
                format: "table".to_string(),
                colored: false,
                decimal_places: 2,
                date_format: "yyyy-mm-dd".to_string(),
            },
            timezone: TimezoneConfig {
                timezone: "UTC".to_string(),
                daily_cutoff_hour: 0,
            },
            pricing: PricingConfig {
                source: "auto".to_string(),
                cache_ttl_minutes: 60,
                offline_fallback: true,
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
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
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
        output.push_str(
            "# All settings have sensible defaults and can be overridden via CLI flags.\n",
        );
        output.push('\n');

        // General settings
        output.push_str(
            "# =============================================================================\n",
        );
        output.push_str("# GENERAL SETTINGS\n");
        output.push_str(
            "# =============================================================================\n",
        );
        output.push('\n');
        output.push_str("[general]\n");
        output.push_str(
            "# Path to Claude projects directory (where JSONL conversation files are stored)\n",
        );
        output.push_str("# Default: ~/.claude/projects\n");
        output.push_str("# The tool will look for JSONL files in subdirectories of this path\n");
        output.push_str(&format!(
            "claude_projects_path = \"{}\"\n",
            self.general.claude_projects_path
        ));
        output.push('\n');
        output.push_str("# Cost calculation mode - controls how costs are determined:\n");
        output.push_str("#   \"auto\"      - Use embedded costUSD if available, calculate if missing (recommended)\n");
        output.push_str("#   \"calculate\" - Always calculate cost from tokens × pricing (ignores embedded costs)\n");
        output.push_str("#   \"display\"   - Only show embedded costUSD, $0.00 if missing\n");
        output.push_str(
            "# Most users should use \"auto\" which provides the most accurate results\n",
        );
        output.push_str(&format!("cost_mode = \"{}\"\n", self.general.cost_mode));
        output.push('\n');

        // Currency settings
        output.push_str(
            "# =============================================================================\n",
        );
        output.push_str("# CURRENCY SETTINGS\n");
        output.push_str(
            "# =============================================================================\n",
        );
        output.push('\n');
        output.push_str("[currency]\n");
        output.push_str("# Default currency for displaying costs\n");
        output.push_str(
            "# Supported: USD, EUR, GBP, JPY, CAD, AUD, CHF, CNY, and other major currencies\n",
        );
        output.push_str("# Exchange rates are fetched from the European Central Bank (ECB) API\n");
        output.push_str("# USD costs from Claude are converted to your preferred currency\n");
        output.push_str(&format!(
            "default_currency = \"{}\"\n",
            self.currency.default_currency
        ));
        output.push('\n');

        // Output settings
        output.push_str(
            "# =============================================================================\n",
        );
        output.push_str("# OUTPUT SETTINGS\n");
        output.push_str(
            "# =============================================================================\n",
        );
        output.push('\n');
        output.push_str("[output]\n");
        output.push_str("# Default output format:\n");
        output.push_str("#   \"table\" - Human-readable tables (recommended for terminal use)\n");
        output.push_str("#   \"json\"  - Machine-readable JSON (good for scripting)\n");
        output.push_str("# Can be overridden with --json flag\n");
        output.push_str(&format!("format = \"{}\"\n", self.output.format));
        output.push('\n');
        output.push_str("# Enable colored table output by default\n");
        output.push_str("# true  - Use colors and enhanced styling for tables\n");
        output.push_str("# false - Plain ASCII tables without colors\n");
        output.push_str("# Can be overridden with --colored flag\n");
        output.push_str(&format!("colored = {}\n", self.output.colored));
        output.push('\n');
        output.push_str("# Number of decimal places for currency display\n");
        output.push_str("# Default: 2 (e.g., $12.34)\n");
        output.push_str("# Increase for more precision, decrease for cleaner display\n");
        output.push_str(&format!(
            "decimal_places = {}\n",
            self.output.decimal_places
        ));
        output.push('\n');
        output.push_str("# Date format for table output display\n");
        output.push_str("# Options:\n");
        output.push_str("#   \"yyyy-mm-dd\" - ISO standard format (2024-03-15) - recommended\n");
        output.push_str("#   \"dd-mm-yyyy\" - European format (15-03-2024)\n");
        output.push_str("#   \"mm-dd-yyyy\" - American format (03-15-2024)\n");
        output.push_str("# Note: JSON output always uses ISO format regardless of this setting\n");
        output.push_str(&format!("date_format = \"{}\"\n", self.output.date_format));
        output.push('\n');

        // Timezone settings
        output.push_str(
            "# =============================================================================\n",
        );
        output.push_str("# TIMEZONE SETTINGS\n");
        output.push_str(
            "# =============================================================================\n",
        );
        output.push('\n');
        output.push_str("[timezone]\n");
        output.push_str("# Your timezone for date filtering and daily cutoffs\n");
        output.push_str(
            "# Examples: \"UTC\", \"America/New_York\", \"Europe/London\", \"Asia/Tokyo\"\n",
        );
        output.push_str("# Use `timedatectl list-timezones` on Linux to see available timezones\n");
        output.push_str("# Affects when \"today\", \"yesterday\" etc. start and end\n");
        output.push_str(&format!("timezone = \"{}\"\n", self.timezone.timezone));
        output.push('\n');
        output.push_str("# Hour of day when a new \"day\" begins (0-23)\n");
        output.push_str("# Default: 0 (midnight)\n");
        output.push_str(
            "# Useful if you work late nights and want \"today\" to start at e.g. 6 AM\n",
        );
        output
            .push_str("# Example: Set to 6 to make \"today\" start at 6 AM instead of midnight\n");
        output.push_str(&format!(
            "daily_cutoff_hour = {}\n",
            self.timezone.daily_cutoff_hour
        ));
        output.push('\n');

        // Pricing settings
        output.push_str(
            "# =============================================================================\n",
        );
        output.push_str("# PRICING SETTINGS\n");
        output.push_str(
            "# =============================================================================\n",
        );
        output.push('\n');
        output.push_str("[pricing]\n");
        output.push_str("# Pricing data source for cost calculations:\n");
        output.push_str("#   \"static\" - Use embedded pricing data (fast, may be outdated)\n");
        output.push_str(
            "#   \"live\"   - Always fetch latest pricing from LiteLLM GitHub repository\n",
        );
        output.push_str("#   \"auto\"   - Use live pricing with offline fallback (recommended)\n");
        output.push_str("# Live pricing provides granular cache costs (creation vs read rates)\n");
        output.push_str(&format!("source = \"{}\"\n", self.pricing.source));
        output.push('\n');
        output.push_str("# Cache duration for live pricing data (in minutes)\n");
        output.push_str("# Default: 60 minutes (1 hour)\n");
        output.push_str("# Reduces API calls while keeping pricing reasonably fresh\n");
        output.push_str(&format!(
            "cache_ttl_minutes = {}\n",
            self.pricing.cache_ttl_minutes
        ));
        output.push('\n');
        output.push_str("# Enable fallback to static pricing when offline or API unavailable\n");
        output.push_str("# true  - Gracefully fallback to embedded pricing (recommended)\n");
        output.push_str("# false - Fail if live pricing cannot be fetched\n");
        output.push_str(&format!(
            "offline_fallback = {}\n",
            self.pricing.offline_fallback
        ));
        output.push('\n');

        // Final notes
        output.push_str(
            "# =============================================================================\n",
        );
        output.push_str("# USAGE NOTES\n");
        output.push_str(
            "# =============================================================================\n",
        );
        output.push_str("#\n");
        output.push_str("# Command-line flags override these configuration values:\n");
        output.push_str("#   --currency EUR        Override default_currency\n");
        output.push_str("#   --timezone UTC         Override timezone\n");
        output.push_str("#   --json                 Override output format\n");
        output.push_str("#   --config /path/file    Use different config file\n");
        output.push_str("#\n");
        output.push_str("# To reset to defaults: ccost config --init\n");
        output
            .push_str("# To modify values:     ccost config --set currency.default_currency EUR\n");
        output.push_str("# To view current:      ccost config --show\n");
        output.push_str("#\n");
        output.push_str("# For more information: https://github.com/carlosarraes/ccost\n");

        Ok(output)
    }

    pub fn default_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to determine home directory")?;
        Ok(home.join(".config").join("ccost").join("config.toml"))
    }

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "general.claude_projects_path" => self.general.claude_projects_path = value.to_string(),
            "general.cost_mode" => {
                if !["auto", "calculate", "display"].contains(&value) {
                    anyhow::bail!(
                        "Invalid cost_mode: {value}. Must be 'auto', 'calculate', or 'display'"
                    );
                }
                self.general.cost_mode = value.to_string();
            }
            "currency.default_currency" => self.currency.default_currency = value.to_string(),
            "output.format" => {
                if !["table", "json"].contains(&value) {
                    anyhow::bail!("Invalid output format: {value}. Must be 'table' or 'json'");
                }
                self.output.format = value.to_string();
            }
            "output.colored" => {
                self.output.colored = value
                    .parse()
                    .with_context(|| format!("Invalid boolean value: {value}"))?;
            }
            "output.decimal_places" => {
                let places: u8 = value
                    .parse()
                    .with_context(|| format!("Invalid decimal places value: {value}"))?;
                if places > 10 {
                    anyhow::bail!("Decimal places must be between 0 and 10");
                }
                self.output.decimal_places = places;
            }
            "timezone.timezone" => self.timezone.timezone = value.to_string(),
            "timezone.daily_cutoff_hour" => {
                let hour: u8 = value
                    .parse()
                    .with_context(|| format!("Invalid hour value: {value}"))?;
                if hour > 23 {
                    anyhow::bail!("Hour must be between 0 and 23");
                }
                self.timezone.daily_cutoff_hour = hour;
            }
            "pricing.source" => {
                if !["static", "live", "auto"].contains(&value) {
                    anyhow::bail!(
                        "Invalid pricing source: {value}. Must be 'static', 'live', or 'auto'"
                    );
                }
                self.pricing.source = value.to_string();
            }
            "pricing.cache_ttl_minutes" => {
                let ttl: u32 = value
                    .parse()
                    .with_context(|| format!("Invalid cache TTL value: {value}"))?;
                if ttl == 0 || ttl > 1440 {
                    anyhow::bail!("Cache TTL must be between 1 and 1440 minutes (24 hours)");
                }
                self.pricing.cache_ttl_minutes = ttl;
            }
            "pricing.offline_fallback" => {
                self.pricing.offline_fallback = value
                    .parse()
                    .with_context(|| format!("Invalid boolean value: {value}"))?;
            }
            _ => anyhow::bail!("Unknown configuration key: {key}"),
        }
        Ok(())
    }
}
