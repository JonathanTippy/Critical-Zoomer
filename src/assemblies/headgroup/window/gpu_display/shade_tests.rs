//! Parity between the shading shader and its cpu oracle.
//!
//! Every test builds a small patch of raw answers, renders it with the real wgsl, and holds
//! the result against `shade_oracle`. Where a test only needs the oracle's own rule, it says
//! so and skips the gpu.

use super::shade_harness::base_uniforms;
use super::shade_harness::frame_from_grid;
use super::shade_harness::gpu_or_skip;
use super::shade_oracle::*;
use super::GpuInstruction;
use super::ShadeUniforms;

/// A layer with no shading curve at all, so its colour lands on the pixel unchanged and the
/// comparison stays exact.
fn flat(opcode: u32, color: (f32, f32, f32), inside: f32, outside: f32) -> GpuInstruction {
    GpuInstruction {
        opcode
        , shading: SHADE_MODULAR
        , normalizing: NORM_NONE
        , thickness: 1
        , opacity_inside: inside
        , opacity_outside: outside
        , range: 0.0
        , period: 1.0
        , phase: 0.0
        , color_r: color.0
        , color_g: color.1
        , color_b: color.2
    }
}

fn shaded(
    opcode: u32
    , normalizing: u32
    , shading: u32
    , period: f32
    , phase: f32
    , range: f32
) -> GpuInstruction {
    GpuInstruction {
        opcode
        , shading
        , normalizing
        , thickness: 1
        , opacity_inside: 255.0
        , opacity_outside: 255.0
        , range
        , period
        , phase
        , color_r: 128.0
        , color_g: 64.0
        , color_b: 32.0
    }
}

/// Render on the gpu and shade with the oracle, then hold every pixel side by side.
fn assert_parity(
    test: &str
    , grid: &RawGrid
    , uniforms: ShadeUniforms
    , instructions: Vec<GpuInstruction>
    , slack: u8
) {
    let Some(gpu) = gpu_or_skip(test) else {
        return;
    };
    let frame = frame_from_grid(grid, uniforms, instructions);
    let expected = shade_frame(&frame.uniforms, &frame.instructions, grid);
    let actual = gpu.render(&frame);
    assert_eq!(actual.len(), expected.len(), "{test}: pixel count");
    let width = frame.uniforms.viewport_size[0] as usize;
    for (index, (got, want)) in actual.iter().zip(expected.iter()).enumerate() {
        for channel in 0..3 {
            let delta = got[channel].abs_diff(want[channel]);
            assert!(
                delta <= slack
                , "{test}: seat ({}, {}) channel {} is {:?} on the gpu and {:?} in the oracle"
                , index % width
                , index / width
                , channel
                , got
                , want
            );
        }
    }
}

fn escaped_at(escape_time: f32) -> RawAnswer {
    // A z just past r=2, so no further bailout iteration is needed at radius 2.
    RawAnswer::outside(escape_time, 3.0, 0.5, (2.5, 0.0))
}

// -------------------------------------------------------------------------------------
// escape phase
// r[verify cz.shade.escape-continues-to-bailout+1]
// -------------------------------------------------------------------------------------

#[test]
fn escape_continues_from_r2_up_to_the_bailout_radius() {
    let mut uniforms = base_uniforms((8, 8));
    uniforms.bailout_radius = 64.0;
    uniforms.bailout_max_extra = 20;
    uniforms.origin_re = -0.5;
    uniforms.origin_im = 0.5;
    uniforms.space = 0.01;

    // z sits just outside r=2, so reaching r=64 must take real extra iterations.
    let raw = RawAnswer::outside(7.0, 2.0, 0.3, (2.1, 0.1));
    let finished = bailout_escape(raw, (3, 4), &uniforms);
    assert!(
        finished.big_time > 7.0
        , "escape must carry the count past the r=2 answer, got {}"
        , finished.big_time
    );
    assert!(
        finished.big_time <= 7.0 + 20.0
        , "escape must respect the extra iteration budget, got {}"
        , finished.big_time
    );
}

