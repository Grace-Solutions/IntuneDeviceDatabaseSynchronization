use anyhow::{Context, Result};
use log::{error, info, warn, debug};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::{interval, sleep};

use crate::auth::AuthClient;
use crate::config::AppConfig;
use crate::endpoint::{EndpointManager, EndpointConfig};
use crate::filter::DeviceOsFilter;
use crate::metrics;
use crate::storage::StorageManager;
use crate::uuid_utils::{get_device_name, get_device_os};

#[derive(Debug, Deserialize, Serialize)]
struct GraphDeviceResponse {
    #[serde(rename = "@odata.context")]
    context: Option<String>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
    value: Vec<serde_json::Value>,
}

pub struct SyncService {
    config: AppConfig,
    auth_client: AuthClient,
    storage: StorageManager,
    os_filter: DeviceOsFilter,
    endpoint_manager: EndpointManager,
}

impl SyncService {
    pub async fn new(config: AppConfig) -> Result<Self> {
        let auth_client = AuthClient::new(config.clone());
        let mut storage = StorageManager::new(&config.database).await?;
        storage.initialize().await?;

        let os_filter = DeviceOsFilter::new(&config.device_os_filter);

        // Get endpoints configuration
        let endpoints_config = config.get_endpoints_config();
        endpoints_config.validate().context("Invalid endpoints configuration")?;

        let endpoint_manager = EndpointManager::new(endpoints_config, auth_client.clone());

        info!("Sync service initialized with backends: {:?}", storage.get_backend_names());
        info!("OS filter configured: {:?}", os_filter.get_filters());
        info!("Endpoints configured: {:?}", endpoint_manager.get_enabled_endpoints().iter().map(|e| &e.name).collect::<Vec<_>>());

        Ok(Self {
            config,
            auth_client,
            storage,
            os_filter,
            endpoint_manager,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Starting sync service with interval: {:?}", self.config.poll_interval);

        // Parse poll interval
        let poll_duration = self.config.parse_poll_interval()
            .context("Failed to parse poll interval")?;

        let mut interval_timer = interval(poll_duration);

        loop {
            interval_timer.tick().await;

            if let Err(e) = self.sync_all_endpoints().await {
                error!("Sync operation failed: {}", e);
                metrics::SYNC_FAILURE_TOTAL.inc();

                // Wait a bit before retrying
                sleep(Duration::from_secs(30)).await;
            }
        }
    }

    async fn sync_all_endpoints(&mut self) -> Result<()> {
        let sync_timer = metrics::Timer::new();
        info!("Starting multi-endpoint sync operation");

        let enabled_endpoints: Vec<_> = self.endpoint_manager.get_enabled_endpoints()
            .into_iter()
            .cloned()
            .collect();

        if enabled_endpoints.is_empty() {
            warn!("No endpoints are enabled for synchronization");
            return Ok(());
        }

        let mut total_processed = 0;
        let mut total_errors = 0;

        for endpoint in enabled_endpoints {
            match self.sync_endpoint(&endpoint).await {
                Ok(processed) => {
                    total_processed += processed;
                    info!("Successfully synced {} items from endpoint: {}", processed, endpoint.name);
                }
                Err(e) => {
                    error!("Failed to sync endpoint {}: {}", endpoint.name, e);
                    total_errors += 1;
                }
            }

            // Small delay between endpoints to avoid rate limiting
            sleep(Duration::from_millis(500)).await;
        }

        let duration = sync_timer.start.elapsed();
        sync_timer.observe_duration(&metrics::SYNC_DURATION_SECONDS);

        if total_errors == 0 {
            metrics::SYNC_SUCCESS_TOTAL.inc();
        } else {
            metrics::SYNC_FAILURE_TOTAL.inc();
        }

        info!(
            "Multi-endpoint sync completed: {} items processed, {} errors, duration: {:?}",
            total_processed, total_errors, duration
        );

        Ok(())
    }

    async fn sync_endpoint(&mut self, endpoint: &EndpointConfig) -> Result<usize> {
        info!("Syncing endpoint: {} -> {}", endpoint.name, endpoint.table_name);

        // Ensure table exists for this endpoint
        self.ensure_endpoint_table_exists(endpoint).await?;

        // Fetch data from the endpoint
        let data = self.endpoint_manager.fetch_all_endpoint_data(endpoint).await?;
        info!("Fetched {} items from endpoint: {}", data.len(), endpoint.name);

        if data.is_empty() {
            return Ok(0);
        }

        // Apply device filtering if this is the devices endpoint
        let filtered_data = if endpoint.name == "devices" {
            self.apply_device_filtering(&data)?
        } else {
            data
        };

        // Store data in the database
        let stored_count = self.storage.store_endpoint_data(&endpoint.table_name, &filtered_data).await?;

        info!("Stored {} items in table: {}", stored_count, endpoint.table_name);

        // Update metrics
        metrics::DEVICES_FETCHED_TOTAL.inc_by(filtered_data.len() as f64);
        metrics::DEVICES_PROCESSED_TOTAL.inc_by(stored_count as f64);

        Ok(stored_count)
    }

    async fn ensure_endpoint_table_exists(&mut self, endpoint: &EndpointConfig) -> Result<()> {
        // Create a generic table schema for the endpoint
        let schema = self.generate_table_schema(&endpoint.table_name);
        self.storage.create_table_if_not_exists(&endpoint.table_name, &schema).await?;
        Ok(())
    }

    fn generate_table_schema(&self, table_name: &str) -> String {
        // Generate a generic schema that can accommodate any JSON data
        // This is database-specific, but we'll use a SQLite-compatible format as the base
        format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id TEXT PRIMARY KEY,
                data TEXT,
                last_sync_date_time TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            table_name
        )
    }

    fn apply_device_filtering(&self, data: &[serde_json::Value]) -> Result<Vec<serde_json::Value>> {
        let mut filtered_data = Vec::new();

        for item in data {
            // Convert to HashMap for easier processing
            if let Some(device_map) = item.as_object() {
                let device_hash: HashMap<String, serde_json::Value> = device_map.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                let device_name = get_device_name(&device_hash);
                let device_os = get_device_os(&device_hash);

                // Apply OS filter
                if self.os_filter.should_include_device(Some(&device_name), device_os.as_deref()) {
                    filtered_data.push(item.clone());
                } else {
                    debug!("Filtered out device: {} (OS: {:?})", device_name, device_os);
                }
            } else {
                // If it's not an object, include it anyway
                filtered_data.push(item.clone());
            }
        }

        info!("Applied device filtering: {} -> {} items", data.len(), filtered_data.len());
        Ok(filtered_data)
    }

    /// Legacy method for backward compatibility - now uses endpoint-based approach
    async fn process_device(&mut self, device_data: HashMap<String, serde_json::Value>) -> Result<bool> {
        warn!("process_device is deprecated - use endpoint-based sync instead");

        // Convert to JSON value for endpoint processing
        let json_value = serde_json::to_value(device_data)?;

        // Find the devices endpoint
        let enabled_endpoints = self.endpoint_manager.get_enabled_endpoints();
        let devices_endpoint = enabled_endpoints
            .iter()
            .find(|e| e.name == "devices")
            .ok_or_else(|| anyhow::anyhow!("Devices endpoint not found or not enabled"))?;

        // Process as single-item endpoint data
        let data = vec![json_value];
        let filtered_data = self.apply_device_filtering(&data)?;

        if filtered_data.is_empty() {
            return Ok(false); // Device was filtered out
        }

        // Store in the devices table
        let stored_count = self.storage.store_endpoint_data(&devices_endpoint.table_name, &filtered_data).await?;

        metrics::DEVICES_PROCESSED_TOTAL.inc();
        Ok(stored_count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_device_filtering() {
        let config = AppConfig {
            client_id: "test".to_string(),
            client_secret: "test".to_string(),
            tenant_id: "test".to_string(),
            poll_interval: Some("1h".to_string()),
            cron_schedule: None,
            device_os_filter: vec!["Windows".to_string()],
            enable_prometheus: false,
            prometheus_port: 9898,
            log_level: "info".to_string(),
            database: crate::config::DatabaseConfig {
                sqlite: Some(crate::config::SqliteConfig {
                    enabled: true,
                    database_path: ":memory:".to_string(),
                }),
                postgres: None,
                mssql: None,
            },
            endpoints: None,
            backup: None,
            webhook: None,
            rate_limit: None,
            mock_graph_api: None,
        };

        let auth_client = AuthClient::new(config.clone());
        let mut storage_manager = StorageManager::new(&config.database).await.unwrap();
        storage_manager.initialize().await.unwrap();

        let endpoints_config = config.get_endpoints_config();
        let endpoint_manager = EndpointManager::new(endpoints_config, auth_client.clone());

        let sync_service = SyncService {
            config: config.clone(),
            auth_client,
            storage: storage_manager,
            os_filter: DeviceOsFilter::new(&["Windows".to_string()]),
            endpoint_manager,
        };

        let test_data = vec![
            json!({
                "deviceName": "Windows Device",
                "operatingSystem": "Windows"
            }),
            json!({
                "deviceName": "Mac Device",
                "operatingSystem": "macOS"
            }),
            json!({
                "deviceName": "Another Windows Device",
                "operatingSystem": "Windows"
            })
        ];

        let filtered_data = sync_service.apply_device_filtering(&test_data).unwrap();

        // Should only include Windows devices
        assert_eq!(filtered_data.len(), 2);
        for device in &filtered_data {
            assert_eq!(device["operatingSystem"], "Windows");
        }
    }
}
