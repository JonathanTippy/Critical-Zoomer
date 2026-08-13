//! Webcam gaze → window pixels, with a four-corner toast calibration.
//!
//! During each toast we average a tiny grayscale downscale of the camera.
//! Live frames are inverse-distance blends of those four snapshots. The
//! window corners themselves are the targets — no full-screen calibration dots.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;

pub const FEAT_W: usize = 24;
pub const FEAT_H: usize = 16;
pub const FEAT_LEN: usize = FEAT_W * FEAT_H;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GazeCorner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl GazeCorner {
    pub const ORDER: [GazeCorner; 4] = [
        GazeCorner::TopLeft,
        GazeCorner::TopRight,
        GazeCorner::BottomRight,
        GazeCorner::BottomLeft,
    ];

    pub fn toast(self) -> &'static str {
        match self {
            GazeCorner::TopLeft => "Look at the top-left corner, then click Yup, doing it",
            GazeCorner::TopRight => "Look at the top-right corner, then click Yup, doing it",
            GazeCorner::BottomRight => "Look at the bottom-right corner, then click Yup, doing it",
            GazeCorner::BottomLeft => "Look at the bottom-left corner, then click Yup, doing it",
        }
    }

    pub fn target_px(self, w: f32, h: f32) -> (f32, f32) {
        let m = 8.0;
        match self {
            GazeCorner::TopLeft => (m, m),
            GazeCorner::TopRight => ((w - 1.0).max(m), m),
            GazeCorner::BottomRight => ((w - 1.0).max(m), (h - 1.0).max(m)),
            GazeCorner::BottomLeft => (m, (h - 1.0).max(m)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GazeMap {
    pub corners: Vec<(Vec<f32>, (f32, f32))>,
}

impl GazeMap {
    pub fn interpolate(&self, live: &[f32]) -> Option<(f32, f32)> {
        interpolate_gaze(live, &self.corners)
    }
}

pub fn interpolate_gaze(live: &[f32], corners: &[(Vec<f32>, (f32, f32))]) -> Option<(f32, f32)> {
    if live.len() != FEAT_LEN || corners.len() < 4 {
        return None;
    }
    let mut wsum = 0.0_f32;
    let mut x = 0.0_f32;
    let mut y = 0.0_f32;
    for (feat, target) in corners {
        if feat.len() != FEAT_LEN {
            continue;
        }
        let d2 = l2_sq(live, feat).max(1e-8);
        let w = 1.0 / d2;
        wsum += w;
        x += w * target.0;
        y += w * target.1;
    }
    if wsum <= 0.0 {
        return None;
    }
    Some((x / wsum, y / wsum))
}

fn l2_sq(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let d = x - y;
            d * d
        })
        .sum()
}

pub fn mean_feature(samples: &[Vec<f32>]) -> Option<Vec<f32>> {
    if samples.is_empty() || samples.iter().any(|s| s.len() != FEAT_LEN) {
        return None;
    }
    let n = samples.len() as f32;
    let mut acc = vec![0.0_f32; FEAT_LEN];
    for s in samples {
        for (a, v) in acc.iter_mut().zip(s.iter()) {
            *a += *v;
        }
    }
    for a in &mut acc {
        *a /= n;
    }
    Some(acc)
}

pub fn downsample_gray(width: u32, height: u32, luma: &[u8]) -> Option<Vec<f32>> {
    if width == 0 || height == 0 || luma.len() < (width as usize) * (height as usize) {
        return None;
    }
    let mut out = vec![0.0_f32; FEAT_LEN];
    let ww = width as usize;
    let hh = height as usize;
    for fy in 0..FEAT_H {
        for fx in 0..FEAT_W {
            let x0 = fx * ww / FEAT_W;
            let x1 = ((fx + 1) * ww / FEAT_W).max(x0 + 1).min(ww);
            let y0 = fy * hh / FEAT_H;
            let y1 = ((fy + 1) * hh / FEAT_H).max(y0 + 1).min(hh);
            let mut sum = 0u32;
            let mut n = 0u32;
            for y in y0..y1 {
                let row = y * ww;
                for x in x0..x1 {
                    sum += luma[row + x] as u32;
                    n += 1;
                }
            }
            out[fy * FEAT_W + fx] = if n == 0 { 0.0 } else { sum as f32 / n as f32 };
        }
    }
    Some(out)
}

pub fn yuyv_to_luma(width: u32, height: u32, yuyv: &[u8]) -> Option<Vec<u8>> {
    let n = (width as usize) * (height as usize);
    if yuyv.len() < n * 2 {
        return None;
    }
    let mut luma = vec![0u8; n];
    for i in 0..n {
        luma[i] = yuyv[i * 2];
    }
    Some(luma)
}

#[derive(Clone, Debug)]
pub enum GazePhase {
    Idle,
    Calibrating { corner: usize },
    Ready(GazeMap),
    Failed(&'static str),
}

#[derive(Clone)]
pub struct GazeSession {
    pub phase: GazePhase,
    collected: Vec<(Vec<f32>, (f32, f32))>,
    cam: GazeCamera,
    enabled: bool,
}

impl Default for GazeSession {
    fn default() -> Self {
        Self::new()
    }
}

impl GazeSession {
    pub fn new() -> Self {
        Self {
            phase: GazePhase::Idle,
            collected: Vec::new(),
            cam: GazeCamera::default(),
            enabled: false,
        }
    }

    pub fn set_enabled(&mut self, on: bool) {
        if on == self.enabled {
            return;
        }
        self.enabled = on;
        if on {
            if matches!(self.phase, GazePhase::Ready(_)) {
                self.cam.start();
            }
        } else {
            self.cam.stop();
            self.phase = GazePhase::Idle;
            self.collected.clear();
        }
    }

    pub fn begin_calibrate(&mut self) {
        self.enabled = true;
        self.cam.start();
        self.collected.clear();
        self.phase = GazePhase::Calibrating { corner: 0 };
    }

    /// Snapshot this pose. Call only when the user confirms they are looking.
    pub fn confirm_pose(&mut self, sampling: (f32, f32)) {
        let GazePhase::Calibrating { corner } = &self.phase else {
            return;
        };
        let c = *corner;
        let Some(feat) = self.cam.latest() else {
            self.phase = GazePhase::Failed("no camera — gaze off");
            return;
        };
        let target = GazeCorner::ORDER[c].target_px(sampling.0, sampling.1);
        self.collected.push((feat, target));
        let next = c + 1;
        if next >= GazeCorner::ORDER.len() {
            if self.collected.len() < 4 {
                self.phase = GazePhase::Failed("calibration incomplete");
                self.collected.clear();
                return;
            }
            self.phase = GazePhase::Ready(GazeMap {
                corners: self.collected.clone(),
            });
        } else {
            self.phase = GazePhase::Calibrating { corner: next };
        }
    }

    #[allow(dead_code)]
    pub fn hud_short(&self) -> &'static str {
        match &self.phase {
            GazePhase::Idle => "off",
            GazePhase::Calibrating { corner, .. } => match GazeCorner::ORDER.get(*corner) {
                Some(GazeCorner::TopLeft) => "cal TL",
                Some(GazeCorner::TopRight) => "cal TR",
                Some(GazeCorner::BottomRight) => "cal BR",
                Some(GazeCorner::BottomLeft) => "cal BL",
                None => "cal",
            },
            GazePhase::Ready(_) => "ok",
            GazePhase::Failed(_) => "fail",
        }
    }

    pub fn toast_text(&self) -> Option<&'static str> {
        match &self.phase {
            GazePhase::Calibrating { corner, .. } => GazeCorner::ORDER.get(*corner).map(|c| c.toast()),
            GazePhase::Failed(s) => Some(*s),
            _ => None,
        }
    }

    pub fn active_corner(&self) -> Option<GazeCorner> {
        match &self.phase {
            GazePhase::Calibrating { corner, .. } => GazeCorner::ORDER.get(*corner).copied(),
            _ => None,
        }
    }

    /// Advance calibration or map a live camera feature to sampling pixels.
    pub fn tick(&mut self, sampling: (f32, f32)) -> Option<(i32, i32)> {
        if !self.enabled {
            return None;
        }
        if matches!(&self.phase, GazePhase::Calibrating { .. }) {
            return None;
        }
        let feat = self.cam.latest();
        match &self.phase {
            GazePhase::Ready(map) => feat.and_then(|f| map.interpolate(&f)).map(|(x, y)| {
                let x = x.clamp(0.0, (sampling.0 - 1.0).max(0.0)) as i32;
                let y = y.clamp(0.0, (sampling.1 - 1.0).max(0.0)) as i32;
                (x, y)
            }),
            _ => None,
        }
    }

    #[cfg(test)]
    fn inject_feature(&self, feat: Vec<f32>) {
        if let Ok(mut g) = self.cam.latest.lock() {
            *g = Some(feat);
        }
    }
}