// D-BAIL-1: bailout recolors from escape_z; membership (inside/outside) stays put.
#[test]
fn bailout_radius_does_not_flip_inside_membership() {
    let mut uniforms = base_uniforms((4, 4));
    let raw = RawAnswer::inside(3.0, 5.0, 0.01);
    for radius in [2.0f32, 16.0, 256.0] {
        uniforms.bailout_radius = radius;
        let finished = bailout_escape(raw, (1, 1), &uniforms);
        assert!(
            finished.is_inside()
            , "Inside must stay Inside under bailout={radius}"
        );
    }
}

#[test]
fn bailout_radius_does_not_flip_outside_membership() {
    let mut uniforms = base_uniforms((4, 4));
    let raw = RawAnswer::outside(9.0, 2.0, 0.2, (3.0, 0.0));
    for radius in [2.0f32, 8.0, 64.0] {
        uniforms.bailout_radius = radius;
        let finished = bailout_escape(raw, (2, 2), &uniforms);
        assert!(
            finished.is_outside()
            , "Outside must stay Outside under bailout={radius}"
        );
    }
}

#[test]
fn larger_bailout_only_extends_escape_time_not_kind() {
    let mut uniforms = base_uniforms((8, 8));
    uniforms.bailout_max_extra = 40;
    uniforms.origin_re = -0.5;
    uniforms.origin_im = 0.0;
    uniforms.space = 0.01;
    let raw = RawAnswer::outside(5.0, 1.0, 0.2, (2.05, 0.0));
    uniforms.bailout_radius = 2.0;
    let small = bailout_escape(raw, (3, 3), &uniforms);
    uniforms.bailout_radius = 64.0;
    let large = bailout_escape(raw, (3, 3), &uniforms);
    assert_eq!(small.is_outside(), large.is_outside());
    assert!(large.big_time >= small.big_time);
}

#[test]
fn escape_never_shortens_when_the_radius_grows() {
    let mut uniforms = base_uniforms((8, 8));
    uniforms.bailout_max_extra = 30;
    uniforms.origin_re = -0.75;
    uniforms.origin_im = 0.1;
    uniforms.space = 0.005;
    let raw = RawAnswer::outside(11.0, 4.0, 0.2, (2.05, -0.3));

    let mut previous = 0.0;
    for radius in [2.0f32, 4.0, 8.0, 16.0, 32.0, 128.0] {
        uniforms.bailout_radius = radius;
        let big_time = bailout_escape(raw, (1, 1), &uniforms).big_time;
        assert!(
            big_time >= previous
            , "radius {radius} gave {big_time}, which is less than the {previous} a smaller radius gave"
        );
        previous = big_time;
    }
}

#[test]
fn inside_and_uncovered_seats_skip_the_escape_loop() {
    let mut uniforms = base_uniforms((8, 8));
    uniforms.bailout_radius = 100.0;
    uniforms.bailout_max_extra = 500;

    let inside = bailout_escape(RawAnswer::inside(6.0, 4.0, 0.01), (0, 0), &uniforms);
    assert!(inside.is_inside(), "an inside answer must stay inside");
    assert_eq!(inside.loop_period, 6.0, "the period must survive the escape phase");
    assert_eq!(inside.small_time, 4.0, "the small time must survive the escape phase");

    // r[verify cz.tenacious.nores-not-flat-black+1]
    let missing = bailout_escape(RawAnswer::missing(), (0, 0), &uniforms);
    assert!(missing.is_outside(), "an uncovered seat must read as outside, never as inside");
    assert_eq!(missing.big_time, 1.0, "an uncovered seat must read as an instant escape");
    assert!(
        missing.smallness > 1.0e29
        , "an uncovered seat must be too large to ever be taken for a node"
    );
}

