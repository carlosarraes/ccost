// File watcher for monitoring JSONL files
use crate::watch::events::FileEvent;
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tokio::sync::mpsc as tokio_mpsc;

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    event_sender: tokio_mpsc::UnboundedSender<FileEvent>,
}

impl FileWatcher {
    pub fn new(
        projects_dir: PathBuf,
        event_sender: tokio_mpsc::UnboundedSender<FileEvent>,
    ) -> Result<(Self, mpsc::Receiver<notify::Result<Event>>)> {
        let (tx, file_receiver) = mpsc::channel();

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                if let Err(e) = tx.send(res) {
                    eprintln!("Failed to send file watch event: {}", e);
                }
            },
            notify::Config::default(),
        )
        .context("Failed to create file watcher")?;

        // Watch the projects directory recursively
        watcher
            .watch(&projects_dir, RecursiveMode::Recursive)
            .context("Failed to watch projects directory")?;

        let file_watcher = FileWatcher {
            _watcher: watcher,
            event_sender,
        };

        Ok((file_watcher, file_receiver))
    }

    pub async fn run_with_receiver(
        &self,
        file_receiver: mpsc::Receiver<notify::Result<Event>>,
    ) -> Result<()> {
        loop {
            // Use try_recv to avoid blocking and check for events periodically
            match file_receiver.try_recv() {
                Ok(Ok(event)) => {
                    self.handle_file_event(event).await?;
                }
                Ok(Err(e)) => {
                    let _ = self
                        .event_sender
                        .send(FileEvent::Error(format!("File watch error: {}", e)));
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No events, sleep briefly and continue
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(anyhow::anyhow!("File watcher disconnected"));
                }
            }
        }
    }

    async fn handle_file_event(&self, event: Event) -> Result<()> {
        match event.kind {
            EventKind::Create(_) => {
                for path in event.paths {
                    if self.is_jsonl_file(&path) {
                        let _ = self.event_sender.send(FileEvent::FileCreated(path));
                    }
                }
            }
            EventKind::Modify(_) => {
                for path in event.paths {
                    if self.is_jsonl_file(&path) {
                        let _ = self.event_sender.send(FileEvent::FileModified(path));
                    }
                }
            }
            _ => {
                // Ignore other events like remove, access, etc.
            }
        }
        Ok(())
    }

    fn is_jsonl_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("jsonl"))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_file_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, _rx) = tokio_mpsc::unbounded_channel();

        let result = FileWatcher::new(temp_dir.path().to_path_buf(), tx);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_jsonl_file_detection() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, mut rx) = tokio_mpsc::unbounded_channel();

        let (watcher, file_receiver) = FileWatcher::new(temp_dir.path().to_path_buf(), tx).unwrap();

        // Start watcher in background
        let watcher_handle = tokio::spawn(async move {
            let _ = watcher.run_with_receiver(file_receiver).await;
        });

        // Create a JSONL file
        let jsonl_path = temp_dir.path().join("test.jsonl");
        fs::write(&jsonl_path, "{}").unwrap();

        // Wait for file event with timeout
        let event = timeout(tokio::time::Duration::from_secs(2), rx.recv()).await;
        assert!(event.is_ok());

        match event.unwrap() {
            Some(FileEvent::FileCreated(path)) => {
                assert_eq!(path, jsonl_path);
            }
            Some(FileEvent::FileModified(path)) => {
                // Some filesystems report modify instead of create
                assert_eq!(path, jsonl_path);
            }
            other => panic!("Unexpected event: {:?}", other),
        }

        watcher_handle.abort();
    }

    #[test]
    fn test_is_jsonl_file() {
        let temp_dir = TempDir::new().unwrap();
        let (tx, _rx) = tokio_mpsc::unbounded_channel();
        let (watcher, _file_receiver) =
            FileWatcher::new(temp_dir.path().to_path_buf(), tx).unwrap();

        assert!(watcher.is_jsonl_file(Path::new("test.jsonl")));
        assert!(watcher.is_jsonl_file(Path::new("test.JSONL")));
        assert!(!watcher.is_jsonl_file(Path::new("test.json")));
        assert!(!watcher.is_jsonl_file(Path::new("test.txt")));
        assert!(!watcher.is_jsonl_file(Path::new("test")));
    }
}
