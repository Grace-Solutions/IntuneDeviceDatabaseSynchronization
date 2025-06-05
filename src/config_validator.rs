use std::fmt;
use std::path::Path;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use log::{error, info};
use url::Url;
use regex::Regex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub suggestions: Vec<ValidationSuggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field_path: String,
    pub error_type: ValidationErrorType,
    pub message: String,
    pub current_value: Option<String>,
    pub expected_format: Option<String>,
    pub line_number: Option<u32>,
    pub column_number: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub field_path: String,
    pub warning_type: ValidationWarningType,
    pub message: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSuggestion {
    pub field_path: String,
    pub suggestion_type: ValidationSuggestionType,
    pub message: String,
    pub suggested_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationErrorType {
    Required,
    InvalidFormat,
    InvalidValue,
    InvalidRange,
    InvalidUrl,
    InvalidPath,
    InvalidDuration,
    InvalidCron,
    InvalidConnectionString,
    InvalidUuid,
    InvalidEmail,
    Conflict,
    TypeMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationWarningType {
    Deprecated,
    Insecure,
    Performance,
    Compatibility,
    BestPractice,
    Security,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationSuggestionType {
    Optimization,
    Security,
    Reliability,
    Maintenance,
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid {
            writeln!(f, "✅ Configuration is valid!")?;
        } else {
            writeln!(f, "❌ Configuration validation failed!")?;
        }

        if !self.errors.is_empty() {
            writeln!(f, "\n🚨 Errors ({}):", self.errors.len())?;
            for (i, error) in self.errors.iter().enumerate() {
                writeln!(f, "  {}. {}", i + 1, error)?;
            }
        }

        if !self.warnings.is_empty() {
            writeln!(f, "\n⚠️  Warnings ({}):", self.warnings.len())?;
            for (i, warning) in self.warnings.iter().enumerate() {
                writeln!(f, "  {}. {}", i + 1, warning)?;
            }
        }

        if !self.suggestions.is_empty() {
            writeln!(f, "\n💡 Suggestions ({}):", self.suggestions.len())?;
            for (i, suggestion) in self.suggestions.iter().enumerate() {
                writeln!(f, "  {}. {}", i + 1, suggestion)?;
            }
        }

        Ok(())
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.field_path, self.message)?;

        if let Some(current) = &self.current_value {
            write!(f, " (current: '{}')", current)?;
        }

        if let Some(expected) = &self.expected_format {
            write!(f, " (expected: {})", expected)?;
        }

        if let (Some(line), Some(col)) = (self.line_number, self.column_number) {
            write!(f, " at line {}, column {}", line, col)?;
        }

        Ok(())
    }
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} (Recommendation: {})",
               self.field_path, self.message, self.recommendation)
    }
}

impl fmt::Display for ValidationSuggestion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.field_path, self.message)?;

        if let Some(suggested) = &self.suggested_value {
            write!(f, " (suggested: '{}')", suggested)?;
        }

        Ok(())
    }
}

pub struct ConfigValidator {
    errors: Vec<ValidationError>,
    warnings: Vec<ValidationWarning>,
    suggestions: Vec<ValidationSuggestion>,
}

impl ConfigValidator {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn validate_config_file<P: AsRef<Path>>(config_path: P) -> Result<ValidationResult> {
        let config_path = config_path.as_ref();
        let content = std::fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        Self::validate_config_content(&content)
    }

    pub fn validate_config_content(content: &str) -> Result<ValidationResult> {
        let mut validator = Self::new();

        // First, try to parse as JSON to get syntax errors with line numbers
        match serde_json::from_str::<serde_json::Value>(content) {
            Ok(json_value) => {
                // Parse into our config structure
                match serde_json::from_value::<crate::config::AppConfig>(json_value.clone()) {
                    Ok(config) => {
                        validator.validate_app_config(&config);
                    }
                    Err(e) => {
                        validator.add_error(
                            "root".to_string(),
                            ValidationErrorType::TypeMismatch,
                            format!("Failed to parse configuration: {}", e),
                            None,
                            None,
                        );
                    }
                }
            }
            Err(e) => {
                let (line, column) = extract_json_error_position(&e);
                validator.add_error(
                    "root".to_string(),
                    ValidationErrorType::InvalidFormat,
                    format!("JSON syntax error: {}", e),
                    None,
                    None,
                );
            }
        }

        Ok(validator.build_result())
    }

