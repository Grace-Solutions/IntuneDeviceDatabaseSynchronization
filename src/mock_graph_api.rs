use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use log::{info, debug, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockGraphApiConfig {
    /// Enable mock mode instead of real Graph API
    pub enabled: bool,

    /// Simulate rate limiting responses
    #[serde(rename = "simulateRateLimits")]
    pub simulate_rate_limits: bool,
    /// Rate limit probability (0.0 to 1.0)
    #[serde(rename = "rateLimitProbability")]
    pub rate_limit_probability: f64,
    /// Simulate authentication failures
    #[serde(rename = "simulateAuthFailures")]
    pub simulate_auth_failures: bool,
    /// Auth failure probability (0.0 to 1.0)
    #[serde(rename = "authFailureProbability")]
    pub auth_failure_probability: f64,
    /// Simulate network errors
    #[serde(rename = "simulateNetworkErrors")]
    pub simulate_network_errors: bool,
    /// Network error probability (0.0 to 1.0)
    #[serde(rename = "networkErrorProbability")]
    pub network_error_probability: f64,
    /// Response delay range in milliseconds
    #[serde(rename = "responseDelayMs")]
    pub response_delay_ms: (u64, u64),
    /// Device update frequency (how often devices change)
    #[serde(rename = "deviceUpdateFrequency")]
    pub device_update_frequency: f64,
}

impl Default for MockGraphApiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            simulate_rate_limits: false,
            rate_limit_probability: 0.1,
            simulate_auth_failures: false,
            auth_failure_probability: 0.05,
            simulate_network_errors: false,
            network_error_probability: 0.02,
            response_delay_ms: (100, 500),
            device_update_frequency: 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockDevice {
    pub id: String,
    #[serde(rename = "deviceName")]
    pub device_name: String,
    #[serde(rename = "operatingSystem")]
    pub operating_system: String,
    #[serde(rename = "osVersion")]
    pub os_version: String,
    #[serde(rename = "serialNumber")]
    pub serial_number: Option<String>,
    #[serde(rename = "imei")]
    pub imei: Option<String>,
    pub model: String,
    pub manufacturer: String,
    #[serde(rename = "enrolledDateTime")]
    pub enrolled_date_time: String,
    #[serde(rename = "lastSyncDateTime")]
    pub last_sync_date_time: String,
    #[serde(rename = "complianceState")]
    pub compliance_state: String,
    #[serde(rename = "azureADDeviceId")]
    pub azure_ad_device_id: Option<String>,
    #[serde(rename = "managedDeviceOwnerType")]
    pub managed_device_owner_type: String,
    #[serde(rename = "deviceType")]
    pub device_type: String,
    #[serde(rename = "deviceRegistrationState")]
    pub device_registration_state: String,
    #[serde(rename = "isEncrypted")]
    pub is_encrypted: bool,
    #[serde(rename = "isSupervised")]
    pub is_supervised: bool,
    #[serde(rename = "emailAddress")]
    pub email_address: Option<String>,
    #[serde(rename = "userDisplayName")]
    pub user_display_name: Option<String>,
    #[serde(rename = "userPrincipalName")]
    pub user_principal_name: Option<String>,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "deviceId")]
    pub device_id: String,
}

#[derive(Debug, Serialize)]
pub struct MockGraphResponse {
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    #[serde(rename = "@odata.count")]
    pub odata_count: Option<u32>,
    pub value: Vec<serde_json::Value>,
    #[serde(rename = "@odata.nextLink")]
    pub odata_next_link: Option<String>,
}

#[derive(Debug)]
pub struct MockGraphApi {
    config: MockGraphApiConfig,
    devices: Arc<RwLock<HashMap<String, MockDevice>>>,
    request_count: Arc<RwLock<u64>>,
}

