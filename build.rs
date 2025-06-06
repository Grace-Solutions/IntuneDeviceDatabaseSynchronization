use chrono::{DateTime, Utc, Datelike, Timelike};
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Always use the version from Cargo.toml for consistency
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "2.0.0".to_string());
    let now: DateTime<Utc> = Utc::now();
    
    // Write version to a file that can be included in the binary
    let version_file_path = Path::new(&env::var("OUT_DIR").unwrap()).join("version.rs");
    let version_content = format!(
        r#"
pub const BUILD_VERSION: &str = "{}";
pub const BUILD_TIMESTAMP: &str = "{}";
pub const PRODUCT_NAME: &str = "MSGraphDBSynchronizer";
pub const COMPANY_NAME: &str = "Grace Solutions";
pub const COPYRIGHT: &str = "Copyright © {} Grace Solutions";
pub const DESCRIPTION: &str = "Microsoft Graph API database synchronization service with multi-endpoint support";
"#,
        version,
        now.format("%Y-%m-%d %H:%M:%S UTC"),
        now.year()
    );
    
    fs::write(&version_file_path, version_content)
        .expect("Failed to write version file");
    
    // Tell Cargo to rerun this build script if any of these change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    
    // Only embed Windows resources on Windows
    #[cfg(windows)]
    embed_windows_resources(&version, &now);
}

#[cfg(windows)]
fn embed_windows_resources(version: &str, build_time: &DateTime<Utc>) {
    
    // Check if icon exists
    let icon_path = "assets/icon.ico";
    if !Path::new(icon_path).exists() {
        println!("cargo:warning=Icon file not found at {}, skipping icon embedding", icon_path);
        return;
    }
    
    // Create Windows resource file
    let mut res = winres::WindowsResource::new();
    
    // Set icon
    res.set_icon(icon_path);
    
    // Set version info
    let version_parts: Vec<&str> = version.split('.').collect();
    if version_parts.len() >= 4 {
        if let (Ok(major), Ok(minor), Ok(patch), Ok(build)) = (
            version_parts[0].parse::<u64>(),
            version_parts[1].parse::<u64>(),
            version_parts[2].parse::<u64>(),
            version_parts[3].parse::<u64>(),
        ) {
            res.set_version_info(winres::VersionInfo::PRODUCTVERSION, (major << 48) | (minor << 32) | (patch << 16) | build);
            res.set_version_info(winres::VersionInfo::FILEVERSION, (major << 48) | (minor << 32) | (patch << 16) | build);
        }
    }
    
    // Set string version info
    res.set("ProductName", "MSGraphDBSynchronizer");
    res.set("ProductVersion", version);
    res.set("FileVersion", version);
    res.set("FileDescription", "Microsoft Graph API database synchronization service with multi-endpoint support");
    res.set("CompanyName", "Grace Solutions");
    res.set("LegalCopyright", &format!("Copyright © {} Grace Solutions", build_time.year()));
    res.set("OriginalFilename", "MSGraphDBSynchronizer.exe");
    res.set("InternalName", "MSGraphDBSynchronizer");
    
    // Compile the resource
    if let Err(e) = res.compile() {
        println!("cargo:warning=Failed to compile Windows resources: {}", e);
    } else {
        println!("cargo:info=Successfully embedded Windows resources and icon");
    }
}

#[cfg(not(windows))]
fn embed_windows_resources(_version: &str, _build_time: &DateTime<Utc>) {
    // No-op on non-Windows platforms
}
