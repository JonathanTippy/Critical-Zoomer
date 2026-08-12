//! Compile-time ban: pipeline vocabulary must not use dirty/clean tokens.
//! Token-based (not substring) so `OracleAnswer` is allowed.

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    for root in ["src", "benches"] {
        let dir = manifest.join(root);
        if dir.is_dir() {
            println!("cargo:rerun-if-changed={}", dir.display());
            walk_rs(&dir);
        }
    }
}

fn walk_rs(dir: &Path) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs(&path);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        scan_file(&path);
    }
}

fn scan_file(path: &Path) {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    for (idx, line) in text.lines().enumerate() {
        for token in tokens_in_line(line) {
            if is_banned(&token) {
                panic!(
                    "banned pipeline token `{token}` in {}:{} — use latest/resident/drain/in-budget wording instead",
                    path.display(),
                    idx + 1
                );
            }
        }
    }
}

fn is_banned(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "dirty" | "clean" | "cleanly" | "cleaner" | "cleanest" | "cleaning"
    )
}

/// Split on non-alphanumeric, then split CamelCase / digit boundaries into segments.
fn tokens_in_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in line.chars() {
        if ch.is_ascii_alphanumeric() {
            cur.push(ch);
        } else if !cur.is_empty() {
            push_camel_segments(&cur, &mut out);
            cur.clear();
        }
    }
    if !cur.is_empty() {
        push_camel_segments(&cur, &mut out);
    }
    out
}

fn push_camel_segments(ident: &str, out: &mut Vec<String>) {
    let chars: Vec<char> = ident.chars().collect();
    if chars.is_empty() {
        return;
    }
    let mut start = 0usize;
    for i in 1..chars.len() {
        let prev = chars[i - 1];
        let c = chars[i];
        let boundary = (prev.is_ascii_lowercase() && c.is_ascii_uppercase())
            || (prev.is_ascii_digit() && c.is_ascii_alphabetic())
            || (prev.is_ascii_alphabetic() && c.is_ascii_digit())
            || (prev.is_ascii_uppercase()
                && c.is_ascii_uppercase()
                && i + 1 < chars.len()
                && chars[i + 1].is_ascii_lowercase());
        if boundary {
            out.push(chars[start..i].iter().collect());
            start = i;
        }
    }
    out.push(chars[start..].iter().collect());
}
