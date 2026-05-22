//! Same-resolution viewport pan: smearing, boundary extension, and frontier queue shift.

use std::collections::VecDeque;

use crate::act::sampling::remap_source_index_smearing;
use crate::act::utils::ObjectivePosAndZoom;
use crate::act::utils::index_from_pos;
use crate::act::workshift::{
    queue_incomplete_neighbors_of_edge, point_is_edge, Point, WorkContext,
};
use crate::actor::work_controller::{get_points, get_evenly_spaced_map, PIXELS_PER_UNIT_POT};

pub(crate) fn viewport_relative_pixel_shift(
    old_location: &ObjectivePosAndZoom,
    new_location: &ObjectivePosAndZoom,
) -> ((i32, i32), i64) {
    let relative_pos = (
        old_location.pos.0.clone() - new_location.pos.0.clone(),
        old_location.pos.1.clone() - new_location.pos.1.clone(),
    );
    let relative_pos_in_pixels: (i32, i32) = (
        relative_pos
            .0
            .clone()
            .shift(new_location.zoom_pot)
            .shift(PIXELS_PER_UNIT_POT)
            .into(),
        relative_pos
            .1
            .clone()
            .shift(new_location.zoom_pot)
            .shift(PIXELS_PER_UNIT_POT)
            .into(),
    );
    let relative_zoom = (new_location.zoom_pot - old_location.zoom_pot) as i64;
    (relative_pos_in_pixels, relative_zoom)
}

fn shift_pos((x, y): (i32, i32), (dx, dy): (i32, i32)) -> (i32, i32) {
    (x + dx, y + dy)
}

fn in_frame(pos: (i32, i32), res: (u32, u32)) -> bool {
    pos.0 >= 0
        && pos.0 < res.0 as i32
        && pos.1 >= 0
        && pos.1 < res.1 as i32
}

fn shift_queue(
    queue: &VecDeque<((i32, i32), u32)>,
    dx: i32,
    dy: i32,
    res: (u32, u32),
) -> VecDeque<((i32, i32), u32)> {
    let mut out = VecDeque::new();
    for (pos, meta) in queue {
        let shifted = shift_pos(*pos, (dx, dy));
        if in_frame(shifted, res) {
            out.push_back((shifted, *meta));
        }
    }
    out
}

fn shift_scredge(scredge: &VecDeque<(i32, i32)>, dx: i32, dy: i32, res: (u32, u32)) -> VecDeque<(i32, i32)> {
    let mut out = VecDeque::new();
    for pos in scredge {
        let shifted = shift_pos(*pos, (dx, dy));
        if in_frame(shifted, res) {
            out.push_back(shifted);
        }
    }
    out
}

/// Perimeter cells not already `delivered` after smearing (newly exposed strip).
pub(crate) fn exposed_perimeter_scredge<T: Copy>(
    points: &[Point<T>],
    res: (u32, u32),
) -> VecDeque<(i32, i32)> {
    let w = res.0 as i32;
    let h = res.1 as i32;
    let mut linear = Vec::new();
    for i in 0..w - 1 {
        linear.push((i, 0));
    }
    for i in 0..h - 1 {
        linear.push((w - 1, i));
    }
    for i in 0..w {
        linear.push((i, h - 1));
    }
    for i in 1..h - 1 {
        linear.push((0, i));
    }
    let map = get_evenly_spaced_map(linear.len());
    let mut out = VecDeque::new();
    for i in map {
        let pos = linear[i];
        let idx = index_from_pos(&pos, res.0);
        if !points[idx].delivered {
            out.push_back(pos);
        }
    }
    out
}

fn copy_delivered_point<T: Copy>(dst: &mut Point<T>, src: &Point<T>) {
    dst.z = src.z;
    dst.real_squared = src.real_squared;
    dst.imag_squared = src.imag_squared;
    dst.real_imag = src.real_imag;
    dst.iterations = src.iterations;
    dst.loop_detection_point = src.loop_detection_point;
    dst.escapes = src.escapes;
    dst.repeats = src.repeats;
    dst.delivered = true;
    dst.period = src.period;
    dst.smallness_squared = src.smallness_squared;
    dst.small_time = src.small_time;
}

/// Seed `edge_queue` along smeared-known vs incomplete boundary (frontier extension).
pub(crate) fn seed_edge_queue_from_smear_boundary<T: Copy + std::ops::Sub<Output = T> + std::ops::Add<Output = T> + std::ops::Mul<Output = T>>(
    ctx: &mut WorkContext<T>,
) {
    let res = ctx.res;
    let w = res.0;
    for row in 0..res.1 as i32 {
        for seat in 0..res.0 as i32 {
            let pos = (seat, row);
            let idx = index_from_pos(&pos, w);
            if !ctx.points[idx].delivered {
                continue;
            }
            if let Some((p1, p2)) = point_is_edge(&pos, res, &ctx.points) {
                queue_incomplete_neighbors_of_edge(&p1, &p2, res, &ctx.points, &mut ctx.edge_queue);
            }
        }
    }
}

pub(crate) fn pan_shift_work_context<T>(
    ctx: &mut WorkContext<T>,
    old_location: &ObjectivePosAndZoom,
    new_location: &ObjectivePosAndZoom,
    new_loc_zoom: ((crate::act::utils::IntExp, crate::act::utils::IntExp), i64),
) where
    T: Copy
        + From<f32>
        + Clone
        + From<crate::act::utils::IntExp>
        + std::ops::Sub<Output = T>
        + std::ops::Add<Output = T>
        + std::ops::Mul<Output = T>
        + PartialOrd
        + crate::act::workshift::Finite
        + crate::act::workshift::Gt
        + crate::act::workshift::Abs
        + Into<f64>,
{
    let res = ctx.res;
    let (relative_pos_in_pixels, relative_zoom) =
        viewport_relative_pixel_shift(old_location, new_location);
    let (dx, dy) = relative_pos_in_pixels;
    let (new_loc, new_zoom_pot) = new_loc_zoom;

    let old_points = std::mem::replace(
        &mut ctx.points,
        get_points(res, new_loc, new_zoom_pot),
    );

    for row in 0..res.1 as usize {
        for seat in 0..res.0 as usize {
            let new_idx = index_from_pos(&(seat as i32, row as i32), res.0);
            let old_src = remap_source_index_smearing(
                seat,
                row,
                relative_pos_in_pixels,
                relative_zoom,
                res,
            );
            if old_points[old_src].delivered {
                copy_delivered_point(&mut ctx.points[new_idx], &old_points[old_src]);
            }
        }
    }

    ctx.edge_queue = shift_queue(&ctx.edge_queue, dx, dy, res);
    ctx.out_queue = shift_queue(&ctx.out_queue, dx, dy, res);
    ctx.in_queue = shift_queue(&ctx.in_queue, dx, dy, res);
    let shifted_scredge = shift_scredge(&ctx.scredge_poses, dx, dy, res);
    ctx.scredge_poses = exposed_perimeter_scredge(&ctx.points, res);
    for pos in shifted_scredge {
        if !ctx.scredge_poses.contains(&pos) {
            ctx.scredge_poses.push_back(pos);
        }
    }

    seed_edge_queue_from_smear_boundary(ctx);
    ctx.time_workshift_started = std::time::Instant::now();
}
