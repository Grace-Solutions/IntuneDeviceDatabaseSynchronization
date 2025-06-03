use log::{debug, warn};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Generates a SHA256 fingerprint from device identifying information
pub fn generate_device_fingerprint(
    serial_number: Option<&str>,
    imei: Option<&str>,
    hardware_id: Option<&str>,
    azure_ad_device_id: Option<&str>,
    model: Option<&str>,
    enrolled_date_time: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    
    // Add available identifying information to the hash
    let mut components = Vec::new();
    
    if let Some(serial) = serial_number.filter(|s| !s.trim().is_empty()) {
        components.push(format!("serial:{}", serial.trim()));
        hasher.update(serial.trim().as_bytes());
    }
    
    if let Some(imei) = imei.filter(|s| !s.trim().is_empty()) {
        components.push(format!("imei:{}", imei.trim()));
        hasher.update(imei.trim().as_bytes());
    }
    
    if let Some(hw_id) = hardware_id.filter(|s| !s.trim().is_empty()) {
        components.push(format!("hardware_id:{}", hw_id.trim()));
        hasher.update(hw_id.trim().as_bytes());
    }
    
    if let Some(azure_id) = azure_ad_device_id.filter(|s| !s.trim().is_empty()) {
        components.push(format!("azure_ad_device_id:{}", azure_id.trim()));
        hasher.update(azure_id.trim().as_bytes());
    }
    
    // Fallback to model + enrollment date if no other identifiers
    if components.is_empty() {
        if let Some(model) = model.filter(|s| !s.trim().is_empty()) {
            components.push(format!("model:{}", model.trim()));
            hasher.update(model.trim().as_bytes());
        }
        
        if let Some(enrolled) = enrolled_date_time.filter(|s| !s.trim().is_empty()) {
            components.push(format!("enrolled:{}", enrolled.trim()));
            hasher.update(enrolled.trim().as_bytes());
        }
    }
    
    if components.is_empty() {
        warn!("No identifying information available for device fingerprint");
        // Use a random component to ensure we still generate something
        hasher.update(b"unknown_device");
        components.push("unknown_device".to_string());
    }
    
    let result = hasher.finalize();
    let fingerprint = hex::encode(result);
    
    debug!("Generated fingerprint {} from components: {:?}", fingerprint, components);
    
    fingerprint
}

/// Calculates a hash of device data for change detection
pub fn calculate_device_hash(device_data: &HashMap<String, serde_json::Value>) -> String {
    let mut hasher = Sha256::new();
    
    // Sort keys to ensure consistent hashing
    let mut sorted_keys: Vec<_> = device_data.keys().collect();
    sorted_keys.sort();
    
    for key in sorted_keys {
        if let Some(value) = device_data.get(key) {
            hasher.update(key.as_bytes());
            hasher.update(b":");
            hasher.update(value.to_string().as_bytes());
            hasher.update(b";");
        }
    }
    
    let result = hasher.finalize();
    hex::encode(result)
}

/// Extracts identifying information from device data for fingerprinting
pub fn extract_device_identifiers(
    device_data: &HashMap<String, serde_json::Value>,
) -> (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>) {
    let serial_number = device_data
        .get("serialNumber")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let imei = device_data
        .get("imei")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let hardware_id = device_data
        .get("hardwareInformation")
        .and_then(|v| v.get("hardwareId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            device_data
                .get("hardwareId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });
    
    let azure_ad_device_id = device_data
        .get("azureADDeviceId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let model = device_data
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let enrolled_date_time = device_data
        .get("enrolledDateTime")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    (serial_number, imei, hardware_id, azure_ad_device_id, model, enrolled_date_time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_generate_device_fingerprint() {
        // Test with serial number
        let fingerprint1 = generate_device_fingerprint(
            Some("ABC123"),
            None,
            None,
            None,
            None,
            None,
        );
        assert!(!fingerprint1.is_empty());
        assert_eq!(fingerprint1.len(), 64); // SHA256 hex length
        
        // Test with multiple identifiers
        let fingerprint2 = generate_device_fingerprint(
            Some("ABC123"),
            Some("123456789012345"),
            Some("HW123"),
            None,
            None,
            None,
        );
        assert!(!fingerprint2.is_empty());
        assert_ne!(fingerprint1, fingerprint2);
        
        // Test with no identifiers (fallback)
        let fingerprint3 = generate_device_fingerprint(
            None,
            None,
            None,
            None,
            Some("iPhone"),
            Some("2023-01-01T00:00:00Z"),
        );
        assert!(!fingerprint3.is_empty());
        
        // Test with completely empty data
        let fingerprint4 = generate_device_fingerprint(
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(!fingerprint4.is_empty());
    }
    
    #[test]
    fn test_calculate_device_hash() {
        let mut device_data = HashMap::new();
        device_data.insert("deviceName".to_string(), json!("Test Device"));
        device_data.insert("operatingSystem".to_string(), json!("Windows"));
        device_data.insert("serialNumber".to_string(), json!("ABC123"));
        
        let hash1 = calculate_device_hash(&device_data);
        assert!(!hash1.is_empty());
        assert_eq!(hash1.len(), 64);
        
        // Same data should produce same hash
        let hash2 = calculate_device_hash(&device_data);
        assert_eq!(hash1, hash2);
        
        // Different data should produce different hash
        device_data.insert("deviceName".to_string(), json!("Different Device"));
        let hash3 = calculate_device_hash(&device_data);
        assert_ne!(hash1, hash3);
    }
    
    #[test]
    fn test_extract_device_identifiers() {
        let mut device_data = HashMap::new();
        device_data.insert("serialNumber".to_string(), json!("ABC123"));
        device_data.insert("imei".to_string(), json!("123456789012345"));
        device_data.insert("azureADDeviceId".to_string(), json!("azure-123"));
        device_data.insert("model".to_string(), json!("iPhone"));
        device_data.insert("enrolledDateTime".to_string(), json!("2023-01-01T00:00:00Z"));
        
        let (serial, imei, hw_id, azure_id, model, enrolled) = extract_device_identifiers(&device_data);
        
        assert_eq!(serial, Some("ABC123".to_string()));
        assert_eq!(imei, Some("123456789012345".to_string()));
        assert_eq!(hw_id, None); // Not present in test data
        assert_eq!(azure_id, Some("azure-123".to_string()));
        assert_eq!(model, Some("iPhone".to_string()));
        assert_eq!(enrolled, Some("2023-01-01T00:00:00Z".to_string()));
    }
}
