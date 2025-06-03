use log::{debug, warn};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

use crate::fingerprint::{extract_device_identifiers, generate_device_fingerprint};

/// Generates or validates a UUID for a device
/// 
/// If the device already has a valid UUID, it returns that UUID.
/// Otherwise, it generates a deterministic UUID based on device fingerprint.
pub fn get_or_generate_device_uuid(
    device_data: &HashMap<String, serde_json::Value>,
) -> Result<Uuid, uuid::Error> {
    // First, check if device already has a valid UUID
    if let Some(existing_uuid) = device_data.get("id").or_else(|| device_data.get("uuid")) {
        if let Some(uuid_str) = existing_uuid.as_str() {
            if let Ok(uuid) = Uuid::parse_str(uuid_str) {
                debug!("Using existing UUID: {}", uuid);
                return Ok(uuid);
            } else {
                warn!("Invalid UUID format found: {}", uuid_str);
            }
        }
    }

    // Generate UUID from device fingerprint
    let (serial, imei, hw_id, azure_id, model, enrolled) = extract_device_identifiers(device_data);
    
    let fingerprint = generate_device_fingerprint(
        serial.as_deref(),
        imei.as_deref(),
        hw_id.as_deref(),
        azure_id.as_deref(),
        model.as_deref(),
        enrolled.as_deref(),
    );

    let uuid = generate_uuid_from_fingerprint(&fingerprint);
    debug!("Generated UUID {} from fingerprint {}", uuid, fingerprint);
    
    Ok(uuid)
}

/// Generates a deterministic UUID from a fingerprint string
/// 
/// Uses SHA256 hash of the fingerprint, truncated to 16 bytes for UUID v4
fn generate_uuid_from_fingerprint(fingerprint: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(fingerprint.as_bytes());
    hasher.update(b"uuid_generation_salt"); // Add salt for UUID generation
    
    let hash = hasher.finalize();
    
    // Take first 16 bytes for UUID
    let mut uuid_bytes = [0u8; 16];
    uuid_bytes.copy_from_slice(&hash[..16]);
    
    // Set version (4) and variant bits for UUID v4
    uuid_bytes[6] = (uuid_bytes[6] & 0x0f) | 0x40; // Version 4
    uuid_bytes[8] = (uuid_bytes[8] & 0x3f) | 0x80; // Variant 10
    
    Uuid::from_bytes(uuid_bytes)
}

/// Validates if a string is a valid UUID
pub fn is_valid_uuid(uuid_str: &str) -> bool {
    Uuid::parse_str(uuid_str).is_ok()
}

/// Extracts device name for logging purposes
pub fn get_device_name(device_data: &HashMap<String, serde_json::Value>) -> String {
    device_data
        .get("deviceName")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            device_data
                .get("displayName")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        })
        .to_string()
}

/// Extracts device OS for filtering purposes
pub fn get_device_os(device_data: &HashMap<String, serde_json::Value>) -> Option<String> {
    device_data
        .get("operatingSystem")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            device_data
                .get("osVersion")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

/// Device information extracted for processing
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub uuid: Uuid,
    pub name: String,
    pub os: Option<String>,
    pub data: HashMap<String, serde_json::Value>,
    pub fingerprint: String,
}

impl DeviceInfo {
    /// Creates a new DeviceInfo from raw device data
    pub fn from_device_data(
        device_data: HashMap<String, serde_json::Value>,
    ) -> Result<Self, uuid::Error> {
        let uuid = get_or_generate_device_uuid(&device_data)?;
        let name = get_device_name(&device_data);
        let os = get_device_os(&device_data);
        
        let (serial, imei, hw_id, azure_id, model, enrolled) = extract_device_identifiers(&device_data);
        let fingerprint = generate_device_fingerprint(
            serial.as_deref(),
            imei.as_deref(),
            hw_id.as_deref(),
            azure_id.as_deref(),
            model.as_deref(),
            enrolled.as_deref(),
        );

        Ok(DeviceInfo {
            uuid,
            name,
            os,
            data: device_data,
            fingerprint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_generate_uuid_from_fingerprint() {
        let fingerprint = "test_fingerprint";
        let uuid1 = generate_uuid_from_fingerprint(fingerprint);
        let uuid2 = generate_uuid_from_fingerprint(fingerprint);
        
        // Same fingerprint should generate same UUID
        assert_eq!(uuid1, uuid2);
        
        // Different fingerprint should generate different UUID
        let uuid3 = generate_uuid_from_fingerprint("different_fingerprint");
        assert_ne!(uuid1, uuid3);
        
        // Verify it's a valid UUID v4
        assert_eq!(uuid1.get_version(), Some(uuid::Version::Random));
    }

    #[test]
    fn test_get_or_generate_device_uuid() {
        // Test with existing valid UUID
        let mut device_data = HashMap::new();
        let existing_uuid = Uuid::new_v4();
        device_data.insert("id".to_string(), json!(existing_uuid.to_string()));
        
        let result = get_or_generate_device_uuid(&device_data).unwrap();
        assert_eq!(result, existing_uuid);
        
        // Test with invalid UUID (should generate new one)
        device_data.insert("id".to_string(), json!("invalid-uuid"));
        device_data.insert("serialNumber".to_string(), json!("ABC123"));
        
        let result = get_or_generate_device_uuid(&device_data).unwrap();
        assert_ne!(result, existing_uuid);
        
        // Test with no UUID (should generate from fingerprint)
        device_data.remove("id");
        let result2 = get_or_generate_device_uuid(&device_data).unwrap();
        assert_eq!(result, result2); // Should be deterministic
    }

    #[test]
    fn test_is_valid_uuid() {
        assert!(is_valid_uuid(&Uuid::new_v4().to_string()));
        assert!(!is_valid_uuid("invalid-uuid"));
        assert!(!is_valid_uuid(""));
    }

    #[test]
    fn test_get_device_name() {
        let mut device_data = HashMap::new();
        device_data.insert("deviceName".to_string(), json!("Test Device"));
        
        assert_eq!(get_device_name(&device_data), "Test Device");
        
        // Test fallback to displayName
        device_data.remove("deviceName");
        device_data.insert("displayName".to_string(), json!("Display Name"));
        
        assert_eq!(get_device_name(&device_data), "Display Name");
        
        // Test fallback to unknown
        device_data.remove("displayName");
        assert_eq!(get_device_name(&device_data), "unknown");
    }

    #[test]
    fn test_get_device_os() {
        let mut device_data = HashMap::new();
        device_data.insert("operatingSystem".to_string(), json!("Windows"));
        
        assert_eq!(get_device_os(&device_data), Some("Windows".to_string()));
        
        // Test fallback to osVersion
        device_data.remove("operatingSystem");
        device_data.insert("osVersion".to_string(), json!("10.0.19041"));
        
        assert_eq!(get_device_os(&device_data), Some("10.0.19041".to_string()));
        
        // Test no OS info
        device_data.remove("osVersion");
        assert_eq!(get_device_os(&device_data), None);
    }

    #[test]
    fn test_device_info_creation() {
        let mut device_data = HashMap::new();
        device_data.insert("deviceName".to_string(), json!("Test Device"));
        device_data.insert("operatingSystem".to_string(), json!("Windows"));
        device_data.insert("serialNumber".to_string(), json!("ABC123"));
        
        let device_info = DeviceInfo::from_device_data(device_data).unwrap();
        
        assert_eq!(device_info.name, "Test Device");
        assert_eq!(device_info.os, Some("Windows".to_string()));
        assert!(!device_info.fingerprint.is_empty());
        assert!(device_info.uuid.get_version().is_some());
    }
}