#[test]
fn escape_phase_matches_the_shader() {
    let mut grid = RawGrid::new((8, 8));
    for y in 0..8 {
        for x in 0..8 {
            grid.set((x, y), RawAnswer::outside(
                (x + y * 8) as f32 + 1.0
                , 2.0
                , 0.4
                , (2.0 + x as f32 * 0.05, y as f32 * 0.05)
            ));
        }
    }
    let mut uniforms = base_uniforms((8, 8));
    uniforms.bailout_radius = 16.0;
    uniforms.bailout_max_extra = 12;
    uniforms.origin_re = -0.6;
    uniforms.origin_im = 0.4;
    uniforms.space = 0.02;

    assert_parity(
        "escape_phase_matches_the_shader"
        , &grid
        , uniforms
        , vec![shaded(OP_ESCAPE_TIME, NORM_NONE, SHADE_MODULAR, 16.0, 0.0, 0.0)]
        , 0
    );
}

// -------------------------------------------------------------------------------------
// edge annotation: in filaments
// r[verify cz.shade.in-filament-slope-inversion+1]
// -------------------------------------------------------------------------------------

/// Escape times which peak in the middle column of the middle row.
fn escape_ridge() -> RawGrid {
    let mut grid = RawGrid::new((5, 5));
    for y in 0..5 {
        for x in 0..5 {
            grid.set((x, y), escaped_at(10.0));
        }
    }
    grid.set((2, 2), escaped_at(40.0));
    grid
}

#[test]
fn a_peak_in_escape_time_is_an_in_filament() {
    let grid = escape_ridge();
    let uniforms = base_uniforms((5, 5));
    assert!(
        is_in_filament(&grid, &uniforms, (2, 2))
        , "a seat whose escape time is higher than both its neighbors is a filament"
    );
    assert!(
        !is_in_filament(&grid, &uniforms, (0, 0))
        , "flat ground is not a filament"
    );
}

#[test]
fn a_monotonic_slope_is_not_an_in_filament() {
    let mut grid = RawGrid::new((5, 5));
    for y in 0..5 {
        for x in 0..5 {
            grid.set((x, y), escaped_at(10.0 + x as f32));
        }
    }
    let uniforms = base_uniforms((5, 5));
    for x in 1..4 {
        assert!(
            !is_in_filament(&grid, &uniforms, (x, 2))
            , "a steady climb has no inversion, so seat {x} is not a filament"
        );
    }
}

#[test]
fn inside_neighbors_do_not_take_part_in_the_in_filament_test() {
    let mut grid = escape_ridge();
    // Drop the escape time at the peak so it only wins against neighbors which opt out.
    grid.set((2, 2), escaped_at(10.0));
    grid.set((2, 1), RawAnswer::inside(3.0, 1.0, 0.01));
    grid.set((2, 3), RawAnswer::inside(3.0, 1.0, 0.01));
    let uniforms = base_uniforms((5, 5));
    assert!(
        !is_in_filament(&grid, &uniforms, (2, 2))
        , "an inside neighbor has no escape time, so it cannot make the seat a peak"
    );
}

#[test]
fn in_filament_highlighting_matches_the_shader() {
    let grid = escape_ridge();
    assert_parity(
        "in_filament_highlighting_matches_the_shader"
        , &grid
        , base_uniforms((5, 5))
        , vec![
            flat(OP_ESCAPE_TIME, (200.0, 200.0, 200.0), 0.0, 255.0)
            , flat(OP_IN_FILAMENT, (0.0, 0.0, 0.0), 0.0, 255.0)
        ]
        , 0
    );
}

// -------------------------------------------------------------------------------------
// edge annotation: out filaments
// r[verify cz.shade.out-filament-period-step+1]
// -------------------------------------------------------------------------------------

fn period_step() -> RawGrid {
    let mut grid = RawGrid::new((5, 5));
    for y in 0..5 {
        for x in 0..5 {
            grid.set((x, y), RawAnswer::inside(8.0, 3.0, 0.05));
        }
    }
    grid.set((2, 1), RawAnswer::inside(2.0, 3.0, 0.05));
    grid
}

