use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

use super::{DeviceRecord, StorageBackend, StorageResult};
use crate::logging::log_database_operation;
use crate::metrics;
use crate::uuid_utils::DeviceInfo;

pub struct PostgresBackend {
    pool: PgPool,
}

impl PostgresBackend {
    pub async fn new(connection_string: &str) -> Result<Self> {
        // Try to connect to the database
        let pool = match PgPool::connect(connection_string).await {
            Ok(pool) => {
                log::info!("Connected to PostgreSQL database successfully");
                pool
            }
            Err(e) => {
                log::warn!("Failed to connect to PostgreSQL database: {}", e);

                // Try to extract database name and create it if it doesn't exist
                if let Some(db_name) = Self::extract_database_name(connection_string) {
                    log::info!("Attempting to create database: {}", db_name);
                    Self::create_database_if_not_exists(connection_string, &db_name).await?;

                    // Retry connection
                    PgPool::connect(connection_string)
                        .await
                        .with_context(|| format!("Failed to connect to PostgreSQL after creating database: {}", connection_string))?
                } else {
                    return Err(e.into());
                }
            }
        };

        Ok(Self { pool })
    }

    fn extract_database_name(connection_string: &str) -> Option<String> {
        // Simple extraction of database name from connection string
        // Format: postgres://user:pass@host:port/database
        if let Some(db_part) = connection_string.split('/').last() {
            if !db_part.is_empty() && !db_part.contains('?') {
                return Some(db_part.to_string());
            }
        }
        None
    }

    async fn create_database_if_not_exists(connection_string: &str, db_name: &str) -> Result<()> {
        // Connect to postgres database to create the target database
        let postgres_conn_string = connection_string.replace(&format!("/{}", db_name), "/postgres");

        match PgPool::connect(&postgres_conn_string).await {
            Ok(pool) => {
                let create_db_sql = format!("CREATE DATABASE \"{}\"", db_name);
                match sqlx::query(&create_db_sql).execute(&pool).await {
                    Ok(_) => log::info!("Created database: {}", db_name),
                    Err(e) => {
                        // Database might already exist, which is fine
                        log::debug!("Database creation result: {}", e);
                    }
                }
                pool.close().await;
            }
            Err(e) => {
                log::warn!("Could not connect to postgres database to create target database: {}", e);
            }
        }
        Ok(())
    }

    /// Convert JSON value to a generic record for database storage
    fn json_to_generic_record(&self, json: &serde_json::Value) -> Result<std::collections::HashMap<String, String>> {
        let mut record = std::collections::HashMap::new();

        if let Some(obj) = json.as_object() {
            for (key, value) in obj {
                // Convert all values to strings for simplicity
                let string_value = match value {
                    serde_json::Value::Null => "".to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                        // Store complex types as JSON strings
                        value.to_string()
                    }
                };

                record.insert(key.clone(), string_value);
            }
        }

        // Add common fields if not present
        if !record.contains_key("id") {
            // Generate a UUID for the record if no ID is present
            record.insert("id".to_string(), uuid::Uuid::new_v4().to_string());
        }

        if !record.contains_key("last_sync_date_time") {
            record.insert("last_sync_date_time".to_string(), chrono::Utc::now().to_rfc3339());
        }

        Ok(record)
    }

    async fn create_tables(&self) -> Result<()> {
        // Main devices table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS devices (
                uuid UUID PRIMARY KEY,
                device_name TEXT,
                operating_system TEXT,
                os_version TEXT,
                serial_number TEXT,
                imei TEXT,
                model TEXT,
                manufacturer TEXT,
                enrolled_date_time TIMESTAMPTZ,
                last_sync_date_time TIMESTAMPTZ,
                compliance_state TEXT,
                azure_ad_device_id TEXT,
                device_hash TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Device metadata table for extra fields
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS device_metadata (
                uuid UUID,
                field_name TEXT,
                raw_value TEXT,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                PRIMARY KEY (uuid, field_name),
                FOREIGN KEY (uuid) REFERENCES devices(uuid) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for better performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_devices_os ON devices(operating_system)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_devices_serial ON devices(serial_number)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_devices_azure_id ON devices(azure_ad_device_id)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_devices_updated ON devices(updated_at)")
            .execute(&self.pool)
            .await?;

        log::info!("PostgreSQL tables created/verified successfully");
        Ok(())
    }

    fn parse_timestamp(timestamp_str: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
        timestamp_str.and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
    }
}

#[async_trait]
impl StorageBackend for PostgresBackend {
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

