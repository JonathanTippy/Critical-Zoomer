// read delivery.md for project context
//! Off-screen / too-small classification for the r=2 circle proxy.
//! Design: docs/design/headgroup.md — red arrow guidance.
// r[impl cz.display.offscreen-r2-circle+1]
// r[impl cz.display.offscreen-arrows+1]

use crate::assemblies::structs::PointStencil;
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::intexp::IntExp;

/// Viewport vs the disk |c| ≤ 2 (design: "r=2 circle").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum R2ScreenRelation {
    OffScreen,
    MostlyOffScreen,
    OnScreen,
    MostlyTooSmall,
    TooSmall,
}

#[derive(Clone, Copy, Debug)]
pub struct ViewportComplexRect {
    pub real_min: f64,
    pub real_max: f64,
    pub imag_min: f64,
    pub imag_max: f64,
    pub seats: usize,
    pub rows: usize,
    pub spacing: f64,
}

impl ViewportComplexRect {
    pub fn from_stencil(stencil: &PointStencil) -> Self {
        let ppu = PIXELS_PER_UNIT_POT + stencil.homothety.2;
        let spacing = 2f64.powi(-ppu);
        let ul_real = stencil.homothety.0.clone().to_f64();
        let ul_imag = stencil.homothety.1.clone().to_f64();
        let seats = stencil.resolution.0;
        let rows = stencil.resolution.1;
        // +seat → +real; +row → −imag (dyadic)
        let real_min = ul_real;
        let real_max = ul_real + (seats as f64) * spacing;
        let imag_max = ul_imag;
        let imag_min = ul_imag - (rows as f64) * spacing;
        Self {
            real_min,
            real_max,
            imag_min,
            imag_max,
            seats,
            rows,
            spacing,
        }
    }

    /// Diameter of |c|=2 circle in screen pixels.
    pub fn r2_diameter_px(&self) -> f64 {
        4.0 / self.spacing
    }

    fn disk_intersects_aabb(cx: f64, cy: f64, r: f64, rect: (f64, f64, f64, f64)) -> bool {
        let (xmin, xmax, ymin, ymax) = rect;
        let closest_x = cx.clamp(xmin, xmax);
        let closest_y = cy.clamp(ymin, ymax);
        let dx = cx - closest_x;
        let dy = cy - closest_y;
        dx * dx + dy * dy <= r * r
    }

    fn aabb(&self) -> (f64, f64, f64, f64) {
        (self.real_min, self.real_max, self.imag_min, self.imag_max)
    }

    /// Inner 90% AABB (10% margin each side) — "within 10% of fully off".
    fn inner_90_aabb(&self) -> (f64, f64, f64, f64) {
        let rw = self.real_max - self.real_min;
        let ih = self.imag_max - self.imag_min;
        let mx = rw * 0.05;
        let my = ih * 0.05;
        (
            self.real_min + mx,
            self.real_max - mx,
            self.imag_min + my,
            self.imag_max - my,
        )
    }

    pub fn classify_r2(&self) -> R2ScreenRelation {
        let diam = self.r2_diameter_px();
        let min_side = self.seats.min(self.rows) as f64;
        if diam <= 1.0 {
            return R2ScreenRelation::TooSmall;
        }
        if diam < 0.1 * min_side {
            return R2ScreenRelation::MostlyTooSmall;
        }
        let full = self.aabb();
        if !Self::disk_intersects_aabb(0.0, 0.0, 2.0, full) {
            return R2ScreenRelation::OffScreen;
        }
        if !Self::disk_intersects_aabb(0.0, 0.0, 2.0, self.inner_90_aabb()) {
            return R2ScreenRelation::MostlyOffScreen;
        }
        R2ScreenRelation::OnScreen
    }

    // r[impl cz.display.offscreen-arrows+1]
    pub fn needs_red_arrows(&self) -> bool {
        matches!(
            self.classify_r2(),
            R2ScreenRelation::OffScreen
                | R2ScreenRelation::MostlyOffScreen
                | R2ScreenRelation::TooSmall
                | R2ScreenRelation::MostlyTooSmall
        )
    }
}

