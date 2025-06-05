use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use log::{info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub enabled: bool,
    pub directory: String,
    #[serde(rename = "maxBackups")]
    pub max_backups: usize,
    #[serde(rename = "scheduleEnabled")]
    pub schedule_enabled: bool,
    #[serde(rename = "scheduleInterval")]
    pub schedule_interval: Option<String>,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: "./backups".to_string(),
            max_backups: 10,
            schedule_enabled: true,
            schedule_interval: Some("24h".to_string()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub created_at: DateTime<Utc>,
    pub database_path: String,
    pub database_size: u64,
    pub version: String,
    pub backup_type: BackupType,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BackupType {
    Manual,
    Scheduled,
    PreUpdate,
}

pub struct SqliteBackupManager {
    backup_dir: PathBuf,
    max_backups: usize,
}

impl SqliteBackupManager {
    pub fn new<P: AsRef<Path>>(backup_dir: P, max_backups: usize) -> Result<Self> {
        let backup_dir = backup_dir.as_ref().to_path_buf();
        
        // Create backup directory if it doesn't exist
        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir)
                .with_context(|| format!("Failed to create backup directory: {}", backup_dir.display()))?;
            info!("Created backup directory: {}", backup_dir.display());
        }

        Ok(Self {
            backup_dir,
            max_backups,
        })
    }

    /// Create a backup of the SQLite database
    pub fn create_backup<P: AsRef<Path>>(&self, db_path: P, backup_type: BackupType) -> Result<PathBuf> {
        let db_path = db_path.as_ref();
        
        if !db_path.exists() {
            return Err(anyhow::anyhow!("Database file does not exist: {}", db_path.display()));
        }

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_filename = format!("devices_backup_{}.db", timestamp);
        let backup_path = self.backup_dir.join(&backup_filename);

        info!("Creating backup: {} -> {}", db_path.display(), backup_path.display());

        // Copy the database file
        fs::copy(db_path, &backup_path)
            .with_context(|| format!("Failed to copy database to backup location"))?;

        // Get file size
        let metadata = fs::metadata(&backup_path)?;
        let file_size = metadata.len();

        // Create metadata file
        let backup_metadata = BackupMetadata {
            created_at: Utc::now(),
            database_path: db_path.to_string_lossy().to_string(),
            database_size: file_size,
            version: env!("CARGO_PKG_VERSION").to_string(),
            backup_type,
        };

        let metadata_filename = format!("devices_backup_{}.json", timestamp);
        let metadata_path = self.backup_dir.join(metadata_filename);
        
        let metadata_json = serde_json::to_string_pretty(&backup_metadata)?;
        fs::write(&metadata_path, metadata_json)
            .with_context(|| format!("Failed to write backup metadata"))?;

        info!("Backup created successfully: {} ({} bytes)", backup_path.display(), file_size);

        // Clean up old backups
        self.cleanup_old_backups()?;

        Ok(backup_path)
    }

    /// Restore a database from backup
    pub fn restore_backup<P: AsRef<Path>>(&self, backup_path: P, target_path: P) -> Result<()> {
        let backup_path = backup_path.as_ref();
        let target_path = target_path.as_ref();

        if !backup_path.exists() {
            return Err(anyhow::anyhow!("Backup file does not exist: {}", backup_path.display()));
        }

        info!("Restoring backup: {} -> {}", backup_path.display(), target_path.display());

        // Create target directory if it doesn't exist
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create target directory"))?;
        }

        // Create a backup of the current database before restoring
        if target_path.exists() {
            let current_backup_path = self.create_backup(target_path, BackupType::PreUpdate)?;
            info!("Created backup of current database: {}", current_backup_path.display());
        }

        // Copy backup to target location
        fs::copy(backup_path, target_path)
            .with_context(|| format!("Failed to restore backup"))?;

        info!("Database restored successfully from backup");

        Ok(())
    }

    /// List available backups
    pub fn list_backups(&self) -> Result<Vec<(PathBuf, BackupMetadata)>> {
        let mut backups = Vec::new();

        if !self.backup_dir.exists() {
            return Ok(backups);
        }

        for entry in fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if stem.starts_with("devices_backup_") {
                        match fs::read_to_string(&path) {
                            Ok(content) => {
                                match serde_json::from_str::<BackupMetadata>(&content) {
                                    Ok(metadata) => {
                                        let db_filename = stem.replace("devices_backup_", "devices_backup_") + ".db";
                                        let db_path = self.backup_dir.join(db_filename);
                                        if db_path.exists() {
                                            backups.push((db_path, metadata));
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse backup metadata {}: {}", path.display(), e);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to read backup metadata {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            }
        }

        // Sort by creation time (newest first)
        backups.sort_by(|a, b| b.1.created_at.cmp(&a.1.created_at));

        Ok(backups)
    }

    /// Clean up old backups, keeping only the most recent ones
    fn cleanup_old_backups(&self) -> Result<()> {
        let backups = self.list_backups()?;
        
        if backups.len() <= self.max_backups {
            return Ok(());
        }

        let to_remove = &backups[self.max_backups..];
        
        for (backup_path, metadata) in to_remove {
            info!("Removing old backup: {} (created: {})", 
                  backup_path.display(), 
                  metadata.created_at.format("%Y-%m-%d %H:%M:%S UTC"));

            // Remove database file
            if let Err(e) = fs::remove_file(backup_path) {
                error!("Failed to remove backup file {}: {}", backup_path.display(), e);
            }

            // Remove metadata file
            let metadata_path = backup_path.with_extension("json");
            if metadata_path.exists() {
                if let Err(e) = fs::remove_file(&metadata_path) {
                    error!("Failed to remove backup metadata {}: {}", metadata_path.display(), e);
                }
            }
        }

        Ok(())
    }

    /// Get backup directory path
    pub fn backup_dir(&self) -> &Path {
        &self.backup_dir
    }

    /// Get backup statistics
    pub fn get_backup_stats(&self) -> Result<BackupStats> {
        let backups = self.list_backups()?;
        let total_count = backups.len();
        let total_size: u64 = backups.iter().map(|(_, metadata)| metadata.database_size).sum();
        
        let oldest = backups.last().map(|(_, metadata)| metadata.created_at);
        let newest = backups.first().map(|(_, metadata)| metadata.created_at);

        Ok(BackupStats {
            total_count,
            total_size,
            oldest_backup: oldest,
            newest_backup: newest,
        })
    }
}

#[derive(Debug)]
pub struct BackupStats {
    pub total_count: usize,
    pub total_size: u64,
    pub oldest_backup: Option<DateTime<Utc>>,
    pub newest_backup: Option<DateTime<Utc>>,
}

impl BackupStats {
    pub fn total_size_mb(&self) -> f64 {
        self.total_size as f64 / (1024.0 * 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_backup_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let backup_manager = SqliteBackupManager::new(temp_dir.path().join("backups"), 5)?;

        // Create a test database file
        let db_path = temp_dir.path().join("test.db");
        let mut file = File::create(&db_path)?;
        file.write_all(b"test database content")?;

        // Create backup
        let backup_path = backup_manager.create_backup(&db_path, BackupType::Manual)?;

        assert!(backup_path.exists());
        assert!(backup_path.with_extension("json").exists());

        Ok(())
    }

    #[test]
    fn test_backup_listing() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let backup_manager = SqliteBackupManager::new(temp_dir.path().join("backups"), 5)?;

        // Create a test database file
        let db_path = temp_dir.path().join("test.db");
        let mut file = File::create(&db_path)?;
        file.write_all(b"test database content")?;

        // Create multiple backups
        backup_manager.create_backup(&db_path, BackupType::Manual)?;
        std::thread::sleep(std::time::Duration::from_millis(10)); // Ensure different timestamps
        backup_manager.create_backup(&db_path, BackupType::Scheduled)?;

        let backups = backup_manager.list_backups()?;
        assert_eq!(backups.len(), 2);

        Ok(())
    }
}
