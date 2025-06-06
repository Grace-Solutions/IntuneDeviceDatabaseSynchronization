use anyhow::{Context, Result};
use async_trait::async_trait;
use tiberius::{Client, Config, Row};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};
use std::collections::{HashMap, HashSet};
use chrono::{TimeZone, Utc};

use super::StorageBackend;

pub struct MssqlBackend {
    client: Client<Compat<TcpStream>>,
}

impl MssqlBackend {
    pub async fn new(connection_string: &str) -> Result<Self> {
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

    async fn create_tables(&mut self) -> Result<()> {
        // No default tables are created - tables are created dynamically via create_table_if_not_exists
        log::info!("MSSQL backend initialized - tables will be created dynamically");
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

    /// Determine the appropriate MSSQL column type for a JSON value
    fn determine_column_type(&self, value: Option<&serde_json::Value>) -> &'static str {
        match value {
            Some(serde_json::Value::Bool(_)) => "BIT",
            Some(serde_json::Value::Number(n)) => {
                if n.is_i64() || n.is_u64() {
                    "BIGINT"
                } else {
                    "FLOAT"
                }
            }
            Some(serde_json::Value::String(s)) => {
                // Check if the string looks like a timestamp/date
                if self.is_timestamp_string(s) {
                    "DATETIME2" // Store timestamps with high precision
                } else {
                    "NVARCHAR(MAX)"
                }
            }
            Some(serde_json::Value::Array(_)) | Some(serde_json::Value::Object(_)) => "NVARCHAR(MAX)", // Store as JSON string
            Some(serde_json::Value::Null) | None => "NVARCHAR(MAX)", // Default to NVARCHAR for unknown/null values
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
            return "DATETIME2";
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
    async fn get_table_columns(&mut self, table_name: &str) -> Result<HashSet<String>> {
        let query = format!(
            "SELECT COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_NAME = '{}'",
            table_name
        );

        let stream = self.client.simple_query(&query).await?;
        let rows = stream.into_first_result().await?;

        let mut columns = HashSet::new();
        for row in rows {
            if let Some(column_name) = row.get::<&str, _>(0) {
                columns.insert(column_name.to_string());
            }
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
                    "ALTER TABLE {} ADD {} {}",
                    table_name, column, column_type
                );

                match self.client.simple_query(&alter_sql).await {
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



    async fn health_check(&mut self) -> Result<()> {
        let stream = self.client.simple_query("SELECT 1").await?;
        let _ = stream.into_row().await?;
        Ok(())
    }

    async fn create_table_if_not_exists(&mut self, table_name: &str, schema: &str) -> Result<()> {
        // Execute the schema directly - it should include CREATE TABLE IF NOT EXISTS equivalent
        self.client.simple_query(schema).await
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

            // For simplicity, use a basic INSERT with ON DUPLICATE KEY UPDATE equivalent
            // In MSSQL, we'll use a simple INSERT and handle conflicts
            let field_names: Vec<String> = record.keys().cloned().collect();
            let placeholders: Vec<String> = (1..=field_names.len())
                .map(|i| format!("@P{}", i))
                .collect();

            // Simple INSERT statement - table should have appropriate constraints
            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                table_name,
                field_names.join(", "),
                placeholders.join(", ")
            );

            let mut query = tiberius::Query::new(sql);
            for field in &field_names {
                query.bind(record.get(field).unwrap().as_str());
            }

            match query.execute(&mut self.client).await {
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
        "MSSQL"
    }

    async fn cleanup(&mut self) -> Result<()> {
        // MSSQL connections are automatically closed when dropped
        // The close() method takes ownership, so we just log the cleanup
        log::info!("Cleaned up MSSQL backend - connection will be closed on drop");
        Ok(())
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
