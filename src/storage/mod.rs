use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use uuid::Uuid;

pub mod sqlite;
pub mod postgres;
pub mod mssql;

use crate::config::DatabaseConfig;
use crate::uuid_utils::DeviceInfo;

/// Represents the result of a storage operation
#[derive(Debug, Clone)]
pub enum StorageResult {
    Inserted,
    Updated,
    Skipped, // No changes detected
}

/// Trait for database storage backends
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Initialize the storage backend (create tables, etc.)
    async fn initialize(&mut self) -> Result<()>;

    /// Create a table if it doesn't exist with the given schema
    async fn create_table_if_not_exists(&mut self, table_name: &str, schema: &str) -> Result<()>;

    /// Store or update a device
    async fn store_device(&mut self, device: &DeviceInfo) -> Result<StorageResult>;

    /// Store generic endpoint data in a specified table
    async fn store_endpoint_data(&mut self, table_name: &str, data: &[serde_json::Value]) -> Result<usize>;

    /// Store metadata for a device (extra fields not in main table)
    async fn store_device_metadata(
        &mut self,
        device_uuid: Uuid,
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()>;

    /// Get device by UUID
    async fn get_device(&mut self, uuid: Uuid) -> Result<Option<DeviceInfo>>;

    /// Check if device exists and get its hash for change detection
    async fn get_device_hash(&mut self, uuid: Uuid) -> Result<Option<String>>;

    /// Get total device count
    async fn get_device_count(&mut self) -> Result<usize>;

    /// Health check for the storage backend
    async fn health_check(&mut self) -> Result<()>;

    /// Get backend name for logging
    fn backend_name(&self) -> &'static str;
}

/// Storage manager that handles multiple backends
pub struct StorageManager {
    backends: Vec<Box<dyn StorageBackend>>,
}

impl StorageManager {
    /// Create a new storage manager from configuration
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        let mut backends: Vec<Box<dyn StorageBackend>> = Vec::new();
        
        for backend_name in &config.backends {
            match backend_name.as_str() {
                "sqlite" => {
                    let backend = sqlite::SqliteBackend::new(&config.sqlite_path).await?;
                    backends.push(Box::new(backend));
                }
                "postgres" => {
                    if let Some(ref postgres_config) = config.postgres {
                        let backend = postgres::PostgresBackend::new(&postgres_config.connection_string).await?;
                        backends.push(Box::new(backend));
                    } else {
                        log::warn!("PostgreSQL backend requested but no configuration provided");
                    }
                }
                "mssql" => {
                    if let Some(ref mssql_config) = config.mssql {
                        let backend = mssql::MssqlBackend::new(
                            &mssql_config.connection_string,
                            &mssql_config.table_name,
                        ).await?;
                        backends.push(Box::new(backend));
                    } else {
                        log::warn!("MSSQL backend requested but no configuration provided");
                    }
                }
                _ => {
                    log::warn!("Unknown storage backend: {}", backend_name);
                }
            }
        }
        
        if backends.is_empty() {
            return Err(anyhow::anyhow!("No valid storage backends configured"));
        }
        
