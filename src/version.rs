// Include the generated version information
include!(concat!(env!("OUT_DIR"), "/version.rs"));

/// Get the full version string including build timestamp
pub fn get_full_version() -> String {
    format!("{} (built {})", BUILD_VERSION, BUILD_TIMESTAMP)
}

/// Get just the version number
pub fn get_version() -> &'static str {
    BUILD_VERSION
}

/// Get the build timestamp
pub fn get_build_timestamp() -> &'static str {
    BUILD_TIMESTAMP
}

/// Get the product name
pub fn get_product_name() -> &'static str {
    PRODUCT_NAME
}

/// Get the company name
pub fn get_company_name() -> &'static str {
    COMPANY_NAME
}

/// Get the copyright string
pub fn get_copyright() -> &'static str {
    COPYRIGHT
}

/// Get the product description
pub fn get_description() -> &'static str {
    DESCRIPTION
}

/// Print version information to stdout
pub fn print_version_info() {
    println!("{} v{}", PRODUCT_NAME, BUILD_VERSION);
    println!("Built: {}", BUILD_TIMESTAMP);
    println!("{}", COPYRIGHT);
    println!();
    println!("{}", DESCRIPTION);
}

/// Get version info as a structured format for logging/metrics
pub fn get_version_info() -> VersionInfo {
    VersionInfo {
        product_name: PRODUCT_NAME,
        version: BUILD_VERSION,
        build_timestamp: BUILD_TIMESTAMP,
        company: COMPANY_NAME,
        copyright: COPYRIGHT,
        description: DESCRIPTION,
    }
}

#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub product_name: &'static str,
    pub version: &'static str,
    pub build_timestamp: &'static str,
    pub company: &'static str,
    pub copyright: &'static str,
    pub description: &'static str,
}
