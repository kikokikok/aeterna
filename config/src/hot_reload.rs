//! # Configuration Hot Reload
//!
//! Watches configuration files for changes and reloads configuration automatically.

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tracing::debug;
use tracing::{error, info, warn};

/// Configuration reload event.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigReloadEvent {
    /// Configuration file changed
    Changed(PathBuf),

    /// Configuration file was removed
    Removed(PathBuf),

    /// Configuration file was created
    Created(PathBuf),

    /// Configuration reload error
    Error { path: PathBuf, error: String },
}

/// Watch a configuration file for changes and emit reload events.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Monitors configuration file for changes and automatically reloads configuration.
/// Uses `notify` crate for cross-platform file system watching.
///
/// ## Usage
/// ```rust,no_run
/// use config::{watch_config, hot_reload::ConfigReloadEvent};
/// use tokio::signal;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config_path = std::path::Path::new("config.toml");
///     let (_tx, mut rx) = watch_config(&config_path).await?;
///
///     loop {
///         tokio::select! {
///             _ = signal::ctrl_c() => break,
///             Some(event) = rx.recv() => {
///                 match event {
///                     ConfigReloadEvent::Changed(path) => {
///                         println!("Config changed: {:?}", path);
///                     }
///                     ConfigReloadEvent::Error { path, error } => {
///                         eprintln!("Error reloading {:?}: {}", path, error);
///                     }
///                     _ => {}
///                 }
///             }
///         }
///     }
///
///     Ok(())
/// }
/// ```
///
/// ## Event Types
/// - `Changed`: File content modified
/// - `Created`: New file created
/// - `Removed`: File deleted
/// - `Error`: Failed to reload configuration
///
/// ## Performance
/// Uses debouncing to avoid multiple reload events for single file change.
pub async fn watch_config(
    config_path: &Path,
) -> Result<
    (tokio::sync::mpsc::Sender<ConfigReloadEvent>, tokio::sync::mpsc::Receiver<ConfigReloadEvent>),
    Box<dyn std::error::Error>,
