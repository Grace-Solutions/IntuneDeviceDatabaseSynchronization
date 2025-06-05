use anyhow::{Context, Result};
use async_trait::async_trait;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

use chrono::{TimeZone, Utc};

use super::StorageBackend;
use crate::path_utils;

pub struct SqliteBackend {
    connection: Arc<Mutex<Connection>>,
    db_path: String,
}

impl SqliteBackend {
    pub async fn new(db_path: &str) -> Result<Self> {
        // Resolve the database path (handles relative/absolute paths and OS-specific separators)
        let resolved_path = path_utils::resolve_path(db_path)
            .with_context(|| format!("Failed to resolve database path: {}", db_path))?;

        // Ensure parent directory exists
        path_utils::ensure_parent_directory_exists(&resolved_path).await
            .with_context(|| format!("Failed to create directory for SQLite database: {}", resolved_path.display()))?;

        // Create or open the database file
        let conn = Connection::open(&resolved_path)
            .with_context(|| format!("Failed to open SQLite database at {}", resolved_path.display()))?;

        log::info!("Connected to SQLite database at: {}", resolved_path.display());

        // Enable foreign keys and WAL mode for better performance
        conn.execute("PRAGMA foreign_keys = ON", [])?;

        // PRAGMA journal_mode returns a result, so we need to use query
        {
            let mut stmt = conn.prepare("PRAGMA journal_mode = WAL")?;
            let _: String = stmt.query_row([], |row| row.get(0))?;
        } // stmt is dropped here

        conn.execute("PRAGMA synchronous = NORMAL", [])?;

        Ok(Self {
            connection: Arc::new(Mutex::new(conn)),
            db_path: resolved_path.to_string_lossy().to_string(),
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
                    serde_json::Value::String(s) => {
                        // Check if this looks like a timestamp and normalize it
                        if self.is_timestamp_string(s) {
                            self.normalize_timestamp_value(s)
                        } else {
                            s.clone()
                        }
                    },
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

    /// Ensure the table schema matches the data structure by analyzing the JSON object
    async fn ensure_table_schema_matches(&mut self, table_name: &str, sample_data: &serde_json::Value) -> Result<()> {
        if let Some(obj) = sample_data.as_object() {
            let connection = self.connection.lock().await;

            // Get current table schema
            let existing_columns = self.get_table_columns(&connection, table_name)?;

            // Determine required columns from the sample data
            let mut required_columns = std::collections::HashSet::new();
            for key in obj.keys() {
                required_columns.insert(key.clone());
            }

            // Add standard columns
            required_columns.insert("id".to_string());
            required_columns.insert("last_sync_date_time".to_string());

            // Find missing columns
            let missing_columns: Vec<String> = required_columns
                .difference(&existing_columns)
                .cloned()
                .collect();

            // Add missing columns
            for column in missing_columns {
                let column_type = self.determine_column_type(obj.get(&column));
                let alter_sql = format!(
                    "ALTER TABLE {} ADD COLUMN {} {}",
                    table_name, column, column_type
                );

                match connection.execute(&alter_sql, []) {
                    Ok(_) => {
                        log::info!("Added column {} to table {}", column, table_name);
                    }
                    Err(e) => {
                        log::warn!("Failed to add column {} to table {}: {}", column, table_name, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get existing column names from a table
    fn get_table_columns(&self, connection: &rusqlite::Connection, table_name: &str) -> Result<std::collections::HashSet<String>> {
        let mut columns = std::collections::HashSet::new();

        let sql = format!("PRAGMA table_info({})", table_name);
        let mut stmt = connection.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            let column_name: String = row.get(1)?; // Column name is at index 1
            Ok(column_name)
        })?;

        for row in rows {
            columns.insert(row?);
        }

        Ok(columns)
    }

    /// Determine the appropriate SQLite column type for a JSON value
    fn determine_column_type(&self, value: Option<&serde_json::Value>) -> &'static str {
        match value {
            Some(serde_json::Value::Bool(_)) => "INTEGER", // SQLite stores booleans as integers
            Some(serde_json::Value::Number(n)) => {
                if n.is_i64() || n.is_u64() {
                    "INTEGER"
                } else {
                    "REAL"
                }
            }
            Some(serde_json::Value::String(s)) => {
                // Check if the string looks like a timestamp/date
                if self.is_timestamp_string(s) {
                    "TEXT" // Store timestamps as TEXT in ISO format
                } else {
                    "TEXT"
                }
            }
            Some(serde_json::Value::Array(_)) | Some(serde_json::Value::Object(_)) => "TEXT", // Store as JSON string
            Some(serde_json::Value::Null) | None => "TEXT", // Default to TEXT for unknown/null values
        }
    }

    /// Check if a string looks like a timestamp
    fn is_timestamp_string(&self, s: &str) -> bool {
        // Check for common timestamp patterns
        s.contains('T') && (s.contains('Z') || s.contains('+') || s.contains('-')) ||
        s.ends_with("DateTime") ||
        s.contains("Date") ||
        chrono::DateTime::parse_from_rfc3339(s).is_ok()
    }

    /// Parse and normalize timestamp values
    fn normalize_timestamp_value(&self, value: &str) -> String {
        // Try to parse as RFC3339 first
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(value) {
            return dt.with_timezone(&chrono::Utc).to_rfc3339();
        }

        // Try other common formats
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S") {
            return Utc.from_utc_datetime(&dt).to_rfc3339();
        }

        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") {
            return Utc.from_utc_datetime(&dt).to_rfc3339();
        }

        // If parsing fails, return the original value
        value.to_string()
    }

}

#[async_trait]
impl StorageBackend for SqliteBackend {
    async fn initialize(&mut self) -> Result<()> {
        log::info!("SQLite backend initialized successfully");
        Ok(())
    }



    async fn health_check(&mut self) -> Result<()> {
        let conn = self.connection.lock().await;
        let mut stmt = conn.prepare("SELECT 1")?;
        let _: i32 = stmt.query_row([], |row| row.get(0))?;
        Ok(())
    }

    async fn create_table_if_not_exists(&mut self, table_name: &str, schema: &str) -> Result<()> {
        let connection = self.connection.lock().await;

        // Log the schema for debugging
        log::debug!("Executing schema for table {}: {}", table_name, schema);

        // Execute the schema directly - it should include CREATE TABLE IF NOT EXISTS
        connection.execute(schema, []).map_err(|e| {
            log::error!("Failed to execute schema for table {}: {} - Error: {}", table_name, schema, e);
            e
        })?;

        log::info!("Created/verified table: {}", table_name);
        Ok(())
    }

    async fn store_endpoint_data(&mut self, table_name: &str, data: &[serde_json::Value]) -> Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }

        // Analyze the first object to determine required schema
        if let Some(first_item) = data.first() {
            self.ensure_table_schema_matches(table_name, first_item).await?;
        }

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

            let values: Vec<String> = field_names.iter()
                .map(|field| record.get(field).unwrap().clone())
                .collect();

            let values_refs: Vec<&str> = values.iter().map(|s| s.as_str()).collect();

            let connection = self.connection.lock().await;
            match connection.execute(&sql, rusqlite::params_from_iter(values_refs.iter())) {
                Ok(_) => {
                    stored_count += 1;
                }
                Err(e) => {
                    log::warn!("Failed to store item in table {}: {}", table_name, e);
                    // Drop the connection lock before trying to update schema
                    drop(connection);

                    // Try to add missing columns and retry once
                    if let Err(schema_err) = self.ensure_table_schema_matches(table_name, item).await {
                        log::error!("Failed to update schema for table {}: {}", table_name, schema_err);
                    } else {
                        // Retry the insert after schema update
                        let connection = self.connection.lock().await;
                        match connection.execute(&sql, rusqlite::params_from_iter(values_refs.iter())) {
                            Ok(_) => {
                                stored_count += 1;
                                log::debug!("Successfully stored item after schema update");
                            }
                            Err(retry_err) => {
                                log::warn!("Failed to store item even after schema update: {}", retry_err);
                            }
                        }
                    }
                }
            }
        }

        log::debug!("Stored {} items in table {}", stored_count, table_name);
        Ok(stored_count)
    }

    fn backend_name(&self) -> &'static str {
        "SQLite"
    }

    async fn cleanup(&mut self) -> Result<()> {
        // SQLite connections are automatically closed when dropped
        // But we can explicitly close the connection for cleaner shutdown
        log::info!("Cleaning up SQLite backend");
        Ok(())
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

        // Test completed successfully
    }
}
