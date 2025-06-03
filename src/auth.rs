use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use log::{debug, info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::metrics;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    scope: String,
}

#[derive(Debug, Clone)]
pub struct AccessToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

impl AccessToken {
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    pub fn is_expiring_soon(&self) -> bool {
        // Consider token expiring if it expires within 5 minutes
        Utc::now() + chrono::Duration::minutes(5) >= self.expires_at
    }
}

pub struct AuthClient {
    config: AppConfig,
    client: Client,
    token: Arc<RwLock<Option<AccessToken>>>,
}

impl AuthClient {
    pub fn new(config: AppConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            client,
            token: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn get_access_token(&self) -> Result<String> {
        // Check if we have a valid token
        {
            let token_guard = self.token.read().await;
            if let Some(ref token) = *token_guard {
                if !token.is_expiring_soon() {
                    debug!("Using cached access token");
                    return Ok(token.token.clone());
                }
            }
        }

        // Need to refresh the token
        info!("Refreshing access token");
        let new_token = self.refresh_token().await?;
        
        // Update the cached token
        {
            let mut token_guard = self.token.write().await;
            *token_guard = Some(new_token.clone());
        }

        metrics::TOKEN_REFRESH_TOTAL.inc();
        Ok(new_token.token)
    }

    async fn refresh_token(&self) -> Result<AccessToken> {
        let token_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.config.tenant_id
        );

        let params = [
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
            ("scope", &"https://graph.microsoft.com/.default".to_string()),
            ("grant_type", &"client_credentials".to_string()),
        ];

        debug!("Requesting access token from: {}", token_url);

        let response = self
            .client
            .post(&token_url)
            .form(&params)
            .send()
            .await
            .context("Failed to send token request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            warn!("Token request failed with status {}: {}", status, error_text);
            return Err(anyhow::anyhow!(
                "Token request failed with status {}: {}",
                status,
                error_text
            ));
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .context("Failed to parse token response")?;

        let expires_at = Utc::now() + chrono::Duration::seconds(token_response.expires_in as i64);

        info!("Successfully obtained access token, expires at: {}", expires_at);

        Ok(AccessToken {
            token: token_response.access_token,
            expires_at,
        })
    }

    pub async fn make_authenticated_request(&self, url: &str) -> Result<reqwest::Response> {
        let token = self.get_access_token().await?;
        
        let response = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .send()
            .await
            .context("Failed to make authenticated request")?;

        if response.status() == 401 {
            // Token might be invalid, clear cache and retry once
            warn!("Received 401, clearing token cache and retrying");
            {
                let mut token_guard = self.token.write().await;
                *token_guard = None;
            }
            
            let new_token = self.get_access_token().await?;
            let retry_response = self
                .client
                .get(url)
                .header("Authorization", format!("Bearer {}", new_token))
                .header("Content-Type", "application/json")
                .send()
                .await
                .context("Failed to make authenticated request on retry")?;
            
            return Ok(retry_response);
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_token_expiry() {
        let token = AccessToken {
            token: "test_token".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
        };
        
        assert!(!token.is_expired());
        assert!(!token.is_expiring_soon());
        
        let expiring_token = AccessToken {
            token: "test_token".to_string(),
            expires_at: Utc::now() + Duration::minutes(2),
        };
        
        assert!(!expiring_token.is_expired());
        assert!(expiring_token.is_expiring_soon());
        
        let expired_token = AccessToken {
            token: "test_token".to_string(),
            expires_at: Utc::now() - Duration::minutes(1),
        };
        
        assert!(expired_token.is_expired());
        assert!(expired_token.is_expiring_soon());
    }
}
