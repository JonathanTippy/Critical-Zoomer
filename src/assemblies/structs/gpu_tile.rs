use crate::assemblies::structs::*;
use crate::assemblies::workgroup::structs::*;
use crate::constants::*;
use crate::range::Range;
use crate::utils::ObjectivePosAndZoom;
use bytemuck::{Pod, Zeroable};

#[derive(Copy, Clone, Debug)]
pub struct GPUAnswer {
    pub result: MandelbrotResult
    , pub min_magnitude_time: u64
    , pub min_magnitude: f64
}

impl From<Answer> for GPUAnswer {
    fn from(answer: Answer) -> Self {
        GPUAnswer {
            result: answer.result
            , min_magnitude_time: answer.min_magnitude_time
            , min_magnitude: answer.min_magnitude
        }
    }
}

impl From<GPUAnswer> for Answer {
    fn from(answer: GPUAnswer) -> Self {
        Answer {
            result: answer.result
            , min_magnitude_time: answer.min_magnitude_time
            , min_magnitude: answer.min_magnitude
            , escape_time_angle: 0
            , min_magnitude_angle: 0
        }
    }
}

/// GPU-resident calibrated answer: ranges survive until the publisher collapses
/// them (gpu_uploader.md / tile_publisher.md). Packed for storage buffers.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GPUCalibratedAnswer {
    // 0 = Agnostic, 1 = Outside, 2 = Inside, 3 = empty/missing
    pub kind: u32
    , pub period_lo: u32
    , pub period_hi: u32
    , pub escape_lo: u32
    , pub escape_hi: u32
    , pub escape_z_re_lo: f32
    , pub escape_z_re_hi: f32
    , pub escape_z_im_lo: f32
    , pub escape_z_im_hi: f32
    , pub min_mag_time_lo: u32
    , pub min_mag_time_hi: u32
    , pub min_mag_lo: f32
    , pub min_mag_hi: f32
    , pub _pad0: u32
    , pub _pad1: u32
    , pub _pad2: u32
}

pub const GPU_CAL_KIND_EMPTY: u32 = 3;
pub const GPU_CAL_KIND_AGNOSTIC: u32 = 0;
pub const GPU_CAL_KIND_OUTSIDE: u32 = 1;
pub const GPU_CAL_KIND_INSIDE: u32 = 2;

impl GPUCalibratedAnswer {
    pub const EMPTY: Self = Self {
        kind: GPU_CAL_KIND_EMPTY
        , period_lo: 0
        , period_hi: 0
        , escape_lo: 0
        , escape_hi: 0
        , escape_z_re_lo: 0.0
        , escape_z_re_hi: 0.0
        , escape_z_im_lo: 0.0
        , escape_z_im_hi: 0.0
        , min_mag_time_lo: 0
        , min_mag_time_hi: 0
        , min_mag_lo: 0.0
        , min_mag_hi: 0.0
        , _pad0: 0
        , _pad1: 0
        , _pad2: 0
    };

    pub fn from_calibrated(answer: CalibratedAnswer) -> Self {
        let min_mag_time_lo = answer.min_magnitude_time.lower_bound.min(u32::MAX as u64) as u32;
        let min_mag_time_hi = answer.min_magnitude_time.upper_bound.min(u32::MAX as u64) as u32;
        let min_mag_lo = answer.min_magnitude.lower_bound as f32;
        let min_mag_hi = answer.min_magnitude.upper_bound as f32;
        match answer.result {
            CalibratedMandelbrotResult::Agnostic {
                period
                , escape_time_r2
                , escape_z
            } => Self {
                kind: GPU_CAL_KIND_AGNOSTIC
                , period_lo: period.lower_bound.min(u32::MAX as u64) as u32
                , period_hi: period.upper_bound.min(u32::MAX as u64) as u32
                , escape_lo: escape_time_r2.lower_bound.min(u32::MAX as u64) as u32
                , escape_hi: escape_time_r2.upper_bound.min(u32::MAX as u64) as u32
                , escape_z_re_lo: escape_z.0.lower_bound
                , escape_z_re_hi: escape_z.0.upper_bound
                , escape_z_im_lo: escape_z.1.lower_bound
                , escape_z_im_hi: escape_z.1.upper_bound
                , min_mag_time_lo
                , min_mag_time_hi
                , min_mag_lo
                , min_mag_hi
                , _pad0: 0
                , _pad1: 0
                , _pad2: 0
            }
            , CalibratedMandelbrotResult::Outside {
                escape_time_r2
                , escape_z
            } => Self {
                kind: GPU_CAL_KIND_OUTSIDE
                , period_lo: 0
                , period_hi: 0
                , escape_lo: escape_time_r2.lower_bound.min(u32::MAX as u64) as u32
                , escape_hi: escape_time_r2.upper_bound.min(u32::MAX as u64) as u32
                , escape_z_re_lo: escape_z.0.lower_bound
                , escape_z_re_hi: escape_z.0.upper_bound
                , escape_z_im_lo: escape_z.1.lower_bound
                , escape_z_im_hi: escape_z.1.upper_bound
                , min_mag_time_lo
                , min_mag_time_hi
                , min_mag_lo
                , min_mag_hi
                , _pad0: 0
                , _pad1: 0
                , _pad2: 0
            }
            , CalibratedMandelbrotResult::Inside { period } => Self {
                kind: GPU_CAL_KIND_INSIDE
                , period_lo: period.lower_bound.min(u32::MAX as u64) as u32
                , period_hi: period.upper_bound.min(u32::MAX as u64) as u32
                , escape_lo: 0
                , escape_hi: 0
                , escape_z_re_lo: 0.0
                , escape_z_re_hi: 0.0
                , escape_z_im_lo: 0.0
                , escape_z_im_hi: 0.0
                , min_mag_time_lo
                , min_mag_time_hi
                , min_mag_lo
                , min_mag_hi
                , _pad0: 0
                , _pad1: 0
                , _pad2: 0
            }
        }
    }

