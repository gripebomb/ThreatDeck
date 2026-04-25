use crate::types::Feed;
use std::time::{Duration, Instant};

pub struct FeedScheduler {
    entries: Vec<ScheduleEntry>,
}

struct ScheduleEntry {
    feed_id: i64,
    interval_secs: u64,
    next_run: Instant,
}

impl FeedScheduler {
    pub fn new(feeds: &[Feed]) -> Self {
        let now = Instant::now();
        let entries = feeds
            .iter()
            .filter(|f| f.enabled)
            .map(|f| ScheduleEntry {
                feed_id: f.id,
                interval_secs: f.interval_secs,
                next_run: f.last_fetch_at.map(|_| now).unwrap_or(now),
            })
            .collect();
        Self { entries }
    }

    pub fn tick(&mut self, now: Instant) -> Vec<i64> {
        let mut due = Vec::new();
        for entry in &mut self.entries {
            if now >= entry.next_run {
                entry.next_run = now + Duration::from_secs(entry.interval_secs);
                due.push(entry.feed_id);
            }
        }
        due
    }

    pub fn update_feed(&mut self, feed: &Feed) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.feed_id == feed.id) {
            entry.interval_secs = feed.interval_secs;
            if !feed.enabled {
                // Remove if disabled
                self.entries.retain(|e| e.feed_id != feed.id);
            }
        } else if feed.enabled {
            self.entries.push(ScheduleEntry {
                feed_id: feed.id,
                interval_secs: feed.interval_secs,
                next_run: Instant::now(),
            });
        }
    }

    pub fn add_feed(&mut self, feed: &Feed) {
        if feed.enabled {
            self.entries.push(ScheduleEntry {
                feed_id: feed.id,
                interval_secs: feed.interval_secs,
                next_run: Instant::now(),
            });
        }
    }

    pub fn remove_feed(&mut self, feed_id: i64) {
        self.entries.retain(|e| e.feed_id != feed_id);
    }
}
