use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppLogger {
    inner: Arc<Mutex<Vec<(DateTime<Utc>, String)>>>,
}

impl AppLogger {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn log<S: Into<String>>(&self, msg: S) {
        self.inner.lock().unwrap().push((Utc::now(), msg.into()));
    }

    pub fn get(&self) -> Arc<Mutex<Vec<(DateTime<Utc>, String)>>> {
        Arc::clone(&self.inner)
    }

    pub fn filter_by_text(&self, keyword: &str) -> Vec<(DateTime<Utc>, String)> {
        self.inner.lock().unwrap()
            .iter()
            .filter(|(_, line)| line.contains(keyword))
            .cloned()
            .collect()
    }

    pub fn filter_by_date_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<(DateTime<Utc>, String)> {
        self.inner.lock().unwrap()
            .iter()
            .filter(|(ts, _)| *ts >= start && *ts <= end)
            .cloned()
            .collect()
    }
}