#[test]
fn a_lower_period_neighbor_makes_an_out_filament() {
    let grid = period_step();
    let uniforms = base_uniforms((5, 5));
    assert!(
        is_out_filament(&grid, &uniforms, (2, 2))
        , "a seat with a shorter period above it sits on a period edge"
    );
    assert!(
        !is_out_filament(&grid, &uniforms, (4, 4))
        , "a seat surrounded by its own period is not an edge"
    );
}

// D-SHADE-3: paint only the higher-period side of a period edge.
#[test]
fn lower_period_side_is_not_an_out_filament() {
    let grid = period_step();
    let uniforms = base_uniforms((5, 5));
    assert!(
        !is_out_filament(&grid, &uniforms, (2, 1))
        , "the shorter-period neighbor must not claim the out-filament"
    );
}

#[test]
fn a_zero_period_neighbor_cannot_make_an_out_filament() {
    let mut grid = period_step();
    grid.set((2, 1), RawAnswer::inside(0.0, 3.0, 0.05));
    let uniforms = base_uniforms((5, 5));
    assert!(
        !is_out_filament(&grid, &uniforms, (2, 2))
        , "a period of zero is not a period and must not invent an edge"
    );
}

#[test]
fn outside_seats_are_never_out_filaments() {
    let mut grid = period_step();
    grid.set((2, 2), escaped_at(30.0));
    let uniforms = base_uniforms((5, 5));
    let instructions = vec![flat(OP_OUT_FILAMENT, (255.0, 255.0, 255.0), 255.0, 255.0)];
    let colour = shade_seat(&uniforms, &instructions, &grid, (2, 2));
    assert_eq!(
        colour
        , [0, 0, 0]
        , "out filaments belong to the inside, so an outside seat stays as it was"
    );
}

#[test]
fn out_filament_highlighting_matches_the_shader() {
    let grid = period_step();
    assert_parity(
        "out_filament_highlighting_matches_the_shader"
        , &grid
        , base_uniforms((5, 5))
        , vec![flat(OP_OUT_FILAMENT, (10.0, 200.0, 30.0), 255.0, 255.0)]
        , 0
    );
}

// -------------------------------------------------------------------------------------
// edge annotation: nodes
// r[verify cz.shade.node-smallness-minimum+1]
// -------------------------------------------------------------------------------------

/// A pit in smallness at the centre, `thickness` seats wide.
///
/// Seats closer than `thickness` sit at the pit floor so a thinner probe sees a
/// plateau; only the probe that reaches the raised rim finds a minimum.
fn smallness_pit(thickness: i32) -> RawGrid {
    let edge = 2 * thickness + 3;
    let mut grid = RawGrid::new((edge, edge));
    for y in 0..edge {
        for x in 0..edge {
            grid.set((x, y), RawAnswer::inside(4.0, 2.0, 1.0));
        }
    }
    let centre = thickness + 1;
    for y in 0..edge {
        for x in 0..edge {
            let dist = (x - centre).abs().max((y - centre).abs());
            if dist < thickness {
                grid.set((x, y), RawAnswer::inside(4.0, 2.0, 0.01));
            }
        }
    }
    grid.set((centre, centre), RawAnswer::inside(4.0, 2.0, 0.01));
    grid
}

#[test]
fn a_smallness_pit_is_a_node() {
    let grid = smallness_pit(1);
    let uniforms = base_uniforms((5, 5));
    assert!(
        is_node(&grid, &uniforms, (2, 2), 1)
        , "smallness lower than all four neighbors is a node"
    );
    assert!(
        !is_node(&grid, &uniforms, (2, 3), 1)
        , "a seat next to the pit is not itself the pit"
    );
}

