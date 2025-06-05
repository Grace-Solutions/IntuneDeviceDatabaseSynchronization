use anyhow::{Context, Result};
use async_trait::async_trait;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::StorageBackend;

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



    async fn health_check(&mut self) -> Result<()> {
        let conn = self.connection.lock().await;
        conn.execute("SELECT 1", [])?;
        Ok(())
    }

    async fn create_table_if_not_exists(&mut self, table_name: &str, schema: &str) -> Result<()> {
        let connection = self.connection.lock().await;

        // Execute the schema directly - it should include CREATE TABLE IF NOT EXISTS
        connection.execute(schema, [])?;

        log::info!("Created/verified table: {}", table_name);
        Ok(())
    }

    async fn store_endpoint_data(&mut self, table_name: &str, data: &[serde_json::Value]) -> Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }

        let connection = self.connection.lock().await;
        let mut stored_count = 0;

        for item in data {
            // Convert JSON to a generic record format
            let record = self.json_to_generic_record(item)?;

            // Create dynamic INSERT statement based on available fields
            let field_names: Vec<String> = record.keys().cloned().collect();
            let placeholders: Vec<String> = field_names.iter().map(|_| "?".to_string()).collect();

            let sql = format!(
                "INSERT OR REPLACE INTO {} ({}) VALUES ({})",
                table_name,
                field_names.join(", "),
                placeholders.join(", ")
            );

            let values: Vec<&str> = field_names.iter()
                .map(|field| record.get(field).unwrap().as_str())
                .collect();

            match connection.execute(&sql, rusqlite::params_from_iter(values)) {
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
        
        let mut backend = SqliteBackend::new(db_path).await.unwrap();
        backend.initialize().await.unwrap();

        // Test health check
        backend.health_check().await.unwrap();

        // Test device count
        let count = backend.get_device_count().await.unwrap();
        assert_eq!(count, 0);
    }
}
