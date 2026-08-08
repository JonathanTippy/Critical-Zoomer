//! Session debug NDJSON logger (temporary; remove after OBO shading fix verified).
#![allow(dead_code)]

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_PATH: &str = "/home/jonathan/git/Critical-Zoomer/.cursor/debug-c33634.log";
const SESSION: &str = "c33634";

static SEQ: AtomicU64 = AtomicU64::new(0);
static SAMPLE: AtomicU64 = AtomicU64::new(0);

/// Append one NDJSON line. `data_json` must be a JSON object body, e.g. `{"k":1}`.
pub fn log(hypothesis_id: &str, location: &str, message: &str, data_json: &str) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let id = SEQ.fetch_add(1, Ordering::Relaxed);
    let line = format!(
        "{{\"sessionId\":\"{SESSION}\",\"id\":\"log_{id}\",\"timestamp\":{ts},\"hypothesisId\":\"{hypothesis_id}\",\"location\":\"{location}\",\"message\":\"{message}\",\"data\":{data_json}}}\n"
    );
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn should_sample(every: u64) -> bool {
    let n = SAMPLE.fetch_add(1, Ordering::Relaxed);
    every > 0 && n % every == 0
}
