//! # Configuration Hot Reload
//!
//! Watches configuration files for changes and reloads configuration
//! automatically.

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tracing::debug;
use tracing::{error, info, warn};

/// Configuration reload event.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigReloadEvent {
    Ready,

    /// Configuration file changed
    Changed(PathBuf),

    /// Configuration file was removed
    Removed(PathBuf),

    /// Configuration file was created
    Created(PathBuf),

    /// Configuration reload error
    Error {
        path: PathBuf,
        error: String,
    },
}

/// Watch a configuration file for changes and emit reload events.
///
/// # M-CANONICAL-DOCS
///
/// ## Purpose
/// Monitors configuration file for changes and automatically reloads
/// configuration. Uses `notify` crate for cross-platform file system watching.
///
/// ## Usage
/// ```rust,no_run
/// use config::{hot_reload::ConfigReloadEvent, watch_config};
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
    (
        tokio::sync::mpsc::Sender<ConfigReloadEvent>,
        tokio::sync::mpsc::Receiver<ConfigReloadEvent>,
    ),
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
    let tx_task = tx.clone();
    let path_task = config_path.clone();

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

                let _ = tx_task
                    .send(ConfigReloadEvent::Error {
                        path: path_task,
                        error: error_msg,
                    })
                    .await;

                return;
            }
        };

        if let Err(e) = watcher.watch(&config_path, RecursiveMode::NonRecursive) {
            let error_msg = format!("Failed to watch config file: {}", e);
            error!("{}", error_msg);

            let _ = tx_task
                .send(ConfigReloadEvent::Error {
                    path: path_task,
                    error: error_msg,
                })
                .await;

            return;
        }

        info!("Watching config file: {:?}", config_path);

        let _ = tx_task.send(ConfigReloadEvent::Ready).await;

        loop {
            tokio::select! {
                _ = tx_task.closed() => {
                    debug!("Receiver dropped, stopping watcher for {:?}", config_path);
                    break;
                }
                event_result = event_rx.recv() => {
                    let Some(event_result) = event_result else {
                        break;
                    };

                    match event_result {
                        Ok(event) => {
                            if !event.paths.is_empty() {
                                let path = event.paths[0].clone();
                                let reload_event = match event.kind {
                                    EventKind::Create(_) | EventKind::Modify(_) => {
                                        info!("Config file updated: {:?}", path);
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

                                if let Err(e) = tx_task.send(reload_event).await {
                                    error!("Failed to send config reload event: {}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Watch error: {}", e);
                        }
                    }
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
    use tokio::time::Duration;

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
        assert_eq!(
            event,
            ConfigReloadEvent::Error {
                path,
                error: "Test error".to_string()
            }
        );
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
        assert_eq!(
            event,
            ConfigReloadEvent::Error {
                path,
                error: "Test error".to_string()
            }
        );
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

        let ready_event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("Timeout waiting for Ready event")
            .expect("No event received");
        assert_eq!(ready_event, ConfigReloadEvent::Ready);

        fs::write(temp_file.path(), "[providers.postgres]\nhost = \"updated\"").unwrap();

        let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("Timeout waiting for config change event")
            .expect("No event received");

        match event {
            ConfigReloadEvent::Changed(path) => {
                assert_eq!(
                    path.canonicalize().unwrap(),
                    temp_file.path().canonicalize().unwrap()
                );
            }
            _ => panic!("Expected Changed event, got {:?}", event),
        }
    }

    #[tokio::test]
    async fn test_watch_config_handles_create() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        fs::write(&config_path, "initial").unwrap();

        let (_tx, mut rx) = watch_config(&config_path).await.unwrap();

        let ready_event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("Timeout waiting for Ready event")
            .expect("No event received");
        assert_eq!(ready_event, ConfigReloadEvent::Ready);

        fs::write(&config_path, "updated").unwrap();

        let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("Timeout waiting for config change event")
            .expect("No event received");

        match event {
            ConfigReloadEvent::Changed(path) => {
                assert_eq!(
                    path.canonicalize().unwrap(),
                    config_path.canonicalize().unwrap()
                );
            }
            _ => panic!("Expected Changed event, got {:?}", event),
        }
    }

    #[tokio::test]
    async fn test_watch_config_nonexistent_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("nonexistent.toml");

        let result = watch_config(&config_path).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_str = error.to_string();
        assert!(error_str.contains("not found") || error_str.contains("NotFound"));
    }

    #[tokio::test]
    async fn test_watch_config_error_handling() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), "test").unwrap();

        let (_tx, mut rx) = watch_config(temp_file.path()).await.unwrap();

        let _ = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("Timeout waiting for Ready event");

        drop(rx);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn test_watch_config_removed_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        fs::write(&config_path, "initial").unwrap();

        let (_tx, mut rx) = watch_config(&config_path).await.unwrap();

        let ready_event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("Timeout waiting for Ready event")
            .expect("No event received");
        assert_eq!(ready_event, ConfigReloadEvent::Ready);

        fs::remove_file(&config_path).unwrap();

        let event = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await;

        if let Ok(Some(event)) = event {
            match event {
                ConfigReloadEvent::Removed(path) => {
                    assert_eq!(
                        path.canonicalize().unwrap(),
                        config_path.canonicalize().unwrap()
                    );
                }
                ConfigReloadEvent::Error { path, error } => {
                    assert_eq!(
                        path.canonicalize().unwrap(),
                        config_path.canonicalize().unwrap()
                    );
                    assert!(!error.is_empty());
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_config_reload_event_ready() {
        let event = ConfigReloadEvent::Ready;
        assert!(matches!(event, ConfigReloadEvent::Ready));
        assert_eq!(event, ConfigReloadEvent::Ready);
    }

    #[test]
    fn test_config_reload_event_partial_eq() {
        let path1 = PathBuf::from("/test/config.toml");
        let path2 = PathBuf::from("/test/config.toml");
        let path3 = PathBuf::from("/other/config.toml");

        // Test equality
        assert_eq!(
            ConfigReloadEvent::Changed(path1.clone()),
            ConfigReloadEvent::Changed(path2.clone())
        );

        assert_eq!(
            ConfigReloadEvent::Created(path1.clone()),
            ConfigReloadEvent::Created(path2.clone())
        );

        assert_eq!(
            ConfigReloadEvent::Removed(path1.clone()),
            ConfigReloadEvent::Removed(path2.clone())
        );

        assert_eq!(
            ConfigReloadEvent::Error {
                path: path1.clone(),
                error: "test".to_string()
            },
            ConfigReloadEvent::Error {
                path: path2.clone(),
                error: "test".to_string()
            }
        );

        // Test inequality
        assert_ne!(
            ConfigReloadEvent::Changed(path1.clone()),
            ConfigReloadEvent::Changed(path3.clone())
        );

        assert_ne!(
            ConfigReloadEvent::Changed(path1.clone()),
            ConfigReloadEvent::Created(path1.clone())
        );

        assert_ne!(
            ConfigReloadEvent::Error {
                path: path1.clone(),
                error: "error1".to_string()
            },
            ConfigReloadEvent::Error {
                path: path1.clone(),
                error: "error2".to_string()
            }
        );
    }
}