#[test]
fn a_node_is_only_seen_at_its_own_thickness() {
    let grid = smallness_pit(3);
    let uniforms = base_uniforms((9, 9));
    assert!(
        is_node(&grid, &uniforms, (4, 4), 3)
        , "the pit is three seats wide, so thickness three must find it"
    );
    assert!(
        !is_node(&grid, &uniforms, (4, 4), 1)
        , "at thickness one the neighbors are level with the pit, so there is no minimum"
    );
}

#[test]
fn a_plateau_holds_no_node() {
    let mut grid = RawGrid::new((5, 5));
    for y in 0..5 {
        for x in 0..5 {
            grid.set((x, y), RawAnswer::inside(4.0, 2.0, 0.7));
        }
    }
    let uniforms = base_uniforms((5, 5));
    for y in 1..4 {
        for x in 1..4 {
            assert!(
                !is_node(&grid, &uniforms, (x, y), 1)
                , "seat ({x}, {y}) is level with its neighbors so it cannot be a minimum"
            );
        }
    }
}

#[test]
fn node_highlighting_matches_the_shader() {
    let grid = smallness_pit(2);
    let mut nodes = flat(OP_NODES, (250.0, 250.0, 10.0), 255.0, 255.0);
    nodes.thickness = 2;
    assert_parity(
        "node_highlighting_matches_the_shader"
        , &grid
        , base_uniforms((7, 7))
        , vec![nodes]
        , 0
    );
}

// -------------------------------------------------------------------------------------
// edge annotation: small time edges
// r[verify cz.shade.small-time-edge-nonzero+1]
// -------------------------------------------------------------------------------------

fn small_time_field(centre: f32, up: f32) -> RawGrid {
    let mut grid = RawGrid::new((3, 3));
    for y in 0..3 {
        for x in 0..3 {
            grid.set((x, y), RawAnswer::inside(1.0, centre, 0.1));
        }
    }
    grid.set((1, 0), RawAnswer::inside(1.0, up, 0.1));
    grid
}

#[test]
fn a_real_step_down_in_small_time_is_an_edge() {
    let grid = small_time_field(8.0, 2.0);
    assert!(
        is_ste(&grid, (1, 1))
        , "a neighbor with a genuinely lower small time makes a ridge"
    );
    assert!(
        !is_ste(&small_time_field(8.0, 8.0), (1, 1))
        , "level small time is not a ridge"
    );
}

#[test]
fn an_unfinished_zero_neighbor_cannot_invent_a_small_time_edge() {
    let grid = small_time_field(5.0, 0.0);
    assert!(
        !is_ste(&grid, (1, 1))
        , "a small time of zero says nothing, so it must not spur an edge"
    );
}

#[test]
fn an_uncovered_neighbor_cannot_invent_a_small_time_edge() {
    let mut grid = RawGrid::new((3, 3));
    for y in 0..3 {
        for x in 0..3 {
            grid.set((x, y), RawAnswer::inside(1.0, 6.0, 0.1));
        }
    }
    grid.set((1, 0), RawAnswer::missing());
    assert!(
        !is_ste(&grid, (1, 1))
        , "a seat no tile covers has no small time to compare against"
    );
}

#[test]
fn a_zero_small_time_is_still_an_ordinary_value_to_paint() {
    let mut grid = RawGrid::new((3, 3));
    grid.fill(RawAnswer::outside(4.0, 0.0, 0.5, (2.5, 0.0)));
    let uniforms = base_uniforms((3, 3));
    let instructions = vec![shaded(OP_SMALL_TIME, NORM_NONE, SHADE_MODULAR, 4.0, 2.0, 255.0)];
    let painted = shade_seat(&uniforms, &instructions, &grid, (1, 1));
    assert_ne!(
        painted
        , [0, 0, 0]
        , "small time zero is excluded from edges only, it must still paint"
    );
}

#[test]
fn small_time_edge_highlighting_matches_the_shader() {
    let grid = small_time_field(9.0, 3.0);
    assert_parity(
        "small_time_edge_highlighting_matches_the_shader"
        , &grid
        , base_uniforms((3, 3))
        , vec![flat(OP_STE, (255.0, 0.0, 0.0), 255.0, 255.0)]
        , 0
    );
}

