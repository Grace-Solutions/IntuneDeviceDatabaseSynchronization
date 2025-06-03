use log::{debug, info};

use crate::metrics;

/// Normalizes a filter string by splitting on commas, trimming whitespace,
/// converting to lowercase, and filtering out empty strings.
pub fn normalize_filter(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Checks if a device OS matches any of the provided filters.
/// 
/// # Arguments
/// * `device_os` - The operating system string from the device
/// * `filters` - List of normalized filter strings
/// 
/// # Returns
/// `true` if the device OS matches any filter, `false` otherwise
/// 
/// # Matching Rules
/// - If filters contain "*", all devices match
/// - Otherwise, case-insensitive substring matching is used
/// - Empty or None device OS is treated as "unknown"
pub fn os_matches_filter(device_os: Option<&str>, filters: &[String]) -> bool {
    // If wildcard is present, match everything
    if filters.contains(&"*".to_string()) {
        debug!("Wildcard filter found, allowing all devices");
        return true;
    }

    // Handle missing/empty OS
    let os = match device_os {
        Some(os) if !os.trim().is_empty() => os.trim().to_lowercase(),
        _ => {
            debug!("Device has no OS information, treating as 'unknown'");
            "unknown".to_string()
        }
    };

    // Check if any filter matches
    let matches = filters.iter().any(|filter| {
        let result = os.contains(filter);
        debug!("Checking OS '{}' against filter '{}': {}", os, filter, result);
        result
    });

    if matches {
        debug!("Device OS '{}' matched filters", os);
        metrics::DEVICE_FILTER_MATCHED_TOTAL.inc();
    } else {
        debug!("Device OS '{}' did not match any filters", os);
        metrics::DEVICE_FILTER_SKIPPED_TOTAL.inc();
    }

    matches
}

/// Logs information about a device being filtered
pub fn log_device_filter_result(
    device_name: Option<&str>,
    device_os: Option<&str>,
    matched: bool,
) {
    let name = device_name.unwrap_or("unknown");
    let os = device_os.unwrap_or("unknown");

    if matched {
        info!("[Filter] - Allowed device '{}' with OS '{}'", name, os);
    } else {
        info!("[Filter] - Skipped device '{}' with OS '{}'", name, os);
    }
}

/// Device OS filter configuration and logic
pub struct DeviceOsFilter {
    filters: Vec<String>,
}

impl DeviceOsFilter {
    /// Creates a new device OS filter from a list of filter strings
    pub fn new(raw_filters: &[String]) -> Self {
        let mut normalized_filters = Vec::new();
        
        for filter in raw_filters {
            let mut normalized = normalize_filter(filter);
            normalized_filters.append(&mut normalized);
        }

        // If no filters provided, default to wildcard
        if normalized_filters.is_empty() {
            normalized_filters.push("*".to_string());
        }

        info!("Initialized OS filter with rules: {:?}", normalized_filters);

        Self {
            filters: normalized_filters,
        }
    }

    /// Checks if a device should be included based on its OS
    pub fn should_include_device(
        &self,
        device_name: Option<&str>,
        device_os: Option<&str>,
    ) -> bool {
        let matches = os_matches_filter(device_os, &self.filters);
        log_device_filter_result(device_name, device_os, matches);
        matches
    }

    /// Returns the active filter rules
    pub fn get_filters(&self) -> &[String] {
        &self.filters
    }

    /// Checks if the filter allows all devices (contains wildcard)
    pub fn allows_all(&self) -> bool {
        self.filters.contains(&"*".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_filter() {
        assert_eq!(
            normalize_filter("Windows, macOS,  Android "),
            vec!["windows", "macos", "android"]
        );
        
        assert_eq!(
            normalize_filter("*"),
            vec!["*"]
        );
        
        assert_eq!(
            normalize_filter(""),
            Vec::<String>::new()
        );
        
        assert_eq!(
            normalize_filter("Windows,,,macOS"),
            vec!["windows", "macos"]
        );
    }

    #[test]
    fn test_os_matches_filter() {
        let filters = vec!["windows".to_string(), "macos".to_string()];
        
        assert!(os_matches_filter(Some("Windows 10"), &filters));
        assert!(os_matches_filter(Some("macOS Big Sur"), &filters));
        assert!(os_matches_filter(Some("WINDOWS"), &filters));
        assert!(!os_matches_filter(Some("Android"), &filters));
        assert!(!os_matches_filter(Some("iOS"), &filters));
        assert!(!os_matches_filter(None, &filters));
        assert!(!os_matches_filter(Some(""), &filters));
    }

    #[test]
    fn test_wildcard_filter() {
        let wildcard_filters = vec!["*".to_string()];
        
        assert!(os_matches_filter(Some("Windows"), &wildcard_filters));
        assert!(os_matches_filter(Some("macOS"), &wildcard_filters));
        assert!(os_matches_filter(Some("Android"), &wildcard_filters));
        assert!(os_matches_filter(None, &wildcard_filters));
        assert!(os_matches_filter(Some(""), &wildcard_filters));
    }

    #[test]
    fn test_device_os_filter() {
        let filter = DeviceOsFilter::new(&["Windows".to_string(), "macOS".to_string()]);
        
        assert!(filter.should_include_device(Some("Test Device"), Some("Windows 10")));
        assert!(filter.should_include_device(Some("Test Device"), Some("macOS Big Sur")));
        assert!(!filter.should_include_device(Some("Test Device"), Some("Android")));
        assert!(!filter.should_include_device(Some("Test Device"), None));
        
        assert!(!filter.allows_all());
        assert_eq!(filter.get_filters(), &["windows", "macos"]);
    }

    #[test]
    fn test_device_os_filter_wildcard() {
        let filter = DeviceOsFilter::new(&["*".to_string()]);
        
        assert!(filter.should_include_device(Some("Test Device"), Some("Windows")));
        assert!(filter.should_include_device(Some("Test Device"), Some("Android")));
        assert!(filter.should_include_device(Some("Test Device"), None));
        
        assert!(filter.allows_all());
    }

    #[test]
    fn test_device_os_filter_empty() {
        let filter = DeviceOsFilter::new(&[]);
        
        // Should default to wildcard
        assert!(filter.should_include_device(Some("Test Device"), Some("Windows")));
        assert!(filter.allows_all());
    }
}