/// Build a stencil with UL at (real,imag) and given mag POT / resolution.
pub fn test_stencil(real: i32, imag: i32, mag: i32, seats: usize, rows: usize) -> PointStencil {
    PointStencil {
        homothety: (IntExp::from(real), IntExp::from(imag), mag),
        resolution: (seats, rows),
        serial_number: 0,
        focus: None,
        hover: None,
            mag_velocity: 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cz.display.offscreen-r2-circle+1]
    // r[verify cz.display.offscreen-arrows+1]
    #[test]
    fn homeish_viewport_sees_r2_disk() {
        // Rough home: UL near (-2,-2) style region covering the set.
        let s = test_stencil(-2, 2, -2, 800, 480);
        let v = ViewportComplexRect::from_stencil(&s);
        assert_eq!(v.classify_r2(), R2ScreenRelation::OnScreen);
        assert!(!v.needs_red_arrows());
    }

    // r[verify cz.display.offscreen-r2-circle+1]
    // r[verify cz.display.offscreen-arrows+1]
    #[test]
    fn far_pan_marks_off_screen() {
        let s = test_stencil(100, 100, 0, 200, 200);
        let v = ViewportComplexRect::from_stencil(&s);
        assert_eq!(v.classify_r2(), R2ScreenRelation::OffScreen);
        assert!(v.needs_red_arrows());
    }

    // r[verify cz.display.offscreen-r2-circle+1]
    // r[verify cz.display.offscreen-arrows+1]
    #[test]
    fn deep_zoom_out_marks_too_small() {
        // Very negative mag → huge spacing → tiny pixel diameter for r=2.
        let s = test_stencil(-1000, 1000, -20, 800, 480);
        let v = ViewportComplexRect::from_stencil(&s);
        assert!(
            matches!(
                v.classify_r2(),
                R2ScreenRelation::TooSmall | R2ScreenRelation::MostlyTooSmall
            ),
            "got {:?}",
            v.classify_r2()
        );
        assert!(v.needs_red_arrows());
    }

    #[test]
    fn diameter_px_scales_with_magnification() {
        let a = ViewportComplexRect::from_stencil(&test_stencil(-2, 2, 0, 100, 100));
        let b = ViewportComplexRect::from_stencil(&test_stencil(-2, 2, 1, 100, 100));
        assert!((b.r2_diameter_px() / a.r2_diameter_px() - 2.0).abs() < 1e-9);
    }

    // r[verify cz.display.offscreen-r2-circle+1]
    // r[verify cz.display.offscreen-arrows+1]
    #[test]
    fn mostly_off_when_disk_only_in_outer_margin() {
        // Tangent-ish viewport: r=2 disk clips the outer AABB at x=2 but misses
        // the inner 90% margin → MostlyOffScreen.
        let s = test_stencil(2, 0, 5, 100, 100);
        let v = ViewportComplexRect::from_stencil(&s);
        assert_eq!(
            v.classify_r2(),
            R2ScreenRelation::MostlyOffScreen,
            "got {:?} diam={}",
            v.classify_r2(),
            v.r2_diameter_px()
        );
        assert!(v.needs_red_arrows());
    }

    // r[verify cz.display.offscreen-r2-circle+1]
    // r[verify cz.display.offscreen-arrows+1]
    #[test]
    fn mostly_too_small_when_diameter_under_ten_percent_of_min_side() {
        // Diameter in (1, 0.1*min_side): TooSmall is ≤1px; MostlyTooSmall is next.
        // mag -12 → spacing 2^(-(9-12))=2^3=8 → diam=4/8=0.5px → TooSmall (≤1).
        // Need diam > 1 and < 0.1*min_side. mag -10 → spacing=2^1=2 → diam=2px.
        // min_side=480, 0.1*480=48 → 2 < 48 → MostlyTooSmall.
        let s = test_stencil(-2, 2, -10, 800, 480);
        let v = ViewportComplexRect::from_stencil(&s);
        assert_eq!(
            v.classify_r2(),
            R2ScreenRelation::MostlyTooSmall,
            "diam={} got {:?}",
            v.r2_diameter_px(),
            v.classify_r2()
        );
        assert!(v.needs_red_arrows());
    }
}
