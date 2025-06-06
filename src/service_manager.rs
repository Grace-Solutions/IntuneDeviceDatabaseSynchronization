use anyhow::{Context, Result};
use std::path::PathBuf;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use log::{info, warn};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::fs;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;

#[cfg(windows)]
use log::warn;

use crate::version;

/// Service management for different platforms
pub struct ServiceManager;

impl ServiceManager {
    /// Install service on the current platform
    pub async fn install() -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            Self::install_systemd_service().await
        }
        #[cfg(target_os = "macos")]
        {
            Self::install_launchd_service().await
        }
        #[cfg(windows)]
        {
            Self::install_windows_service().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            Err(anyhow::anyhow!("Service installation not supported on this platform"))
        }
    }

    /// Uninstall service on the current platform
    pub async fn uninstall() -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            Self::uninstall_systemd_service().await
        }
        #[cfg(target_os = "macos")]
        {
            Self::uninstall_launchd_service().await
        }
        #[cfg(windows)]
        {
            Self::uninstall_windows_service().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            Err(anyhow::anyhow!("Service uninstallation not supported on this platform"))
        }
    }

    /// Start service on the current platform
    pub async fn start() -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            Self::start_systemd_service().await
        }
        #[cfg(target_os = "macos")]
        {
            Self::start_launchd_service().await
        }
        #[cfg(windows)]
        {
            Self::start_windows_service().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            Err(anyhow::anyhow!("Service start not supported on this platform"))
        }
    }

    /// Stop service on the current platform
    pub async fn stop() -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            Self::stop_systemd_service().await
        }
        #[cfg(target_os = "macos")]
        {
            Self::stop_launchd_service().await
        }
        #[cfg(windows)]
        {
            Self::stop_windows_service().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            Err(anyhow::anyhow!("Service stop not supported on this platform"))
        }
    }

    /// Restart service on the current platform
    pub async fn restart() -> Result<()> {
        Self::stop().await.ok(); // Don't fail if stop fails
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        Self::start().await
    }

    /// Show service status on the current platform
    pub async fn status() -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            Self::status_systemd_service().await
        }
        #[cfg(target_os = "macos")]
        {
            Self::status_launchd_service().await
        }
        #[cfg(windows)]
        {
            Self::status_windows_service().await
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            Err(anyhow::anyhow!("Service status not supported on this platform"))
        }
    }

    /// Get the service name for the current platform
    fn get_service_name() -> &'static str {
        "msgraph-db-synchronizer"
    }

    /// Get the service display name
    fn get_service_display_name() -> String {
        format!("{} Service", version::get_product_name())
    }

    /// Get the current executable path
    fn get_executable_path() -> Result<PathBuf> {
        std::env::current_exe()
            .context("Failed to get current executable path")
    }

    /// Check if running as root/administrator
    fn is_elevated() -> bool {
        #[cfg(unix)]
        {
            unsafe { libc::geteuid() == 0 }
        }
        #[cfg(windows)]
        {
            // For Windows, we'll assume elevated if we can write to system directories
            // This is a simplified check - in practice, you'd use Windows APIs
            true
        }
    }

    /// Ensure the process is running with elevated privileges
    fn ensure_elevated() -> Result<()> {
        if !Self::is_elevated() {
            return Err(anyhow::anyhow!(
                "This operation requires elevated privileges. Please run as root/administrator."
            ));
        }
        Ok(())
    }

    // Linux systemd implementation
    #[cfg(target_os = "linux")]
    async fn install_systemd_service() -> Result<()> {
        Self::ensure_elevated()?;

        let service_name = Self::get_service_name();
        let service_file_path = format!("/etc/systemd/system/{}.service", service_name);
        let executable_path = Self::get_executable_path()?;

        info!("Installing systemd service: {}", service_name);

        // Create service user if it doesn't exist
        Self::create_service_user().await?;

        // Ensure log directory exists and has proper permissions
        Self::setup_log_directory().await?;

        // Create service file content
        let service_content = format!(
            r#"[Unit]
Description={}
After=network.target
Wants=network.target

[Service]
Type=simple
User={}
Group={}
WorkingDirectory={}
ExecStart={} run
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier={}

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths={}

[Install]
WantedBy=multi-user.target
"#,
            Self::get_service_display_name(),
            service_name,
            service_name,
            executable_path.parent().unwrap().display(),
            executable_path.display(),
            service_name,
            executable_path.parent().unwrap().display()
        );

        // Write service file
        fs::write(&service_file_path, service_content)
            .with_context(|| format!("Failed to write service file: {}", service_file_path))?;

        info!("Service file created: {}", service_file_path);

        // Reload systemd daemon
        let output = Command::new("systemctl")
            .args(&["daemon-reload"])
            .output()
            .context("Failed to reload systemd daemon")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to reload systemd daemon: {}", stderr));
        }

        // Enable service
        let output = Command::new("systemctl")
            .args(&["enable", service_name])
            .output()
            .context("Failed to enable service")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to enable service: {}", stderr));
        }

        println!("✅ Service installed and enabled successfully");
        println!("   Service name: {}", service_name);
        println!("   Service file: {}", service_file_path);
        println!("   To start: sudo systemctl start {}", service_name);
        println!("   To check status: sudo systemctl status {}", service_name);

        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn create_service_user() -> Result<()> {
        let service_name = Self::get_service_name();

        // Check if user already exists
        let output = Command::new("id")
            .arg(service_name)
            .output();

        match output {
            Ok(output) if output.status.success() => {
                info!("Service user '{}' already exists", service_name);
                return Ok(());
            }
            _ => {
                info!("Creating service user: {}", service_name);
            }
        }

        // Create system user
        let output = Command::new("useradd")
            .args(&[
                "--system",
                "--no-create-home",
                "--shell", "/bin/false",
                "--comment", &format!("{} service user", version::get_product_name()),
                service_name
            ])
            .output()
            .context("Failed to create service user")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to create service user: {}", stderr));
        }

        info!("Service user '{}' created successfully", service_name);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn setup_log_directory() -> Result<()> {
        let service_name = Self::get_service_name();
        let executable_path = Self::get_executable_path()?;
        let log_dir = executable_path.parent().unwrap().join("logs");

        // Create logs directory
        if !log_dir.exists() {
            fs::create_dir_all(&log_dir)
                .with_context(|| format!("Failed to create log directory: {}", log_dir.display()))?;
            info!("Created log directory: {}", log_dir.display());
        }

        // Set ownership to service user
        let output = Command::new("chown")
            .args(&["-R", &format!("{}:{}", service_name, service_name), &log_dir.to_string_lossy()])
            .output()
            .context("Failed to set log directory ownership")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to set log directory ownership: {}", stderr);
        } else {
            info!("Set log directory ownership to {}", service_name);
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn uninstall_systemd_service() -> Result<()> {
        Self::ensure_elevated()?;

        let service_name = Self::get_service_name();
        let service_file_path = format!("/etc/systemd/system/{}.service", service_name);

        info!("Uninstalling systemd service: {}", service_name);

        // Stop service if running
        info!("Stopping service if running...");
        let output = Command::new("systemctl")
            .args(&["stop", service_name])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                info!("Service stopped successfully");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to stop service (may not be running): {}", stderr);
            }
            Err(e) => {
                warn!("Error stopping service: {}", e);
            }
        }

        // Disable service
        info!("Disabling service...");
        let output = Command::new("systemctl")
            .args(&["disable", service_name])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                info!("Service disabled successfully");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to disable service: {}", stderr);
            }
            Err(e) => {
                warn!("Error disabling service: {}", e);
            }
        }

        // Remove service file
        if Path::new(&service_file_path).exists() {
            fs::remove_file(&service_file_path)
                .with_context(|| format!("Failed to remove service file: {}", service_file_path))?;
            info!("Service file removed: {}", service_file_path);
        } else {
            warn!("Service file not found: {}", service_file_path);
        }

        // Reload systemd daemon
        info!("Reloading systemd daemon...");
        let _ = Command::new("systemctl")
            .args(&["daemon-reload"])
            .output();

        println!("✅ Service uninstalled successfully");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn start_systemd_service() -> Result<()> {
        Self::ensure_elevated()?;

        let service_name = Self::get_service_name();

        let output = Command::new("systemctl")
            .args(&["start", service_name])
            .output()
            .context("Failed to start service")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to start service: {}", stderr));
        }

        println!("✅ Service started successfully");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn stop_systemd_service() -> Result<()> {
        Self::ensure_elevated()?;

        let service_name = Self::get_service_name();

        let output = Command::new("systemctl")
            .args(&["stop", service_name])
            .output()
            .context("Failed to stop service")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to stop service: {}", stderr));
        }

        println!("✅ Service stopped successfully");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn status_systemd_service() -> Result<()> {
        let service_name = Self::get_service_name();

        let output = Command::new("systemctl")
            .args(&["status", service_name, "--no-pager"])
            .output()
            .context("Failed to get service status")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stderr.is_empty() {
            println!("Status output:\n{}", stderr);
        }

        println!("{}", stdout);
        Ok(())
    }

    // macOS launchd implementation
    #[cfg(target_os = "macos")]
    async fn install_launchd_service() -> Result<()> {
        Self::ensure_elevated()?;

        let service_name = format!("com.gracesolutions.{}", Self::get_service_name());
        let plist_path = format!("/Library/LaunchDaemons/{}.plist", service_name);
        let executable_path = Self::get_executable_path()?;

        info!("Installing launchd service: {}", service_name);

        // Create plist content
        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>run</string>
    </array>
    <key>WorkingDirectory</key>
    <string>{}</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/var/log/msgraph-db-synchronizer.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/msgraph-db-synchronizer.error.log</string>
    <key>UserName</key>
    <string>_msgraphsync</string>
    <key>GroupName</key>
    <string>_msgraphsync</string>
</dict>
</plist>
"#,
            service_name,
            executable_path.display(),
            executable_path.parent().unwrap().display()
        );

        // Create service user if it doesn't exist
        Self::create_macos_service_user().await?;

        // Setup log files with proper permissions
        Self::setup_macos_log_files().await?;

        // Write plist file
        fs::write(&plist_path, plist_content)
            .with_context(|| format!("Failed to write plist file: {}", plist_path))?;

        // Set proper permissions
        let output = Command::new("chown")
            .args(&["root:wheel", &plist_path])
            .output()
            .context("Failed to set plist ownership")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to set plist ownership: {}", stderr);
        }

        let output = Command::new("chmod")
            .args(&["644", &plist_path])
            .output()
            .context("Failed to set plist permissions")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to set plist permissions: {}", stderr);
        }

        // Load the service
        let output = Command::new("launchctl")
            .args(&["load", &plist_path])
            .output()
            .context("Failed to load service")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to load service: {}", stderr));
        }

        println!("✅ Service installed and loaded successfully");
        println!("   Service name: {}", service_name);
        println!("   Plist file: {}", plist_path);
        println!("   To start: sudo launchctl start {}", service_name);
        println!("   To check status: sudo launchctl list | grep {}", Self::get_service_name());

        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn create_macos_service_user() -> Result<()> {
        let username = "_msgraphsync";

        // Check if user already exists
        let output = Command::new("id")
            .arg(username)
            .output();

        match output {
            Ok(output) if output.status.success() => {
                info!("Service user '{}' already exists", username);
                return Ok(());
            }
            _ => {
                info!("Creating service user: {}", username);
            }
        }

        // Find next available UID in system range (200-400)
        let mut uid = 200;
        loop {
            let output = Command::new("dscl")
                .args(&[".", "list", "/Users", "UniqueID"])
                .output()
                .context("Failed to list existing UIDs")?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.contains(&uid.to_string()) {
                break;
            }
            uid += 1;
            if uid > 400 {
                return Err(anyhow::anyhow!("No available UID found in system range"));
            }
        }

        // Create user
        let commands = vec![
            vec![".", "create", &format!("/Users/{}", username)],
            vec![".", "create", &format!("/Users/{}", username), "UserShell", "/usr/bin/false"],
            vec![".", "create", &format!("/Users/{}", username), "RealName", &format!("{} Service User", version::get_product_name())],
            vec![".", "create", &format!("/Users/{}", username), "UniqueID", &uid.to_string()],
            vec![".", "create", &format!("/Users/{}", username), "PrimaryGroupID", &uid.to_string()],
            vec![".", "create", &format!("/Users/{}", username), "NFSHomeDirectory", "/var/empty"],
        ];

        for cmd_args in commands {
            let output = Command::new("dscl")
                .args(&cmd_args)
                .output()
                .context("Failed to create service user")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("Failed to create service user: {}", stderr));
            }
        }

        info!("Service user '{}' created successfully with UID {}", username, uid);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn setup_macos_log_files() -> Result<()> {
        let username = "_msgraphsync";
        let log_files = [
            "/var/log/msgraph-db-synchronizer.log",
            "/var/log/msgraph-db-synchronizer.error.log",
        ];

        for log_file in &log_files {
            // Create log file if it doesn't exist
            if !Path::new(log_file).exists() {
                fs::write(log_file, "")
                    .with_context(|| format!("Failed to create log file: {}", log_file))?;
                info!("Created log file: {}", log_file);
            }

            // Set ownership to service user
            let output = Command::new("chown")
                .args(&[&format!("{}:{}", username, username), log_file])
                .output()
                .context("Failed to set log file ownership")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to set log file ownership for {}: {}", log_file, stderr);
            } else {
                info!("Set log file ownership for {}", log_file);
            }

            // Set permissions (644 - readable by all, writable by owner)
            let output = Command::new("chmod")
                .args(&["644", log_file])
                .output()
                .context("Failed to set log file permissions")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to set log file permissions for {}: {}", log_file, stderr);
            }
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn uninstall_launchd_service() -> Result<()> {
        Self::ensure_elevated()?;

        let service_name = format!("com.gracesolutions.{}", Self::get_service_name());
        let plist_path = format!("/Library/LaunchDaemons/{}.plist", service_name);

        info!("Uninstalling launchd service: {}", service_name);

        // Stop service if running
        info!("Stopping service if running...");
        let output = Command::new("launchctl")
            .args(&["stop", &service_name])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                info!("Service stopped successfully");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to stop service (may not be running): {}", stderr);
            }
            Err(e) => {
                warn!("Error stopping service: {}", e);
            }
        }

        // Unload service if loaded
        info!("Unloading service...");
        let output = Command::new("launchctl")
            .args(&["unload", &plist_path])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                info!("Service unloaded successfully");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to unload service: {}", stderr);
            }
            Err(e) => {
                warn!("Error unloading service: {}", e);
            }
        }

        // Remove plist file
        if Path::new(&plist_path).exists() {
            fs::remove_file(&plist_path)
                .with_context(|| format!("Failed to remove plist file: {}", plist_path))?;
            info!("Plist file removed: {}", plist_path);
        } else {
            warn!("Plist file not found: {}", plist_path);
        }

        println!("✅ Service uninstalled successfully");
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn start_launchd_service() -> Result<()> {
        Self::ensure_elevated()?;

        let service_name = format!("com.gracesolutions.{}", Self::get_service_name());

        let output = Command::new("launchctl")
            .args(&["start", &service_name])
            .output()
            .context("Failed to start service")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to start service: {}", stderr));
        }

        println!("✅ Service started successfully");
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn stop_launchd_service() -> Result<()> {
        Self::ensure_elevated()?;

        let service_name = format!("com.gracesolutions.{}", Self::get_service_name());

        let output = Command::new("launchctl")
            .args(&["stop", &service_name])
            .output()
            .context("Failed to stop service")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to stop service: {}", stderr));
        }

        println!("✅ Service stopped successfully");
        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn status_launchd_service() -> Result<()> {
        let service_name = format!("com.gracesolutions.{}", Self::get_service_name());

        let output = Command::new("launchctl")
            .args(&["list", &service_name])
            .output()
            .context("Failed to get service status")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("Service Status:\n{}", stdout);
        } else {
            println!("Service is not loaded or does not exist");
        }

        Ok(())
    }

    // Windows service implementation (delegated to existing code)
    #[cfg(windows)]
    async fn install_windows_service() -> Result<()> {
        use std::ffi::OsString;
        use windows_service::{
            service::{ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType},
            service_manager::{ServiceManager, ServiceManagerAccess},
        };

        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CREATE_SERVICE)?;

        let service_info = ServiceInfo {
            name: OsString::from(version::get_product_name()),
            display_name: OsString::from(format!("{} Service", version::get_product_name())),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: std::env::current_exe()?,
            launch_arguments: vec![OsString::from("run")],
            dependencies: vec![],
            account_name: None,
            account_password: None,
        };

        let _service = manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
        println!("✅ Service installed successfully");
        Ok(())
    }

    #[cfg(windows)]
    async fn uninstall_windows_service() -> Result<()> {
        use windows_service::{
            service_manager::{ServiceManager, ServiceManagerAccess},
            service::ServiceAccess,
        };

        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

        // Try to stop the service first
        match manager.open_service(version::get_product_name(), ServiceAccess::STOP) {
            Ok(service) => {
                match service.stop() {
                    Ok(_) => {
                        println!("Service stopped successfully");
                        // Wait a moment for the service to fully stop
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    }
                    Err(e) => {
                        warn!("Failed to stop service (may not be running): {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Could not open service for stopping: {}", e);
            }
        }

        // Now delete the service
        let service = manager.open_service(version::get_product_name(), ServiceAccess::DELETE)?;
        service.delete()?;
        println!("✅ Service uninstalled successfully");
        Ok(())
    }

    #[cfg(windows)]
    async fn start_windows_service() -> Result<()> {
        use windows_service::{
            service::ServiceAccess,
            service_manager::{ServiceManager, ServiceManagerAccess},
        };

        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service(version::get_product_name(), ServiceAccess::START)?;
        service.start(&[] as &[&str])?;
        println!("✅ Service started successfully");
        Ok(())
    }

    #[cfg(windows)]
    async fn stop_windows_service() -> Result<()> {
        use windows_service::{
            service::ServiceAccess,
            service_manager::{ServiceManager, ServiceManagerAccess},
        };

        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service(version::get_product_name(), ServiceAccess::STOP)?;
        service.stop()?;
        println!("✅ Service stopped successfully");
        Ok(())
    }

    #[cfg(windows)]
    async fn status_windows_service() -> Result<()> {
        use windows_service::{
            service::ServiceAccess,
            service_manager::{ServiceManager, ServiceManagerAccess},
        };

        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        match manager.open_service(version::get_product_name(), ServiceAccess::QUERY_STATUS) {
            Ok(service) => {
                let status = service.query_status()?;
                println!("Service Status: {:?}", status.current_state);
            }
            Err(_) => {
                println!("Service not installed");
            }
        }
        Ok(())
    }
}
