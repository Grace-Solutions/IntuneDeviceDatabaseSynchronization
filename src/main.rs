use anyhow::Result;
use clap::{Parser, Subcommand};
use log::{error, info};
use std::process;
use std::path::{Path, PathBuf};
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
mod path_utils;
mod rate_limiter;
mod service_manager;
mod storage;
mod sync;
mod uuid_utils;
mod version;
mod webhook;

use config::AppConfig;
use logging::setup_logging;
use sync::SyncService;

#[derive(Parser)]
#[command(name = "MSGraphDBSynchronizer")]
#[command(about = "Microsoft Graph API database synchronization service with multi-endpoint support")]
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
    service_manager::ServiceManager::install().await
}

async fn uninstall_service() -> Result<()> {
    service_manager::ServiceManager::uninstall().await
}

async fn start_service() -> Result<()> {
    service_manager::ServiceManager::start().await
}

async fn stop_service() -> Result<()> {
    service_manager::ServiceManager::stop().await
}

async fn restart_service() -> Result<()> {
    service_manager::ServiceManager::restart().await
}

async fn show_status() -> Result<()> {
    service_manager::ServiceManager::status().await
}

async fn run_service() -> Result<()> {
    // Load configuration
    println!("Loading configuration...");
    let config = AppConfig::load().await?;
    println!("Configuration loaded");

    // Setup logging
    println!("Setting up logging...");
    setup_logging(&config).await?;
    println!("Logging setup complete");

    info!("Starting {} v{}", version::get_product_name(), version::get_version());

    // Initialize metrics if enabled
    if config.enable_prometheus {
        info!("Initializing Prometheus metrics");
        metrics::init_metrics();
        tokio::spawn(metrics::start_metrics_server(config.prometheus_port));
    }

    // Create and start sync service
    info!("Creating sync service");
    let mut sync_service = SyncService::new(config).await?;
    info!("Sync service created");
    
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

    // Clean up resources
    info!("Cleaning up resources...");
    if let Err(e) = sync_service.cleanup().await {
        error!("Error during cleanup: {}", e);
    }

    Ok(())
}
