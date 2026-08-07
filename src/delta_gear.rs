//! Compute gears for perturbation delta recurrence: f64, scaled-f64, FloatExp.
//!
//! Legal transitions: F64 → ScaledF64 → FloatExp. No silent underflow.

use crate::floatexp::{ComplexFloatExp, FloatExp};

// r[impl cz.depth.compute-gear+1]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ComputeGear {
    #[default]
    F64,
    ScaledF64,
    FloatExp,
    Mixed,
}

impl ComputeGear {
    pub fn hud_label(self) -> &'static str {
        match self {
            ComputeGear::F64 => "F64",
            ComputeGear::ScaledF64 => "S-F64",
            ComputeGear::FloatExp => "FE",
            ComputeGear::Mixed => "MIXED",
        }
    }
}

/// Floor below which f64 delta components must promote (not flush to zero).
const F64_UNDERFLOW_FLOOR: f64 = 1e-300;
/// Ceiling above which scaled-f64 inner values must rescale or promote.
const F64_OVERFLOW_CEIL: f64 = 1e300;

#[inline(always)]
fn cscale(a: (f64, f64), s: f64) -> (f64, f64) {
    (a.0 * s, a.1 * s)
}

#[inline(always)]
fn cmul(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

#[inline(always)]
fn cadd(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 + b.0, a.1 + b.1)
}

#[inline(always)]
fn cnorm_sq(a: (f64, f64)) -> f64 {
    a.0 * a.0 + a.1 * a.1
}

#[inline(always)]
fn fe_to_f64_pair(z: ComplexFloatExp) -> Option<(f64, f64)> {
    let re = z.re.to_f64();
    let im = z.im.to_f64();
    if re.is_finite() && im.is_finite() {
        Some((re, im))
    } else {
        None
    }
}