    pub fn to_calibrated(self) -> Option<CalibratedAnswer> {
        if self.kind == GPU_CAL_KIND_EMPTY {
            return None;
        }
        let min_magnitude_time = Range {
            lower_bound: self.min_mag_time_lo as u64
            , upper_bound: self.min_mag_time_hi as u64
        };
        let min_magnitude = Range {
            lower_bound: self.min_mag_lo as f64
            , upper_bound: self.min_mag_hi as f64
        };
        let highlights = CalibratedHighlights {
            in_filament: Range { lower_bound: false, upper_bound: false }
            , out_filament: Range { lower_bound: false, upper_bound: false }
            , small_time_edge: Range { lower_bound: false, upper_bound: false }
            , node: Range { lower_bound: false, upper_bound: false }
        };
        let result = match self.kind {
            GPU_CAL_KIND_AGNOSTIC => CalibratedMandelbrotResult::Agnostic {
                period: Range {
                    lower_bound: self.period_lo as u64
                    , upper_bound: self.period_hi as u64
                }
                , escape_time_r2: Range {
                    lower_bound: self.escape_lo as u64
                    , upper_bound: self.escape_hi as u64
                }
                , escape_z: (
                    Range {
                        lower_bound: self.escape_z_re_lo
                        , upper_bound: self.escape_z_re_hi
                    }
                    , Range {
                        lower_bound: self.escape_z_im_lo
                        , upper_bound: self.escape_z_im_hi
                    }
                )
            }
            , GPU_CAL_KIND_OUTSIDE => CalibratedMandelbrotResult::Outside {
                escape_time_r2: Range {
                    lower_bound: self.escape_lo as u64
                    , upper_bound: self.escape_hi as u64
                }
                , escape_z: (
                    Range {
                        lower_bound: self.escape_z_re_lo
                        , upper_bound: self.escape_z_re_hi
                    }
                    , Range {
                        lower_bound: self.escape_z_im_lo
                        , upper_bound: self.escape_z_im_hi
                    }
                )
            }
            , GPU_CAL_KIND_INSIDE => CalibratedMandelbrotResult::Inside {
                period: Range {
                    lower_bound: self.period_lo as u64
                    , upper_bound: self.period_hi as u64
                }
            }
            , _ => return None
        };
        Some(CalibratedAnswer {
            result
            , min_magnitude_time
            , min_magnitude
            , highlights
            , escape_time_angle: 0
            , min_magnitude_angle: 0
        })
    }
}

/// A finished tile as it travels between actors: metadata plus a slot in the
/// workgroup's production atlas. The answers themselves stay on the GPU; only
/// this handle crosses a channel (architecture.md: gpu-native answers).
#[derive(Clone, Debug)]
pub struct GpuTileHandle {
    pub origin_seat: (usize, usize)
    , pub magnification_pot: i32
    , pub screen_res: (usize, usize)
    , pub location: ObjectivePosAndZoom
    // Slot in the workgroup production atlas. Present when the tile was written
    // there; absent only on the CPU-only fallback that still packs bytes for
    // the headgroup's old upload path.
    , pub production_slot: Option<u32>
    // How many seats carry an answer. Used for "prefer fuller tile" without
    // reading the atlas back.
    , pub filled_seats: u32
    // Whether the production slot holds calibrated ranges (pre-publisher) or
    // collapsed answers (post-publisher / CPU fallback).
    , pub calibrated: bool
    // CPU-side answers, kept only when there is no production atlas to hand
    // off from. Live GPU path leaves this empty.
    , pub cpu_fallback: Option<Box<GPUTile>>
    // CPU-side calibrated answers when the publisher has not yet collapsed them
    // and there is no production atlas.
    , pub cpu_calibrated: Option<Box<[Option<GPUCalibratedAnswer>; TILE_SEAT_COUNT]>>
}