#[derive(Clone, Default)]
struct GazeCamera {
    latest: Arc<Mutex<Option<Vec<f32>>>>,
    run: Arc<AtomicBool>,
    join: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
}

impl GazeCamera {
    fn start(&mut self) {
        if cfg!(test) {
            return;
        }
        if self.run.load(Ordering::Relaxed) {
            return;
        }
        self.run.store(true, Ordering::Relaxed);
        let latest = self.latest.clone();
        let run = self.run.clone();
        let handle = thread::Builder::new()
            .name("cz-gaze-cam".into())
            .spawn(move || camera_loop(latest, run));
        if let Ok(h) = handle {
            if let Ok(mut g) = self.join.lock() {
                *g = Some(h);
            }
        }
    }

    fn stop(&mut self) {
        self.run.store(false, Ordering::Relaxed);
        if let Ok(mut g) = self.join.lock() {
            if let Some(h) = g.take() {
                let _ = h.join();
            }
        }
        if let Ok(mut latest) = self.latest.lock() {
            *latest = None;
        }
    }

    fn latest(&self) -> Option<Vec<f32>> {
        self.latest.lock().ok().and_then(|g| g.clone())
    }
}

impl Drop for GazeCamera {
    fn drop(&mut self) {
        if Arc::strong_count(&self.run) == 1 {
            self.stop();
        }
    }
}

