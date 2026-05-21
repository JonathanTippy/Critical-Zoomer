//! Cold-start pipeline timing (debug session 419d19).
use std::io::Write;
use std::sync::OnceLock;
use std::time::Instant;

const LOG_PATH: &str = "/home/jonathan/git/Critical-Zoomer/.cursor/debug-419d19.log";

static BOOT_T0: OnceLock<Instant> = OnceLock::new();

pub(crate) fn boot_t0() -> Instant {
    *BOOT_T0.get_or_init(Instant::now)
}

pub(crate) fn boot_ms() -> u128 {
    boot_t0().elapsed().as_millis()
}

/// One-shot milestone (logged once per `stage` string).
pub(crate) fn boot_once(stage: &str, detail: &str) {
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
    let line = format!(
        r#"{{"sessionId":"419d19","kind":"boot","stage":"{}","detail":{},"boot_ms":{}}}"#,
        stage,
        detail,
        boot_ms()
    );
    let _ = std::io::stderr().write_all(format!("[boot +{}ms] {} {}\n", boot_ms(), stage, detail).as_bytes());
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

pub(crate) fn boot_span(stage: &str, detail: &str, elapsed_ms: u128) {
    let line = format!(
        r#"{{"sessionId":"419d19","kind":"boot_span","stage":"{}","detail":{},"elapsed_ms":{},"boot_ms":{}}}"#,
        stage,
        detail,
        elapsed_ms,
        boot_ms()
    );
    let _ = std::io::stderr().write_all(
        format!(
            "[boot +{}ms] {} took {}ms {}\n",
            boot_ms(),
            stage,
            elapsed_ms,
            detail
        )
        .as_bytes(),
    );
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