impl GpuTileHandle {
    pub fn from_gpu_tile(tile: GPUTile, production_slot: Option<u32>) -> Self {
        // r[impl cz.pub.gpu-native-work+1]
        let filled_seats = tile.data.iter().filter(|c| c.is_some()).count() as u32;
        let keep_cpu = production_slot.is_none();
        GpuTileHandle {
            origin_seat: tile.origin_seat
            , magnification_pot: tile.magnification_pot
            , screen_res: tile.screen_res
            , location: tile.location.clone()
            , production_slot
            , filled_seats
            , calibrated: false
            , cpu_fallback: if keep_cpu { Some(Box::new(tile)) } else { None }
            , cpu_calibrated: None
        }
    }

    pub fn absolute_key(&self) -> (i32, i32, i32) {
        (
            self.location.zoom_pot
            , self.origin_seat.0 as i32
            , self.origin_seat.1 as i32
        )
    }
}

#[derive(Clone, Debug)]
pub struct GPUTile {
    pub origin_seat: (usize, usize)
    , pub magnification_pot: i32
    , pub screen_res: (usize, usize)
    , pub location: ObjectivePosAndZoom
    // Boxed so a 64x64 tile never has to live on the stack.
    , pub data: Box<[Option<GPUAnswer>; TILE_SEAT_COUNT]>
}

impl GPUTile {
    pub fn from_answer_tile(
        tile: &Tile<Answer>
        , screen_res: (usize, usize)
        , location: ObjectivePosAndZoom
    ) -> Self {
        let mut data = vec![None; TILE_SEAT_COUNT];
        for i in 0..TILE_SEAT_COUNT {
            data[i] = tile.data[i].map(GPUAnswer::from);
        }
        let data: Box<[Option<GPUAnswer>; TILE_SEAT_COUNT]> = data
            .into_boxed_slice()
            .try_into()
            .unwrap_or_else(|_| panic!("tile seat count mismatch"));
        GPUTile {
            origin_seat: tile.origin_seat
            , magnification_pot: tile.magnification_pot
            , screen_res
            , location
            , data
        }
    }

    pub fn from_calibrated_tile(
        tile: &Tile<CalibratedAnswer>
        , screen_res: (usize, usize)
        , location: ObjectivePosAndZoom
        , proximate: Option<&Tile<Answer>>
    ) -> Self {
        let published = crate::assemblies::workgroup::tile_publisher::publish_tile(
            tile
            , proximate
        );
        Self::from_answer_tile(&published, screen_res, location)
    }

    pub fn get(&self, local_seat: (usize, usize)) -> Option<GPUAnswer> {
        self.data[Tile::<()>::in_tile_index(local_seat)]
    }

    pub fn set(&mut self, local_seat: (usize, usize), value: GPUAnswer) {
        self.data[Tile::<()>::in_tile_index(local_seat)] = Some(value);
    }

    pub fn screen_seat(&self, local_seat: (usize, usize)) -> (usize, usize) {
        (
            self.origin_seat.0 + local_seat.0
            , self.origin_seat.1 + local_seat.1
        )
    }

    pub fn empty(
        origin_seat: (usize, usize)
        , magnification_pot: i32
        , screen_res: (usize, usize)
        , location: ObjectivePosAndZoom
    ) -> Self {
        let data: Box<[Option<GPUAnswer>; TILE_SEAT_COUNT]> = vec![None; TILE_SEAT_COUNT]
            .into_boxed_slice()
            .try_into()
            .unwrap_or_else(|_| panic!("tile seat count mismatch"));
        GPUTile {
            origin_seat
            , magnification_pot
            , screen_res
            , location
            , data
        }
    }
}

#[cfg(test)]
mod gpu_tile_upload_tests {
    use super::*;
    use crate::constants::NORES_ANSWER;
    use crate::intexp::IntExp;