    fn validate_app_config(&mut self, config: &crate::config::AppConfig) {
        // Validate authentication
        self.validate_auth_config(config);

        // Validate sync settings
        self.validate_sync_config(config);

        // Validate database configuration
        self.validate_database_config(config);

        // Validate monitoring settings
        self.validate_monitoring_config(config);

        // Validate webhook configuration
        if let Some(webhook_config) = &config.webhook {
            self.validate_webhook_config(webhook_config);
        }

        // Validate backup configuration
        if let Some(backup_config) = &config.backup {
            self.validate_backup_config(backup_config);
        }

        // Validate rate limiting configuration
        if let Some(rate_limit_config) = &config.rate_limit {
            self.validate_rate_limit_config(rate_limit_config);
        }

        // Validate mock API configuration
        if let Some(mock_config) = &config.mock_graph_api {
            self.validate_mock_config(mock_config);
        }
    }

    fn validate_auth_config(&mut self, config: &crate::config::AppConfig) {
        // Client ID validation
        if config.client_id.is_empty() {
            self.add_error(
                "clientId".to_string(),
                ValidationErrorType::Required,
                "Client ID is required for Azure authentication".to_string(),
                Some(config.client_id.clone()),
                None,
            );
        } else if !is_valid_uuid(&config.client_id) {
            self.add_error(
                "clientId".to_string(),
                ValidationErrorType::InvalidUuid,
                "Client ID must be a valid UUID".to_string(),
                Some(config.client_id.clone()),
                Some("XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX".to_string()),
            );
        }

        // Client Secret validation
        if config.client_secret.is_empty() {
            self.add_error(
                "clientSecret".to_string(),
                ValidationErrorType::Required,
                "Client secret is required for Azure authentication".to_string(),
                None,
                None,
            );
        } else if config.client_secret.len() < 10 {
            self.add_warning(
                "clientSecret".to_string(),
                ValidationWarningType::Security,
                "Client secret appears to be very short".to_string(),
                "Ensure you're using a proper Azure client secret".to_string(),
            );
        }

        // Tenant ID validation
        if config.tenant_id.is_empty() {
            self.add_error(
                "tenantId".to_string(),
                ValidationErrorType::Required,
                "Tenant ID is required for Azure authentication".to_string(),
                Some(config.tenant_id.clone()),
                None,
            );
        } else if !is_valid_uuid(&config.tenant_id) {
            self.add_error(
                "tenantId".to_string(),
                ValidationErrorType::InvalidUuid,
                "Tenant ID must be a valid UUID".to_string(),
                Some(config.tenant_id.clone()),
                Some("XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX".to_string()),
            );
        }

        // Check for placeholder values
        if config.client_id.contains("YOUR_") || config.client_id.contains("your-") {
            self.add_error(
                "clientId".to_string(),
                ValidationErrorType::InvalidValue,
                "Client ID appears to be a placeholder value".to_string(),
                Some(config.client_id.clone()),
                Some("Replace with actual Azure client ID".to_string()),
            );
        }

        if config.client_secret.contains("YOUR_") || config.client_secret.contains("your-") {
            self.add_error(
                "clientSecret".to_string(),
                ValidationErrorType::InvalidValue,
                "Client secret appears to be a placeholder value".to_string(),
                None,
                Some("Replace with actual Azure client secret".to_string()),
            );
        }

        if config.tenant_id.contains("YOUR_") || config.tenant_id.contains("your-") {
            self.add_error(
                "tenantId".to_string(),
                ValidationErrorType::InvalidValue,
                "Tenant ID appears to be a placeholder value".to_string(),
                Some(config.tenant_id.clone()),
                Some("Replace with actual Azure tenant ID".to_string()),
            );
        }
    }

