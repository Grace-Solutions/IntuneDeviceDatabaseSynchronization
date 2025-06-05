use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;

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
    pub backup: Option<crate::backup::BackupConfig>,
    pub webhook: Option<crate::webhook::WebhookConfig>,
    #[serde(rename = "rateLimit")]
    pub rate_limit: Option<crate::rate_limiter::RateLimitConfig>,
    #[serde(rename = "mockGraphApi")]
    pub mock_graph_api: Option<crate::mock_graph_api::MockGraphApiConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub backends: Vec<String>,
    #[serde(rename = "tableName", default = "default_table_name")]
    pub table_name: String,
    #[serde(rename = "sqlitePath", default = "default_sqlite_path")]
    pub sqlite_path: String,
    pub postgres: Option<PostgresConfig>,
    pub mssql: Option<MssqlConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    #[serde(rename = "connectionString")]
    pub connection_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MssqlConfig {
    #[serde(rename = "connectionString")]
    pub connection_string: String,
    #[serde(rename = "tableName", default = "default_table_name")]
    pub table_name: String,
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
    "./output/devices.db".to_string()
}

fn default_table_name() -> String {
    "devices".to_string()
}

impl AppConfig {
    pub async fn load() -> Result<Self> {
        // Load from environment variables first
        dotenvy::dotenv().ok();

        // Try to load from config.json
        let mut config = if Path::new("config.json").exists() {
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
                    backends: vec!["sqlite".to_string()],
                    table_name: default_table_name(),
                    sqlite_path: default_sqlite_path(),
                    postgres: None,
                    mssql: None,
                },
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
                    connection_string: mssql_connection,
                    table_name: default_table_name(),
                });
            } else {
                config.database.mssql.as_mut().unwrap().connection_string = mssql_connection;
            }
        }
        if let Ok(mssql_table) = env::var("MSSQL_TABLE_NAME") {
            if let Some(ref mut mssql) = config.database.mssql {
                mssql.table_name = mssql_table;
            }
        }

        // Validate required fields
        if config.client_id.is_empty() {
            return Err(anyhow::anyhow!("GRAPH_CLIENT_ID is required"));
        }
        if config.client_secret.is_empty() {
            return Err(anyhow::anyhow!("GRAPH_CLIENT_SECRET is required"));
        }
        if config.tenant_id.is_empty() {
            return Err(anyhow::anyhow!("GRAPH_TENANT_ID is required"));
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
