use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub theme: String,
    pub alert_retention_days: u32,
    pub dashboard_refresh_secs: u64,
    pub tick_rate_ms: u64,
    pub max_health_log_entries: usize,
    pub ioc: IocConfig,
    pub enrichment: EnrichmentConfig,
    pub triage: TriageConfig,
    pub auto_fetch: AutoFetchConfig,
    pub reports: ReportConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            alert_retention_days: 30,
            dashboard_refresh_secs: 30,
            tick_rate_ms: 250,
            max_health_log_entries: 100,
            ioc: IocConfig::default(),
            enrichment: EnrichmentConfig::default(),
            triage: TriageConfig::default(),
            auto_fetch: AutoFetchConfig::default(),
            reports: ReportConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TriageConfig {
    pub hide_closed_by_default: bool,
    pub require_disposition_on_close: bool,
}

impl Default for TriageConfig {
    fn default() -> Self {
        Self {
            hide_closed_by_default: true,
            require_disposition_on_close: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoFetchConfig {
    pub enabled: bool,
    pub interval_minutes: u32,
}

impl Default for AutoFetchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_minutes: 15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IocConfig {
    pub enabled: bool,
    pub extract_from_raw_json: bool,
    pub max_indicators_per_content_item: usize,
}

impl Default for IocConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            extract_from_raw_json: true,
            max_indicators_per_content_item: 250,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EnrichmentConfig {
    pub enabled: bool,
    pub enrich_only_alert_indicators: bool,
}

impl Default for EnrichmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            enrich_only_alert_indicators: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportConfig {
    pub export_dir: String,
    pub include_iocs_by_default: bool,
    pub include_enrichment_by_default: bool,
    pub include_triage_history_by_default: bool,
    pub include_raw_content_by_default: bool,
    pub redact_secrets: bool,
    pub overwrite_existing: bool,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            export_dir: "./exports".to_string(),
            include_iocs_by_default: true,
            include_enrichment_by_default: true,
            include_triage_history_by_default: true,
            include_raw_content_by_default: false,
            redact_secrets: true,
            overwrite_existing: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
    pub db_file: PathBuf,
}

impl Paths {
    pub fn new() -> Result<Self> {
        let dirs = ProjectDirs::from("", "", "ThreatDeck")
            .context("could not determine project directories")?;
        let config_dir = dirs
            .config_dir()
            .parent()
            .map(|parent| parent.join("ThreatDeck"))
            .unwrap_or_else(|| dirs.config_dir().to_path_buf());
        let data_dir = dirs
            .data_dir()
            .parent()
            .map(|parent| parent.join("ThreatDeck"))
            .unwrap_or_else(|| dirs.data_dir().to_path_buf());
        Ok(Self {
            config_file: config_dir.join("config.toml"),
            db_file: data_dir.join("ThreatDeck.db"),
            config_dir,
            data_dir,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.config_dir)
            .with_context(|| format!("creating config dir: {}", self.config_dir.display()))?;
        fs::create_dir_all(&self.data_dir)
            .with_context(|| format!("creating data dir: {}", self.data_dir.display()))?;
        Ok(())
    }
}

pub fn load_app_config(path: &Path) -> Result<AppConfig> {
    if !path.exists() {
        let default = AppConfig::default();
        save_app_config(path, &default)?;
        return Ok(default);
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading config file: {}", path.display()))?;
    let config: AppConfig = toml::from_str(&content)
        .with_context(|| format!("parsing config file: {}", path.display()))?;
    Ok(config)
}

pub fn save_app_config(path: &Path, config: &AppConfig) -> Result<()> {
    let content = toml::to_string_pretty(config)?;
    fs::write(path, content).with_context(|| format!("writing config file: {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_config_defaults_new_ioc_sections_when_missing() {
        let config: AppConfig = toml::from_str(
            r#"
theme = "ansi"
alert_retention_days = 14
dashboard_refresh_secs = 10
tick_rate_ms = 100
max_health_log_entries = 50
"#,
        )
        .unwrap();

        assert_eq!(config.theme, "ansi");
        assert!(config.ioc.enabled);
        assert!(config.ioc.extract_from_raw_json);
        assert!(config.enrichment.enabled);
    }

    #[test]
    fn auto_fetch_config_defaults() {
        let config: AppConfig = toml::from_str("").unwrap();
        assert!(config.auto_fetch.enabled);
        assert_eq!(config.auto_fetch.interval_minutes, 15);
    }

    #[test]
    fn auto_fetch_config_parses_custom_values() {
        let config: AppConfig = toml::from_str(
            r#"
[auto_fetch]
enabled = false
interval_minutes = 30
"#,
        )
        .unwrap();
        assert!(!config.auto_fetch.enabled);
        assert_eq!(config.auto_fetch.interval_minutes, 30);
    }
}
