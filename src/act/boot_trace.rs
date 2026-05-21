//! Cold-start pipeline timing (debug session 419d19).
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

const LOG_PATH: &str = "/home/jonathan/git/Critical-Zoomer/.cursor/debug-419d19.log";

static BOOT_T0: OnceLock<Instant> = OnceLock::new();
static BOOT_ACTIVE: AtomicBool = AtomicBool::new(true);

pub(crate) fn boot_t0() -> Instant {
    *BOOT_T0.get_or_init(Instant::now)
}

pub(crate) fn boot_ms() -> u128 {
    boot_t0().elapsed().as_millis()
}

/// Stop all boot logging after the first fully computed frame (see colorer).
pub(crate) fn boot_complete() {
    if BOOT_ACTIVE
        .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    write_boot_line(
        "boot",
        "boot_trace_end",
        r#"{"reason":"first_full_frame"}"#,
        None,
    );
}

fn boot_enabled() -> bool {
    BOOT_ACTIVE.load(Ordering::Relaxed)
}

/// One-shot milestone (logged once per `stage` string).
pub(crate) fn boot_once(stage: &str, detail: &str) {
    if !boot_enabled() {
        return;
    }
    static SEEN: OnceLock<std::sync::Mutex<Vec<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    let mut g = match seen.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if g.iter().any(|s| s == stage) {
        return;
    }
    g.push(stage.to_string());
    drop(g);
    boot_event(stage, detail);
}

pub(crate) fn boot_event(stage: &str, detail: &str) {
    if !boot_enabled() {
        return;
    }
    write_boot_line("boot", stage, detail, None);
}

pub(crate) fn boot_span(stage: &str, detail: &str, elapsed_ms: u128) {
    if !boot_enabled() {
        return;
    }
    write_boot_line("boot_span", stage, detail, Some(elapsed_ms));
}

fn write_boot_line(kind: &str, stage: &str, detail: &str, elapsed_ms: Option<u128>) {
    let line = match elapsed_ms {
        Some(ms) => format!(
            r#"{{"sessionId":"419d19","kind":"{}","stage":"{}","detail":{},"elapsed_ms":{},"boot_ms":{}}}"#,
            kind,
            stage,
            detail,
            ms,
            boot_ms()
        ),
        None => format!(
            r#"{{"sessionId":"419d19","kind":"{}","stage":"{}","detail":{},"boot_ms":{}}}"#,
            kind, stage, detail, boot_ms()
        ),
    };
    let stderr_line = match elapsed_ms {
        Some(ms) => format!(
            "[boot +{}ms] {} took {}ms {}\n",
            boot_ms(),
            stage,
            ms,
            detail
        ),
        None => format!("[boot +{}ms] {} {}\n", boot_ms(), stage, detail),
    };
    let _ = std::io::stderr().write_all(stderr_line.as_bytes());
    let _ = std::io::stderr().flush();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
    {
        let _ = writeln!(f, "{}", line);
        let _ = f.flush();
    }
}
