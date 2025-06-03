use anyhow::{Context, Result};
use async_trait::async_trait;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{DeviceRecord, StorageBackend, StorageResult};
use crate::logging::log_database_operation;
use crate::metrics;
use crate::uuid_utils::DeviceInfo;

pub struct SqliteBackend {
    connection: Arc<Mutex<Connection>>,
    db_path: String,
}

impl SqliteBackend {
    pub async fn new(db_path: &str) -> Result<Self> {
        // Ensure directory exists
        if let Some(parent) = Path::new(db_path).parent() {
            tokio::fs::create_dir_all(parent).await
                .with_context(|| format!("Failed to create directory for SQLite database: {:?}", parent))?;
            log::info!("Created directory for SQLite database: {:?}", parent);
        }

        // Create or open the database file
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open SQLite database at {}", db_path))?;

        log::info!("Connected to SQLite database at: {}", db_path);

        // Enable foreign keys and WAL mode for better performance
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        conn.execute("PRAGMA journal_mode = WAL", [])?;
        conn.execute("PRAGMA synchronous = NORMAL", [])?;

        Ok(Self {
            connection: Arc::new(Mutex::new(conn)),
            db_path: db_path.to_string(),
        })
    }

    async fn create_tables(&self) -> Result<()> {
        let conn = self.connection.lock().await;

        // Main devices table
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS devices (
                uuid TEXT PRIMARY KEY,
                device_name TEXT,
                operating_system TEXT,
                os_version TEXT,
                serial_number TEXT,
                imei TEXT,
                model TEXT,
                manufacturer TEXT,
                enrolled_date_time TEXT,
                last_sync_date_time TEXT,
                compliance_state TEXT,
                azure_ad_device_id TEXT,
                device_hash TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
            [],
        )?;

        // Device metadata table for extra fields
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS device_metadata (
                uuid TEXT,
                field_name TEXT,
                raw_value TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (uuid, field_name),
                FOREIGN KEY (uuid) REFERENCES devices(uuid) ON DELETE CASCADE
            )
            "#,
            [],
        )?;

        // Create indexes for better performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_devices_os ON devices(operating_system)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_devices_serial ON devices(serial_number)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_devices_azure_id ON devices(azure_ad_device_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_devices_updated ON devices(updated_at)",
            [],
        )?;

        log::info!("SQLite tables created/verified successfully");
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for SqliteBackend {
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

        let conn = self.connection.lock().await;
        let is_update = conn
            .prepare("SELECT 1 FROM devices WHERE uuid = ?1")?
            .query_row(params![record.uuid.to_string()], |_| Ok(()))
            .optional()?
            .is_some();

        if is_update {
            // Update existing device
            conn.execute(
                r#"
                UPDATE devices SET
                    device_name = ?2,
                    operating_system = ?3,
                    os_version = ?4,
                    serial_number = ?5,
                    imei = ?6,
                    model = ?7,
                    manufacturer = ?8,
                    enrolled_date_time = ?9,
                    last_sync_date_time = ?10,
                    compliance_state = ?11,
                    azure_ad_device_id = ?12,
                    device_hash = ?13,
                    fingerprint = ?14,
                    updated_at = CURRENT_TIMESTAMP
                WHERE uuid = ?1
                "#,
                params![
                    record.uuid.to_string(),
                    record.device_name,
                    record.operating_system,
                    record.os_version,
                    record.serial_number,
                    record.imei,
                    record.model,
                    record.manufacturer,
                    record.enrolled_date_time,
                    record.last_sync_date_time,
                    record.compliance_state,
                    record.azure_ad_device_id,
                    record.device_hash,
                    record.fingerprint,
                ],
            )?;

            let elapsed = timer.start.elapsed();
            timer.observe_duration(&metrics::DB_OPERATION_DURATION_SECONDS);
            metrics::DB_UPDATE_TOTAL.inc();
            log_database_operation("SQLite", "UPDATE", Some(1), Some(elapsed));
            Ok(StorageResult::Updated)
        } else {
            // Insert new device
            conn.execute(
                r#"
                INSERT INTO devices (
                    uuid, device_name, operating_system, os_version, serial_number,
                    imei, model, manufacturer, enrolled_date_time, last_sync_date_time,
                    compliance_state, azure_ad_device_id, device_hash, fingerprint
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                "#,
                params![
                    record.uuid.to_string(),
                    record.device_name,
                    record.operating_system,
                    record.os_version,
                    record.serial_number,
                    record.imei,
                    record.model,
                    record.manufacturer,
                    record.enrolled_date_time,
                    record.last_sync_date_time,
                    record.compliance_state,
                    record.azure_ad_device_id,
                    record.device_hash,
                    record.fingerprint,
                ],
            )?;

            let elapsed = timer.start.elapsed();
            timer.observe_duration(&metrics::DB_OPERATION_DURATION_SECONDS);
            metrics::DB_INSERT_TOTAL.inc();
            log_database_operation("SQLite", "INSERT", Some(1), Some(elapsed));
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

        let conn = self.connection.lock().await;

        // Clear existing metadata for this device
        conn.execute(
            "DELETE FROM device_metadata WHERE uuid = ?1",
            params![device_uuid.to_string()],
        )?;

        // Insert new metadata
        let mut stmt = conn.prepare(
            "INSERT INTO device_metadata (uuid, field_name, raw_value) VALUES (?1, ?2, ?3)",
        )?;

        for (field_name, value) in metadata {
            stmt.execute(params![
                device_uuid.to_string(),
                field_name,
                value.to_string()
            ])?;
        }

        log_database_operation("SQLite", "METADATA_INSERT", Some(metadata.len()), None);
        Ok(())
    }

    async fn get_device(&mut self, uuid: Uuid) -> Result<Option<DeviceInfo>> {
        let conn = self.connection.lock().await;
        
        let result = conn
            .prepare("SELECT device_name, operating_system FROM devices WHERE uuid = ?1")?
            .query_row(params![uuid.to_string()], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            })
            .optional()?;

        if let Some((name, os)) = result {
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
        let conn = self.connection.lock().await;
        
        let hash = conn
            .prepare("SELECT device_hash FROM devices WHERE uuid = ?1")?
            .query_row(params![uuid.to_string()], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;

        Ok(hash)
    }

    async fn get_device_count(&mut self) -> Result<usize> {
        let conn = self.connection.lock().await;
        
        let count: i64 = conn
            .prepare("SELECT COUNT(*) FROM devices")?
            .query_row([], |row| row.get(0))?;

        Ok(count as usize)
    }

    async fn health_check(&mut self) -> Result<()> {
        let conn = self.connection.lock().await;
        conn.execute("SELECT 1", [])?;
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "SQLite"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_sqlite_backend() {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path().to_str().unwrap();
        
        let backend = SqliteBackend::new(db_path).await.unwrap();
        backend.initialize().await.unwrap();
        
        // Test health check
        backend.health_check().await.unwrap();
        
        // Test device count
        let count = backend.get_device_count().await.unwrap();
        assert_eq!(count, 0);
    }
}