    fn validate_sync_config(&mut self, config: &crate::config::AppConfig) {
        // Poll interval validation
        if let Some(poll_interval) = &config.poll_interval {
            if !is_valid_duration(poll_interval) {
                self.add_error(
                    "pollInterval".to_string(),
                    ValidationErrorType::InvalidDuration,
                    "Poll interval must be a valid duration".to_string(),
                    Some(poll_interval.clone()),
                    Some("Examples: '30s', '5m', '1h', '2h30m'".to_string()),
                );
            } else {
                let duration = parse_duration(poll_interval);
                if let Some(duration) = duration {
                    if duration.as_secs() < 60 {
                        self.add_warning(
                            "pollInterval".to_string(),
                            ValidationWarningType::Performance,
                            "Very short poll interval may cause rate limiting".to_string(),
                            "Consider using at least 1 minute interval".to_string(),
                        );
                    } else if duration.as_secs() > 86400 {
                        self.add_warning(
                            "pollInterval".to_string(),
                            ValidationWarningType::BestPractice,
                            "Very long poll interval may result in stale data".to_string(),
                            "Consider using at most 24 hours interval".to_string(),
                        );
                    }
                }
            }
        }

        // Cron schedule validation
        if let Some(cron_schedule) = &config.cron_schedule {
            if !is_valid_cron(cron_schedule) {
                self.add_error(
                    "cronSchedule".to_string(),
                    ValidationErrorType::InvalidCron,
                    "Cron schedule format is invalid".to_string(),
                    Some(cron_schedule.clone()),
                    Some("Format: 'sec min hour day month weekday' or '* * * * *'".to_string()),
                );
            }
        }

        // Check for conflicting schedule settings
        if config.poll_interval.is_some() && config.cron_schedule.is_some() {
            self.add_warning(
                "cronSchedule".to_string(),
                ValidationWarningType::Conflict,
                "Both pollInterval and cronSchedule are set".to_string(),
                "cronSchedule will take precedence over pollInterval".to_string(),
            );
        }

        // Device OS filter validation
        if config.device_os_filter.is_empty() {
            self.add_suggestion(
                "deviceOsFilter".to_string(),
                ValidationSuggestionType::Optimization,
                "No OS filter specified, will sync all devices".to_string(),
                Some("[\"Windows\", \"macOS\"]".to_string()),
            );
        } else {
            let valid_os_types = vec!["Windows", "macOS", "Android", "iOS", "Linux", "*"];
            for (i, os) in config.device_os_filter.iter().enumerate() {
                if !valid_os_types.contains(&os.as_str()) && os != "*" {
                    self.add_warning(
                        format!("deviceOsFilter[{}]", i),
                        ValidationWarningType::Compatibility,
                        format!("Unknown OS type: '{}'", os),
                        "Valid types: Windows, macOS, Android, iOS, Linux, *".to_string(),
                    );
                }
            }
        }
    }

