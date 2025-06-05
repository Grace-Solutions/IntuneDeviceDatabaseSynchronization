use anyhow::Result;
use flexi_logger::{
    Age, Cleanup, Criterion, DeferredNow, FileSpec, Logger, Naming, Record, WriteMode,
};
use log::LevelFilter;
use std::io::{self, Write};

use crate::config::AppConfig;
use crate::path_utils;

/// Custom log format: 2025/06/02 23:58:36.434 - [ProcessID:ThreadID] - [Level] - [Component] - Message
pub fn custom_format(
    w: &mut dyn Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), io::Error> {
    let process_id = std::process::id();

    // Extract just the thread number from ThreadId
    let thread_id_str = format!("{:?}", std::thread::current().id());
    let thread_id = thread_id_str
        .strip_prefix("ThreadId(")
        .and_then(|s| s.strip_suffix(")"))
        .unwrap_or("0");

    // Extract component from target or use module path
    let component = if record.target().is_empty() {
        record.module_path().unwrap_or("unknown")
    } else {
        record.target()
    };

    write!(
        w,
        "{} - [{}:{}] - [{}] - [{}] - {}",
        now.format("%Y/%m/%d %H:%M:%S%.3f"),
        process_id,
        thread_id,
        record.level(),
        component,
        record.args()
    )
}

/// Sets up structured logging with rotation
pub async fn setup_logging(_config: &AppConfig) -> Result<()> {
    let log_level = determine_log_level();

    // Determine logs directory - default to "logs" next to executable
    let logs_dir = path_utils::resolve_logs_path("logs")?;

    // Ensure logs directory exists
    path_utils::ensure_directory_exists(&logs_dir).await?;

    let _logger = Logger::try_with_str(&log_level)?
        .log_to_file(
            FileSpec::default()
                .directory(&logs_dir)
                .basename("MSGraphDBSynchronizer")
                .suffix("log")
        )
        .rotate(
            Criterion::Age(Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(30), // Keep 30 days of logs
        )
        .write_mode(WriteMode::Async)
        .format(custom_format)
        .duplicate_to_stderr(flexi_logger::Duplicate::Info) // Also log to stderr for service mode
        .start()?;

    // Set global logger
    log::set_max_level(parse_log_level(&log_level));

    log::info!("Logging initialized with level: {}", log_level);
    log::info!("Log files will be written to: {}", logs_dir.display());

    Ok(())
}

/// Determines the appropriate log level from environment or defaults to INFO
fn determine_log_level() -> String {
    std::env::var("RUST_LOG").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "debug".to_string()
        } else {
            "info".to_string()
        }
    })
}

/// Parses log level string to LevelFilter
fn parse_log_level(level: &str) -> LevelFilter {
    match level.to_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

/// Sanitizes sensitive information from log messages
pub fn sanitize_log_message(message: &str) -> String {
    let mut sanitized = message.to_string();
    
    // List of patterns to sanitize
    let sensitive_patterns = [
        (r"client_secret=[^&\s]+", "client_secret=***"),
        (r"password=[^&\s]+", "password=***"),
        (r"token=[^&\s]+", "token=***"),
        (r"Bearer [A-Za-z0-9\-._~+/]+=*", "Bearer ***"),
        (r"Authorization: [^\r\n]+", "Authorization: ***"),
    ];
    
    for (pattern, replacement) in &sensitive_patterns {
        if let Ok(regex) = regex::Regex::new(pattern) {
            sanitized = regex.replace_all(&sanitized, *replacement).to_string();
        }
    }
    
    sanitized
}

/// Logs device processing information safely
pub fn log_device_processing(
    device_name: &str,
    device_id: &str,
    operation: &str,
    details: Option<&str>,
) {
    let sanitized_name = sanitize_log_message(device_name);
    let sanitized_id = if device_id.len() > 8 {
        format!("{}***", &device_id[..8])
    } else {
        "***".to_string()
    };
    
    if let Some(details) = details {
        let sanitized_details = sanitize_log_message(details);
        log::info!(
            "[Device] - {} device '{}' (ID: {}) - {}",
            operation,
            sanitized_name,
            sanitized_id,
            sanitized_details
        );
    } else {
        log::info!(
            "[Device] - {} device '{}' (ID: {})",
            operation,
            sanitized_name,
            sanitized_id
        );
    }
}

/// Logs authentication events safely
pub fn log_auth_event(event: &str, details: Option<&str>) {
    if let Some(details) = details {
        let sanitized_details = sanitize_log_message(details);
        log::info!("[Auth] - {} - {}", event, sanitized_details);
    } else {
        log::info!("[Auth] - {}", event);
    }
}

/// Logs database operations
pub fn log_database_operation(
    backend: &str,
    operation: &str,
    affected_rows: Option<usize>,
    duration: Option<std::time::Duration>,
) {
    let mut message = format!("[Database] - {} - {}", backend, operation);
    
    if let Some(rows) = affected_rows {
        message.push_str(&format!(" - {} rows", rows));
    }
    
    if let Some(duration) = duration {
        message.push_str(&format!(" - {:.2}ms", duration.as_millis()));
    }
    
    log::info!("{}", message);
}

/// Logs sync operations with metrics
pub fn log_sync_operation(
    operation: &str,
    devices_processed: usize,
    devices_filtered: usize,
    duration: std::time::Duration,
    errors: usize,
) {
    log::info!(
        "[Sync] - {} - Processed: {}, Filtered: {}, Errors: {}, Duration: {:.2}s",
        operation,
        devices_processed,
        devices_filtered,
        errors,
        duration.as_secs_f64()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_log_message() {
        let message = "client_secret=super_secret_value&other=data";
        let sanitized = sanitize_log_message(message);
        assert_eq!(sanitized, "client_secret=***&other=data");
        
        let bearer_message = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        let sanitized_bearer = sanitize_log_message(bearer_message);
        assert_eq!(sanitized_bearer, "Authorization: ***");
        
        let normal_message = "This is a normal log message";
        let sanitized_normal = sanitize_log_message(normal_message);
        assert_eq!(sanitized_normal, normal_message);
    }

    #[test]
    fn test_parse_log_level() {
        assert_eq!(parse_log_level("error"), LevelFilter::Error);
        assert_eq!(parse_log_level("ERROR"), LevelFilter::Error);
        assert_eq!(parse_log_level("info"), LevelFilter::Info);
        assert_eq!(parse_log_level("debug"), LevelFilter::Debug);
        assert_eq!(parse_log_level("invalid"), LevelFilter::Info);
    }

    #[test]
    fn test_determine_log_level() {
        // This test depends on environment, so just verify it returns a string
        let level = determine_log_level();
        assert!(!level.is_empty());
    }
}
