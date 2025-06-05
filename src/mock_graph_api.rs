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
    /// Number of mock devices to generate
    #[serde(rename = "deviceCount")]
    pub device_count: u32,
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
            device_count: 100,
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
}

#[derive(Debug, Serialize)]
pub struct MockGraphResponse {
    #[serde(rename = "@odata.context")]
    pub odata_context: String,
    #[serde(rename = "@odata.count")]
    pub odata_count: Option<u32>,
    pub value: Vec<MockDevice>,
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

        Ok(MockGraphResponse {
            odata_context: "https://graph.microsoft.com/v1.0/$metadata#deviceManagement/managedDevices".to_string(),
            odata_count: Some(total_count as u32),
            value: page_devices,
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

    async fn generate_mock_devices(&self) {
        info!("Generating {} mock devices", self.config.device_count);
        
        let operating_systems = vec!["Windows", "macOS", "Android", "iOS"];
        let manufacturers = vec!["Microsoft", "Apple", "Samsung", "Google", "Dell", "HP", "Lenovo"];
        let compliance_states = vec!["compliant", "noncompliant", "conflict", "error", "unknown"];
        let device_types = vec!["desktop", "laptop", "tablet", "phone"];

        let mut devices = self.devices.write().await;
        
        for i in 0..self.config.device_count {
            let os = operating_systems[i as usize % operating_systems.len()];
            let manufacturer = manufacturers[i as usize % manufacturers.len()];
            let device_type = device_types[i as usize % device_types.len()];
            
            let device_id = Uuid::new_v4().to_string();
            let device_name = format!("{}-{}-{:04}", manufacturer, device_type, i + 1);
            
            let os_version = match os {
                "Windows" => format!("10.0.{}.{}", 19041 + (i % 5), 1000 + (i % 100)),
                "macOS" => format!("12.{}.{}", i % 7, i % 10),
                "Android" => format!("{}.{}", 11 + (i % 3), i % 10),
                "iOS" => format!("15.{}.{}", i % 8, i % 10),
                _ => "1.0.0".to_string(),
            };

            let model = match os {
                "Windows" => format!("{} {}", manufacturer, device_type),
                "macOS" => format!("MacBook {}", if i % 2 == 0 { "Pro" } else { "Air" }),
                "Android" => format!("Galaxy {}", device_type),
                "iOS" => format!("iPhone {}", 12 + (i % 4)),
                _ => format!("{} Device", manufacturer),
            };

            let enrolled_time = SystemTime::now() - Duration::from_secs((i as u64 % 365) * 86400);
            let last_sync_time = SystemTime::now() - Duration::from_secs((i as u64 % 24) * 3600);

            let device = MockDevice {
                id: device_id.clone(),
                device_name,
                operating_system: os.to_string(),
                os_version,
                serial_number: Some(format!("SN{:08}", i)),
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
                azure_ad_device_id: Some(Uuid::new_v4().to_string()),
                managed_device_owner_type: "company".to_string(),
                device_type: device_type.to_string(),
                device_registration_state: "registered".to_string(),
                is_encrypted: i % 3 != 0, // Most devices encrypted
                is_supervised: i % 4 == 0, // Some devices supervised
                email_address: Some(format!("user{}@company.com", i + 1)),
                user_display_name: Some(format!("User {}", i + 1)),
                user_principal_name: Some(format!("user{}@company.com", i + 1)),
            };

            devices.insert(device_id, device);
        }

        info!("Generated {} mock devices", devices.len());
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
            device_count: 5,
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
            device_count: 10,
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
