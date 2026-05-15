use crate::alert::AlertEngine;
use crate::db::Db;
use crate::feed::FeedManager;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum AutoFetchMessage {
    Completed {
        feeds_attempted: usize,
        feeds_succeeded: usize,
        alerts_created: usize,
        errors: Vec<String>,
    },
    Stopped,
}

const MAX_ERRORS_PER_CYCLE: usize = 100;

fn push_error(errors: &mut Vec<String>, msg: String) {
    if errors.len() < MAX_ERRORS_PER_CYCLE {
        errors.push(msg);
    } else if errors.len() == MAX_ERRORS_PER_CYCLE {
        errors.push("Too many errors, truncating...".to_string());
    }
}

#[derive(Debug)]
pub struct AutoFetcher {
    handle: Option<thread::JoinHandle<()>>,
    stop_tx: mpsc::Sender<()>,
}

impl AutoFetcher {
    pub fn spawn(
        db_path: PathBuf,
        interval_minutes: u32,
        tx: mpsc::Sender<AutoFetchMessage>,
    ) -> Self {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let interval = Duration::from_secs((interval_minutes.max(1) as u64) * 60);

        let handle = thread::Builder::new()
            .name("auto-fetcher".to_owned())
            .spawn(move || {
            let db = match Db::open(&db_path) {
                Ok(db) => db,
                Err(e) => {
                    let _ = tx.send(AutoFetchMessage::Completed {
                        feeds_attempted: 0,
                        feeds_succeeded: 0,
                        alerts_created: 0,
                        errors: vec![format!("Failed to open DB: {}", e)],
                    });
                    return;
                }
            };

            loop {
                match stop_rx.recv_timeout(interval) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        let _ = tx.send(AutoFetchMessage::Stopped);
                        return;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }

                let feeds = match db.list_feeds(None) {
                    Ok(feeds) => feeds.into_iter().filter(|f| f.enabled).collect::<Vec<_>>(),
                    Err(e) => {
                        let _ = tx.send(AutoFetchMessage::Completed {
                            feeds_attempted: 0,
                            feeds_succeeded: 0,
                            alerts_created: 0,
                            errors: vec![format!("Failed to list feeds: {}", e)],
                        });
                        continue;
                    }
                };

                let mut alerts_created = 0usize;
                let mut feeds_succeeded = 0usize;
                let mut errors = Vec::new();

                let keywords = match db.list_keywords(true) {
                    Ok(k) => k,
                    Err(e) => {
                        let _ = tx.send(AutoFetchMessage::Completed {
                            feeds_attempted: 0,
                            feeds_succeeded: 0,
                            alerts_created: 0,
                            errors: vec![format!("Failed to list keywords: {}", e)],
                        });
                        continue;
                    }
                };

                for feed in &feeds {
                    let template = match feed.api_template_id {
                        Some(id) => match db.get_template(id) {
                            Ok(template) => template,
                            Err(e) => {
                                push_error(&mut errors, format!("Feed '{}' template lookup failed: {}", feed.name, e));
                                None
                            }
                        },
                        None => None,
                    };

                    let outcome = FeedManager::run_fetch_attempt(feed, template);
                    match outcome.result {
                        Some(result) => {
                            let _ = db.record_feed_fetch_outcome(feed.id, &outcome.attempt, Some(result.content_hash.as_str()));
                            feeds_succeeded += 1;
                            match AlertEngine::process_feed_result(&db, feed, &result, &keywords,
                            ) {
                                Ok(alerts) => {
                                    alerts_created += alerts.len();
                                }
                                Err(e) => {
                                    push_error(&mut errors, format!("Feed '{}' alert processing failed: {}", feed.name, e));
                                }
                            }
                        }
                        None => {
                            let summary = outcome
                                .attempt
                                .diagnostic
                                .as_ref()
                                .map(|diagnostic| diagnostic.summary.as_str())
                                .unwrap_or("Fetch failed");
                            let _ = db.record_feed_fetch_outcome(feed.id, &outcome.attempt, None);
                            push_error(&mut errors, format!("Feed '{}' fetch failed: {}", feed.name, summary));
                        }
                    }
                }

                let _ = tx.send(AutoFetchMessage::Completed {
                    feeds_attempted: feeds.len(),
                    feeds_succeeded,
                    alerts_created,
                    errors,
                });
            }
        }).expect("auto-fetcher thread spawn failed");

        AutoFetcher { handle: Some(handle), stop_tx }
    }

    pub fn stop(mut self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AutoFetcher {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn push_error_caps_at_max() {
        let mut errors = Vec::new();
        for i in 0..MAX_ERRORS_PER_CYCLE + 5 {
            push_error(&mut errors,
                format!("error {}", i)
            );
        }
        assert_eq!(errors.len(), MAX_ERRORS_PER_CYCLE + 1);
        assert!(errors.last().unwrap().contains("truncating"));
    }

    #[test]
    fn auto_fetcher_stop_sends_stopped_message() {
        let db_path = std::env::temp_dir().join(format!(
            "threatdeck-autofetch-stop-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);

        let (tx, rx) = mpsc::channel();
        // Use a long interval so the thread doesn't do work before we stop it
        let fetcher = AutoFetcher::spawn(db_path.clone(), 9999, tx);
        fetcher.stop();

        // We may get a Completed message first (DB open failed or empty feeds),
        // followed by Stopped. Collect all messages.
        let mut got_stopped = false;
        while let Ok(msg) = rx.recv_timeout(Duration::from_secs(2)) {
            if matches!(msg, AutoFetchMessage::Stopped) {
                got_stopped = true;
                break;
            }
        }
        assert!(got_stopped, "Expected Stopped message after stop()");

        let _ = std::fs::remove_file(&db_path);
    }
}
