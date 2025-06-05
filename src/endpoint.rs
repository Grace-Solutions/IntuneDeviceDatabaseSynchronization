use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use log::{info, debug};
use std::collections::HashMap;
use reqwest::Client;
use crate::auth::AuthClient;
use crate::mock_graph_api::MockGraphApi;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    /// Name/identifier for this endpoint
    pub name: String,
    /// Microsoft Graph API endpoint URL
    #[serde(rename = "endpointUrl")]
    pub endpoint_url: String,
    /// Database table name for this endpoint's data
    #[serde(rename = "tableName")]
    pub table_name: String,
    /// Enable this endpoint for synchronization
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Sync interval override (optional, uses global if not set)
    #[serde(rename = "syncInterval")]
    pub sync_interval: Option<String>,
    /// Additional query parameters for the endpoint
    #[serde(rename = "queryParams", default)]
    pub query_params: HashMap<String, String>,
    /// Fields to select from the API response (optional)
    #[serde(rename = "selectFields")]
    pub select_fields: Option<Vec<String>>,
    /// Filter expression for the API query (optional)
    pub filter: Option<String>,
    /// Custom field mappings for database storage
    #[serde(rename = "fieldMappings", default)]
    pub field_mappings: HashMap<String, String>,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            name: "devices".to_string(),
            endpoint_url: "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices".to_string(),
            table_name: "devices".to_string(),
            enabled: true,
            sync_interval: None,
            query_params: HashMap::new(),
            select_fields: None,
            filter: None,
            field_mappings: HashMap::new(),
        }
    }
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointsConfig {
    /// List of endpoints to synchronize
    pub endpoints: Vec<EndpointConfig>,
}

impl Default for EndpointsConfig {
    fn default() -> Self {
        Self {
            endpoints: vec![EndpointConfig::default()],
        }
    }
}

impl EndpointsConfig {
    /// Get all enabled endpoints
    pub fn get_enabled_endpoints(&self) -> Vec<&EndpointConfig> {
        self.endpoints.iter().filter(|e| e.enabled).collect()
    }

    /// Get endpoint by name
    pub fn get_endpoint_by_name(&self, name: &str) -> Option<&EndpointConfig> {
        self.endpoints.iter().find(|e| e.name == name)
    }

