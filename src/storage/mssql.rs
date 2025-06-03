use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use tiberius::{Client, Config, Query};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};
use uuid::Uuid;

use super::{DeviceRecord, StorageBackend, StorageResult};
use crate::logging::log_database_operation;
use crate::metrics;
use crate::uuid_utils::DeviceInfo;

pub struct MssqlBackend {
    client: Client<Compat<TcpStream>>,
    table_name: String,
}

impl MssqlBackend {
    pub async fn new(connection_string: &str, table_name: &str) -> Result<Self> {
        // Parse connection string using tiberius Config
        let config = Config::from_ado_string(connection_string)
            .with_context(|| format!("Failed to parse MSSQL connection string: {}", connection_string))?;

        // Try to connect to the specified database
        let client = match Self::connect_with_config(&config).await {
            Ok(client) => {
                log::info!("Connected to MSSQL database successfully");
                client
            }
            Err(e) => {
                log::warn!("Failed to connect to MSSQL database: {}", e);

                // Try to extract database name and create it if it doesn't exist
                if let Some(db_name) = Self::extract_database_name(&config) {
                    log::info!("Attempting to create database: {}", db_name);
                    Self::create_database_if_not_exists(&config, &db_name).await?;

                    // Retry connection
                    Self::connect_with_config(&config).await
                        .context("Failed to connect to MSSQL after creating database")?
                } else {
                    return Err(e);
                }
            }
        };

        Ok(Self {
            client,
            table_name: table_name.to_string(),
        })
    }

    async fn connect_with_config(config: &Config) -> Result<Client<Compat<TcpStream>>> {
        let tcp = TcpStream::connect(config.get_addr())
            .await
            .context("Failed to connect to MSSQL server")?;
        tcp.set_nodelay(true)?;

        let client = Client::connect(config.clone(), tcp.compat_write())
            .await
            .context("Failed to authenticate with MSSQL server")?;

        Ok(client)
    }

    fn extract_database_name(_config: &Config) -> Option<String> {
        // Extract database name from tiberius Config
        // This is a simplified approach - in practice you might need to access private fields
        // For now, we'll return None and rely on manual database creation
        None
    }

    async fn create_database_if_not_exists(config: &Config, db_name: &str) -> Result<()> {
        // Create a config for master database
        let mut master_config = config.clone();
        master_config.database("master");

        match Self::connect_with_config(&master_config).await {
            Ok(mut client) => {
                let create_db_sql = format!("IF NOT EXISTS (SELECT name FROM sys.databases WHERE name = '{}') CREATE DATABASE [{}]", db_name, db_name);
                match client.simple_query(&create_db_sql).await {
                    Ok(_) => log::info!("Created or verified database: {}", db_name),
                    Err(e) => {
                        log::warn!("Database creation result: {}", e);
                    }
                }
            }
            Err(e) => {
                log::warn!("Could not connect to master database to create target database: {}", e);
            }
        }
        Ok(())
    }

    async fn create_tables(&mut self) -> Result<()> {
        // Main devices table
        let create_devices_sql = format!(
            r#"
            IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='{}' AND xtype='U')
            CREATE TABLE {} (
                uuid UNIQUEIDENTIFIER PRIMARY KEY,
                device_name NVARCHAR(255),
                operating_system NVARCHAR(100),
                os_version NVARCHAR(100),
                serial_number NVARCHAR(100),
                imei NVARCHAR(50),
                model NVARCHAR(100),
                manufacturer NVARCHAR(100),
                enrolled_date_time DATETIME2,
                last_sync_date_time DATETIME2,
                compliance_state NVARCHAR(50),
                azure_ad_device_id NVARCHAR(100),
                device_hash NVARCHAR(64) NOT NULL,
                fingerprint NVARCHAR(64) NOT NULL,
                created_at DATETIME2 DEFAULT GETDATE(),
                updated_at DATETIME2 DEFAULT GETDATE()
            )
            "#,
            self.table_name, self.table_name
        );

        self.client.simple_query(&create_devices_sql).await?;

