use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use crate::path_utils;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(rename = "clientId")]
    pub client_id: String,
    #[serde(rename = "clientSecret")]
    pub client_secret: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "pollInterval", default = "default_poll_interval_option")]
    pub poll_interval: Option<String>,
    #[serde(rename = "cronSchedule")]
    pub cron_schedule: Option<String>,
    #[serde(rename = "deviceOsFilter", default = "default_device_os_filter")]
    pub device_os_filter: Vec<String>,
    #[serde(rename = "enablePrometheus", default = "default_enable_prometheus")]
    pub enable_prometheus: bool,
    #[serde(rename = "prometheusPort", default = "default_prometheus_port")]
    pub prometheus_port: u16,
    #[serde(rename = "logLevel", default = "default_log_level")]
    pub log_level: String,
    pub database: DatabaseConfig,
    pub endpoints: Option<crate::endpoint::EndpointsConfig>,
    pub backup: Option<crate::backup::BackupConfig>,
    pub webhook: Option<crate::webhook::WebhookConfig>,
    #[serde(rename = "rateLimit")]
    pub rate_limit: Option<crate::rate_limiter::RateLimitConfig>,
    #[serde(rename = "mockGraphApi")]
    pub mock_graph_api: Option<crate::mock_graph_api::MockGraphApiConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub sqlite: Option<SqliteConfig>,
    pub postgres: Option<PostgresConfig>,
    pub mssql: Option<MssqlConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteConfig {
    pub enabled: bool,
    #[serde(rename = "databasePath", default = "default_sqlite_path")]
    pub database_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub enabled: bool,
    #[serde(rename = "connectionString")]
    pub connection_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MssqlConfig {
    pub enabled: bool,
    #[serde(rename = "connectionString")]
    pub connection_string: String,
}

// Default values
fn default_poll_interval() -> String {
    "1h".to_string()
}

fn default_poll_interval_option() -> Option<String> {
    Some("1h".to_string())
}

fn default_device_os_filter() -> Vec<String> {
    vec!["*".to_string()]
}

fn default_enable_prometheus() -> bool {
    true
}

fn default_prometheus_port() -> u16 {
    9898
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_sqlite_path() -> String {
    "./data/msgraph_data.db".to_string()
}

#[allow(dead_code)]
fn default_table_name() -> String {
    "devices".to_string()
}

impl AppConfig {
    pub async fn load() -> Result<Self> {
        // Load from environment variables first
        dotenvy::dotenv().ok();

        // Try to load config from next to executable first, then current directory
        let config_path = path_utils::get_default_config_path()
            .unwrap_or_else(|_| std::path::PathBuf::from("config.json"));

        let mut config = if config_path.exists() {
            let config_content = tokio::fs::read_to_string(&config_path)
                .await
                .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
            serde_json::from_str::<AppConfig>(&config_content)
                .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?
        } else if Path::new("config.json").exists() {
            // Fallback to current directory for backward compatibility
            let config_content = tokio::fs::read_to_string("config.json")
                .await
                .context("Failed to read config.json")?;
            serde_json::from_str::<AppConfig>(&config_content)
                .context("Failed to parse config.json")?
        } else {
            // Create default config if no file exists
            AppConfig {
                client_id: String::new(),
                client_secret: String::new(),
                tenant_id: String::new(),
                poll_interval: Some(default_poll_interval()),
                cron_schedule: None,
                device_os_filter: default_device_os_filter(),
                enable_prometheus: default_enable_prometheus(),
                prometheus_port: default_prometheus_port(),
                log_level: default_log_level(),
                database: DatabaseConfig {
                    sqlite: Some(SqliteConfig {
                        enabled: true,
                        database_path: default_sqlite_path(),
                    }),
                    postgres: None,
                    mssql: None,
                },
                endpoints: None,
                backup: None,
                webhook: None,
                rate_limit: None,
                mock_graph_api: None,
            }
        };

        // Override with environment variables
        if let Ok(client_id) = env::var("GRAPH_CLIENT_ID") {
            config.client_id = client_id;
        }
        if let Ok(client_secret) = env::var("GRAPH_CLIENT_SECRET") {
            config.client_secret = client_secret;
        }
        if let Ok(tenant_id) = env::var("GRAPH_TENANT_ID") {
            config.tenant_id = tenant_id;
        }
        if let Ok(poll_interval) = env::var("POLL_INTERVAL") {
            config.poll_interval = Some(poll_interval);
        }
        if let Ok(device_os_filter) = env::var("DEVICE_OS_FILTER") {
            config.device_os_filter = device_os_filter
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if let Ok(enable_prometheus) = env::var("ENABLE_PROMETHEUS") {
            config.enable_prometheus = enable_prometheus.parse().unwrap_or(true);
        }
        if let Ok(prometheus_port) = env::var("PROMETHEUS_PORT") {
            config.prometheus_port = prometheus_port.parse().unwrap_or(9898);
        }
        // Remove prometheus_scrape_interval - no longer used
        if let Ok(mssql_connection) = env::var("MSSQL_CONNECTION_STRING") {
            if config.database.mssql.is_none() {
                config.database.mssql = Some(MssqlConfig {
                    enabled: true,
                    connection_string: mssql_connection,
                });
            } else {
                config.database.mssql.as_mut().unwrap().connection_string = mssql_connection;
            }
        }

        // Validate required fields (unless mock API is enabled)
        let mock_api_enabled = config.mock_graph_api.as_ref().map_or(false, |m| m.enabled);

        if !mock_api_enabled {
            if config.client_id.is_empty() {
                return Err(anyhow::anyhow!("GRAPH_CLIENT_ID is required (unless mock API is enabled)"));
            }
            if config.client_secret.is_empty() {
                return Err(anyhow::anyhow!("GRAPH_CLIENT_SECRET is required (unless mock API is enabled)"));
            }
            if config.tenant_id.is_empty() {
                return Err(anyhow::anyhow!("GRAPH_TENANT_ID is required (unless mock API is enabled)"));
            }
        }

        // Ensure device OS filter has at least one entry
        if config.device_os_filter.is_empty() {
            config.device_os_filter = default_device_os_filter();
        }

        Ok(config)
    }

    pub fn parse_poll_interval(&self) -> Result<std::time::Duration> {
        if let Some(ref interval) = self.poll_interval {
            parse_duration(interval)
        } else {
            parse_duration("1h") // Default fallback
        }
    }

    /// Get endpoints configuration with defaults if not specified
    pub fn get_endpoints_config(&self) -> crate::endpoint::EndpointsConfig {
        self.endpoints.clone().unwrap_or_else(|| {
            // Default to just the devices endpoint for backward compatibility
            crate::endpoint::EndpointsConfig {
                endpoints: vec![crate::endpoint::PredefinedEndpoints::managed_devices()],
            }
        })
    }
}

fn parse_duration(input: &str) -> Result<std::time::Duration> {
    let input = input.trim();
    
    if input.ends_with('s') {
        let num: u64 = input[..input.len()-1].parse()?;
        Ok(std::time::Duration::from_secs(num))
    } else if input.ends_with('m') {
        let num: u64 = input[..input.len()-1].parse()?;
        Ok(std::time::Duration::from_secs(num * 60))
    } else if input.ends_with('h') {
        let num: u64 = input[..input.len()-1].parse()?;
        Ok(std::time::Duration::from_secs(num * 3600))
    } else {
        // Try to parse as seconds
        let num: u64 = input.parse()?;
        Ok(std::time::Duration::from_secs(num))
    }
}