// -------------------------------------------------------------------------------------
// layered coloring
// r[verify cz.shade.layers-in-script-order+1]
// -------------------------------------------------------------------------------------

fn flat_outside_grid() -> RawGrid {
    let mut grid = RawGrid::new((4, 4));
    grid.fill(escaped_at(12.0));
    grid
}

#[test]
fn layers_land_in_the_order_they_are_written() {
    let grid = flat_outside_grid();
    let uniforms = base_uniforms((4, 4));
    let red = flat(OP_ESCAPE_TIME, (255.0, 0.0, 0.0), 0.0, 255.0);
    let blue = flat(OP_ESCAPE_TIME, (0.0, 0.0, 255.0), 0.0, 128.0);

    let red_then_blue = shade_seat(&uniforms, &[red, blue], &grid, (1, 1));
    let blue_then_red = shade_seat(&uniforms, &[blue, red], &grid, (1, 1));
    assert_ne!(
        red_then_blue, blue_then_red
        , "a half opaque layer over a full one must not look the same as the reverse"
    );
    assert_eq!(
        blue_then_red, [254, 0, 0]
        , "the last fully opaque layer must own the pixel"
    );
}

#[test]
fn the_order_the_shader_uses_is_the_order_the_oracle_uses() {
    let grid = flat_outside_grid();
    assert_parity(
        "the_order_the_shader_uses_is_the_order_the_oracle_uses"
        , &grid
        , base_uniforms((4, 4))
        , vec![
            flat(OP_ESCAPE_TIME, (200.0, 30.0, 30.0), 0.0, 255.0)
            , flat(OP_SMALL_TIME, (30.0, 200.0, 30.0), 200.0, 100.0)
            , flat(OP_SMALLNESS, (30.0, 30.0, 200.0), 200.0, 60.0)
        ]
        , 0
    );
}

#[test]
fn a_disabled_layer_leaves_the_pixel_exactly_as_it_was() {
    let grid = flat_outside_grid();
    let uniforms = base_uniforms((4, 4));
    let paint = flat(OP_ESCAPE_TIME, (200.0, 100.0, 50.0), 0.0, 255.0);
    let with_one = shade_seat(&uniforms, &[paint], &grid, (2, 2));

    // Every kind of layer, all switched off, must be free of consequence.
    let mut off = Vec::new();
    for opcode in [
        OP_ESCAPE_TIME
        , OP_SMALL_TIME
        , OP_SMALLNESS
        , OP_IN_FILAMENT
        , OP_OUT_FILAMENT
        , OP_NODES
        , OP_STE
    ] {
        off.push(flat(opcode, (255.0, 255.0, 255.0), 0.0, 0.0));
    }
    let mut script = vec![paint];
    script.extend(off);
    let with_disabled = shade_seat(&uniforms, &script, &grid, (2, 2));
    assert_eq!(
        with_one, with_disabled
        , "layers at zero opacity must not darken the picture"
    );
}

#[test]
fn disabled_layers_are_free_of_consequence_on_the_shader_too() {
    let grid = flat_outside_grid();
    let mut script = vec![flat(OP_ESCAPE_TIME, (200.0, 100.0, 50.0), 0.0, 255.0)];
    for opcode in [OP_SMALL_TIME, OP_IN_FILAMENT, OP_NODES, OP_STE] {
        script.push(flat(opcode, (255.0, 255.0, 255.0), 0.0, 0.0));
    }
    assert_parity(
        "disabled_layers_are_free_of_consequence_on_the_shader_too"
        , &grid
        , base_uniforms((4, 4))
        , script
        , 0
    );
}

