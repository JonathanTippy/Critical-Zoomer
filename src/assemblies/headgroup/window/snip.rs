//! In-app viewport snip for faux-user / harness paths.
//!
//! Trigger: write a destination path into `$CZ_SNIPREQ` (default
//! `/tmp/cz_ctl.snip`). The headgroup consumes the request, writes an ASCII
//! PPM of the current sampler buffer, and deletes the request file.

use egui::Color32;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn snip_request_path() -> String {
    std::env::var("CZ_SNIPREQ").unwrap_or_else(|_| "/tmp/cz_ctl.snip".to_string())
}

/// If a snip request file is present, write `size`/`pixels` as PPM to the
/// destination named inside it and remove the request.
pub fn maybe_write_viewport_snip(size: (usize, usize), pixels: &[Color32]) -> bool {
    let req = snip_request_path();
    let Ok(dest) = std::fs::read_to_string(&req) else {
        return false;
    };
    let _ = std::fs::remove_file(&req);
    let dest = dest.trim();
    if dest.is_empty() {
        return false;
    }
    match write_ppm(Path::new(dest), size, pixels) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("viewport snip failed for {dest}: {e}");
            false
        }
    }
}

pub fn write_ppm(path: &Path, size: (usize, usize), pixels: &[Color32]) -> std::io::Result<()> {
    let (w, h) = size;
    assert_eq!(pixels.len(), w.saturating_mul(h), "snip buffer size mismatch");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut f = File::create(path)?;
    writeln!(f, "P3")?;
    writeln!(f, "{w} {h}")?;
    writeln!(f, "255")?;
    for row in 0..h {
        for col in 0..w {
            let c = pixels[row * w + col];
            write!(f, "{} {} {} ", c.r(), c.g(), c.b())?;
        }
        writeln!(f)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_ppm_roundtrips_header() {
        let dir = std::env::temp_dir().join("cz_snip_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("t.ppm");
        let pixels = vec![Color32::from_rgb(10, 20, 30); 4];
        write_ppm(&path, (2, 2), &pixels).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("P3\n2 2\n255\n"));
        assert!(text.contains("10 20 30"));
    }

    /// Thought-killed pins for PPM layout (`row * w + col`, dimensions, P3 header).
    #[test]
    fn mutant_kill_snip_ppm_layout() {
        let dir = std::env::temp_dir().join("cz_snip_mk");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("mk.ppm");
        // Distinct pixels so row-major indexing mutants fail.
        let pixels = vec![
            Color32::from_rgb(1, 0, 0),
            Color32::from_rgb(0, 2, 0),
            Color32::from_rgb(0, 0, 3),
            Color32::from_rgb(4, 5, 6),
            Color32::from_rgb(7, 8, 9),
            Color32::from_rgb(10, 11, 12),
        ];
        write_ppm(&path, (3, 2), &pixels).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("P3\n3 2\n255\n"));
        assert_ne!(text.starts_with("P3\n2 3\n255\n"), true); // w/h swap
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 5); // header 3 + 2 rows
        assert!(lines[3].contains("1 0 0"));
        assert!(lines[3].contains("0 2 0"));
        assert!(lines[3].contains("0 0 3"));
        assert!(lines[4].starts_with("4 5 6"));
        assert!(lines[4].contains("10 11 12"));
        // row*w+col vs row+w*col / row+w+col: second row first pixel is index 3.
        assert!(!lines[3].contains("4 5 6"));

        assert_eq!(snip_request_path().contains("snip") || std::env::var("CZ_SNIPREQ").is_ok(), true);

        // 1×1 layout + saturating size check identity.
        let path1 = dir.join("one.ppm");
        write_ppm(&path1, (1, 1), &[Color32::from_rgb(11, 22, 33)]).unwrap();
        let t1 = std::fs::read_to_string(&path1).unwrap();
        assert!(t1.starts_with("P3\n1 1\n255\n"));
        assert!(t1.contains("11 22 33"));
        assert!(!t1.contains("22 11 33")); // channel order R G B
        // Default request path mentions snip when env unset.
        let prev = std::env::var("CZ_SNIPREQ").ok();
        std::env::remove_var("CZ_SNIPREQ");
        assert!(snip_request_path().contains("snip"));
        match prev {
            Some(v) => std::env::set_var("CZ_SNIPREQ", v),
            None => {}
        }
    }
}