    fn loc() -> ObjectivePosAndZoom {
        ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        }
    }

    // r[verify cz.display.nores-when-no-proximate+1]
    // r[verify cz.tenacious.nores-not-flat-black+1]
    #[test]
    fn from_answer_tile_preserves_nores() {
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), NORES_ANSWER);
        let gpu = GPUTile::from_answer_tile(&tile, (64, 64), loc());
        let a = gpu.get((0, 0)).expect("seat");
        match a.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => assert_eq!(escape_time_r2, 1),
            MandelbrotResult::Inside { .. } => panic!("NORES must stay Outside"),
        }
        assert!(a.min_magnitude.is_infinite());
    }

    #[test]
    fn from_answer_tile_roundtrips_answer_fields() {
        let answer = Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: 12,
                escape_z: (3.0, 4.0),
            },
            min_magnitude_time: 2,
            min_magnitude: 0.5,
            escape_time_angle: 0,
            min_magnitude_angle: 0
};
        let mut tile = Tile::new((64, 0), 3);
        tile.set((1, 2), answer);
        let gpu = GPUTile::from_answer_tile(&tile, (128, 128), loc());
        assert_eq!(gpu.origin_seat, (64, 0));
        assert_eq!(gpu.magnification_pot, 3);
        let back: Answer = gpu.get((1, 2)).unwrap().into();
        match back.result {
            MandelbrotResult::Outside {
                escape_time_r2,
                escape_z,
            } => {
                assert_eq!(escape_time_r2, 12);
                assert_eq!(escape_z, (3.0, 4.0));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(back.min_magnitude_time, 2);
        assert_eq!(back.min_magnitude, 0.5);
    }

    #[test]
    fn empty_seats_stay_none_through_upload() {
        let tile = Tile::<Answer>::new((0, 0), 0);
        let gpu = GPUTile::from_answer_tile(&tile, (64, 64), loc());
        assert!(gpu.get((0, 0)).is_none());
        assert!(gpu.get((63, 63)).is_none());
    }

    // r[impl cz.pub.gpu-native-work+1]
    // r[verify cz.seamless.gpu-preferred+1]
    // r[verify cz.pub.gpu-native-work+1]
    #[test]
    fn a_handle_with_a_production_slot_does_not_carry_cpu_answers() {
        let tile = GPUTile::from_answer_tile(&Tile::new((0, 0), 0), (64, 64), loc());
        let handle = GpuTileHandle::from_gpu_tile(tile, Some(7));
        assert_eq!(handle.production_slot, Some(7));
        assert!(
            handle.cpu_fallback.is_none()
            , "gpu-native handoff must not drag the answers back onto the channel"
        );
    }

    // r[verify cz.pub.gpu-native-work+1]
    #[test]
    fn a_handle_without_a_slot_keeps_cpu_answers_for_fallback() {
        let tile = GPUTile::from_answer_tile(&Tile::new((0, 0), 0), (64, 64), loc());
        let handle = GpuTileHandle::from_gpu_tile(tile, None);
        assert!(handle.cpu_fallback.is_some());
    }

    // r[verify cz.pub.gpu-native-work+1]
    #[test]
    fn production_slot_clears_cpu_fallback_for_gpu_native_path() {
        let with_slot = GpuTileHandle::from_gpu_tile(
            GPUTile::from_answer_tile(&Tile::new((1, 1), 0), (64, 64), loc()),
            Some(0),
        );
        let without = GpuTileHandle::from_gpu_tile(
            GPUTile::from_answer_tile(&Tile::new((1, 1), 0), (64, 64), loc()),
            None,
        );
        assert!(with_slot.cpu_fallback.is_none());
        assert!(without.cpu_fallback.is_some());
    }

    // r[verify cz.range.guess-biased-nearest+1]
    #[test]
    fn calibrated_gpu_roundtrips_outside_ranges() {
        let cal = CalibratedAnswer {
            result: CalibratedMandelbrotResult::Outside {
                escape_time_r2: Range { lower_bound: 3, upper_bound: 9 }
                , escape_z: (
                    Range { lower_bound: 1.0, upper_bound: 2.0 }
                    , Range { lower_bound: -1.0, upper_bound: 1.5 }
                )
            }
            , min_magnitude_time: Range { lower_bound: 1, upper_bound: 4 }
            , min_magnitude: Range { lower_bound: 0.25, upper_bound: 0.75 }
            , highlights: CalibratedHighlights {
                in_filament: Range { lower_bound: false, upper_bound: false }
                , out_filament: Range { lower_bound: false, upper_bound: false }
                , small_time_edge: Range { lower_bound: false, upper_bound: false }
                , node: Range { lower_bound: false, upper_bound: false }
            }
            , escape_time_angle: 0
            , min_magnitude_angle: 0
        };
        let gpu = GPUCalibratedAnswer::from_calibrated(cal);
        let back = gpu.to_calibrated().expect("outside");
        match back.result {
            CalibratedMandelbrotResult::Outside { escape_time_r2, escape_z } => {
                assert_eq!(escape_time_r2.lower_bound, 3);
                assert_eq!(escape_time_r2.upper_bound, 9);
                assert_eq!(escape_z.0.lower_bound, 1.0);
                assert_eq!(escape_z.1.upper_bound, 1.5);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(back.min_magnitude_time.upper_bound, 4);
    }

    #[test]
    fn empty_calibrated_gpu_stays_none() {
        assert!(GPUCalibratedAnswer::EMPTY.to_calibrated().is_none());
    }
}
