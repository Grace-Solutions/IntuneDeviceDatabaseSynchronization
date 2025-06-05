use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::env;
use anyhow::{Result, Context};
use tokio::fs;

/// Get the directory where the current executable is located
pub fn get_executable_dir() -> Result<PathBuf> {
    let exe_path = env::current_exe()
        .context("Failed to get current executable path")?;
    
    let exe_dir = exe_path.parent()
        .context("Failed to get executable directory")?
        .to_path_buf();
    
    Ok(exe_dir)
}

/// Normalize a path to use the correct path separators for the current OS
pub fn normalize_path_separators(path: &str) -> String {
    // Replace both forward slashes and backslashes with the OS-specific separator
    path.replace('/', &MAIN_SEPARATOR.to_string())
        .replace('\\', &MAIN_SEPARATOR.to_string())
}

/// Resolve a path that can be either absolute or relative
/// If relative, it will be resolved relative to the executable directory
/// If absolute, it will be used as-is but still normalized
pub fn resolve_path(path: &str) -> Result<PathBuf> {
    let normalized_path = normalize_path_separators(path);
    let path_buf = PathBuf::from(&normalized_path);

    if path_buf.is_absolute() {
        // Absolute path - use as-is but normalized
        Ok(path_buf)
    } else {
        // Relative path - resolve relative to executable directory
        let exe_dir = get_executable_dir()?;
        Ok(exe_dir.join(path_buf))
    }
}

/// Ensure a directory exists, creating it if necessary
pub async fn ensure_directory_exists<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    
    if !path.exists() {
        fs::create_dir_all(path).await
            .with_context(|| format!("Failed to create directory: {}", path.display()))?;
        log::info!("Created directory: {}", path.display());
    }
    
    Ok(())
}

/// Ensure the parent directory of a file exists
pub async fn ensure_parent_directory_exists<P: AsRef<Path>>(file_path: P) -> Result<()> {
    let file_path = file_path.as_ref();
    
    if let Some(parent) = file_path.parent() {
        ensure_directory_exists(parent).await?;
    }
    
    Ok(())
}

/// Get the default config file path (next to executable)
pub fn get_default_config_path() -> Result<PathBuf> {
    let exe_dir = get_executable_dir()?;
    Ok(exe_dir.join("config.json"))
}

/// Resolve and sanitize a database path from configuration
pub fn resolve_database_path(configured_path: &str) -> Result<PathBuf> {
    resolve_path(configured_path)
}

/// Resolve and sanitize a logs directory path from configuration
pub fn resolve_logs_path(configured_path: &str) -> Result<PathBuf> {
    resolve_path(configured_path)
}

/// Resolve and sanitize a backup directory path from configuration
pub fn resolve_backup_path(configured_path: &str) -> Result<PathBuf> {
    resolve_path(configured_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_separators() {
        let path = "./data/test.db";
        let normalized = normalize_path_separators(path);
        
        #[cfg(windows)]
        assert_eq!(normalized, ".\\data\\test.db");
        
        #[cfg(unix)]
        assert_eq!(normalized, "./data/test.db");
    }

    #[test]
    fn test_normalize_mixed_separators() {
        let path = "./data\\subdir/test.db";
        let normalized = normalize_path_separators(path);
        
        #[cfg(windows)]
        assert_eq!(normalized, ".\\data\\subdir\\test.db");
        
        #[cfg(unix)]
        assert_eq!(normalized, "./data/subdir/test.db");
    }
}
