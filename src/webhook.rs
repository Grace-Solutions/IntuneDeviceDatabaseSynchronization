use std::collections::HashMap;
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use log::{info, warn, error, debug};
use reqwest::Client;
use tokio::time::timeout;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub enabled: bool,
    pub url: String,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
    pub retry_delay_seconds: u64,
    pub events: Vec<WebhookEvent>,
    pub headers: Option<HashMap<String, String>>,
    pub secret: Option<String>,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            timeout_seconds: 30,
            retry_attempts: 3,
            retry_delay_seconds: 5,
            events: vec![
                WebhookEvent::SyncStarted,
                WebhookEvent::SyncCompleted,
                WebhookEvent::SyncFailed,
                WebhookEvent::DevicesUpdated,
            ],
            headers: None,
            secret: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEvent {
    SyncStarted,
    SyncCompleted,
    SyncFailed,
    DevicesUpdated,
    DatabaseError,
    AuthenticationFailed,
    ConfigurationChanged,
}

#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    pub event: WebhookEvent,
    pub timestamp: DateTime<Utc>,
    pub service: String,
    pub version: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct SyncStartedData {
    pub sync_id: String,
    pub scheduled: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncCompletedData {
    pub sync_id: String,
    pub duration_seconds: f64,
    pub devices_fetched: u32,
    pub devices_updated: u32,
    pub devices_inserted: u32,
    pub devices_skipped: u32,
}

#[derive(Debug, Serialize)]
pub struct SyncFailedData {
    pub sync_id: String,
    pub error: String,
    pub duration_seconds: f64,
}

#[derive(Debug, Serialize)]
pub struct DevicesUpdatedData {
    pub sync_id: String,
    pub updated_count: u32,
    pub inserted_count: u32,
    pub total_devices: u32,
}

#[derive(Debug, Serialize)]
pub struct DatabaseErrorData {
    pub operation: String,
    pub error: String,
    pub table: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthenticationFailedData {
    pub error: String,
    pub tenant_id: String,
}

pub struct WebhookManager {
    config: WebhookConfig,
    client: Client,
}

impl WebhookManager {
    pub fn new(config: WebhookConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("Failed to create HTTP client for webhooks");

        Self { config, client }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled && !self.config.url.is_empty()
    }

    pub fn should_send_event(&self, event: &WebhookEvent) -> bool {
        self.is_enabled() && self.config.events.contains(event)
    }

    pub async fn send_sync_started(&self, sync_id: String, scheduled: bool) -> Result<()> {
        if !self.should_send_event(&WebhookEvent::SyncStarted) {
            return Ok(());
        }

        let data = SyncStartedData { sync_id, scheduled };
        self.send_webhook(WebhookEvent::SyncStarted, serde_json::to_value(data)?).await
    }

    pub async fn send_sync_completed(
        &self,
        sync_id: String,
        duration_seconds: f64,
        devices_fetched: u32,
        devices_updated: u32,
        devices_inserted: u32,
        devices_skipped: u32,
    ) -> Result<()> {
        if !self.should_send_event(&WebhookEvent::SyncCompleted) {
            return Ok(());
        }

        let data = SyncCompletedData {
            sync_id,
            duration_seconds,
            devices_fetched,
            devices_updated,
            devices_inserted,
            devices_skipped,
        };
        self.send_webhook(WebhookEvent::SyncCompleted, serde_json::to_value(data)?).await
    }

    pub async fn send_sync_failed(&self, sync_id: String, error: String, duration_seconds: f64) -> Result<()> {
        if !self.should_send_event(&WebhookEvent::SyncFailed) {
            return Ok(());
        }

        let data = SyncFailedData {
            sync_id,
            error,
            duration_seconds,
        };
        self.send_webhook(WebhookEvent::SyncFailed, serde_json::to_value(data)?).await
    }

    pub async fn send_devices_updated(&self, sync_id: String, updated_count: u32, inserted_count: u32, total_devices: u32) -> Result<()> {
        if !self.should_send_event(&WebhookEvent::DevicesUpdated) {
            return Ok(());
        }

        let data = DevicesUpdatedData {
            sync_id,
            updated_count,
            inserted_count,
            total_devices,
        };
        self.send_webhook(WebhookEvent::DevicesUpdated, serde_json::to_value(data)?).await
    }

    pub async fn send_database_error(&self, operation: String, error: String, table: Option<String>) -> Result<()> {
        if !self.should_send_event(&WebhookEvent::DatabaseError) {
            return Ok(());
        }

        let data = DatabaseErrorData {
            operation,
            error,
            table,
        };
        self.send_webhook(WebhookEvent::DatabaseError, serde_json::to_value(data)?).await
    }

    pub async fn send_authentication_failed(&self, error: String, tenant_id: String) -> Result<()> {
        if !self.should_send_event(&WebhookEvent::AuthenticationFailed) {
            return Ok(());
        }

        let data = AuthenticationFailedData { error, tenant_id };
        self.send_webhook(WebhookEvent::AuthenticationFailed, serde_json::to_value(data)?).await
    }

    async fn send_webhook(&self, event: WebhookEvent, data: serde_json::Value) -> Result<()> {
        let payload = WebhookPayload {
            event: event.clone(),
            timestamp: Utc::now(),
            service: "IntuneDeviceDatabaseSynchronization".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            data,
        };

        debug!("Sending webhook for event: {:?}", event);

        for attempt in 1..=self.config.retry_attempts {
            match self.send_webhook_attempt(&payload).await {
                Ok(_) => {
                    info!("Webhook sent successfully for event: {:?}", event);
                    return Ok(());
                }
                Err(e) => {
                    warn!("Webhook attempt {} failed for event {:?}: {}", attempt, event, e);
                    
                    if attempt < self.config.retry_attempts {
                        tokio::time::sleep(Duration::from_secs(self.config.retry_delay_seconds)).await;
                    }
                }
            }
        }

        error!("All webhook attempts failed for event: {:?}", event);
        Err(anyhow::anyhow!("Failed to send webhook after {} attempts", self.config.retry_attempts))
    }

    async fn send_webhook_attempt(&self, payload: &WebhookPayload) -> Result<()> {
        let mut request = self.client.post(&self.config.url);

        // Add custom headers
        if let Some(headers) = &self.config.headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }

        // Add content type
        request = request.header("Content-Type", "application/json");

        // Add signature if secret is configured (simplified - just add as header)
        if let Some(secret) = &self.config.secret {
            request = request.header("X-Webhook-Secret", secret);
        }

        // Send request with timeout
        let response = timeout(
            Duration::from_secs(self.config.timeout_seconds),
            request.json(payload).send()
        ).await
        .context("Webhook request timed out")?
        .context("Failed to send webhook request")?;

        if response.status().is_success() {
            debug!("Webhook response: {}", response.status());
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unable to read response body".to_string());
            Err(anyhow::anyhow!("Webhook failed with status {}: {}", status, body))
        }
    }



    pub fn update_config(&mut self, config: WebhookConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_config_default() {
        let config = WebhookConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.retry_attempts, 3);
        assert!(config.events.contains(&WebhookEvent::SyncStarted));
    }

    #[test]
    fn test_webhook_manager_enabled() {
        let config = WebhookConfig {
            enabled: true,
            url: "https://example.com/webhook".to_string(),
            ..Default::default()
        };
        
        let manager = WebhookManager::new(config);
        assert!(manager.is_enabled());
        assert!(manager.should_send_event(&WebhookEvent::SyncStarted));
    }

    #[test]
    fn test_webhook_manager_disabled() {
        let config = WebhookConfig::default();
        let manager = WebhookManager::new(config);
        assert!(!manager.is_enabled());
        assert!(!manager.should_send_event(&WebhookEvent::SyncStarted));
    }

    #[test]
    fn test_webhook_secret_header() {
        let config = WebhookConfig {
            secret: Some("test-secret".to_string()),
            ..Default::default()
        };

        let manager = WebhookManager::new(config);
        assert!(manager.config.secret.is_some());
        assert_eq!(manager.config.secret.as_ref().unwrap(), "test-secret");
    }
}