    fn validate_database_config(&mut self, config: &crate::config::AppConfig) {
        if config.database.backends.is_empty() {
            self.add_error(
                "database.backends".to_string(),
                ValidationErrorType::Required,
                "At least one database backend must be specified".to_string(),
                None,
                Some("[\"sqlite\"]".to_string()),
            );
        }

        let valid_backends = vec!["sqlite", "postgres", "mssql"];
        for (i, backend) in config.database.backends.iter().enumerate() {
            if !valid_backends.contains(&backend.as_str()) {
                self.add_error(
                    format!("database.backends[{}]", i),
                    ValidationErrorType::InvalidValue,
                    format!("Unknown database backend: '{}'", backend),
                    Some(backend.clone()),
                    Some("Valid backends: sqlite, postgres, mssql".to_string()),
                );
            }
        }

        // SQLite validation
        if config.database.backends.contains(&"sqlite".to_string()) {
            let sqlite_path = &config.database.sqlite_path;
            if sqlite_path.is_empty() {
                if sqlite_path.is_empty() {
                    self.add_error(
                        "database.sqlitePath".to_string(),
                        ValidationErrorType::Required,
                        "SQLite path is required when using sqlite backend".to_string(),
                        None,
                        Some("./output/devices.db".to_string()),
                    );
                } else {
                    // Check if directory exists or can be created
                    if let Some(parent) = Path::new(sqlite_path).parent() {
                        if !parent.exists() {
                            self.add_warning(
                                "database.sqlitePath".to_string(),
                                ValidationWarningType::BestPractice,
                                format!("SQLite directory does not exist: {}", parent.display()),
                                "Directory will be created automatically".to_string(),
                            );
                        }
                    }
                }
            }
        }

        // PostgreSQL validation
        if config.database.backends.contains(&"postgres".to_string()) {
            if let Some(postgres_config) = &config.database.postgres {
                if postgres_config.connection_string.is_empty() {
                    self.add_error(
                        "database.postgres.connectionString".to_string(),
                        ValidationErrorType::Required,
                        "PostgreSQL connection string is required".to_string(),
                        None,
                        Some("postgres://user:password@localhost:5432/database".to_string()),
                    );
                } else if !is_valid_postgres_connection_string(&postgres_config.connection_string) {
                    self.add_error(
                        "database.postgres.connectionString".to_string(),
                        ValidationErrorType::InvalidConnectionString,
                        "Invalid PostgreSQL connection string format".to_string(),
                        Some(mask_connection_string(&postgres_config.connection_string)),
                        Some("postgres://user:password@host:port/database".to_string()),
                    );
                }
            } else {
                self.add_error(
                    "database.postgres".to_string(),
                    ValidationErrorType::Required,
                    "PostgreSQL configuration is required when using postgres backend".to_string(),
                    None,
                    None,
                );
            }
        }

        // MSSQL validation
        if config.database.backends.contains(&"mssql".to_string()) {
            if let Some(mssql_config) = &config.database.mssql {
                if mssql_config.connection_string.is_empty() {
                    self.add_error(
                        "database.mssql.connectionString".to_string(),
                        ValidationErrorType::Required,
                        "MSSQL connection string is required".to_string(),
                        None,
                        Some("server=localhost;database=db;trusted_connection=true".to_string()),
                    );
                } else if !is_valid_mssql_connection_string(&mssql_config.connection_string) {
                    self.add_error(
                        "database.mssql.connectionString".to_string(),
                        ValidationErrorType::InvalidConnectionString,
                        "Invalid MSSQL connection string format".to_string(),
                        Some(mask_connection_string(&mssql_config.connection_string)),
                        Some("server=host;database=db;uid=user;pwd=password".to_string()),
                    );
                }
            } else {
                self.add_error(
                    "database.mssql".to_string(),
                    ValidationErrorType::Required,
                    "MSSQL configuration is required when using mssql backend".to_string(),
                    None,
                    None,
                );
            }
        }

        // Table name validation
        if config.database.table_name.is_empty() {
            self.add_error(
                "database.tableName".to_string(),
                ValidationErrorType::Required,
                "Database table name is required".to_string(),
                None,
                Some("devices".to_string()),
            );
        } else if !is_valid_table_name(&config.database.table_name) {
            self.add_error(
                "database.tableName".to_string(),
                ValidationErrorType::InvalidFormat,
                "Invalid table name format".to_string(),
                Some(config.database.table_name.clone()),
                Some("Use alphanumeric characters and underscores only".to_string()),
            );
        }
    }