#[test]
fn every_normalizer_and_curve_agrees_with_the_shader() {
    let mut grid = RawGrid::new((8, 8));
    for y in 0..8 {
        for x in 0..8 {
            grid.set((x, y), RawAnswer::outside(
                (1 + x + y * 8) as f32
                , (x + 1) as f32
                , 0.05 + y as f32 * 0.1
                , (2.5, 0.0)
            ));
        }
    }
    for normalizing in [NORM_NONE, NORM_LN, NORM_LNLN, NORM_RECIP, NORM_RECIP_LN] {
        for shading in [SHADE_MODULAR, SHADE_SINUS] {
            for opcode in [OP_ESCAPE_TIME, OP_SMALL_TIME, OP_SMALLNESS] {
                assert_parity(
                    "every_normalizer_and_curve_agrees_with_the_shader"
                    , &grid
                    , base_uniforms((8, 8))
                    , vec![shaded(opcode, normalizing, shading, 7.0, 1.5, 200.0)]
                    , 1
                );
            }
        }
    }
}

#[test]
fn normalizers_stay_finite_at_the_edge_of_their_domain() {
    for method in [NORM_NONE, NORM_LN, NORM_LNLN, NORM_RECIP, NORM_RECIP_LN] {
        for input in [0.0f32, 1.0, 2.718_281_7, 1.0e30] {
            let n = normalize_value(input, method);
            assert!(
                n.is_finite()
                , "normalizer {method} turned {input} into {n}"
            );
        }
    }
}

#[test]
fn modular_brightness_never_leaves_the_unit_range() {
    for phase in [-1000.0f32, -7.5, 0.0, 3.25, 900.0] {
        for n in [0.0f32, 1.0, 13.0, 1.0e6] {
            let brightness = shade_value(n, 7.0, phase, SHADE_MODULAR);
            assert!(
                (0.0..1.0).contains(&brightness)
                , "brightness {brightness} from value {n} and phase {phase} is out of range"
            );
        }
    }
}

#[test]
fn a_negative_phase_agrees_with_the_shader() {
    let grid = flat_outside_grid();
    assert_parity(
        "a_negative_phase_agrees_with_the_shader"
        , &grid
        , base_uniforms((4, 4))
        , vec![shaded(OP_ESCAPE_TIME, NORM_NONE, SHADE_MODULAR, 5.0, -13.0, 255.0)]
        , 1
    );
}

// -------------------------------------------------------------------------------------
// uncovered seats
// r[verify cz.display.nores-when-no-proximate+1]
// -------------------------------------------------------------------------------------

#[test]
fn a_frame_with_no_answers_at_all_is_not_flat_black() {
    let grid = RawGrid::new((4, 4));
    let uniforms = base_uniforms((4, 4));
    let script = vec![shaded(OP_ESCAPE_TIME, NORM_NONE, SHADE_SINUS, 4.0, 1.0, 255.0)];
    let frame = shade_frame(&uniforms, &script, &grid);
    assert!(
        frame.iter().all(|pixel| *pixel != [0, 0, 0])
        , "uncovered seats must still be painted, not left black"
    );
    assert_parity(
        "a_frame_with_no_answers_at_all_is_not_flat_black"
        , &grid
        , uniforms
        , script
        , 1
    );
}

// -------------------------------------------------------------------------------------
// whole frame
// -------------------------------------------------------------------------------------

/// A busy patch holding every kind of answer next to every other kind.
fn mixed_grid() -> RawGrid {
    let mut grid = RawGrid::new((16, 16));
    for y in 0..16 {
        for x in 0..16 {
            let raw = match (x + y * 3) % 5 {
                0 => RawAnswer::missing()
                , 1 => RawAnswer::inside(((x % 7) + 1) as f32, (y % 5) as f32, 0.02 * (x + 1) as f32)
                , 2 => RawAnswer::inside(1.0, 0.0, 0.9)
                , 3 => RawAnswer::outside(
                    (x * y % 23 + 1) as f32
                    , (x % 4) as f32
                    , 0.3 + 0.05 * y as f32
                    , (2.02 + 0.01 * x as f32, 0.01 * y as f32)
                )
                , _ => RawAnswer::outside(
                    (x + y + 1) as f32
                    , ((x + y) % 6) as f32
                    , 0.11 * (y + 1) as f32
                    , (2.5, -0.4)
                )
            };
            grid.set((x, y), raw);
        }
    }
    grid
}

