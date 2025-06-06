use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet};
use chrono::{TimeZone, Utc};

use super::StorageBackend;
use crate::path_utils;

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
                    serde_json::Value::String(s) => {
                        // Check if this looks like a timestamp and normalize it
                        if self.is_timestamp_string(s) || self.is_timestamp_field_name(key) {
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
            return dt.with_timezone(&Utc).to_rfc3339();
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

    /// Determine the appropriate PostgreSQL column type for a JSON value
    fn determine_column_type(&self, value: Option<&serde_json::Value>) -> &'static str {
        match value {
            Some(serde_json::Value::Bool(_)) => "BOOLEAN",
            Some(serde_json::Value::Number(n)) => {
                if n.is_i64() || n.is_u64() {
                    "BIGINT"
                } else {
                    "DOUBLE PRECISION"
                }
            }
            Some(serde_json::Value::String(s)) => {
                // Check if the string looks like a timestamp/date
                if self.is_timestamp_string(s) {
                    "TIMESTAMPTZ" // Store timestamps with timezone
                } else {
                    "TEXT"
                }
            }
            Some(serde_json::Value::Array(_)) | Some(serde_json::Value::Object(_)) => "JSONB", // Store as JSONB
            Some(serde_json::Value::Null) | None => "TEXT", // Default to TEXT for unknown/null values
        }
    }

    /// Determine column type by field name patterns (for better timestamp detection)
    fn determine_column_type_by_name(&self, field_name: &str, value: Option<&serde_json::Value>) -> &'static str {
        // Check if field name suggests it's a timestamp
        let field_lower = field_name.to_lowercase();
        if field_lower.contains("date") || field_lower.contains("time") ||
           field_lower.ends_with("_at") || field_lower.ends_with("_on") ||
           field_lower.contains("created") || field_lower.contains("updated") ||
           field_lower.contains("modified") || field_lower.contains("enrolled") ||
           field_lower.contains("last_sync") {
            return "TIMESTAMPTZ";
        }

        // Fall back to value-based detection
        self.determine_column_type(value)
    }

    /// Check if a field name suggests it contains timestamp data
    fn is_timestamp_field_name(&self, field_name: &str) -> bool {
        let field_lower = field_name.to_lowercase();
        field_lower.contains("date") || field_lower.contains("time") ||
        field_lower.ends_with("_at") || field_lower.ends_with("_on") ||
        field_lower.contains("created") || field_lower.contains("updated") ||
        field_lower.contains("modified") || field_lower.contains("enrolled") ||
        field_lower.contains("last_sync")
    }

    /// Get existing table columns
    async fn get_table_columns(&self, table_name: &str) -> Result<HashSet<String>> {
        let rows = sqlx::query(
            "SELECT column_name FROM information_schema.columns WHERE table_name = $1"
        )
        .bind(table_name)
        .fetch_all(&self.pool)
        .await?;

        let mut columns = HashSet::new();
        for row in rows {
            let column_name: String = row.get("column_name");
            columns.insert(column_name);
        }

        Ok(columns)
    }

    /// Ensure the table schema matches the data structure by analyzing the JSON object
    async fn ensure_table_schema_matches(&mut self, table_name: &str, sample_data: &serde_json::Value) -> Result<()> {
        if let Some(obj) = sample_data.as_object() {
            // Get current table schema
            let existing_columns = self.get_table_columns(table_name).await?;

            // Determine required columns from the sample data
            let mut required_columns = HashSet::new();
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
                let column_type = self.determine_column_type_by_name(&column, obj.get(&column));
                let alter_sql = format!(
                    "ALTER TABLE {} ADD COLUMN {} {}",
                    table_name, column, column_type
                );

                match sqlx::query(&alter_sql).execute(&self.pool).await {
                    Ok(_) => {
                        log::info!("Added column {} ({}) to table {}", column, column_type, table_name);
                    }
                    Err(e) => {
                        log::warn!("Failed to add column {} to table {}: {}", column, table_name, e);
                    }
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
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

        // Ensure table schema matches the data structure using the first item as a sample
        if let Some(first_item) = data.first() {
            if let Err(e) = self.ensure_table_schema_matches(table_name, first_item).await {
                log::warn!("Failed to update table schema for {}: {}", table_name, e);
                // Continue anyway - might work with existing schema
            }
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

    async fn cleanup(&mut self) -> Result<()> {
        // Close the connection pool
        self.pool.close().await;
        log::info!("Cleaned up PostgreSQL backend - connection pool closed");
        Ok(())
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