    fn validate_monitoring_config(&mut self, config: &crate::config::AppConfig) {
        // Prometheus port validation
        if config.prometheus_port == 0 {
            self.add_error(
                "prometheusPort".to_string(),
                ValidationErrorType::InvalidValue,
                "Prometheus port cannot be 0".to_string(),
                Some("0".to_string()),
                Some("9898".to_string()),
            );
        } else if config.prometheus_port < 1024 {
            self.add_warning(
                "prometheusPort".to_string(),
                ValidationWarningType::Security,
                "Using privileged port (< 1024) for Prometheus metrics".to_string(),
                "Consider using a port >= 1024".to_string(),
            );
        } else if config.prometheus_port > 65535 {
            self.add_error(
                "prometheusPort".to_string(),
                ValidationErrorType::InvalidRange,
                "Port number must be between 1 and 65535".to_string(),
                Some(config.prometheus_port.to_string()),
                Some("9898".to_string()),
            );
        }

        // Log level validation
        let valid_log_levels = vec!["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&config.log_level.as_str()) {
            self.add_error(
                "logLevel".to_string(),
                ValidationErrorType::InvalidValue,
                format!("Invalid log level: '{}'", config.log_level),
                Some(config.log_level.clone()),
                Some("Valid levels: trace, debug, info, warn, error".to_string()),
            );
        }

        // Performance suggestions
        if config.log_level == "trace" || config.log_level == "debug" {
            self.add_suggestion(
                "logLevel".to_string(),
                ValidationSuggestionType::Optimization,
                "Debug/trace logging may impact performance in production".to_string(),
                Some("info".to_string()),
            );
        }
    }

    fn validate_webhook_config(&mut self, webhook_config: &crate::webhook::WebhookConfig) {
        if webhook_config.enabled {
            // URL validation
            if webhook_config.url.is_empty() {
                self.add_error(
                    "webhook.url".to_string(),
                    ValidationErrorType::Required,
                    "Webhook URL is required when webhooks are enabled".to_string(),
                    None,
                    Some("https://your-webhook-endpoint.com/webhook".to_string()),
                );
            } else if let Err(_) = Url::parse(&webhook_config.url) {
                self.add_error(
                    "webhook.url".to_string(),
                    ValidationErrorType::InvalidUrl,
                    "Invalid webhook URL format".to_string(),
                    Some(webhook_config.url.clone()),
                    Some("https://example.com/webhook".to_string()),
                );
            } else {
                let url = Url::parse(&webhook_config.url).unwrap();
                if url.scheme() != "https" {
                    self.add_warning(
                        "webhook.url".to_string(),
                        ValidationWarningType::Security,
                        "Webhook URL should use HTTPS for security".to_string(),
                        "Use https:// instead of http://".to_string(),
                    );
                }
            }

            // Timeout validation
            if webhook_config.timeout_seconds == 0 {
                self.add_error(
                    "webhook.timeout_seconds".to_string(),
                    ValidationErrorType::InvalidValue,
                    "Webhook timeout cannot be 0".to_string(),
                    Some("0".to_string()),
                    Some("30".to_string()),
                );
            } else if webhook_config.timeout_seconds > 300 {
                self.add_warning(
                    "webhook.timeout_seconds".to_string(),
                    ValidationWarningType::Performance,
                    "Very long webhook timeout may block operations".to_string(),
                    "Consider using a timeout <= 60 seconds".to_string(),
                );
            }

            // Retry validation
            if webhook_config.retry_attempts > 10 {
                self.add_warning(
                    "webhook.retry_attempts".to_string(),
                    ValidationWarningType::Performance,
                    "Too many retry attempts may cause delays".to_string(),
                    "Consider using <= 5 retry attempts".to_string(),
                );
            }

            // Events validation
            if webhook_config.events.is_empty() {
                self.add_warning(
                    "webhook.events".to_string(),
                    ValidationWarningType::BestPractice,
                    "No webhook events specified".to_string(),
                    "Specify which events to send to webhook".to_string(),
                );
            }

            // Secret validation
            if webhook_config.secret.is_none() {
                self.add_suggestion(
                    "webhook.secret".to_string(),
                    ValidationSuggestionType::Security,
                    "Consider adding a webhook secret for authentication".to_string(),
                    Some("your-webhook-secret".to_string()),
                );
            }
        }
    }

