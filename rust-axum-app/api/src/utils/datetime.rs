use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_epoch() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize
}
