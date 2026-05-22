use steady_state::*;

use std::collections::*;
use std::io::Write;

// #region agent log
const DEBUG_LOG_PATH: &str = "/home/jonathan/git/Critical-Zoomer/.cursor/debug-419d19.log";

fn agent_debug_log(hypothesis_id: &str, location: &str, message: &str, data: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let line = format!(
        r#"{{"sessionId":"419d19","hypothesisId":"{}","location":"{}","message":"{}","data":{},"timestamp":{}}}"#,
        hypothesis_id, location, message, data, ts
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(DEBUG_LOG_PATH)
    {
        let _ = writeln!(f, "{}", line);
    }
}
// #endregion
use crate::actor::window::*;
use crate::act::workshift::*;
use crate::act::sampling::*;
use crate::actor::screen_worker::*;

use crate::act::utils::*;
use crate::act::constants::*;
use crate::act::boot_trace;

pub(crate) enum WorkerCommand<T: Copy> {
    Replace {
        frame_info: (ObjectivePosAndZoom, (u32, u32)),
        context: WorkContext<T>,
    },
    /// Same-resolution pan: shift existing worker context (smearing), do not rebuild mixmap.
    Pan {
        frame_info: (ObjectivePosAndZoom, (u32, u32)),
    },
}


pub(crate) struct WorkControllerState {
    mixmap: Vec<usize>
    , loc: (IntExp, IntExp)
    , zoom_pot: i64
    , worker_res: (u32, u32)
    , percent_completed: u16
    , last_sampler_location: Option<ObjectivePosAndZoom>
}


pub(crate) const WORKER_INIT_RES:(u32, u32) = DEFAULT_WINDOW_RES;
pub(crate) const WORKER_INIT_ZOOM_POT: i64 = -2;
pub(crate) const WORKER_INIT_ZOOM:f64 = if WORKER_INIT_ZOOM_POT>0 {(1<<WORKER_INIT_ZOOM_POT) as f64} else {1.0 / (1<<-WORKER_INIT_ZOOM_POT) as f64};

pub(crate) const PIXELS_PER_UNIT_POT:i32 = 9;
pub(crate) const PIXELS_PER_UNIT: u64 = 1<<(PIXELS_PER_UNIT_POT);

pub async fn run(
    actor: SteadyActorShadow,
    from_sampler: SteadyRx<(ObjectivePosAndZoom, (u32, u32))>,
    to_worker: SteadyTx<WorkerCommand<f64>>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {
    // The worker is tested by its simulated neighbors, so we always use internal_behavior.
    internal_behavior(
        actor.into_spotlight([&from_sampler], [&to_worker]),
        from_sampler,
        to_worker,
        state,
    )
        .await
}

async fn internal_behavior<A: SteadyActor, T:Clone + From<f32> + From<f32> + Clone + From<IntExp> + Sub<Output=T> + Add<Output=T> + Mul<Output=T> + PartialOrd + crate::act::workshift::Finite + crate::act::workshift::Gt + crate::act::workshift::Abs + From<f32> + Into<f64> + Copy>(
    mut actor: A,
    from_sampler: SteadyRx<(ObjectivePosAndZoom, (u32, u32))>,
    to_worker: SteadyTx<WorkerCommand<T>>,
    state: SteadyState<WorkControllerState>,
) -> Result<(), Box<dyn Error>> {

    let mut from_sampler = from_sampler.lock().await;
    let mut to_worker = to_worker.lock().await;

    let pixel_count = (WORKER_INIT_RES.0 * WORKER_INIT_RES.1) as usize;
    boot_trace::boot_once(
        "wc_mixmap_build_start",
        &format!(r#"{{"pixels":{}}}"#, pixel_count),
    );
    let mixmap_start = std::time::Instant::now();
    let mut state = state.lock(|| WorkControllerState {
        mixmap: get_evenly_spaced_map(pixel_count)
        , loc: (IntExp::from(0), IntExp::from(0))
        , zoom_pot: WORKER_INIT_ZOOM_POT
        , worker_res: WORKER_INIT_RES
        , percent_completed: 0
        , last_sampler_location: None
    }).await;
    boot_trace::boot_span(
        "wc_mixmap_build_done",
        &format!(r#"{{"pixels":{}}}"#, pixel_count),
        mixmap_start.elapsed().as_millis(),
    );
    boot_trace::boot_once("wc_actor_ready", r#"{}"#);


    let max_sleep = Duration::from_millis(50);

    let res = state.worker_res.clone();
    //let ctx = handle_sampler_stuff(&mut state, (
    //    (IntExp::from(0), IntExp::from(0)), res));
    //actor.try_send(&mut to_worker, WorkerCommand::Replace{context:ctx});

    while actor.is_running(
        || i!(to_worker.mark_closed())
    ) {

        await_for_any!(
            actor.wait_periodic(max_sleep),
            actor.wait_avail(&mut from_sampler, 1),
        );

        //info!("work controller alive");
        if actor.avail_units(&mut from_sampler) > 0 {
            let mut stuff = actor.try_take(&mut from_sampler).expect("internal error");
            while actor.avail_units(&mut from_sampler) > 0 {
                stuff = actor.try_take(&mut from_sampler).expect("internal error");
            }

            // #region agent log
            agent_debug_log(
                "H-D",
                "work_controller.rs:internal_behavior",
                "sampler message received",
                &format!(
                    r#"{{"res":[{},{}],"worker_res":[{},{}],"res_changed":{}}}"#,
                    stuff.1.0,
                    stuff.1.1,
                    state.worker_res.0,
                    state.worker_res.1,
                    stuff.1 != state.worker_res
                ),
            );
            // #endregion

            match handle_sampler_stuff(&mut state, stuff.clone()) {
                Some(cmd) => {
                    match &cmd {
                        WorkerCommand::Replace { context, .. } => {
                            boot_trace::boot_once(
                                "wc_replace_sent",
                                &format!(r#"{{"res":[{},{}]}}"#, stuff.1.0, stuff.1.1),
                            );
                            agent_debug_log(
                                "H-F",
                                "work_controller.rs:internal_behavior",
                                "sending Replace to screen worker",
                                &format!(
                                    r#"{{"res":[{},{}],"mixmap_len":{}}}"#,
                                    stuff.1.0,
                                    stuff.1.1,
                                    context.random_map.len()
                                ),
                            );
                        }
                        WorkerCommand::Pan { .. } => {
                            agent_debug_log(
                                "H-F",
                                "work_controller.rs:internal_behavior",
                                "sending Pan to screen worker",
                                &format!(r#"{{"res":[{},{}]}}"#, stuff.1.0, stuff.1.1),
                            );
                        }
                    }
                    actor.try_send(&mut to_worker, cmd);
                }
                None => {
                    // #region agent log
                    agent_debug_log(
                        "H-F",
                        "work_controller.rs:internal_behavior",
                        "handle_sampler_stuff returned None (dedup)",
                        &format!(
                            r#"{{"res":[{},{}],"worker_res":[{},{}]}}"#,
                            stuff.1.0,
                            stuff.1.1,
                            state.worker_res.0,
                            state.worker_res.1
                        ),
                    );
                    // #endregion
                }
            }
        }
    }
    // Final shutdown log, reporting all statistics.
    info!("Computer shutting down.");
    Ok(())
}

use std::ops::*;
pub(crate) fn objective_to_worker_loc(obj: &ObjectivePosAndZoom) -> ((IntExp, IntExp), i64) {
    let loc: (IntExp, IntExp) = (obj.pos.0.clone(), obj.pos.1.clone());
    (
        (loc.0.clone(), IntExp::from(0) - loc.1.clone()),
        obj.zoom_pot as i64,
    )
}

pub(crate) fn get_points<T: From<f32> + Clone + From<IntExp> + Sub<Output=T> + Add<Output=T> + Mul<Output=T> + PartialOrd + crate::act::workshift::Finite + crate::act::workshift::Gt + crate::act::workshift::Abs + From<f32> + Into<f64> + Copy>
    (res: (u32, u32), loc:(IntExp, IntExp), zoom: i64) -> Vec<Point<T>> {
    let mut out:Vec<Point<T>> = Vec::with_capacity((res.0*res.1) as usize);

        let significant_res = PIXELS_PER_UNIT;//min(res.0, res.1);

        let real_center:T = loc.0.into();
        let imag_center:T = loc.1.into();


        let zoom_factor:IntExp;

        if zoom > 0 {
            zoom_factor = IntExp::from(1) >> (zoom as u32);
        } else {
            zoom_factor = IntExp::from(1) << ((-zoom) as u32);
        }

        for row in 0..res.1 {
            for seat in 0..res.0 {

                let row = row as f32;
                let seat = seat as f32;

                let point:(T, T) = (
                    /*(real_center + ((seat as f32 / significant_res as f32 - 0.5) / zoom_factor) as f64) as f32
                    , (imag_center + (-((row as f32 / significant_res as f32 - 0.5) / zoom_factor)) as f64) as f32*/
                    real_center + (T::from((seat / significant_res as f32)) * zoom_factor.clone().into())
                    , imag_center + (T::from(-((row / significant_res as f32))) * zoom_factor.clone().into())
                );

                out.push(
                    Point{
                        c: point.clone()
                        , z: point.clone()
                        , real_squared: 0.0.into()
                        , imag_squared: 0.0.into()
                        , real_imag: 0.0.into()
                        , iterations: 0
                        , loop_detection_point: ((0.0.into(), 0.0.into()), 0)
                        , escapes: false
                        , repeats: false
                        , delivered: false
                        , period: 0
                        , smallness_squared: 100.0.into()
                        , small_time:0
                    }
                )
            }
        }
    out
}




fn handle_sampler_stuff<T: Clone + From<f32> + From<f32> + Clone + From<IntExp> + Sub<Output=T> + Add<Output=T> + Mul<Output=T> + PartialOrd + crate::act::workshift::Finite + crate::act::workshift::Gt + crate::act::workshift::Abs + From<f32> + Into<f64> + Copy>(
    state: &mut WorkControllerState,
    stuff: (ObjectivePosAndZoom, (u32, u32)),
) -> Option<WorkerCommand<T>> {

    let obj = stuff.0.clone();
    let same_res_pan = state.worker_res == stuff.1
        && state.last_sampler_location.is_some()
        && obj.zoom_pot == state.zoom_pot as i32;

    if stuff.1.0 == 0 || stuff.1.1 == 0 {
        // #region agent log
        agent_debug_log(
            "H-B",
            "work_controller.rs:handle_sampler_stuff",
            "reject zero-sized resolution",
            &format!(r#"{{"res":[{},{}]}}"#, stuff.1.0, stuff.1.1),
        );
        // #endregion
        return None;
    }

    if let Some(loc) = state.last_sampler_location.clone() {
        if !((obj != loc) || stuff.1 != state.worker_res) {
            // #region agent log
            agent_debug_log(
                "H-F",
                "work_controller.rs:handle_sampler_stuff",
                "dedup skip same location and resolution",
                &format!(
                    r#"{{"res":[{},{}],"worker_res":[{},{}]}}"#,
                    stuff.1.0, stuff.1.1, state.worker_res.0, state.worker_res.1
                ),
            );
            // #endregion
            return None;
        }
    }

    if state.worker_res != stuff.1 {
        let pixel_count = (stuff.1.0 as u64) * (stuff.1.1 as u64);
        boot_trace::boot_reboot(
            "resize",
            &format!(
                r#"{{"old_res":[{},{}],"new_res":[{},{}],"pixels":{}}}"#,
                state.worker_res.0,
                state.worker_res.1,
                stuff.1.0,
                stuff.1.1,
                pixel_count
            ),
        );
        boot_trace::boot_once(
            "wc_mixmap_build_start",
            &format!(r#"{{"pixels":{}}}"#, pixel_count),
        );
        let mixmap_start = std::time::Instant::now();
        // #region agent log
        agent_debug_log(
            "H-A",
            "work_controller.rs:handle_sampler_stuff",
            "resolution change rebuilds mixmap via get_evenly_spaced_map",
            &format!(
                r#"{{"old_res":[{},{}],"new_res":[{},{}],"pixel_count":{}}}"#,
                state.worker_res.0,
                state.worker_res.1,
                stuff.1.0,
                stuff.1.1,
                pixel_count
            ),
        );
        // #endregion
        state.mixmap = get_evenly_spaced_map(pixel_count as usize);
        boot_trace::boot_span(
            "wc_mixmap_build_done",
            &format!(r#"{{"pixels":{}}}"#, pixel_count),
            mixmap_start.elapsed().as_millis(),
        );
        // #region agent log
        agent_debug_log(
            "H-A",
            "work_controller.rs:handle_sampler_stuff",
            "mixmap rebuilt",
            &format!(r#"{{"mixmap_len":{}}}"#, state.mixmap.len()),
        );
        // #endregion
    }

    state.worker_res = stuff.1;
    let (loc, zoom_pot) = objective_to_worker_loc(&obj);
    state.loc = loc;
    state.zoom_pot = zoom_pot;

    if same_res_pan {
        state.last_sampler_location = Some(obj);
        return Some(WorkerCommand::Pan { frame_info: stuff });
    }

    let zoomed = obj.zoom_pot > state.zoom_pot as i32;

    let mut edges = Vec::new();
    let mut linear_edge_map = Vec::new();
    let res = state.worker_res;
    for i in 0..(res.0-1) as i32 {
        linear_edge_map.push((i, 0))
    }
    for i in 0..(res.1-1) as i32 {
        linear_edge_map.push(((res.0-1) as i32, i))
    }
    for i in 0..(res.0) as i32 {
        linear_edge_map.push((i , (res.1-1) as i32))
    }
    for i in 1..(res.1-1) as i32 {
        linear_edge_map.push((0, i))
    }

    let length = linear_edge_map.len();
    let map = get_evenly_spaced_map(length);
    for i in 0..length {
        edges.push(linear_edge_map[map[i]]);
    }






    let work_context = WorkContext {
        points: get_points(stuff.1, state.loc.clone(), state.zoom_pot)
        , completed_points: vec!()
        , index: 0
        , random_index: 0
        , time_created: Instant::now()
        , time_workshift_started: Instant::now()
        , percent_completed: 0.0
        , random_map: state.mixmap.clone()
        , workshifts: 0u32
        , total_iterations: 0
        , spent_tokens_today: 0
        , total_iterations_today: 0
        , total_points_today: 0
        , total_bouts_today: 0
        , last_update: 0
        , res: state.worker_res
        , scredge_poses: VecDeque::from(edges)
        , edge_queue: VecDeque::new()
        , out_queue: VecDeque::new()
        , in_queue: VecDeque::new()
        , zoomed
        , attention: (0, 0)
    };
    state.last_sampler_location = Some(obj);
    // #region agent log
    agent_debug_log(
        "H-A",
        "work_controller.rs:handle_sampler_stuff",
        "built WorkContext for Replace",
        &format!(
            r#"{{"res":[{},{}],"points":{},"mixmap_len":{}}}"#,
            state.worker_res.0,
            state.worker_res.1,
            work_context.points.len(),
            work_context.random_map.len()
        ),
    );
    // #endregion
    Some(WorkerCommand::Replace {
        frame_info: stuff,
        context: work_context,
    })
}

/// Interleaved front / middle / back order on `0..length`.
/// O(n log n) via implicit treap; must not use `Vec::remove` (O(n²) at 384k).
pub(crate) fn get_evenly_spaced_map(length: usize) -> Vec<usize> {
    if length == 0 {
        return Vec::new();
    }
    let mut nodes: Vec<TreapNode> = (0..length)
        .map(|i| TreapNode {
            val: i,
            prio: (i.wrapping_mul(0x9E37_79B9) as u32) | 1,
            left: -1,
            right: -1,
            size: 1,
        })
        .collect();
    let mut root = -1i32;
    for i in 0..length {
        root = treap_insert(root, i as i32, &mut nodes);
    }
    let mut out = Vec::with_capacity(length);
    for step in 0..length {
        let len = treap_size(root, &nodes);
        let k = match step % 3 {
            0 => 0,
            1 => (len - 1) / 2,
            2 => len - 1,
            _ => unreachable!(),
        };
        let (v, new_root) = treap_erase_kth(root, k, &mut nodes);
        out.push(v);
        root = new_root;
    }
    out
}

#[derive(Clone, Copy)]
struct TreapNode {
    val: usize,
    prio: u32,
    left: i32,
    right: i32,
    size: usize,
}

fn treap_size(t: i32, nodes: &[TreapNode]) -> usize {
    if t < 0 {
        0
    } else {
        nodes[t as usize].size
    }
}

fn treap_pull(t: i32, nodes: &mut [TreapNode]) {
    if t < 0 {
        return;
    }
    let i = t as usize;
    let l = nodes[i].left;
    let r = nodes[i].right;
    nodes[i].size = 1 + treap_size(l, nodes) + treap_size(r, nodes);
}

fn treap_split(t: i32, k: usize, nodes: &mut [TreapNode]) -> (i32, i32) {
    if t < 0 {
        return (-1, -1);
    }
    let i = t as usize;
    let left_sz = treap_size(nodes[i].left, nodes);
    if k <= left_sz {
        let (a, b) = treap_split(nodes[i].left, k, nodes);
        nodes[i].left = b;
        treap_pull(t, nodes);
        (a, t)
    } else {
        let (a, b) = treap_split(nodes[i].right, k - left_sz - 1, nodes);
        nodes[i].right = a;
        treap_pull(t, nodes);
        (t, b)
    }
}

fn treap_merge(a: i32, b: i32, nodes: &mut [TreapNode]) -> i32 {
    if a < 0 {
        return b;
    }
    if b < 0 {
        return a;
    }
    let ai = a as usize;
    let bi = b as usize;
    if nodes[ai].prio > nodes[bi].prio {
        nodes[ai].right = treap_merge(nodes[ai].right, b, nodes);
        treap_pull(a, nodes);
        a
    } else {
        nodes[bi].left = treap_merge(a, nodes[bi].left, nodes);
        treap_pull(b, nodes);
        b
    }
}

fn treap_insert(root: i32, idx: i32, nodes: &mut [TreapNode]) -> i32 {
    let (left, right) = treap_split(root, idx as usize, nodes);
    let mid = idx;
    let left = treap_merge(left, mid, nodes);
    treap_merge(left, right, nodes)
}

fn treap_erase_kth(t: i32, k: usize, nodes: &mut [TreapNode]) -> (usize, i32) {
    let (left, rest) = treap_split(t, k, nodes);
    let (mid, right) = treap_split(rest, 1, nodes);
    let val = nodes[mid as usize].val;
    (val, treap_merge(left, right, nodes))
}

#[cfg(test)]
mod mixmap_tests {
    use super::get_evenly_spaced_map;

    fn get_evenly_spaced_map_reference(length: usize) -> Vec<usize> {
        let mut a: Vec<usize> = (0..length).collect();
        let mut b = Vec::with_capacity(length);
        for i in 0..length {
            match i % 3 {
                0 => b.push(a.remove(0)),
                1 => b.push(a.remove((a.len() - 1) / 2)),
                2 => b.push(a.remove(a.len() - 1)),
                _ => unreachable!(),
            }
        }
        b
    }

    #[test]
    fn evenly_spaced_matches_reference() {
        for length in [0, 1, 2, 5, 100, 1000, 384_000] {
            assert_eq!(
                get_evenly_spaced_map(length),
                get_evenly_spaced_map_reference(length),
                "length {length}"
            );
        }
    }
}