    fn validate_backup_config(&mut self, backup_config: &crate::backup::BackupConfig) {
        if backup_config.enabled {
            // Directory validation
            if backup_config.directory.is_empty() {
                self.add_error(
                    "backup.directory".to_string(),
                    ValidationErrorType::Required,
                    "Backup directory is required when backups are enabled".to_string(),
                    None,
                    Some("./backups".to_string()),
                );
            }

            // Max backups validation
            if backup_config.max_backups == 0 {
                self.add_error(
                    "backup.maxBackups".to_string(),
                    ValidationErrorType::InvalidValue,
                    "Maximum backup count cannot be 0".to_string(),
                    Some("0".to_string()),
                    Some("10".to_string()),
                );
            } else if backup_config.max_backups > 100 {
                self.add_warning(
                    "backup.maxBackups".to_string(),
                    ValidationWarningType::Performance,
                    "Very high backup count may consume excessive disk space".to_string(),
                    "Consider using <= 50 backups".to_string(),
                );
            }

            // Schedule interval validation
            if backup_config.schedule_enabled {
                if let Some(interval) = &backup_config.schedule_interval {
                    if !is_valid_duration(interval) {
                        self.add_error(
                            "backup.scheduleInterval".to_string(),
                            ValidationErrorType::InvalidDuration,
                            "Invalid backup schedule interval".to_string(),
                            Some(interval.clone()),
                            Some("24h".to_string()),
                        );
                    }
                }
            }
        }
    }

    fn validate_rate_limit_config(&mut self, rate_limit_config: &crate::rate_limiter::RateLimitConfig) {
        // Max requests validation
        if rate_limit_config.max_requests_per_minute == 0 {
            self.add_error(
                "rateLimit.maxRequestsPerMinute".to_string(),
                ValidationErrorType::InvalidValue,
                "Maximum requests per minute cannot be 0".to_string(),
                Some("0".to_string()),
                Some("60".to_string()),
            );
        } else if rate_limit_config.max_requests_per_minute > 1000 {
            self.add_warning(
                "rateLimit.maxRequestsPerMinute".to_string(),
                ValidationWarningType::Performance,
                "Very high request rate may trigger API rate limiting".to_string(),
                "Microsoft Graph API has rate limits".to_string(),
            );
        }

        // Retry delay validation
        if rate_limit_config.max_retry_delay_seconds > 3600 {
            self.add_warning(
                "rateLimit.maxRetryDelaySeconds".to_string(),
                ValidationWarningType::Performance,
                "Very long maximum retry delay may cause long sync times".to_string(),
                "Consider using <= 300 seconds (5 minutes)".to_string(),
            );
        }

        // Backoff multiplier validation
        if rate_limit_config.backoff_multiplier < 1.0 {
            self.add_error(
                "rateLimit.backoffMultiplier".to_string(),
                ValidationErrorType::InvalidValue,
                "Backoff multiplier must be >= 1.0".to_string(),
                Some(rate_limit_config.backoff_multiplier.to_string()),
                Some("2.0".to_string()),
            );
        } else if rate_limit_config.backoff_multiplier > 10.0 {
            self.add_warning(
                "rateLimit.backoffMultiplier".to_string(),
                ValidationWarningType::Performance,
                "Very high backoff multiplier may cause excessive delays".to_string(),
                "Consider using <= 3.0".to_string(),
            );
        }
    }