/// The script the app actually ships, near enough: an escape time wash, subtle small time,
/// black in filaments, out filaments, disabled nodes and a red small time edge overlay.
fn default_shaped_script() -> Vec<GpuInstruction> {
    let mut nodes = flat(OP_NODES, (128.0, 128.0, 128.0), 0.0, 0.0);
    nodes.thickness = 3;
    vec![
        shaded(OP_ESCAPE_TIME, NORM_NONE, SHADE_SINUS, 12.0, 0.0, 255.0)
        , shaded(OP_SMALL_TIME, NORM_LN, SHADE_MODULAR, 3.0, 0.5, 60.0)
        , flat(OP_IN_FILAMENT, (0.0, 0.0, 0.0), 0.0, 255.0)
        , flat(OP_OUT_FILAMENT, (128.0, 128.0, 128.0), 0.0, 255.0)
        , nodes
        , flat(OP_STE, (255.0, 0.0, 0.0), 40.0, 40.0)
    ]
}

#[test]
fn the_shipped_script_over_a_mixed_frame_matches_the_shader() {
    let grid = mixed_grid();
    let mut uniforms = base_uniforms((16, 16));
    uniforms.bailout_radius = 8.0;
    uniforms.bailout_max_extra = 6;
    uniforms.origin_re = -0.35;
    uniforms.origin_im = 0.28;
    uniforms.space = 1.0 / 512.0;
    assert_parity(
        "the_shipped_script_over_a_mixed_frame_matches_the_shader"
        , &grid
        , uniforms
        , default_shaped_script()
        , 2
    );
}

#[test]
fn a_grey_frame_is_grey_on_both_sides() {
    let grid = mixed_grid();
    let mut uniforms = base_uniforms((8, 8));
    uniforms.zoom_match = 0;
    assert_parity(
        "a_grey_frame_is_grey_on_both_sides"
        , &grid
        , uniforms
        , default_shaped_script()
        , 0
    );
}

proptest::proptest! {
    // A gpu render per case, so the case count is deliberately small; the shapes vary far
    // more than the count does.
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(24))]

    /// Whatever the answers are, the shader and the oracle must agree pixel for pixel.
    #[test]
    fn random_answers_shade_the_same_on_both_sides(
        kinds in proptest::collection::vec(0u8..3, 64)
        , escape_times in proptest::collection::vec(1u32..200, 64)
        , small_times in proptest::collection::vec(0u32..12, 64)
        , smallnesses in proptest::collection::vec(1u32..400, 64)
        , radius in 2.0f32..24.0
        , extra in 0u32..8
    ) {
        let mut grid = RawGrid::new((8, 8));
        for seat in 0..64 {
            let escape_time = escape_times[seat] as f32;
            let small_time = small_times[seat] as f32;
            let smallness = smallnesses[seat] as f32 / 100.0;
            let raw = match kinds[seat] {
                0 => RawAnswer::missing()
                , 1 => RawAnswer::inside(escape_time % 9.0, small_time, smallness)
                , _ => RawAnswer::outside(
                    escape_time
                    , small_time
                    , smallness
                    , (2.0 + smallness, small_time * 0.1)
                )
            };
            grid.set(((seat % 8) as i32, (seat / 8) as i32), raw);
        }
        let mut uniforms = base_uniforms((8, 8));
        uniforms.bailout_radius = radius;
        uniforms.bailout_max_extra = extra;
        uniforms.origin_re = -0.4;
        uniforms.origin_im = 0.3;
        uniforms.space = 1.0 / 256.0;
        assert_parity(
            "random_answers_shade_the_same_on_both_sides"
            , &grid
            , uniforms
            , default_shaped_script()
            , 2
        );
    }
}
