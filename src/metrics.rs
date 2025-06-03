use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use lazy_static::lazy_static;
use log::{error, info};
use prometheus::{
    register_counter, register_gauge, register_histogram, Counter, Gauge, Histogram,
    TextEncoder,
};
use std::net::SocketAddr;

lazy_static! {
    // Sync metrics
    pub static ref SYNC_SUCCESS_TOTAL: Counter = register_counter!(
        "sync_success_total",
        "Total number of successful sync operations"
    ).unwrap();
    
    pub static ref SYNC_FAILURE_TOTAL: Counter = register_counter!(
        "sync_failure_total", 
        "Total number of failed sync operations"
    ).unwrap();
    
    pub static ref SYNC_DURATION_SECONDS: Histogram = register_histogram!(
        "sync_duration_seconds",
        "Duration of sync operations in seconds"
    ).unwrap();
    
    // Device metrics
    pub static ref DEVICES_FETCHED_TOTAL: Counter = register_counter!(
        "devices_fetched_total",
        "Total number of devices fetched from Intune"
    ).unwrap();
    
    pub static ref DEVICES_PROCESSED_TOTAL: Counter = register_counter!(
        "devices_processed_total",
        "Total number of devices processed"
    ).unwrap();
    
    pub static ref DEVICES_CURRENT_COUNT: Gauge = register_gauge!(
        "devices_current_count",
        "Current number of devices in the system"
    ).unwrap();
    
    // Filter metrics
    pub static ref DEVICE_FILTER_MATCHED_TOTAL: Counter = register_counter!(
        "device_filter_matched_total",
        "Number of devices allowed by OS filter"
    ).unwrap();
    
    pub static ref DEVICE_FILTER_SKIPPED_TOTAL: Counter = register_counter!(
        "device_filter_skipped_total",
        "Number of devices skipped due to OS filter"
    ).unwrap();
    
    // Authentication metrics
    pub static ref TOKEN_REFRESH_TOTAL: Counter = register_counter!(
        "token_refresh_total",
        "Total number of token refresh operations"
    ).unwrap();
    
    pub static ref AUTH_FAILURE_TOTAL: Counter = register_counter!(
        "auth_failure_total",
        "Total number of authentication failures"
    ).unwrap();
    
    // Database metrics
    pub static ref DB_INSERT_TOTAL: Counter = register_counter!(
        "db_insert_total",
        "Total number of database insert operations"
    ).unwrap();
    
    pub static ref DB_UPDATE_TOTAL: Counter = register_counter!(
        "db_update_total",
        "Total number of database update operations"
    ).unwrap();
    
    pub static ref DB_SKIP_TOTAL: Counter = register_counter!(
        "db_skip_total",
        "Total number of database operations skipped (no changes)"
    ).unwrap();
    
    pub static ref DB_ERROR_TOTAL: Counter = register_counter!(
        "db_error_total",
        "Total number of database errors"
    ).unwrap();
    
    pub static ref DB_OPERATION_DURATION_SECONDS: Histogram = register_histogram!(
        "db_operation_duration_seconds",
        "Duration of database operations in seconds"
    ).unwrap();
    
    // HTTP metrics
    pub static ref HTTP_REQUESTS_TOTAL: Counter = register_counter!(
        "http_requests_total",
        "Total number of HTTP requests made"
    ).unwrap();
    
    pub static ref HTTP_REQUEST_DURATION_SECONDS: Histogram = register_histogram!(
        "http_request_duration_seconds",
        "Duration of HTTP requests in seconds"
    ).unwrap();
    
    pub static ref HTTP_ERRORS_TOTAL: Counter = register_counter!(
        "http_errors_total",
        "Total number of HTTP errors"
    ).unwrap();
}

pub fn init_metrics() {
    info!("Initializing Prometheus metrics");
    
    // Initialize all metrics to ensure they appear in /metrics even with zero values
    SYNC_SUCCESS_TOTAL.inc_by(0.0);
    SYNC_FAILURE_TOTAL.inc_by(0.0);
    DEVICES_FETCHED_TOTAL.inc_by(0.0);
    DEVICES_PROCESSED_TOTAL.inc_by(0.0);
    DEVICES_CURRENT_COUNT.set(0.0);
    DEVICE_FILTER_MATCHED_TOTAL.inc_by(0.0);
    DEVICE_FILTER_SKIPPED_TOTAL.inc_by(0.0);
    TOKEN_REFRESH_TOTAL.inc_by(0.0);
    AUTH_FAILURE_TOTAL.inc_by(0.0);
    DB_INSERT_TOTAL.inc_by(0.0);
    DB_UPDATE_TOTAL.inc_by(0.0);
    DB_SKIP_TOTAL.inc_by(0.0);
    DB_ERROR_TOTAL.inc_by(0.0);
    HTTP_REQUESTS_TOTAL.inc_by(0.0);
    HTTP_ERRORS_TOTAL.inc_by(0.0);
    
    info!("Prometheus metrics initialized");
}

pub async fn start_metrics_server(port: u16) {
    let app = Router::new().route("/metrics", get(metrics_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Starting Prometheus metrics server on {}", addr);

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Failed to bind metrics server: {}", e);
            return;
        }
    };

    if let Err(e) = axum::serve(listener, app).await {
        error!("Metrics server error: {}", e);
    }
}

async fn metrics_handler() -> Response {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    
    match encoder.encode_to_string(&metric_families) {
        Ok(output) => (StatusCode::OK, output).into_response(),
        Err(e) => {
            error!("Failed to encode metrics: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to encode metrics").into_response()
        }
    }
}

/// Helper struct for timing operations
pub struct Timer {
    pub start: std::time::Instant,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }
    
    pub fn observe_duration(self, histogram: &Histogram) {
        let duration = self.start.elapsed();
        histogram.observe(duration.as_secs_f64());
    }
}

/// Macro for timing operations
#[macro_export]
macro_rules! time_operation {
    ($histogram:expr, $operation:expr) => {{
        let timer = $crate::metrics::Timer::new();
        let result = $operation;
        timer.observe_duration(&$histogram);
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_metrics_initialization() {
        init_metrics();
        
        // Verify metrics are initialized
        assert_eq!(SYNC_SUCCESS_TOTAL.get(), 0.0);
        assert_eq!(DEVICES_CURRENT_COUNT.get(), 0.0);
    }
    
    #[test]
    fn test_timer() {
        let timer = Timer::new();
        std::thread::sleep(Duration::from_millis(10));
        
        // Just verify the timer doesn't panic
        timer.observe_duration(&SYNC_DURATION_SECONDS);
    }
}
