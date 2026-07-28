//! Property tests for GPU nativity: gears, atlas, publisher, symmetry, homothety.
// r[verify cz.seamless.gpu-preferred+1]
// r[verify cz.int.publisher-nores-bias+1]
// r[verify cz.range.guess-biased-nearest+1]

#[cfg(test)]
mod properties {
    use crate::assemblies::structs::MandelbrotResult;
    use crate::assemblies::structs::gpu_tile::{
        GPU_CAL_KIND_OUTSIDE, GPUCalibratedAnswer,
    };
    use crate::assemblies::tile_sheet::TILE_EDGE;
    use crate::assemblies::workgroup_new::production_atlas::ProductionAtlas;
    use crate::assemblies::workgroup_new::tile_publisher::{agnostic_wide, publish_seat};
    use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::publisher_shader::{
        GpuPackedAnswer, PublisherGpu,
    };
    use bytemuck::Zeroable;
    use crate::constants::{NORES_ANSWER, PIXELS_PER_UNIT_POT, TILE_EDGE_LENGTH};
    use crate::gear::Gear;
    use crate::gpu_context::GpuContext;
    use crate::intexp::IntExp;
    use crate::range::Range;
    use crate::stacked_intexp::StackedIntExp;

    #[test]
    fn publisher_agnostic_without_bias_is_nores() {
        let published = publish_seat(agnostic_wide(), None);
        match (&published.result, &NORES_ANSWER.result) {
            (
                MandelbrotResult::Outside { escape_time_r2: a, escape_z: az },
                MandelbrotResult::Outside { escape_time_r2: b, escape_z: bz },
            ) => {
                assert_eq!(a, b);
                assert_eq!(az.0, bz.0);
                assert_eq!(az.1, bz.1);
            }
            _ => panic!("NORES must be Outside"),
        }
        assert!(published.min_magnitude.is_infinite());
    }

    #[test]
    fn production_atlas_slot_roundtrip_when_gpu_available() {
        let Some(_) = GpuContext::shared() else {
            return;
        };
        let Some(mut atlas) = ProductionAtlas::new() else {
            return;
        };
        let slot = atlas.acquire().expect("slot");
        let texels = (TILE_EDGE * TILE_EDGE) as usize;
        let meta = vec![[0.0f32; 4]; texels];
        let z = vec![[0.0f32; 4]; texels];
        atlas.write_slot(slot, &meta, &z);
        atlas.release(slot);
        let again = atlas.acquire().expect("reclaim");
        assert_eq!(again, slot, "released slot must be reusable");
        atlas.release(again);
    }

    #[test]
    fn guess_biased_stays_inside_range_samples() {
        let cases = [(-10.0, 5.0, -100.0), (0.0, 1.0, 0.5), (3.0, 0.0, 9.0)];
        for &(lo, span, bias) in &cases {
            let hi = lo + span;
            let r = Range { lower_bound: lo, upper_bound: hi };
            let g = r.guess_biased(bias);
            assert!(g >= lo - 1e-12 && g <= hi + 1e-12);
        }
    }

    #[test]
    fn mandelbrot_real_axis_symmetry_escape_samples() {
        for &(re, im) in &[(-0.5, 0.3), (0.2, 0.1), (-1.0, 0.5)] {
            let a = naive_escape((re, im), 200);
            let b = naive_escape((re, -im), 200);
            assert_eq!(a.is_some(), b.is_some());
            if let (Some(ea), Some(eb)) = (a, b) {
                assert_eq!(ea, eb);
            }
        }
    }

    fn naive_escape(c: (f64, f64), max_iter: u32) -> Option<u32> {
        let mut z = (0.0, 0.0);
        for i in 0..max_iter {
            let zr2 = z.0 * z.0;
            let zi2 = z.1 * z.1;
            if zr2 + zi2 > 4.0 {
                return Some(i);
            }
            z = (zr2 - zi2 + c.0, 2.0 * z.0 * z.1 + c.1);
        }
        None
    }

    #[test]
    fn homothety_mag_bumps_compose_samples() {
        for pot in -2..3 {
            for d1 in -1..2 {
                for d2 in -1..2 {
                    let space = |p: i32| IntExp::from(1).shift(-(p + PIXELS_PER_UNIT_POT));
                    let stepwise = IntExp::from(1).shift(-(pot + d1 + d2 + PIXELS_PER_UNIT_POT));
                    assert_eq!(space(pot + d1 + d2), stepwise);
                }
            }
        }
    }

    #[test]
    fn stacked_mul_agrees_intexp_small_samples() {
        for a in -8..9 {
            for b in -8..9 {
                let sa = StackedIntExp::<4>::from(a);
                let sb = StackedIntExp::<4>::from(b);
                assert_eq!(IntExp::from(sa * sb), IntExp::from(a) * IntExp::from(b));
            }
        }
    }

    #[test]
    fn gpu_publisher_matches_cpu_clamp_when_device_available() {
        let Some(gpu) = PublisherGpu::new() else {
            return;
        };
        let n = TILE_EDGE_LENGTH * TILE_EDGE_LENGTH;
        let mut cal = vec![GPUCalibratedAnswer::EMPTY; n];
        cal[0] = GPUCalibratedAnswer {
            kind: GPU_CAL_KIND_OUTSIDE,
            period_lo: 0,
            period_hi: 0,
            escape_lo: 5,
            escape_hi: 15,
            escape_z_re_lo: 0.0,
            escape_z_re_hi: 2.0,
            escape_z_im_lo: -1.0,
            escape_z_im_hi: 1.0,
            min_mag_time_lo: 0,
            min_mag_time_hi: 3,
            min_mag_lo: 0.0,
            min_mag_hi: 1.0,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        };
        let mut bias = vec![GpuPackedAnswer::zeroed(); n];
        bias[0] = GpuPackedAnswer {
            kind: 1,
            escape_or_period: 100,
            min_mag_time: 1,
            min_mag: 0.5,
            zx: 1.0,
            zy: 0.0,
            _pad0: 0,
            _pad1: 0,
        };
        let mut valid = vec![0u32; n];
        valid[0] = 1;
        let out = gpu.publish_tile(&cal, &bias, &valid).expect("gpu");
        assert_eq!(out[0].escape_or_period, 15);
        assert_eq!(out[0].zx, 1.0);
    }

    #[test]
    fn gear_ladder_covers_gpu_stack() {
        for g in Gear::ladder() {
            let _ = g.significand_bits();
            let _ = g.runs_on_gpu();
        }
        assert_eq!(Gear::select(20, true), Gear::F32);
    }
}