fn camera_loop(latest: Arc<Mutex<Option<Vec<f32>>>>, run: Arc<AtomicBool>) {
    #[cfg(target_os = "linux")]
    {
        if let Err(e) = camera_loop_v4l(&latest, &run) {
            eprintln!("gaze camera: {e}");
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (latest, run);
    }
}

#[cfg(target_os = "linux")]
fn camera_loop_v4l(
    latest: &Arc<Mutex<Option<Vec<f32>>>>,
    run: &Arc<AtomicBool>,
) -> Result<(), String> {
    use v4l::buffer::Type;
    use v4l::io::traits::CaptureStream;
    use v4l::prelude::*;
    use v4l::video::Capture;

    let mut dev = Device::new(0).map_err(|e| format!("open /dev/video0: {e}"))?;
    let fmt = dev.format().map_err(|e| format!("format: {e}"))?;
    let w = fmt.width;
    let h = fmt.height;
    let mut stream =
        MmapStream::with_buffers(&mut dev, Type::VideoCapture, 4).map_err(|e| format!("mmap: {e}"))?;
    stream.set_timeout(Duration::from_millis(100));
    while run.load(Ordering::Relaxed) {
        let Ok((buf, _meta)) = stream.next() else {
            continue;
        };
        let luma = yuyv_to_luma(w, h, buf).unwrap_or_else(|| buf.to_vec());
        if let Some(feat) = downsample_gray(w, h, &luma) {
            if let Ok(mut g) = latest.lock() {
                *g = Some(feat);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feat_const(v: f32) -> Vec<f32> {
        vec![v; FEAT_LEN]
    }

    fn feat_mark(i: usize) -> Vec<f32> {
        let mut f = vec![0.0; FEAT_LEN];
        f[i] = 1.0;
        f
    }

    #[test]
    fn interpolate_exact_corner_matches_target() {
        let corners = vec![
            (feat_mark(0), (8.0, 8.0)),
            (feat_mark(1), (100.0, 8.0)),
            (feat_mark(2), (100.0, 50.0)),
            (feat_mark(3), (8.0, 50.0)),
        ];
        let p = interpolate_gaze(&feat_mark(0), &corners).unwrap();
        assert!((p.0 - 8.0).abs() < 1.0 && (p.1 - 8.0).abs() < 1.0);
        let p = interpolate_gaze(&feat_mark(2), &corners).unwrap();
        assert!((p.0 - 100.0).abs() < 1.0 && (p.1 - 50.0).abs() < 1.0);
    }

    #[test]
    fn interpolate_needs_four_corners() {
        let corners = vec![(feat_const(1.0), (0.0, 0.0)); 3];
        assert!(interpolate_gaze(&feat_const(1.0), &corners).is_none());
    }

    #[test]
    fn downsample_preserves_constant_field() {
        let w = 48u32;
        let h = 32u32;
        let luma = vec![90u8; (w * h) as usize];
        let f = downsample_gray(w, h, &luma).unwrap();
        assert_eq!(f.len(), FEAT_LEN);
        assert!(f.iter().all(|x| (*x - 90.0).abs() < 0.01));
    }

    #[test]
    fn yuyv_takes_luma_even_bytes() {
        let w = 2u32;
        let h = 1u32;
        let yuyv = [10u8, 99, 20, 88];
        let luma = yuyv_to_luma(w, h, &yuyv).unwrap();
        assert_eq!(luma, vec![10, 20]);
    }

    #[test]
    fn mean_feature_averages() {
        let a = feat_const(0.0);
        let mut b = feat_const(2.0);
        b[0] = 4.0;
        let m = mean_feature(&[a, b]).unwrap();
        assert!((m[0] - 2.0).abs() < 1e-5);
        assert!((m[1] - 1.0).abs() < 1e-5);
    }

    #[test]
    #[test]
    fn confirm_pose_waits_until_clicked_then_finishes() {
        let mut s = GazeSession::new();
        s.begin_calibrate();
        assert!(matches!(s.phase, GazePhase::Calibrating { corner: 0 }));
        s.tick((200.0, 100.0));
        assert!(matches!(s.phase, GazePhase::Calibrating { corner: 0 }));
        for i in 0..4 {
            s.inject_feature(feat_mark(i));
            s.confirm_pose((200.0, 100.0));
        }
        assert!(matches!(s.phase, GazePhase::Ready(_)));
    }

    #[test]
    fn confirm_without_frame_fails_open() {
        let mut s = GazeSession::new();
        s.begin_calibrate();
        s.confirm_pose((200.0, 100.0));
        assert!(matches!(s.phase, GazePhase::Failed(_)));
    }
}