    fn validate_mock_config(&mut self, mock_config: &crate::mock_graph_api::MockGraphApiConfig) {
        if mock_config.enabled {
            self.add_suggestion(
                "mockGraphApi.enabled".to_string(),
                ValidationSuggestionType::Reliability,
                "Mock API is enabled - ensure this is intended for testing".to_string(),
                None,
            );

            // Device count validation
            if mock_config.device_count > 10000 {
                self.add_warning(
                    "mockGraphApi.deviceCount".to_string(),
                    ValidationWarningType::Performance,
                    "Very high mock device count may impact performance".to_string(),
                    "Consider using <= 1000 devices for testing".to_string(),
                );
            }

            // Probability validations
            if mock_config.rate_limit_probability > 1.0 || mock_config.rate_limit_probability < 0.0 {
                self.add_error(
                    "mockGraphApi.rateLimitProbability".to_string(),
                    ValidationErrorType::InvalidRange,
                    "Probability must be between 0.0 and 1.0".to_string(),
                    Some(mock_config.rate_limit_probability.to_string()),
                    Some("0.1".to_string()),
                );
            }
        }
    }

    fn add_error(&mut self, field_path: String, error_type: ValidationErrorType, message: String, current_value: Option<String>, expected_format: Option<String>) {
        self.add_error_with_position(field_path, error_type, message, current_value, expected_format, None);
    }

    fn add_error_with_position(&mut self, field_path: String, error_type: ValidationErrorType, message: String, current_value: Option<String>, expected_format: Option<String>, position: Option<(u32, u32)>) {
        let (line_number, column_number) = position.unwrap_or((0, 0));
        self.errors.push(ValidationError {
            field_path,
            error_type,
            message,
            current_value,
            expected_format,
            line_number: Some(line_number),
            column_number: Some(column_number),
        });
    }

    fn add_warning(&mut self, field_path: String, warning_type: ValidationWarningType, message: String, recommendation: String) {
        self.warnings.push(ValidationWarning {
            field_path,
            warning_type,
            message,
            recommendation,
        });
    }

    fn add_suggestion(&mut self, field_path: String, suggestion_type: ValidationSuggestionType, message: String, suggested_value: Option<String>) {
        self.suggestions.push(ValidationSuggestion {
            field_path,
            suggestion_type,
            message,
            suggested_value,
        });
    }

    fn build_result(self) -> ValidationResult {
        ValidationResult {
            is_valid: self.errors.is_empty(),
            errors: self.errors,
            warnings: self.warnings,
            suggestions: self.suggestions,
        }
    }
}

// Helper functions for validation

fn is_valid_uuid(s: &str) -> bool {
    Uuid::parse_str(s).is_ok()
}

fn is_valid_duration(s: &str) -> bool {
    parse_duration(s).is_some()
}

fn parse_duration(s: &str) -> Option<std::time::Duration> {
    // Simple duration parser for common formats
    let re = Regex::new(r"^(\d+)([smhd])$").ok()?;
    let caps = re.captures(s)?;

    let value: u64 = caps.get(1)?.as_str().parse().ok()?;
    let unit = caps.get(2)?.as_str();

    match unit {
        "s" => Some(std::time::Duration::from_secs(value)),
        "m" => Some(std::time::Duration::from_secs(value * 60)),
        "h" => Some(std::time::Duration::from_secs(value * 3600)),
        "d" => Some(std::time::Duration::from_secs(value * 86400)),
        _ => None,
    }
}

fn is_valid_cron(s: &str) -> bool {
    // Basic cron validation - 5 or 6 fields
    let fields: Vec<&str> = s.split_whitespace().collect();
    fields.len() == 5 || fields.len() == 6
}

fn is_valid_postgres_connection_string(s: &str) -> bool {
    s.starts_with("postgres://") || s.starts_with("postgresql://")
}

fn is_valid_mssql_connection_string(s: &str) -> bool {
    s.contains("server=") || s.contains("Server=") || s.contains("data source=") || s.contains("Data Source=")
}

fn is_valid_table_name(s: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap();
    re.is_match(s)
}

