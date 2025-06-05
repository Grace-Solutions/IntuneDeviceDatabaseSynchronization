use anyhow::Result;
use async_trait::async_trait;

pub mod sqlite;
pub mod postgres;
pub mod mssql;

use crate::config::DatabaseConfig;

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

    /// Store generic endpoint data in a specified table
    async fn store_endpoint_data(&mut self, table_name: &str, data: &[serde_json::Value]) -> Result<usize>;

    /// Health check for the storage backend
    async fn health_check(&mut self) -> Result<()>;

    /// Get backend name for logging
    fn backend_name(&self) -> &'static str;

    /// Clean up resources and close connections
    async fn cleanup(&mut self) -> Result<()>;
}

/// Storage manager that handles multiple backends
pub struct StorageManager {
    backends: Vec<Box<dyn StorageBackend>>,
}

impl StorageManager {
    /// Create a new storage manager from configuration
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        let mut backends: Vec<Box<dyn StorageBackend>> = Vec::new();

        // Check SQLite backend
        if let Some(ref sqlite_config) = config.sqlite {
            if sqlite_config.enabled {
                let backend = sqlite::SqliteBackend::new(&sqlite_config.database_path).await?;
                backends.push(Box::new(backend));
            }
        }

        // Check PostgreSQL backend
        if let Some(ref postgres_config) = config.postgres {
            if postgres_config.enabled {
                let backend = postgres::PostgresBackend::new(&postgres_config.connection_string).await?;
                backends.push(Box::new(backend));
            }
        }

        // Check MSSQL backend
        if let Some(ref mssql_config) = config.mssql {
            if mssql_config.enabled {
                let backend = mssql::MssqlBackend::new(&mssql_config.connection_string).await?;
                backends.push(Box::new(backend));
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

    /// Get list of active backend names
    pub fn get_backend_names(&self) -> Vec<&'static str> {
        self.backends.iter().map(|b| b.backend_name()).collect()
    }

    /// Clean up all storage backends
    pub async fn cleanup(&mut self) -> Result<()> {
        for backend in &mut self.backends {
            if let Err(e) = backend.cleanup().await {
                log::warn!("Failed to cleanup backend {}: {}", backend.backend_name(), e);
            } else {
                log::info!("Successfully cleaned up backend: {}", backend.backend_name());
            }
        }
        Ok(())
    }
}