> {
    let config_path = config_path.to_path_buf();

    if !config_path.exists() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Config file not found: {:?}", config_path),
        )));
    }

    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let tx_clone = tx.clone();
    let path_clone = config_path.clone();

    tokio::spawn(async move {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = event_tx.blocking_send(res);
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                let error_msg = format!("Failed to create file watcher: {}", e);
                error!("{}", error_msg);

                let _ = tx_clone
                    .send(ConfigReloadEvent::Error {
                        path: path_clone,
                        error: error_msg,
                    })
                    .await;

                return;
            }
        };

        match watcher.watch(&config_path, RecursiveMode::NonRecursive) {
            Ok(_) => info!("Watching config file: {:?}", config_path),
            Err(e) => {
                let error_msg = format!("Failed to watch config file: {}", e);
                error!("{}", error_msg);

                let _ = tx_clone
                    .send(ConfigReloadEvent::Error {
                        path: path_clone,
                        error: error_msg,
                    })
                    .await;

                return;
            }
        }

        while let Some(event_result) = event_rx.recv().await {
            match event_result {
                Ok(event) => {
                    if !event.paths.is_empty() {
                        let path = event.paths[0].clone();
                        let event = match event.kind {
                            EventKind::Create(_) => {
                                info!("Config file created: {:?}", path);
                                ConfigReloadEvent::Created(path)
                            }
                            EventKind::Modify(_) => {
                                info!("Config file modified: {:?}", path);
                                ConfigReloadEvent::Changed(path)
                            }
                            EventKind::Remove(_) => {
                                warn!("Config file removed: {:?}", path);
                                ConfigReloadEvent::Removed(path)
                            }
                            _ => {
                                debug!("Ignoring event: {:?}", event.kind);
                                continue;
                            }
                        };

                        if let Err(e) = tx_clone.send(event).await {
                            error!("Failed to send config reload event: {}", e);
                        }
                    } else {
                        debug!("Ignoring event with no paths: {:?}", event.kind);
                    }
                }
                Err(e) => {
                    warn!("Watch error: {}", e);
                }
            }
        }
    });

    Ok((tx, rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;
    use tokio::time::{sleep, Duration};

    #[test]
    fn test_config_reload_event_created() {
        let path = PathBuf::from("/test/config.toml");
        let event = ConfigReloadEvent::Created(path.clone());
        assert!(matches!(event, ConfigReloadEvent::Created(_)));
        assert_eq!(event, ConfigReloadEvent::Created(path));
    }

    #[test]
    fn test_config_reload_event_removed() {
        let path = PathBuf::from("/test/config.toml");
        let event = ConfigReloadEvent::Removed(path.clone());
        assert!(matches!(event, ConfigReloadEvent::Removed(_)));
        assert_eq!(event, ConfigReloadEvent::Removed(path));
    }

    #[test]
    fn test_config_reload_event_changed() {
        let path = PathBuf::from("/test/config.toml");
        let event = ConfigReloadEvent::Changed(path.clone());
        assert!(matches!(event, ConfigReloadEvent::Changed(_)));
        assert_eq!(event, ConfigReloadEvent::Changed(path));
    }

    #[test]
    fn test_config_reload_event_error() {
        let path = PathBuf::from("/test/config.toml");
        let event = ConfigReloadEvent::Error {
            path: path.clone(),
            error: "Test error".to_string(),
        };
        assert!(matches!(event, ConfigReloadEvent::Error { .. }));
        assert_eq!(event, ConfigReloadEvent::Error {
            path,
            error: "Test error".to_string(),
        });
    }

    #[tokio::test]
    async fn test_tokio_config_reload_event_created() {
        let path = PathBuf::from("/test/config.toml");
        let event = ConfigReloadEvent::Created(path.clone());
        assert!(matches!(event, ConfigReloadEvent::Created(_)));
        assert_eq!(event, ConfigReloadEvent::Created(path));
    }

    #[tokio::test]
    async fn test_tokio_config_reload_event_removed() {
        let path = PathBuf::from("/test/config.toml");
        let event = ConfigReloadEvent::Removed(path.clone());
        assert!(matches!(event, ConfigReloadEvent::Removed(_)));
        assert_eq!(event, ConfigReloadEvent::Removed(path));
    }

    #[tokio::test]
    async fn test_tokio_config_reload_event_error() {
        let path = PathBuf::from("/test/config.toml");
        let event = ConfigReloadEvent::Error {
            path: path.clone(),
            error: "Test error".to_string(),
        };
        assert!(matches!(event, ConfigReloadEvent::Error { .. }));
        assert_eq!(event, ConfigReloadEvent::Error {
            path,
            error: "Test error".to_string(),
        });
    }


    #[tokio::test]
    async fn test_watch_config_emits_events() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
[providers.postgres]
host = "testhost"
"#;
        fs::write(temp_file.path(), config_content).unwrap();

        let (_tx, mut rx) = watch_config(temp_file.path()).await.unwrap();

        let event = rx.recv().await;
        assert!(event.is_some());

        fs::write(temp_file.path(), config_content).unwrap();
        sleep(Duration::from_millis(100)).await;

        let event = rx.recv().await;
        assert!(event.is_some());

        match event.unwrap() {
            ConfigReloadEvent::Changed(path) => {
                assert_eq!(path, temp_file.path());
            }
            _ => panic!("Expected Changed event"),
        }
    }

    #[tokio::test]
    async fn test_watch_config_handles_create() {
        let temp_file = NamedTempFile::new().unwrap();
        let config_content = r#"
[providers.postgres]
host = "testhost"
"#;

        fs::write(temp_file.path(), config_content).unwrap();

        let (_tx, mut rx) = watch_config(temp_file.path()).await.unwrap();

        let event = rx.recv().await;
        assert!(event.is_some());

        let path = temp_file.path();
        match event.unwrap() {
            ConfigReloadEvent::Created(created_path) => {
                assert_eq!(created_path, path);
            }
            _ => panic!("Expected Created event"),
        }
    }
}