        // Use PostgreSQL UPSERT (ON CONFLICT)
        let result = sqlx::query(
            r#"
            INSERT INTO devices (
                uuid, device_name, operating_system, os_version, serial_number,
                imei, model, manufacturer, enrolled_date_time, last_sync_date_time,
                compliance_state, azure_ad_device_id, device_hash, fingerprint
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (uuid) DO UPDATE SET
                device_name = EXCLUDED.device_name,
                operating_system = EXCLUDED.operating_system,
                os_version = EXCLUDED.os_version,
                serial_number = EXCLUDED.serial_number,
                imei = EXCLUDED.imei,
                model = EXCLUDED.model,
                manufacturer = EXCLUDED.manufacturer,
                enrolled_date_time = EXCLUDED.enrolled_date_time,
                last_sync_date_time = EXCLUDED.last_sync_date_time,
                compliance_state = EXCLUDED.compliance_state,
                azure_ad_device_id = EXCLUDED.azure_ad_device_id,
                device_hash = EXCLUDED.device_hash,
                fingerprint = EXCLUDED.fingerprint,
                updated_at = NOW()
            "#,
        )
        .bind(record.uuid)
        .bind(record.device_name)
        .bind(record.operating_system)
        .bind(record.os_version)
        .bind(record.serial_number)
        .bind(record.imei)
        .bind(record.model)
        .bind(record.manufacturer)
        .bind(enrolled_dt)
        .bind(last_sync_dt)
        .bind(record.compliance_state)
        .bind(record.azure_ad_device_id)
        .bind(record.device_hash)
        .bind(record.fingerprint)
        .execute(&self.pool)
        .await?;

        let elapsed = timer.start.elapsed();
        timer.observe_duration(&metrics::DB_OPERATION_DURATION_SECONDS);

        if result.rows_affected() > 0 {
            // Check if it was an insert or update by checking if device existed before
            let was_insert = sqlx::query("SELECT 1 FROM devices WHERE uuid = $1 AND created_at = updated_at")
                .bind(record.uuid)
                .fetch_optional(&self.pool)
                .await?
                .is_some();

            if was_insert {
                metrics::DB_INSERT_TOTAL.inc();
                log_database_operation("PostgreSQL", "INSERT", Some(1), Some(elapsed));
                Ok(StorageResult::Inserted)
            } else {
                metrics::DB_UPDATE_TOTAL.inc();
                log_database_operation("PostgreSQL", "UPDATE", Some(1), Some(elapsed));
                Ok(StorageResult::Updated)
            }
        } else {
            metrics::DB_SKIP_TOTAL.inc();
            Ok(StorageResult::Skipped)
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

        // Clear existing metadata for this device
        sqlx::query("DELETE FROM device_metadata WHERE uuid = $1")
            .bind(device_uuid)
            .execute(&self.pool)
            .await?;

        // Insert new metadata
        for (field_name, value) in metadata {
            sqlx::query("INSERT INTO device_metadata (uuid, field_name, raw_value) VALUES ($1, $2, $3)")
                .bind(device_uuid)
                .bind(field_name)
                .bind(value.to_string())
                .execute(&self.pool)
                .await?;
        }

        log_database_operation("PostgreSQL", "METADATA_INSERT", Some(metadata.len()), None);
        Ok(())
    }

    async fn get_device(&mut self, uuid: Uuid) -> Result<Option<DeviceInfo>> {
        let result = sqlx::query("SELECT device_name, operating_system FROM devices WHERE uuid = $1")
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = result {
            let name: Option<String> = row.get("device_name");
            let os: Option<String> = row.get("operating_system");

            // This is a simplified version - in a real implementation,
            // you'd reconstruct the full device data
            let mut data = HashMap::new();
            if let Some(name) = &name {
                data.insert("deviceName".to_string(), serde_json::Value::String(name.clone()));
            }
            if let Some(os) = &os {
                data.insert("operatingSystem".to_string(), serde_json::Value::String(os.clone()));
            }

            let device_info = DeviceInfo {
                uuid,
                name: name.unwrap_or_else(|| "unknown".to_string()),
                os,
                data,
                fingerprint: String::new(), // Would need to fetch from DB
            };

            Ok(Some(device_info))
        } else {
            Ok(None)
        }
    }

    async fn get_device_hash(&mut self, uuid: Uuid) -> Result<Option<String>> {
        let result = sqlx::query("SELECT device_hash FROM devices WHERE uuid = $1")
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await?;

        Ok(result.map(|row| row.get("device_hash")))
    }

    async fn get_device_count(&mut self) -> Result<usize> {
        let result = sqlx::query("SELECT COUNT(*) as count FROM devices")
            .fetch_one(&self.pool)
            .await?;

        let count: i64 = result.get("count");
        Ok(count as usize)
    }

    async fn health_check(&mut self) -> Result<()> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await?;
        Ok(())
    }

    async fn create_table_if_not_exists(&mut self, table_name: &str, schema: &str) -> Result<()> {
        sqlx::query(schema)
            .execute(&self.pool)
            .await
            .context("Failed to create table")?;

        log::info!("Created/verified table: {}", table_name);
        Ok(())
    }

    async fn store_endpoint_data(&mut self, table_name: &str, data: &[serde_json::Value]) -> Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }

        let mut stored_count = 0;

        for item in data {
            // Convert JSON to a generic record format
            let record = self.json_to_generic_record(item)?;

            // Create dynamic INSERT statement based on available fields
            let field_names: Vec<String> = record.keys().cloned().collect();
            let placeholders: Vec<String> = (1..=field_names.len())
                .map(|i| format!("${}", i))
                .collect();

            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT (id) DO UPDATE SET {}",
                table_name,
                field_names.join(", "),
                placeholders.join(", "),
                field_names.iter()
                    .enumerate()
                    .map(|(i, field)| format!("{} = ${}", field, i + 1))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            let mut query = sqlx::query(&sql);
            for field in &field_names {
                query = query.bind(record.get(field).unwrap());
            }

            match query.execute(&self.pool).await {
                Ok(_) => {
                    stored_count += 1;
                }
                Err(e) => {
                    log::warn!("Failed to store item in table {}: {}", table_name, e);
                    // Continue with other items rather than failing completely
                }
            }
        }

        log::debug!("Stored {} items in table {}", stored_count, table_name);
        Ok(stored_count)
    }

    fn backend_name(&self) -> &'static str {
        "PostgreSQL"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_timestamp() {
        let valid_timestamp = "2023-01-01T00:00:00Z";
        let parsed = PostgresBackend::parse_timestamp(Some(valid_timestamp));
        assert!(parsed.is_some());

        let invalid_timestamp = "invalid";
        let parsed = PostgresBackend::parse_timestamp(Some(invalid_timestamp));
        assert!(parsed.is_none());

        let none_timestamp = PostgresBackend::parse_timestamp(None);
        assert!(none_timestamp.is_none());
    }
}