        // Device metadata table
        let metadata_table = format!("{}_metadata", self.table_name);
        let create_metadata_sql = format!(
            r#"
            IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='{}' AND xtype='U')
            CREATE TABLE {} (
                uuid UNIQUEIDENTIFIER,
                field_name NVARCHAR(255),
                raw_value NVARCHAR(MAX),
                created_at DATETIME2 DEFAULT GETDATE(),
                PRIMARY KEY (uuid, field_name),
                FOREIGN KEY (uuid) REFERENCES {}(uuid) ON DELETE CASCADE
            )
            "#,
            metadata_table, metadata_table, self.table_name
        );

        self.client.simple_query(&create_metadata_sql).await?;

        // Create indexes
        let indexes = vec![
            format!("IF NOT EXISTS (SELECT * FROM sys.indexes WHERE name = 'IX_{}_OS') CREATE NONCLUSTERED INDEX IX_{}_OS ON {}(operating_system)", self.table_name, self.table_name, self.table_name),
            format!("IF NOT EXISTS (SELECT * FROM sys.indexes WHERE name = 'IX_{}_Serial') CREATE NONCLUSTERED INDEX IX_{}_Serial ON {}(serial_number)", self.table_name, self.table_name, self.table_name),
            format!("IF NOT EXISTS (SELECT * FROM sys.indexes WHERE name = 'IX_{}_AzureID') CREATE NONCLUSTERED INDEX IX_{}_AzureID ON {}(azure_ad_device_id)", self.table_name, self.table_name, self.table_name),
            format!("IF NOT EXISTS (SELECT * FROM sys.indexes WHERE name = 'IX_{}_Updated') CREATE NONCLUSTERED INDEX IX_{}_Updated ON {}(updated_at)", self.table_name, self.table_name, self.table_name),
        ];

        for index_sql in indexes {
            if let Err(e) = self.client.simple_query(&index_sql).await {
                log::warn!("Failed to create index: {}", e);
            }
        }

        log::info!("MSSQL tables created/verified successfully");
        Ok(())
    }

    fn parse_timestamp(timestamp_str: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
        timestamp_str.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok()
        })
    }
}

#[async_trait]
impl StorageBackend for MssqlBackend {
    async fn initialize(&mut self) -> Result<()> {
        self.create_tables().await
    }

    async fn store_device(&mut self, device: &DeviceInfo) -> Result<StorageResult> {
        let timer = crate::metrics::Timer::new();
        let record = DeviceRecord::from_device_info(device);

        // Check if device exists and compare hash
        if let Some(existing_hash) = self.get_device_hash(device.uuid).await? {
            if existing_hash == record.device_hash {
                log::debug!("Device {} unchanged, skipping update", device.uuid);
                timer.observe_duration(&metrics::DB_OPERATION_DURATION_SECONDS);
                metrics::DB_SKIP_TOTAL.inc();
                return Ok(StorageResult::Skipped);
            }
        }

        let enrolled_dt = Self::parse_timestamp(record.enrolled_date_time.as_deref());
        let last_sync_dt = Self::parse_timestamp(record.last_sync_date_time.as_deref());

        // Check if device exists
        let exists_query = format!("SELECT COUNT(*) FROM {} WHERE uuid = @P1", self.table_name);
        let mut query = Query::new(&exists_query);
        query.bind(record.uuid);

        let stream = query.query(&mut self.client).await?;
        let row = stream.into_row().await?;
        let exists = if let Some(row) = row {
            let count: i32 = row.get(0).unwrap_or(0);
            count > 0
        } else {
            false
        };

        let elapsed = timer.start.elapsed();

        if exists {
            // Update existing device
            let update_sql = format!(
                r#"
                UPDATE {} SET
                    device_name = @P2,
                    operating_system = @P3,
                    os_version = @P4,
                    serial_number = @P5,
                    imei = @P6,
                    model = @P7,
                    manufacturer = @P8,
                    enrolled_date_time = @P9,
                    last_sync_date_time = @P10,
                    compliance_state = @P11,
                    azure_ad_device_id = @P12,
                    device_hash = @P13,
                    fingerprint = @P14,
                    updated_at = GETDATE()
                WHERE uuid = @P1
                "#,
                self.table_name
            );

            let mut update_query = Query::new(&update_sql);
            update_query.bind(record.uuid);
            update_query.bind(record.device_name.as_deref());
            update_query.bind(record.operating_system.as_deref());
            update_query.bind(record.os_version.as_deref());
            update_query.bind(record.serial_number.as_deref());
            update_query.bind(record.imei.as_deref());
            update_query.bind(record.model.as_deref());
            update_query.bind(record.manufacturer.as_deref());
            update_query.bind(enrolled_dt);
            update_query.bind(last_sync_dt);
            update_query.bind(record.compliance_state.as_deref());
            update_query.bind(record.azure_ad_device_id.as_deref());
            update_query.bind(&record.device_hash);
            update_query.bind(&record.fingerprint);

            update_query.execute(&mut self.client).await?;

            timer.observe_duration(&metrics::DB_OPERATION_DURATION_SECONDS);
            metrics::DB_UPDATE_TOTAL.inc();
            log_database_operation("MSSQL", "UPDATE", Some(1), Some(elapsed));
            Ok(StorageResult::Updated)
        } else {
            // Insert new device
            let insert_sql = format!(
                r#"
                INSERT INTO {} (
                    uuid, device_name, operating_system, os_version, serial_number,
                    imei, model, manufacturer, enrolled_date_time, last_sync_date_time,
                    compliance_state, azure_ad_device_id, device_hash, fingerprint
                ) VALUES (@P1, @P2, @P3, @P4, @P5, @P6, @P7, @P8, @P9, @P10, @P11, @P12, @P13, @P14)
                "#,
                self.table_name
            );

            let mut insert_query = Query::new(&insert_sql);
            insert_query.bind(record.uuid);
            insert_query.bind(record.device_name.as_deref());
            insert_query.bind(record.operating_system.as_deref());
            insert_query.bind(record.os_version.as_deref());
            insert_query.bind(record.serial_number.as_deref());
            insert_query.bind(record.imei.as_deref());
            insert_query.bind(record.model.as_deref());
            insert_query.bind(record.manufacturer.as_deref());
            insert_query.bind(enrolled_dt);
            insert_query.bind(last_sync_dt);
            insert_query.bind(record.compliance_state.as_deref());
            insert_query.bind(record.azure_ad_device_id.as_deref());
            insert_query.bind(&record.device_hash);
            insert_query.bind(&record.fingerprint);

            insert_query.execute(&mut self.client).await?;

            timer.observe_duration(&metrics::DB_OPERATION_DURATION_SECONDS);
            metrics::DB_INSERT_TOTAL.inc();
            log_database_operation("MSSQL", "INSERT", Some(1), Some(elapsed));
            Ok(StorageResult::Inserted)
        }
    }