impl MockGraphApi {
    pub fn new(config: MockGraphApiConfig) -> Self {
        let api = Self {
            config: config.clone(),
            devices: Arc::new(RwLock::new(HashMap::new())),
            request_count: Arc::new(RwLock::new(0)),
        };

        // Generate initial mock devices
        if config.enabled {
            tokio::spawn({
                let api = api.clone();
                async move {
                    api.generate_mock_devices().await;
                }
            });
        }

        api
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub async fn get_managed_devices(&self, skip: Option<u32>, top: Option<u32>) -> Result<MockGraphResponse> {
        if !self.config.enabled {
            return Err(anyhow::anyhow!("Mock API is not enabled"));
        }

        // Increment request count
        {
            let mut count = self.request_count.write().await;
            *count += 1;
        }

        // Simulate various failure scenarios
        self.simulate_failures().await?;

        // Simulate response delay
        self.simulate_delay().await;

        // Update some devices randomly
        self.update_random_devices().await;

        // Get devices with pagination
        let devices = self.devices.read().await;
        let all_devices: Vec<MockDevice> = devices.values().cloned().collect();
        
        let skip = skip.unwrap_or(0) as usize;
        let top = top.unwrap_or(1000) as usize;
        
        let total_count = all_devices.len();
        let end_index = std::cmp::min(skip + top, total_count);
        let page_devices = if skip < total_count {
            all_devices[skip..end_index].to_vec()
        } else {
            Vec::new()
        };

        // Determine if there's a next page
        let next_link = if end_index < total_count {
            Some(format!(
                "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices?$skip={}&$top={}",
                end_index, top
            ))
        } else {
            None
        };

        debug!("Mock API: Returning {} devices (skip: {}, top: {})", page_devices.len(), skip, top);

        // Convert MockDevice to JSON for consistency
        let json_devices: Vec<serde_json::Value> = page_devices
            .into_iter()
            .map(|device| serde_json::to_value(device).unwrap_or_default())
            .collect();

        Ok(MockGraphResponse {
            odata_context: "https://graph.microsoft.com/v1.0/$metadata#deviceManagement/managedDevices".to_string(),
            odata_count: Some(total_count as u32),
            value: json_devices,
            odata_next_link: next_link,
        })
    }

    pub async fn get_device_by_id(&self, device_id: &str) -> Result<MockDevice> {
        if !self.config.enabled {
            return Err(anyhow::anyhow!("Mock API is not enabled"));
        }

        self.simulate_failures().await?;
        self.simulate_delay().await;

        let devices = self.devices.read().await;
        devices.get(device_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", device_id))
    }

    pub async fn get_request_count(&self) -> u64 {
        *self.request_count.read().await
    }

    pub async fn reset_request_count(&self) {
        let mut count = self.request_count.write().await;
        *count = 0;
    }

    pub async fn add_mock_device(&self, device: MockDevice) {
        let mut devices = self.devices.write().await;
        devices.insert(device.id.clone(), device);
    }

    pub async fn remove_mock_device(&self, device_id: &str) -> bool {
        let mut devices = self.devices.write().await;
        devices.remove(device_id).is_some()
    }

    pub async fn get_device_count(&self) -> usize {
        let devices = self.devices.read().await;
        devices.len()
    }

    /// Regenerate devices with a specific count
    async fn regenerate_devices_with_count(&self, count: u32) {
        info!("Regenerating {} mock devices", count);

        // Clear existing devices
        {
            let mut devices = self.devices.write().await;
            devices.clear();
        }

        // Generate new devices with the specified count
        self.generate_mock_devices_internal(count).await;
    }

    /// Dynamic endpoint data generation - supports any enabled endpoint
    pub async fn get_endpoint_data(
        &self,
        endpoint_name: &str,
        endpoint_config: Option<&crate::endpoint::EndpointConfig>,
        skip: Option<u32>,
        top: Option<u32>,
    ) -> Result<MockGraphResponse> {
        if !self.config.enabled {
            return Err(anyhow::anyhow!("Mock API is not enabled"));
        }

        // For devices endpoint, use the existing implementation but check if we need to regenerate
        if endpoint_name == "devices" {
            // Check if we need to regenerate devices based on endpoint config
            let expected_count = endpoint_config
                .and_then(|config| config.mock_object_count)
                .unwrap_or(30000);

            let current_count = self.get_device_count().await;
            if current_count != expected_count as usize {
                info!("Regenerating devices: current={}, expected={}", current_count, expected_count);
                self.regenerate_devices_with_count(expected_count).await;
            }

            return self.get_managed_devices(skip, top).await;
        }

        // For other endpoints, generate dynamic mock data
        self.generate_dynamic_endpoint_data(endpoint_name, endpoint_config, skip, top).await
    }

    /// Generate dynamic mock data for any endpoint
    async fn generate_dynamic_endpoint_data(
        &self,
        endpoint_name: &str,
        endpoint_config: Option<&crate::endpoint::EndpointConfig>,
        skip: Option<u32>,
        top: Option<u32>,
    ) -> Result<MockGraphResponse> {
        // Increment request count
        {
            let mut count = self.request_count.write().await;
            *count += 1;
        }

        // Simulate various failure scenarios
        self.simulate_failures().await?;

        // Simulate response delay
        self.simulate_delay().await;

        // Get object count from endpoint config or use default
        let object_count = endpoint_config
            .and_then(|config| config.mock_object_count)
            .unwrap_or(1000);

        // Generate mock data based on endpoint type
        let mock_data = self.generate_mock_objects_for_endpoint(endpoint_name, endpoint_config, object_count).await;

        // Apply pagination
        let skip = skip.unwrap_or(0) as usize;
        let top = top.unwrap_or(1000) as usize;

        let total_count = mock_data.len();
        let end_index = std::cmp::min(skip + top, total_count);
        let page_data = if skip < total_count {
            mock_data[skip..end_index].to_vec()
        } else {
            Vec::new()
        };

        // Determine if there's a next page
        let next_link = if end_index < total_count {
            Some(format!(
                "https://graph.microsoft.com/v1.0/{}?$skip={}&$top={}",
                self.get_endpoint_path(endpoint_name), end_index, top
            ))
        } else {
            None
        };

        debug!("Mock API: Returning {} {} objects (skip: {}, top: {})",
               page_data.len(), endpoint_name, skip, top);

        Ok(MockGraphResponse {
            odata_context: format!("https://graph.microsoft.com/v1.0/$metadata#{}", endpoint_name),
            odata_count: Some(total_count as u32),
            value: page_data,
            odata_next_link: next_link,
        })
    }

    /// Generate mock objects for a specific endpoint
    async fn generate_mock_objects_for_endpoint(
        &self,
        endpoint_name: &str,
        endpoint_config: Option<&crate::endpoint::EndpointConfig>,
        count: u32
    ) -> Vec<serde_json::Value> {
        let mut objects = Vec::new();

        for i in 0..count {
            let mock_object = match endpoint_name.to_lowercase().as_str() {
                "users" => self.generate_mock_user_object(i, endpoint_config),
                "groups" => self.generate_mock_group_object(i, endpoint_config),
                "compliance_policies" => self.generate_mock_compliance_policy_object(i, endpoint_config),
                "devices" => {
                    // Convert MockDevice to JSON for consistency
                    let device = self.generate_mock_user(i); // Temporary - will fix this
                    serde_json::to_value(device).unwrap_or_default()
                },
                _ => {
                    // Generic object generation for unknown endpoints
                    let generic = self.generate_generic_mock_object(endpoint_name, i);
                    serde_json::to_value(generic).unwrap_or_default()
                }
            };
            objects.push(mock_object);
        }

        objects
    }

    /// Generate a mock user object
    fn generate_mock_user(&self, index: u32) -> MockDevice {
        let first_names = vec!["John", "Jane", "Michael", "Sarah", "David", "Emily"];
        let last_names = vec!["Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia"];

        let first_name = first_names[index as usize % first_names.len()];
        let last_name = last_names[(index as usize * 7) % last_names.len()];
        let display_name = format!("{} {}", first_name, last_name);
        let upn = format!("{}.{}@company.com", first_name.to_lowercase(), last_name.to_lowercase());

        MockDevice {
            id: Uuid::new_v4().to_string(),
            device_name: display_name.clone(),
            operating_system: "User".to_string(),
            os_version: "1.0".to_string(),
            serial_number: None,
            imei: None,
            model: "User Account".to_string(),
            manufacturer: "Microsoft".to_string(),
            enrolled_date_time: format_system_time(SystemTime::now()),
            last_sync_date_time: format_system_time(SystemTime::now()),
            compliance_state: "active".to_string(),
            azure_ad_device_id: Some(Uuid::new_v4().to_string()),
            managed_device_owner_type: "user".to_string(),
            device_type: "user".to_string(),
            device_registration_state: "registered".to_string(),
            is_encrypted: false,
            is_supervised: false,
            email_address: Some(upn.clone()),
            user_display_name: Some(display_name),
            user_principal_name: Some(upn),
            tenant_id: Uuid::new_v4().to_string(),
            device_id: Uuid::new_v4().to_string(),
        }
    }

    /// Generate a mock group object
    fn generate_mock_group(&self, index: u32) -> MockDevice {
        let group_types = vec!["Security", "Distribution", "Microsoft 365", "Dynamic"];
        let group_type = group_types[index as usize % group_types.len()];
        let group_name = format!("{} Group {}", group_type, index + 1);

        MockDevice {
            id: Uuid::new_v4().to_string(),
            device_name: group_name.clone(),
            operating_system: "Group".to_string(),
            os_version: "1.0".to_string(),
            serial_number: None,
            imei: None,
            model: group_type.to_string(),
            manufacturer: "Microsoft".to_string(),
            enrolled_date_time: format_system_time(SystemTime::now()),
            last_sync_date_time: format_system_time(SystemTime::now()),
            compliance_state: "active".to_string(),
            azure_ad_device_id: Some(Uuid::new_v4().to_string()),
            managed_device_owner_type: "group".to_string(),
            device_type: "group".to_string(),
            device_registration_state: "registered".to_string(),
            is_encrypted: false,
            is_supervised: false,
            email_address: Some(format!("{}@company.com", group_name.to_lowercase().replace(" ", ""))),
            user_display_name: Some(group_name),
            user_principal_name: None,
            tenant_id: Uuid::new_v4().to_string(),
            device_id: Uuid::new_v4().to_string(),
        }
    }

    /// Generate a mock compliance policy object
    fn generate_mock_compliance_policy(&self, index: u32) -> MockDevice {
        let policy_types = vec!["Windows", "iOS", "Android", "macOS"];
        let policy_type = policy_types[index as usize % policy_types.len()];
        let policy_name = format!("{} Compliance Policy {}", policy_type, index + 1);

        MockDevice {
            id: Uuid::new_v4().to_string(),
            device_name: policy_name.clone(),
            operating_system: policy_type.to_string(),
            os_version: "1.0".to_string(),
            serial_number: None,
            imei: None,
            model: "Compliance Policy".to_string(),
            manufacturer: "Microsoft".to_string(),
            enrolled_date_time: format_system_time(SystemTime::now()),
            last_sync_date_time: format_system_time(SystemTime::now()),
            compliance_state: "enabled".to_string(),
            azure_ad_device_id: Some(Uuid::new_v4().to_string()),
            managed_device_owner_type: "policy".to_string(),
            device_type: "policy".to_string(),
            device_registration_state: "active".to_string(),
            is_encrypted: false,
            is_supervised: false,
            email_address: None,
            user_display_name: Some(policy_name),
            user_principal_name: None,
            tenant_id: Uuid::new_v4().to_string(),
            device_id: Uuid::new_v4().to_string(),
        }
    }

    /// Generate a generic mock object for unknown endpoints
    fn generate_generic_mock_object(&self, endpoint_name: &str, index: u32) -> MockDevice {
        let object_name = format!("{} Object {}", endpoint_name, index + 1);

        MockDevice {
            id: Uuid::new_v4().to_string(),
            device_name: object_name.clone(),
            operating_system: endpoint_name.to_string(),
            os_version: "1.0".to_string(),
            serial_number: None,
            imei: None,
            model: "Generic Object".to_string(),
            manufacturer: "Microsoft".to_string(),
            enrolled_date_time: format_system_time(SystemTime::now()),
            last_sync_date_time: format_system_time(SystemTime::now()),
            compliance_state: "active".to_string(),
            azure_ad_device_id: Some(Uuid::new_v4().to_string()),
            managed_device_owner_type: "object".to_string(),
            device_type: endpoint_name.to_string(),
            device_registration_state: "active".to_string(),
            is_encrypted: false,
            is_supervised: false,
            email_address: None,
            user_display_name: Some(object_name),
            user_principal_name: None,
            tenant_id: Uuid::new_v4().to_string(),
            device_id: Uuid::new_v4().to_string(),
        }
    }

    /// Generate a mock user object based on endpoint configuration
    fn generate_mock_user_object(&self, index: u32, endpoint_config: Option<&crate::endpoint::EndpointConfig>) -> serde_json::Value {
        let first_names = vec!["John", "Jane", "Michael", "Sarah", "David", "Emily", "Robert", "Jessica"];
        let last_names = vec!["Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis"];
        let departments = vec!["Engineering", "Marketing", "Sales", "HR", "Finance", "Operations"];
        let job_titles = vec!["Manager", "Developer", "Analyst", "Coordinator", "Director", "Specialist"];

        let first_name = first_names[index as usize % first_names.len()];
        let last_name = last_names[(index as usize * 7) % last_names.len()];
        let display_name = format!("{} {}", first_name, last_name);
        let upn = format!("{}.{}@company.com", first_name.to_lowercase(), last_name.to_lowercase());
        let department = departments[index as usize % departments.len()];
        let job_title = job_titles[index as usize % job_titles.len()];

        // Get select fields from endpoint config or use defaults
        let select_fields = endpoint_config
            .and_then(|config| config.select_fields.as_ref())
            .cloned()
            .unwrap_or_else(|| vec![
                "id".to_string(), "userPrincipalName".to_string(), "displayName".to_string(),
                "mail".to_string(), "jobTitle".to_string(), "department".to_string(),
                "companyName".to_string(), "accountEnabled".to_string(), "createdDateTime".to_string()
            ]);

        let mut user_object = serde_json::Map::new();

        for field in select_fields {
            let value = match field.as_str() {
                "id" => serde_json::Value::String(Uuid::new_v4().to_string()),
                "userPrincipalName" => serde_json::Value::String(upn.clone()),
                "displayName" => serde_json::Value::String(display_name.clone()),
                "mail" => serde_json::Value::String(upn.clone()),
                "jobTitle" => serde_json::Value::String(format!("{} {}", job_title, department)),
                "department" => serde_json::Value::String(department.to_string()),
                "companyName" => serde_json::Value::String("Contoso Corporation".to_string()),
                "accountEnabled" => serde_json::Value::Bool(index % 10 != 0), // 90% enabled
                "createdDateTime" => serde_json::Value::String(format_system_time(SystemTime::now())),
                "lastSignInDateTime" => serde_json::Value::String(format_system_time(SystemTime::now())),
                _ => serde_json::Value::String(format!("{}_{}", field, index)),
            };
            user_object.insert(field, value);
        }

        serde_json::Value::Object(user_object)
    }

    /// Generate a mock group object based on endpoint configuration
    fn generate_mock_group_object(&self, index: u32, endpoint_config: Option<&crate::endpoint::EndpointConfig>) -> serde_json::Value {
        let group_types = vec!["Security", "Distribution", "Microsoft 365", "Dynamic"];
        let group_type = group_types[index as usize % group_types.len()];
        let group_name = format!("{} Group {}", group_type, index + 1);
        let description = format!("This is a {} group for organizational purposes", group_type.to_lowercase());

        // Get select fields from endpoint config or use defaults
        let select_fields = endpoint_config
            .and_then(|config| config.select_fields.as_ref())
            .cloned()
            .unwrap_or_else(|| vec![
                "id".to_string(), "displayName".to_string(), "description".to_string(),
                "groupTypes".to_string(), "mail".to_string(), "mailEnabled".to_string(),
                "securityEnabled".to_string(), "createdDateTime".to_string()
            ]);

        let mut group_object = serde_json::Map::new();

        for field in select_fields {
            let value = match field.as_str() {
                "id" => serde_json::Value::String(Uuid::new_v4().to_string()),
                "displayName" => serde_json::Value::String(group_name.clone()),
                "description" => serde_json::Value::String(description.clone()),
                "groupTypes" => {
                    let types = if group_type == "Microsoft 365" {
                        vec!["Unified"]
                    } else if group_type == "Dynamic" {
                        vec!["DynamicMembership"]
                    } else {
                        vec![]
                    };
                    serde_json::Value::Array(types.into_iter().map(|t| serde_json::Value::String(t.to_string())).collect())
                },
                "mail" => serde_json::Value::String(format!("{}@company.com", group_name.to_lowercase().replace(" ", ""))),
                "mailEnabled" => serde_json::Value::Bool(group_type == "Distribution" || group_type == "Microsoft 365"),
                "securityEnabled" => serde_json::Value::Bool(group_type == "Security" || group_type == "Dynamic"),
                "createdDateTime" => serde_json::Value::String(format_system_time(SystemTime::now())),
                _ => serde_json::Value::String(format!("{}_{}", field, index)),
            };
            group_object.insert(field, value);
        }

        serde_json::Value::Object(group_object)
    }

    /// Generate a mock compliance policy object based on endpoint configuration
    fn generate_mock_compliance_policy_object(&self, index: u32, endpoint_config: Option<&crate::endpoint::EndpointConfig>) -> serde_json::Value {
        let policy_types = vec!["Windows", "iOS", "Android", "macOS"];
        let policy_type = policy_types[index as usize % policy_types.len()];
        let policy_name = format!("{} Compliance Policy {}", policy_type, index + 1);
        let description = format!("Compliance policy for {} devices", policy_type);

        // Get select fields from endpoint config or use defaults
        let select_fields = endpoint_config
            .and_then(|config| config.select_fields.as_ref())
            .cloned()
            .unwrap_or_else(|| vec![
                "id".to_string(), "displayName".to_string(), "description".to_string(),
                "platformType".to_string(), "createdDateTime".to_string(), "lastModifiedDateTime".to_string()
            ]);

        let mut policy_object = serde_json::Map::new();

        for field in select_fields {
            let value = match field.as_str() {
                "id" => serde_json::Value::String(Uuid::new_v4().to_string()),
                "displayName" => serde_json::Value::String(policy_name.clone()),
                "description" => serde_json::Value::String(description.clone()),
                "platformType" => serde_json::Value::String(policy_type.to_lowercase()),
                "createdDateTime" => serde_json::Value::String(format_system_time(SystemTime::now())),
                "lastModifiedDateTime" => serde_json::Value::String(format_system_time(SystemTime::now())),
                _ => serde_json::Value::String(format!("{}_{}", field, index)),
            };
            policy_object.insert(field, value);
        }

        serde_json::Value::Object(policy_object)
    }

    /// Get the API path for an endpoint
    fn get_endpoint_path(&self, endpoint_name: &str) -> String {
        match endpoint_name {
            "devices" => "deviceManagement/managedDevices".to_string(),
            "users" => "users".to_string(),
            "groups" => "groups".to_string(),
            "compliance_policies" => "deviceManagement/deviceCompliancePolicies".to_string(),
            _ => endpoint_name.to_string(),
        }
    }

    async fn generate_mock_devices(&self) {
        // Use default device count since it's now per-endpoint
        let count = 30000; // Default fallback
        self.generate_mock_devices_internal(count).await;
    }

    async fn generate_mock_devices_internal(&self, device_count: u32) {
        info!("Generating {} mock devices", device_count);

        let operating_systems = vec!["Windows", "macOS", "Android", "iOS"];
        let manufacturers = vec!["Microsoft", "Apple", "Samsung", "Google", "Dell", "HP", "Lenovo"];
        let compliance_states = vec!["compliant", "noncompliant", "conflict", "error", "unknown"];
        let device_types = vec!["desktop", "laptop", "tablet", "phone"];

        // Realistic first and last names for user generation
        let first_names = vec![
            "John", "Jane", "Michael", "Sarah", "David", "Emily", "Robert", "Jessica",
            "William", "Ashley", "James", "Amanda", "Christopher", "Stephanie", "Daniel",
            "Melissa", "Matthew", "Nicole", "Anthony", "Elizabeth", "Mark", "Helen",
            "Donald", "Deborah", "Steven", "Rachel", "Paul", "Carolyn", "Andrew", "Janet"
        ];
        let last_names = vec![
            "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis",
            "Rodriguez", "Martinez", "Hernandez", "Lopez", "Gonzalez", "Wilson", "Anderson",
            "Thomas", "Taylor", "Moore", "Jackson", "Martin", "Lee", "Perez", "Thompson",
            "White", "Harris", "Sanchez", "Clark", "Ramirez", "Lewis", "Robinson"
        ];

        let tenant_id = Uuid::new_v4().to_string(); // Single tenant for all devices
        let mut devices = self.devices.write().await;

        for i in 0..device_count {
            let os = operating_systems[i as usize % operating_systems.len()];
            let manufacturer = manufacturers[i as usize % manufacturers.len()];
            let device_type = device_types[i as usize % device_types.len()];

            let device_id = Uuid::new_v4().to_string();
            let azure_ad_device_id = Uuid::new_v4().to_string();

            // Generate realistic user
            let first_name = first_names[i as usize % first_names.len()];
            let last_name = last_names[(i as usize * 7) % last_names.len()]; // Different pattern for variety
            let user_display_name = format!("{} {}", first_name, last_name);
            let user_principal_name = format!("{}.{}@company.com",
                first_name.to_lowercase(), last_name.to_lowercase());
            let email_address = user_principal_name.clone();

            // Generate realistic serial numbers based on manufacturer
            let serial_number = self.generate_realistic_serial_number(manufacturer, os, i);

            // Use serial number as device name (uppercase, real-world practice)
            let device_name = serial_number.clone();

            let os_version = match os {
                "Windows" => format!("10.0.{}.{}", 19041 + (i % 5), 1000 + (i % 100)),
                "macOS" => format!("12.{}.{}", i % 7, i % 10),
                "Android" => format!("{}.{}", 11 + (i % 3), i % 10),
                "iOS" => format!("15.{}.{}", i % 8, i % 10),
                _ => "1.0.0".to_string(),
            };

            let model = match (manufacturer, os) {
                ("Apple", "macOS") => format!("MacBook {}", if i % 2 == 0 { "Pro" } else { "Air" }),
                ("Apple", "iOS") => format!("iPhone {}", 12 + (i % 4)),
                ("Samsung", "Android") => format!("Galaxy {}", if device_type == "phone" { "S22" } else { "Tab S8" }),
                ("Google", "Android") => format!("Pixel {}", 6 + (i % 3)),
                ("Dell", "Windows") => format!("OptiPlex {}", if device_type == "desktop" { "7090" } else { "Latitude 5520" }),
                ("HP", "Windows") => format!("EliteBook {}", if device_type == "laptop" { "850" } else { "ProDesk 600" }),
                ("Lenovo", "Windows") => format!("ThinkPad {}", if device_type == "laptop" { "X1" } else { "M720q" }),
                ("Microsoft", "Windows") => format!("Surface {}", if device_type == "laptop" { "Laptop 4" } else { "Pro 8" }),
                _ => format!("{} {}", manufacturer, device_type),
            };

            let enrolled_time = SystemTime::now() - Duration::from_secs((i as u64 % 365) * 86400);
            let last_sync_time = SystemTime::now() - Duration::from_secs((i as u64 % 24) * 3600);

            let device = MockDevice {
                id: device_id.clone(),
                device_name,
                operating_system: os.to_string(),
                os_version,
                serial_number: Some(serial_number),
                imei: if os == "Android" || os == "iOS" {
                    Some(format!("{:015}", 123456789012345u64 + i as u64))
                } else {
                    None
                },
                model,
                manufacturer: manufacturer.to_string(),
                enrolled_date_time: format_system_time(enrolled_time),
                last_sync_date_time: format_system_time(last_sync_time),
                compliance_state: compliance_states[i as usize % compliance_states.len()].to_string(),
                azure_ad_device_id: Some(azure_ad_device_id),
                managed_device_owner_type: "company".to_string(),
                device_type: device_type.to_string(),
                device_registration_state: "registered".to_string(),
                is_encrypted: i % 3 != 0, // Most devices encrypted
                is_supervised: i % 4 == 0, // Some devices supervised
                email_address: Some(email_address),
                user_display_name: Some(user_display_name),
                user_principal_name: Some(user_principal_name),
                tenant_id: tenant_id.clone(),
                device_id: device_id.clone(),
            };

            devices.insert(device_id, device);
        }

        info!("Generated {} mock devices", devices.len());
    }

    fn generate_realistic_serial_number(&self, manufacturer: &str, os: &str, index: u32) -> String {
        match manufacturer {
            "Dell" => {
                // Dell service tags are typically 7 characters, alphanumeric
                format!("{:07X}", 0x1000000 + index)
            },
            "HP" => {
                // HP serial numbers often start with manufacturer code
                format!("CND{:07}", 1000000 + index)
            },
            "Lenovo" => {
                // Lenovo serial numbers are often 8 characters
                format!("PC{:06X}", 0x100000 + index)
            },
            "Apple" => {
                if os == "macOS" {
                    // Mac serial numbers are 10-12 characters
                    format!("C02{:07X}", 0x1000000 + index)
                } else {
                    // iOS device serial numbers
                    format!("F{:010X}", 0x1000000000 + index as u64)
                }
            },
            "Microsoft" => {
                // Surface devices
                format!("MS{:08X}", 0x10000000 + index)
            },
            "Samsung" => {
                // Samsung device serial numbers
                format!("RF{:08X}", 0x10000000 + index)
            },
            "Google" => {
                // Google Pixel serial numbers
                format!("GP{:08X}", 0x10000000 + index)
            },
            _ => {
                // Generic format
                format!("SN{:08X}", 0x10000000 + index)
            }
        }
    }



    async fn simulate_failures(&self) -> Result<()> {
        // Simple pseudo-random using system time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let random_value = (now.subsec_nanos() % 1000) as f64 / 1000.0;

        // Simulate rate limiting
        if self.config.simulate_rate_limits && random_value < self.config.rate_limit_probability {
            warn!("Mock API: Simulating rate limit response");
            return Err(anyhow::anyhow!("Rate limited (429): Too Many Requests"));
        }

        // Simulate authentication failures
        if self.config.simulate_auth_failures && random_value < self.config.auth_failure_probability {
            warn!("Mock API: Simulating authentication failure");
            return Err(anyhow::anyhow!("Authentication failed (401): Unauthorized"));
        }

        // Simulate network errors
        if self.config.simulate_network_errors && random_value < self.config.network_error_probability {
            warn!("Mock API: Simulating network error");
            return Err(anyhow::anyhow!("Network error: Connection timeout"));
        }

        Ok(())
    }

    async fn simulate_delay(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();

        let (min_delay, max_delay) = self.config.response_delay_ms;
        let range = max_delay - min_delay;
        let delay_ms = min_delay + (now.subsec_nanos() % (range as u32 + 1)) as u64;

        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }

    async fn update_random_devices(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let random_value = (now.subsec_nanos() % 1000) as f64 / 1000.0;

        if random_value < self.config.device_update_frequency {
            let mut devices = self.devices.write().await;
            let device_ids: Vec<String> = devices.keys().cloned().collect();

            if !device_ids.is_empty() {
                let random_index = (now.subsec_nanos() as usize) % device_ids.len();
                let random_id = &device_ids[random_index];
                if let Some(device) = devices.get_mut(random_id) {
                    // Update last sync time
                    device.last_sync_date_time = format_system_time(SystemTime::now());

                    // Occasionally change compliance state
                    if (now.subsec_micros() % 10) == 0 {
                        let states = vec!["compliant", "noncompliant", "conflict", "error", "unknown"];
                        let state_index = (now.subsec_micros() as usize) % states.len();
                        device.compliance_state = states[state_index].to_string();
                    }

                    debug!("Mock API: Updated device {}", random_id);
                }
            }
        }
    }
}

impl Clone for MockGraphApi {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            devices: Arc::clone(&self.devices),
            request_count: Arc::clone(&self.request_count),
        }
    }
}