fn mask_connection_string(s: &str) -> String {
    // Mask passwords in connection strings
    let password_patterns = vec![
        (r"password=([^;]+)", "password=***"),
        (r"pwd=([^;]+)", "pwd=***"),
        (r"://[^:]+:([^@]+)@", "://*:***@"),
    ];

    let mut masked = s.to_string();
    for (pattern, replacement) in password_patterns {
        if let Ok(re) = Regex::new(pattern) {
            masked = re.replace_all(&masked, replacement).to_string();
        }
    }
    masked
}

fn extract_json_error_position(error: &serde_json::Error) -> (Option<u32>, Option<u32>) {
    // Extract line and column from JSON error if available
    let line = error.line();
    (Some(line as u32), Some(error.column() as u32))
}

// CLI command for config validation
pub fn validate_config_command(config_path: Option<String>) -> Result<()> {
    let config_path = config_path.unwrap_or_else(|| "config.json".to_string());

    info!("Validating configuration file: {}", config_path);

    match ConfigValidator::validate_config_file(&config_path) {
        Ok(result) => {
            println!("{}", result);

            if !result.is_valid {
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!("Failed to validate configuration: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_valid_config() {
        let config_content = r#"
        {
            "clientId": "12345678-1234-1234-1234-123456789012",
            "clientSecret": "valid-secret-here",
            "tenantId": "87654321-4321-4321-4321-210987654321",
            "pollInterval": "1h",
            "deviceOsFilter": ["Windows", "macOS"],
            "enablePrometheus": true,
            "prometheusPort": 9898,
            "logLevel": "info",
            "database": {
                "backends": ["sqlite"],
                "tableName": "devices",
                "sqlitePath": "./output/devices.db"
            }
        }
        "#;

        let result = ConfigValidator::validate_config_content(config_content).unwrap();
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_invalid_config() {
        let config_content = r#"
        {
            "clientId": "invalid-uuid",
            "clientSecret": "",
            "tenantId": "YOUR_TENANT_ID",
            "pollInterval": "invalid-duration",
            "prometheusPort": 0,
            "logLevel": "invalid-level",
            "database": {
                "backends": [],
                "tableName": ""
            }
        }
        "#;

        let result = ConfigValidator::validate_config_content(config_content).unwrap();
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_json_syntax_error() {
        let config_content = r#"
        {
            "clientId": "12345678-1234-1234-1234-123456789012",
            "clientSecret": "valid-secret",
            "tenantId": "87654321-4321-4321-4321-210987654321"
            // Missing comma here
            "pollInterval": "1h"
        }
        "#;

        let result = ConfigValidator::validate_config_content(config_content).unwrap();
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].message.contains("JSON syntax error"));
    }

    #[test]
    fn test_duration_parsing() {
        assert!(is_valid_duration("30s"));
        assert!(is_valid_duration("5m"));
        assert!(is_valid_duration("2h"));
        assert!(is_valid_duration("1d"));
        assert!(!is_valid_duration("invalid"));
        assert!(!is_valid_duration("30"));
        assert!(!is_valid_duration("5x"));
    }

    #[test]
    fn test_uuid_validation() {
        assert!(is_valid_uuid("12345678-1234-1234-1234-123456789012"));
        assert!(!is_valid_uuid("invalid-uuid"));
        assert!(!is_valid_uuid("12345678-1234-1234-1234"));
        assert!(!is_valid_uuid(""));
    }

    #[test]
    fn test_connection_string_masking() {
        let postgres = "postgres://user:secret123@localhost:5432/db";
        let masked = mask_connection_string(postgres);
        assert!(!masked.contains("secret123"));
        assert!(masked.contains("***"));

        let mssql = "server=localhost;database=db;uid=user;pwd=secret123";
        let masked = mask_connection_string(mssql);
        assert!(!masked.contains("secret123"));
        assert!(masked.contains("***"));
    }
}