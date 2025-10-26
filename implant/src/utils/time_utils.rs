use chrono::{DateTime, Utc};

pub fn epoch_now() -> i64 {
    let dt = Utc::now();
    dt.timestamp()
}
