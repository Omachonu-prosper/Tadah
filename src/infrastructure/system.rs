use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::ports::{Clock, IdGenerator};

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

pub struct UuidGenerator;

impl IdGenerator for UuidGenerator {
    fn next(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}