    /// Validate endpoint configurations
    pub fn validate(&self) -> Result<()> {
        if self.endpoints.is_empty() {
            return Err(anyhow::anyhow!("At least one endpoint must be configured"));
        }

        let mut names = std::collections::HashSet::new();
        let mut tables = std::collections::HashSet::new();

        for endpoint in &self.endpoints {
            // Check for duplicate names
            if !names.insert(&endpoint.name) {
                return Err(anyhow::anyhow!("Duplicate endpoint name: {}", endpoint.name));
            }

            // Check for duplicate table names
            if !tables.insert(&endpoint.table_name) {
                return Err(anyhow::anyhow!("Duplicate table name: {}", endpoint.table_name));
            }

            // Validate endpoint URL
            if endpoint.endpoint_url.is_empty() {
                return Err(anyhow::anyhow!("Endpoint URL cannot be empty for endpoint: {}", endpoint.name));
            }

            // Validate table name
            if endpoint.table_name.is_empty() {
                return Err(anyhow::anyhow!("Table name cannot be empty for endpoint: {}", endpoint.name));
            }

            // Validate URL format
            if let Err(_) = url::Url::parse(&endpoint.endpoint_url) {
                return Err(anyhow::anyhow!("Invalid endpoint URL for {}: {}", endpoint.name, endpoint.endpoint_url));
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct EndpointManager {
    config: EndpointsConfig,
    auth_client: AuthClient,
    http_client: Client,
    mock_api: Option<MockGraphApi>,
}

impl EndpointManager {
    pub fn new(config: EndpointsConfig, auth_client: AuthClient, mock_api_config: Option<crate::mock_graph_api::MockGraphApiConfig>) -> Self {
        let http_client = Client::new();
        let mock_api = mock_api_config.map(|config| MockGraphApi::new(config));

        Self {
            config,
            auth_client,
            http_client,
            mock_api,
        }
    }

    /// Get all enabled endpoints
    pub fn get_enabled_endpoints(&self) -> Vec<&EndpointConfig> {
        self.config.get_enabled_endpoints()
    }

    /// Fetch data from a specific endpoint
    pub async fn fetch_endpoint_data(&self, endpoint: &EndpointConfig) -> Result<serde_json::Value> {
        info!("Fetching data from endpoint: {} ({})", endpoint.name, endpoint.endpoint_url);

        // Check if mock API is enabled and handle devices endpoint
        if let Some(ref mock_api) = self.mock_api {
            if mock_api.is_enabled() && endpoint.name == "devices" {
                info!("Using mock API for devices endpoint");
                let mock_response = mock_api.get_managed_devices(None, None).await?;
                return Ok(serde_json::to_value(mock_response)?);
            }
        }

        // Get access token for real API
        let token = self.auth_client.get_access_token().await
            .context("Failed to get access token")?;

        // Build query parameters
        let mut query_params = endpoint.query_params.clone();
        
        // Add select fields if specified
        if let Some(ref fields) = endpoint.select_fields {
            query_params.insert("$select".to_string(), fields.join(","));
        }

        // Add filter if specified
        if let Some(ref filter) = endpoint.filter {
            query_params.insert("$filter".to_string(), filter.clone());
        }

        // Make API request
        let mut request = self.http_client
            .get(&endpoint.endpoint_url)
            .bearer_auth(&token)
            .header("Content-Type", "application/json");

        // Add query parameters
        for (key, value) in &query_params {
            request = request.query(&[(key, value)]);
        }

        debug!("Making request to: {} with params: {:?}", endpoint.endpoint_url, query_params);

        let response = request.send().await
            .context("Failed to send request to endpoint")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("API request failed with status {}: {}", status, error_text));
        }

        let data: serde_json::Value = response.json().await
            .context("Failed to parse response JSON")?;

        debug!("Successfully fetched data from endpoint: {}", endpoint.name);
        Ok(data)
    }

    /// Fetch paginated data from an endpoint
    pub async fn fetch_all_endpoint_data(&self, endpoint: &EndpointConfig) -> Result<Vec<serde_json::Value>> {
        let mut all_data = Vec::new();
        let mut next_url = Some(endpoint.endpoint_url.clone());

        while let Some(url) = next_url {
            // Create a temporary endpoint config with the current URL
            let temp_endpoint = EndpointConfig {
                endpoint_url: url,
                ..endpoint.clone()
            };

            let response = self.fetch_endpoint_data(&temp_endpoint).await?;

            // Extract data array
            if let Some(value_array) = response.get("value").and_then(|v| v.as_array()) {
                all_data.extend(value_array.iter().cloned());
            } else {
                // If no "value" array, treat the whole response as a single item
                all_data.push(response.clone());
            }

            // Check for next page
            next_url = response.get("@odata.nextLink")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if next_url.is_some() {
                debug!("Found next page for endpoint: {}", endpoint.name);
            }
        }

        info!("Fetched {} total items from endpoint: {}", all_data.len(), endpoint.name);
        Ok(all_data)
    }

    /// Apply field mappings to data
    pub fn apply_field_mappings(&self, endpoint: &EndpointConfig, data: &mut serde_json::Value) {
        if endpoint.field_mappings.is_empty() {
            return;
        }

        if let Some(obj) = data.as_object_mut() {
            let mut new_fields = HashMap::new();
            
            for (source_field, target_field) in &endpoint.field_mappings {
                if let Some(value) = obj.remove(source_field) {
                    new_fields.insert(target_field.clone(), value);
                }
            }

            // Add mapped fields back to the object
            for (key, value) in new_fields {
                obj.insert(key, value);
            }
        }
    }

    /// Get endpoint configuration
    pub fn get_config(&self) -> &EndpointsConfig {
        &self.config
    }
}

/// Predefined endpoint configurations for common Microsoft Graph endpoints
pub struct PredefinedEndpoints;

impl PredefinedEndpoints {
    /// Managed devices endpoint
    pub fn managed_devices() -> EndpointConfig {
        EndpointConfig {
            name: "devices".to_string(),
            endpoint_url: "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices".to_string(),
            table_name: "devices".to_string(),
            enabled: true,
            sync_interval: None,
            query_params: HashMap::new(),
            select_fields: None,
            filter: None,
            field_mappings: HashMap::new(),
        }
    }

    /// Users endpoint
    pub fn users() -> EndpointConfig {
        EndpointConfig {
            name: "users".to_string(),
            endpoint_url: "https://graph.microsoft.com/v1.0/users".to_string(),
            table_name: "users".to_string(),
            enabled: false, // Disabled by default
            sync_interval: None,
            query_params: HashMap::new(),
            select_fields: Some(vec![
                "id".to_string(),
                "userPrincipalName".to_string(),
                "displayName".to_string(),
                "mail".to_string(),
                "jobTitle".to_string(),
                "department".to_string(),
                "companyName".to_string(),
                "accountEnabled".to_string(),
                "createdDateTime".to_string(),
                "lastSignInDateTime".to_string(),
            ]),
            filter: None,
            field_mappings: HashMap::new(),
        }
    }

    /// Groups endpoint
    pub fn groups() -> EndpointConfig {
        EndpointConfig {
            name: "groups".to_string(),
            endpoint_url: "https://graph.microsoft.com/v1.0/groups".to_string(),
            table_name: "groups".to_string(),
            enabled: false, // Disabled by default
            sync_interval: None,
            query_params: HashMap::new(),
            select_fields: Some(vec![
                "id".to_string(),
                "displayName".to_string(),
                "description".to_string(),
                "groupTypes".to_string(),
                "mail".to_string(),
                "mailEnabled".to_string(),
                "securityEnabled".to_string(),
                "createdDateTime".to_string(),
            ]),
            filter: None,
            field_mappings: HashMap::new(),
        }
    }

    /// Device compliance policies endpoint
    pub fn device_compliance_policies() -> EndpointConfig {
        EndpointConfig {
            name: "compliance_policies".to_string(),
            endpoint_url: "https://graph.microsoft.com/v1.0/deviceManagement/deviceCompliancePolicies".to_string(),
            table_name: "compliance_policies".to_string(),
            enabled: false, // Disabled by default
            sync_interval: None,
            query_params: HashMap::new(),
            select_fields: None,
            filter: None,
            field_mappings: HashMap::new(),
        }
    }

    /// Get all predefined endpoints
    pub fn all() -> Vec<EndpointConfig> {
        vec![
            Self::managed_devices(),
            Self::users(),
            Self::groups(),
            Self::device_compliance_policies(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_config_default() {
        let config = EndpointConfig::default();
        assert_eq!(config.name, "devices");
        assert_eq!(config.endpoint_url, "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices");
        assert_eq!(config.table_name, "devices");
        assert!(config.enabled);
    }

    #[test]
    fn test_endpoints_config_validation() {
        let mut config = EndpointsConfig {
            endpoints: vec![
                EndpointConfig {
                    name: "devices".to_string(),
                    endpoint_url: "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices".to_string(),
                    table_name: "devices".to_string(),
                    enabled: true,
                    sync_interval: None,
                    query_params: HashMap::new(),
                    select_fields: None,
                    filter: None,
                    field_mappings: HashMap::new(),
                },
                EndpointConfig {
                    name: "users".to_string(),
                    endpoint_url: "https://graph.microsoft.com/v1.0/users".to_string(),
                    table_name: "users".to_string(),
                    enabled: true,
                    sync_interval: None,
                    query_params: HashMap::new(),
                    select_fields: None,
                    filter: None,
                    field_mappings: HashMap::new(),
                },
            ],
        };

        // Valid configuration should pass
        assert!(config.validate().is_ok());

        // Duplicate names should fail
        config.endpoints[1].name = "devices".to_string();
        assert!(config.validate().is_err());

        // Reset and test duplicate table names
        config.endpoints[1].name = "users".to_string();
        config.endpoints[1].table_name = "devices".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_predefined_endpoints() {
        let devices = PredefinedEndpoints::managed_devices();
        assert_eq!(devices.name, "devices");
        assert!(devices.enabled);

        let users = PredefinedEndpoints::users();
        assert_eq!(users.name, "users");
        assert!(!users.enabled); // Should be disabled by default

        let all = PredefinedEndpoints::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn test_get_enabled_endpoints() {
        let config = EndpointsConfig {
            endpoints: vec![
                EndpointConfig {
                    name: "devices".to_string(),
                    enabled: true,
                    ..Default::default()
                },
                EndpointConfig {
                    name: "users".to_string(),
                    enabled: false,
                    ..Default::default()
                },
                EndpointConfig {
                    name: "groups".to_string(),
                    enabled: true,
                    ..Default::default()
                },
            ],
        };

        let enabled = config.get_enabled_endpoints();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0].name, "devices");
        assert_eq!(enabled[1].name, "groups");
    }
}
