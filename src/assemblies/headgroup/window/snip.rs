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
}