#[inline(always)]
fn fe_to_f64_or_zero(z: FloatExp) -> f64 {
    let v = z.to_f64();
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// Pick the view's default gear from generator admission (no user setting).
pub fn view_gear_from_generators<T: crate::assemblies::workgroup::c_generator::Mandelbrotable>(
    relative_ok: bool,
) -> ComputeGear {
    if relative_ok {
        // Shallow / moderate views: f64 relative grid is admitted.
        if T::max_value().to_f64() > 1e200 {
            ComputeGear::F64
        } else {
            ComputeGear::FloatExp
        }
    } else {
        ComputeGear::FloatExp
    }
}

/// Whether a complex pair is safely representable as hardware f64 deltas.
pub fn f64_delta_admitted(re: f64, im: f64) -> bool {
    let m = re.abs().max(im.abs());
    m == 0.0 || (m >= F64_UNDERFLOW_FLOOR && m <= F64_OVERFLOW_CEIL)
}

/// Promote FloatExp complex delta to the strongest gear that still admits it.
pub fn gear_for_delta(dc: ComplexFloatExp, dz: ComplexFloatExp) -> ComputeGear {
    let dc_f = fe_to_f64_pair(dc);
    let dz_f = fe_to_f64_pair(dz);
    match (dc_f, dz_f) {
        (Some(dc), Some(dz)) if f64_delta_admitted(dc.0, dc.1) && f64_delta_admitted(dz.0, dz.1) => {
            ComputeGear::F64
        }
        _ => {
            // Scaled-f64 when inner f64 can hold scaled w after choosing scale from dz.
            if let Some(dz_f) = dz_f {
                let scale = scale_for_pair(dz_f);
                if scale.mantissa != 0.0 {
                    return ComputeGear::ScaledF64;
                }
            }
            ComputeGear::FloatExp
        }
    }
}

fn scale_for_pair(v: (f64, f64)) -> FloatExp {
    let m = v.0.abs().max(v.1.abs());
    if m == 0.0 {
        return FloatExp::ONE;
    }
    FloatExp::from(m)
}

/// Scaled-f64 scale from current mathematical dz.
pub fn scaled_scale_from_dz(dz: ComplexFloatExp) -> FloatExp {
    let re = dz.re.abs();
    let im = dz.im.abs();
    let m = if re > im { re } else { im };
    if m == FloatExp::ZERO {
        FloatExp::ONE
    } else {
        m
    }
}

/// One scaled-f64 step: w' = 2·Z·w + S·w² + d; returns (new_w, new_d, maybe new_scale).
pub fn scaled_f64_step(
    z_ref: (f64, f64),
    w: (f64, f64),
    d: (f64, f64),
    scale: FloatExp,
    dc_fe: ComplexFloatExp,
    is_zero_ref: bool,
) -> (
    (f64, f64),
    (f64, f64),
    FloatExp,
    ComputeGear,
) {
    let s = scale.to_f64();
    if !s.is_finite() || s == 0.0 {
        return (w, d, scale, ComputeGear::FloatExp);
    }
    let two_zw = if is_zero_ref {
        (0.0, 0.0)
    } else {
        cscale(cmul(z_ref, w), 2.0)
    };
    let w2 = cmul(w, w);
    let sw2 = (w2.0 * s, w2.1 * s);
    let w_next = cadd(cadd(two_zw, sw2), d);
    let z = if is_zero_ref {
        w_next
    } else {
        cadd(z_ref, (w_next.0 * s, w_next.1 * s))
    };
    let d_next = cadd(cscale(cmul(z, d), 2.0), (1.0, 0.0));
    let mag = w_next.0.abs().max(w_next.1.abs());
    if mag > F64_OVERFLOW_CEIL || !mag.is_finite() {
        return (w_next, d_next, scale, ComputeGear::FloatExp);
    }
  if mag < F64_UNDERFLOW_FLOOR && mag > 0.0 {
        return (w_next, d_next, scale, ComputeGear::FloatExp);
    }
    let _ = dc_fe;
    (w_next, d_next, scale, ComputeGear::ScaledF64)
}

/// One f64-hardware step. Returns promoted gear if f64 cannot continue.
pub fn f64_step(
    z_ref: (f64, f64),
    dz: (f64, f64),
    dc: (f64, f64),
    dd: (f64, f64),
    is_zero_ref: bool,
) -> ((f64, f64), (f64, f64), ComputeGear) {
    let z = cadd(z_ref, dz);
    let two_zdz = if is_zero_ref {
        (0.0, 0.0)
    } else {
        cscale(cmul(z_ref, dz), 2.0)
    };
    let dz2 = cmul(dz, dz);
    let dz_next = cadd(cadd(two_zdz, dz2), dc);
    let dd_next = cadd(cscale(cmul(z, dd), 2.0), (1.0, 0.0));
    if !f64_delta_admitted(dz_next.0, dz_next.1) || !f64_delta_admitted(dc.0, dc.1) {
        return (dz_next, dd_next, ComputeGear::ScaledF64);
    }
    (dz_next, dd_next, ComputeGear::F64)
}

/// Aggregate per-seat gears for HUD (honest mixed display).
pub fn aggregate_seat_gears(gears: &[ComputeGear]) -> ComputeGear {
    let mut saw_f64 = false;
    let mut saw_scaled = false;
    let mut saw_fe = false;
    for g in gears {
        match g {
            ComputeGear::F64 => saw_f64 = true,
            ComputeGear::ScaledF64 => saw_scaled = true,
            ComputeGear::FloatExp => saw_fe = true,
            ComputeGear::Mixed => return ComputeGear::Mixed,
        }
    }
    let count = saw_f64 as u8 + saw_scaled as u8 + saw_fe as u8;
    if count > 1 {
        ComputeGear::Mixed
    } else if saw_fe {
        ComputeGear::FloatExp
    } else if saw_scaled {
        ComputeGear::ScaledF64
    } else {
        ComputeGear::F64
    }
}

/// Narrow reference iterate to f64 when safe; None forces seat promotion.
pub fn narrow_z_ref(z: ComplexFloatExp) -> Option<(f64, f64)> {
    let re = z.re.to_f64();
    let im = z.im.to_f64();
    if re.is_finite() && im.is_finite() {
        Some((re, im))
    } else {
        None
    }
}

pub fn floatexp_from_f64_pair(v: (f64, f64)) -> ComplexFloatExp {
    ComplexFloatExp::new(FloatExp::from(v.0), FloatExp::from(v.1))
}

pub fn f64_from_fe(z: ComplexFloatExp) -> (f64, f64) {
    (fe_to_f64_or_zero(z.re), fe_to_f64_or_zero(z.im))
}
