use anyhow::{Context, Result};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::{interval, sleep};

use crate::auth::AuthClient;
use crate::config::AppConfig;
use crate::filter::DeviceOsFilter;
use crate::logging::{log_device_processing, log_sync_operation};
use crate::metrics;
use crate::storage::StorageManager;
use crate::uuid_utils::{DeviceInfo, get_device_name, get_device_os};

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
}

impl SyncService {
    pub async fn new(config: AppConfig) -> Result<Self> {
        let auth_client = AuthClient::new(config.clone());
        let mut storage = StorageManager::new(&config.database).await?;
        storage.initialize().await?;
        
        let os_filter = DeviceOsFilter::new(&config.device_os_filter);
        
        info!("Sync service initialized with backends: {:?}", storage.get_backend_names());
        info!("OS filter configured: {:?}", os_filter.get_filters());
        
        Ok(Self {
            config,
            auth_client,
            storage,
            os_filter,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Starting sync service with interval: {}", self.config.poll_interval);
        
        // Parse poll interval
        let poll_duration = self.config.parse_poll_interval()
            .context("Failed to parse poll interval")?;
        
        let mut interval_timer = interval(poll_duration);
        
        loop {
            interval_timer.tick().await;
            
            if let Err(e) = self.sync_devices().await {
                error!("Sync operation failed: {}", e);
                metrics::SYNC_FAILURE_TOTAL.inc();
                
                // Wait a bit before retrying
                sleep(Duration::from_secs(30)).await;
            }
        }
    }

    async fn sync_devices(&mut self) -> Result<()> {
        let sync_timer = metrics::Timer::new();
        info!("Starting device sync operation");
        
        let mut devices_processed = 0;
        let mut devices_filtered = 0;
        let mut errors = 0;
        
        // Fetch devices from Microsoft Graph
        let devices = self.fetch_all_devices().await?;
        info!("Fetched {} devices from Microsoft Graph", devices.len());
        
        metrics::DEVICES_FETCHED_TOTAL.inc_by(devices.len() as f64);
        
        for device_data in devices {
            match self.process_device(device_data).await {
                Ok(processed) => {
                    devices_processed += 1;
                    if !processed {
                        devices_filtered += 1;
                    }
                }
                Err(e) => {
                    error!("Failed to process device: {}", e);
                    errors += 1;
                }
            }
        }
        
        // Update device count metric
        if let Ok(count) = self.storage.get_device_count().await {
            metrics::DEVICES_CURRENT_COUNT.set(count as f64);
        }
        
        let duration = sync_timer.start.elapsed();
        sync_timer.observe_duration(&metrics::SYNC_DURATION_SECONDS);
        
        if errors == 0 {
            metrics::SYNC_SUCCESS_TOTAL.inc();
        } else {
            metrics::SYNC_FAILURE_TOTAL.inc();
        }
        
        log_sync_operation(
            "Device sync completed",
            devices_processed,
            devices_filtered,
            duration,
            errors,
        );
        
        Ok(())
    }

    async fn fetch_all_devices(&self) -> Result<Vec<HashMap<String, serde_json::Value>>> {
        let mut all_devices = Vec::new();
        let mut next_url = Some("https://graph.microsoft.com/v1.0/deviceManagement/managedDevices".to_string());
        
        while let Some(url) = next_url {
            let response = self.auth_client.make_authenticated_request(&url).await?;
            
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!(
                    "Graph API request failed with status {}: {}",
                    status,
                    error_text
                ));
            }
            
            let graph_response: GraphDeviceResponse = response.json().await
                .context("Failed to parse Graph API response")?;
            
            // Convert each device to HashMap for easier processing
            for device_value in graph_response.value {
                if let serde_json::Value::Object(device_map) = device_value {
                    let device_hash: HashMap<String, serde_json::Value> = device_map.into_iter().collect();
                    all_devices.push(device_hash);
                }
            }
            
            next_url = graph_response.next_link;
            
            // Add a small delay to avoid rate limiting
            if next_url.is_some() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        
        Ok(all_devices)
    }

    async fn process_device(&mut self, device_data: HashMap<String, serde_json::Value>) -> Result<bool> {
        // Extract device information
        let device_name = get_device_name(&device_data);
        let device_os = get_device_os(&device_data);
        
        // Apply OS filter
        if !self.os_filter.should_include_device(Some(&device_name), device_os.as_deref()) {
            return Ok(false); // Device was filtered out
        }
        
        // Create device info
        let device_info = DeviceInfo::from_device_data(device_data.clone())
            .context("Failed to create device info")?;
        
        log_device_processing(
            &device_info.name,
            &device_info.uuid.to_string(),
            "Processing",
            None,
        );
        
        // Store device in all backends
        let results = self.storage.store_device(&device_info).await?;
        
        // Extract metadata (fields not in main device record)
        let metadata = self.extract_metadata(&device_data);
        if !metadata.is_empty() {
            self.storage.store_device_metadata(device_info.uuid, &metadata).await?;
        }
        
        // Log storage results
        let backend_names = self.storage.get_backend_names();
        for (i, result) in results.iter().enumerate() {
            let backend_name = backend_names.get(i).unwrap_or(&"Unknown");
            log_device_processing(
                &device_info.name,
                &device_info.uuid.to_string(),
                &format!("Stored in {} ({:?})", backend_name, result),
                None,
            );
        }
        
        metrics::DEVICES_PROCESSED_TOTAL.inc();
        Ok(true) // Device was processed
    }

    fn extract_metadata(&self, device_data: &HashMap<String, serde_json::Value>) -> HashMap<String, serde_json::Value> {
        let mut metadata = HashMap::new();
        
        // Define known fields that are stored in the main device table
        let known_fields = [
            "id", "uuid", "deviceName", "displayName", "operatingSystem", "osVersion",
            "serialNumber", "imei", "model", "manufacturer", "enrolledDateTime",
            "complianceState", "azureADDeviceId"
        ];
        
        // Extract any fields not in the known list as metadata
        for (key, value) in device_data {
            if !known_fields.contains(&key.as_str()) {
                metadata.insert(key.clone(), value.clone());
            }
        }
        
        metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_metadata() {
        let config = AppConfig {
            client_id: "test".to_string(),
            client_secret: "test".to_string(),
            tenant_id: "test".to_string(),
            poll_interval: "1h".to_string(),
            device_os_filter: vec!["*".to_string()],
            enable_prometheus: false,
            prometheus_port: 9898,
            prometheus_scrape_interval: "1h".to_string(),
            database: crate::config::DatabaseConfig {
                backends: vec!["sqlite".to_string()],
                sqlite_path: ":memory:".to_string(),
                postgres: None,
                mssql: None,
            },
        };
        
        let auth_client = AuthClient::new(config);
        let sync_service = SyncService {
            config: auth_client.config.clone(),
            auth_client,
            storage: StorageManager { backends: vec![] }, // Mock for test
            os_filter: DeviceOsFilter::new(&["*".to_string()]),
        };
        
        let mut device_data = HashMap::new();
        device_data.insert("deviceName".to_string(), json!("Test Device"));
        device_data.insert("operatingSystem".to_string(), json!("Windows"));
        device_data.insert("customField1".to_string(), json!("Custom Value"));
        device_data.insert("customField2".to_string(), json!(123));
        
        let metadata = sync_service.extract_metadata(&device_data);
        
        assert_eq!(metadata.len(), 2);
        assert!(metadata.contains_key("customField1"));
        assert!(metadata.contains_key("customField2"));
        assert!(!metadata.contains_key("deviceName"));
        assert!(!metadata.contains_key("operatingSystem"));
    }
}