        Ok(Self { backends })
    }
    
    /// Initialize all backends
    pub async fn initialize(&mut self) -> Result<()> {
        for backend in &mut self.backends {
            log::info!("Initializing {} backend", backend.backend_name());
            backend.initialize().await?;
        }
        Ok(())
    }
    
    /// Store device in all backends
    pub async fn store_device(&mut self, device: &DeviceInfo) -> Result<Vec<StorageResult>> {
        let mut results = Vec::new();

        for backend in &mut self.backends {
            match backend.store_device(device).await {
                Ok(result) => {
                    log::debug!(
                        "Device {} stored in {} backend: {:?}",
                        device.uuid,
                        backend.backend_name(),
                        result
                    );
                    results.push(result);
                }
                Err(e) => {
                    log::error!(
                        "Failed to store device {} in {} backend: {}",
                        device.uuid,
                        backend.backend_name(),
                        e
                    );
                    crate::metrics::DB_ERROR_TOTAL.inc();
                    return Err(e);
                }
            }
        }
        
        Ok(results)
    }
    
    /// Store metadata in all backends
    pub async fn store_device_metadata(
        &mut self,
        device_uuid: Uuid,
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        for backend in &mut self.backends {
            if let Err(e) = backend.store_device_metadata(device_uuid, metadata).await {
                log::error!(
                    "Failed to store metadata for device {} in {} backend: {}",
                    device_uuid,
                    backend.backend_name(),
                    e
                );
                crate::metrics::DB_ERROR_TOTAL.inc();
                return Err(e);
            }
        }
        Ok(())
    }
    
    /// Get device count from first available backend
    pub async fn get_device_count(&mut self) -> Result<usize> {
        if let Some(backend) = self.backends.first_mut() {
            backend.get_device_count().await
        } else {
            Ok(0)
        }
    }

    /// Perform health check on all backends
    pub async fn health_check(&mut self) -> Result<()> {
        for backend in &mut self.backends {
            backend.health_check().await.map_err(|e| {
                anyhow::anyhow!(
                    "Health check failed for {} backend: {}",
                    backend.backend_name(),
                    e
                )
            })?;
        }
        Ok(())
    }
    
    /// Create table in all backends if it doesn't exist
    pub async fn create_table_if_not_exists(&mut self, table_name: &str, schema: &str) -> Result<()> {
        for backend in &mut self.backends {
            backend.create_table_if_not_exists(table_name, schema).await
                .map_err(|e| anyhow::anyhow!(
                    "Failed to create table {} in {} backend: {}",
                    table_name,
                    backend.backend_name(),
                    e
                ))?;
        }
        Ok(())
    }

    /// Store endpoint data in all backends
    pub async fn store_endpoint_data(&mut self, table_name: &str, data: &[serde_json::Value]) -> Result<usize> {
        let mut total_stored = 0;

        for backend in &mut self.backends {
            match backend.store_endpoint_data(table_name, data).await {
                Ok(count) => {
                    log::debug!(
                        "Stored {} items in table {} using {} backend",
                        count,
                        table_name,
                        backend.backend_name()
                    );
                    total_stored = count; // Use the count from the last successful backend
                }
                Err(e) => {
                    log::error!(
                        "Failed to store endpoint data in table {} using {} backend: {}",
                        table_name,
                        backend.backend_name(),
                        e
                    );
                    crate::metrics::DB_ERROR_TOTAL.inc();
                    return Err(e);
                }
            }
        }

        Ok(total_stored)
    }

    /// Get count from a specific table
    pub async fn get_table_count(&mut self, _table_name: &str) -> Result<usize> {
        if let Some(backend) = self.backends.first_mut() {
            // For now, we'll use the device count method as a fallback
            // Each backend implementation should override this for specific tables
            backend.get_device_count().await
        } else {
            Ok(0)
        }
    }

    /// Get list of active backend names
    pub fn get_backend_names(&self) -> Vec<&'static str> {
        self.backends.iter().map(|b| b.backend_name()).collect()
    }
}

/// Common device fields for database storage
#[derive(Debug, Clone)]
pub struct DeviceRecord {
    pub uuid: Uuid,
    pub device_name: Option<String>,
    pub operating_system: Option<String>,
    pub os_version: Option<String>,
    pub serial_number: Option<String>,
    pub imei: Option<String>,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub enrolled_date_time: Option<String>,
    pub last_sync_date_time: Option<String>,
    pub compliance_state: Option<String>,
    pub azure_ad_device_id: Option<String>,
    pub device_hash: String,
    pub fingerprint: String,
}

impl DeviceRecord {
    /// Convert DeviceInfo to DeviceRecord for storage
    pub fn from_device_info(device: &DeviceInfo) -> Self {
        let data = &device.data;
        
        Self {
            uuid: device.uuid,
            device_name: Some(device.name.clone()),
            operating_system: device.os.clone(),
            os_version: data.get("osVersion").and_then(|v| v.as_str()).map(|s| s.to_string()),
            serial_number: data.get("serialNumber").and_then(|v| v.as_str()).map(|s| s.to_string()),
            imei: data.get("imei").and_then(|v| v.as_str()).map(|s| s.to_string()),
            model: data.get("model").and_then(|v| v.as_str()).map(|s| s.to_string()),
            manufacturer: data.get("manufacturer").and_then(|v| v.as_str()).map(|s| s.to_string()),
            enrolled_date_time: data.get("enrolledDateTime").and_then(|v| v.as_str()).map(|s| s.to_string()),
            last_sync_date_time: Some(chrono::Utc::now().to_rfc3339()),
            compliance_state: data.get("complianceState").and_then(|v| v.as_str()).map(|s| s.to_string()),
            azure_ad_device_id: data.get("azureADDeviceId").and_then(|v| v.as_str()).map(|s| s.to_string()),
            device_hash: crate::fingerprint::calculate_device_hash(data),
            fingerprint: device.fingerprint.clone(),
        }
    }
}