fn format_system_time(time: SystemTime) -> String {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let timestamp = duration.as_secs();
    
    // Convert to ISO 8601 format (simplified)
    let datetime = chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .unwrap_or_else(|| chrono::Utc::now());
    datetime.format("%Y-%m-%dT%H:%M:%S.%3fZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_api_creation() {
        let config = MockGraphApiConfig {
            enabled: true,
            ..Default::default()
        };
        
        let api = MockGraphApi::new(config);
        assert!(api.is_enabled());
        
        // Wait a bit for device generation
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let device_count = api.get_device_count().await;
        assert_eq!(device_count, 5);
    }

    #[tokio::test]
    async fn test_mock_api_pagination() {
        let config = MockGraphApiConfig {
            enabled: true,
            ..Default::default()
        };
        
        let api = MockGraphApi::new(config);
        
        // Wait for device generation
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Test pagination
        let response = api.get_managed_devices(Some(0), Some(5)).await.unwrap();
        assert_eq!(response.value.len(), 5);
        assert!(response.odata_next_link.is_some());
        
        let response2 = api.get_managed_devices(Some(5), Some(5)).await.unwrap();
        assert_eq!(response2.value.len(), 5);
        assert!(response2.odata_next_link.is_none());
    }

    #[tokio::test]
    async fn test_mock_api_disabled() {
        let config = MockGraphApiConfig {
            enabled: false,
            ..Default::default()
        };
        
        let api = MockGraphApi::new(config);
        assert!(!api.is_enabled());
        
        let result = api.get_managed_devices(None, None).await;
        assert!(result.is_err());
    }
}
