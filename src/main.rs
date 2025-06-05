use anyhow::Result;
use clap::{Parser, Subcommand};
use log::{error, info};
use std::process;
use tokio::signal;

mod auth;
mod backup;
mod config;
mod config_validator;
mod endpoint;
mod filter;
mod fingerprint;
mod logging;
mod metrics;
mod mock_graph_api;
mod rate_limiter;
mod storage;
mod sync;
mod uuid_utils;
mod version;
mod webhook;

use config::AppConfig;
use logging::setup_logging;
use sync::SyncService;

#[derive(Parser)]
#[command(name = "IntuneDeviceDatabaseSynchronization")]
#[command(about = "Microsoft Intune device synchronization service with OS filtering and multi-database support")]
#[command(version = version::get_version())]
#[command(author = version::get_company_name())]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install the service
    Install,
    /// Uninstall the service
    Uninstall,
    /// Start the service
    Start,
    /// Stop the service
    Stop,
    /// Restart the service
    Restart,
    /// Show service status
    Status,
    /// Run the service in foreground
    Run,
    /// Show detailed version information
    Version,
    /// Validate configuration file
    Validate {
        /// Path to configuration file (default: config.json)
        #[arg(short, long)]
        config: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Install => install_service().await,
        Commands::Uninstall => uninstall_service().await,
        Commands::Start => start_service().await,
        Commands::Stop => stop_service().await,
        Commands::Restart => restart_service().await,
        Commands::Status => show_status().await,
        Commands::Run => run_service().await,
        Commands::Version => {
            version::print_version_info();
            Ok(())
        }
        Commands::Validate { config } => {
            config_validator::validate_config_command(config)
        }
    }
}

async fn install_service() -> Result<()> {
    #[cfg(windows)]
    {
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
        println!("Service installed successfully");
    }

    #[cfg(not(windows))]
    {
        println!("Service installation not implemented for this platform");
    }

    Ok(())
}

async fn uninstall_service() -> Result<()> {
    #[cfg(windows)]
    {
        use windows_service::{
            service_manager::{ServiceManager, ServiceManagerAccess},
        };

        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service(version::get_product_name(), windows_service::service::ServiceAccess::DELETE)?;
        service.delete()?;
        println!("Service uninstalled successfully");
    }

    #[cfg(not(windows))]
    {
        println!("Service uninstallation not implemented for this platform");
    }

    Ok(())
}

async fn start_service() -> Result<()> {
    #[cfg(windows)]
    {
        use windows_service::{
            service::ServiceAccess,
            service_manager::{ServiceManager, ServiceManagerAccess},
        };

        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service(version::get_product_name(), ServiceAccess::START)?;
        service.start(&[] as &[&str])?;
        println!("Service started successfully");
    }

    #[cfg(not(windows))]
    {
        println!("Service start not implemented for this platform");
    }

    Ok(())
}

async fn stop_service() -> Result<()> {
    #[cfg(windows)]
    {
        use windows_service::{
            service::ServiceAccess,
            service_manager::{ServiceManager, ServiceManagerAccess},
        };

        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service(version::get_product_name(), ServiceAccess::STOP)?;
        service.stop()?;
        println!("Service stopped successfully");
    }

    #[cfg(not(windows))]
    {
        println!("Service stop not implemented for this platform");
    }

    Ok(())
}

async fn restart_service() -> Result<()> {
    stop_service().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    start_service().await
}

async fn show_status() -> Result<()> {
    #[cfg(windows)]
    {
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
    }

    #[cfg(not(windows))]
    {
        println!("Service status not implemented for this platform");
    }

    Ok(())
}

async fn run_service() -> Result<()> {
    // Load configuration
    let config = AppConfig::load().await?;
    
    // Setup logging
    setup_logging(&config)?;
    
    info!("Starting {} v{}", version::get_product_name(), version::get_version());
    
    // Initialize metrics if enabled
    if config.enable_prometheus {
        metrics::init_metrics();
        tokio::spawn(metrics::start_metrics_server(config.prometheus_port));
    }
    
    // Create and start sync service
    let mut sync_service = SyncService::new(config).await?;
    
    // Setup graceful shutdown
    let shutdown_signal = async {
        signal::ctrl_c().await.expect("Failed to install CTRL+C signal handler");
        info!("Shutdown signal received");
    };
    
    // Run the service
    tokio::select! {
        result = sync_service.run() => {
            if let Err(e) = result {
                error!("Service error: {}", e);
                process::exit(1);
            }
        }
        _ = shutdown_signal => {
            info!("Shutting down gracefully");
        }
    }
    
    Ok(())
}