    async fn store_device_metadata(
        &mut self,
        device_uuid: Uuid,
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        if metadata.is_empty() {
            return Ok(());
        }

        let metadata_table = format!("{}_metadata", self.table_name);

        // Clear existing metadata for this device
        let delete_sql = format!("DELETE FROM {} WHERE uuid = @P1", metadata_table);
        let mut delete_query = Query::new(&delete_sql);
        delete_query.bind(device_uuid);
        delete_query.execute(&mut self.client).await?;

        // Insert new metadata
        let insert_sql = format!(
            "INSERT INTO {} (uuid, field_name, raw_value) VALUES (@P1, @P2, @P3)",
            metadata_table
        );

        for (field_name, value) in metadata {
            let mut insert_query = Query::new(&insert_sql);
            insert_query.bind(device_uuid);
            insert_query.bind(field_name);
            let value_str = value.to_string();
            insert_query.bind(&value_str);
            insert_query.execute(&mut self.client).await?;
        }

        log_database_operation("MSSQL", "METADATA_INSERT", Some(metadata.len()), None);
        Ok(())
    }

    async fn get_device(&mut self, uuid: Uuid) -> Result<Option<DeviceInfo>> {
        let query_sql = format!("SELECT device_name, operating_system FROM {} WHERE uuid = @P1", self.table_name);
        let mut query = Query::new(&query_sql);
        query.bind(uuid);

        let stream = query.query(&mut self.client).await?;
        let row = stream.into_row().await?;

        if let Some(row) = row {
            let name: Option<&str> = row.get(0);
            let os: Option<&str> = row.get(1);

            // This is a simplified version - in a real implementation,
            // you'd reconstruct the full device data
            let mut data = HashMap::new();
            if let Some(name) = name {
                data.insert("deviceName".to_string(), serde_json::Value::String(name.to_string()));
            }
            if let Some(os) = os {
                data.insert("operatingSystem".to_string(), serde_json::Value::String(os.to_string()));
            }

            let device_info = DeviceInfo {
                uuid,
                name: name.unwrap_or("unknown").to_string(),
                os: os.map(|s| s.to_string()),
                data,
                fingerprint: String::new(), // Would need to fetch from DB
            };

            Ok(Some(device_info))
        } else {
            Ok(None)
        }
    }

    async fn get_device_hash(&mut self, uuid: Uuid) -> Result<Option<String>> {
        let query_sql = format!("SELECT device_hash FROM {} WHERE uuid = @P1", self.table_name);
        let mut query = Query::new(&query_sql);
        query.bind(uuid);

        let stream = query.query(&mut self.client).await?;
        let row = stream.into_row().await?;

        Ok(row.and_then(|row| row.get::<&str, _>(0).map(|s| s.to_string())))
    }

    async fn get_device_count(&mut self) -> Result<usize> {
        let query_sql = format!("SELECT COUNT(*) FROM {}", self.table_name);
        let stream = self.client.simple_query(&query_sql).await?;
        let row = stream.into_row().await?;

        let count = if let Some(row) = row {
            row.get::<i32, _>(0).unwrap_or(0)
        } else {
            0
        };

        Ok(count as usize)
    }

    async fn health_check(&mut self) -> Result<()> {
        let stream = self.client.simple_query("SELECT 1").await?;
        let _ = stream.into_row().await?;
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "MSSQL"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_timestamp() {
        let valid_timestamp = "2023-01-01T00:00:00Z";
        let parsed = MssqlBackend::parse_timestamp(Some(valid_timestamp));
        assert!(parsed.is_some());

        let invalid_timestamp = "invalid";
        let parsed = MssqlBackend::parse_timestamp(Some(invalid_timestamp));
        assert!(parsed.is_none());

        let none_timestamp = MssqlBackend::parse_timestamp(None);
        assert!(none_timestamp.is_none());
    }
}
