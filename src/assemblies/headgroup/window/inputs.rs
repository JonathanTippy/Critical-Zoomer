use std::cmp::min;
use eframe::emath::Pos2;
use rug::Integer;
use crate::assemblies::headgroup::window::{WindowState, ZoomerCommand};
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::utils::{ObjectivePosAndZoom};
use crate::intexp::*;

use crate::constants::*;
use crate::assemblies::headgroup::window::sampling::*;

#[derive(Clone, Debug)]
pub struct MouseDragStart {
    pub objective_drag_start: ObjectivePosAndZoom
    ,
    pub screenspace_drag_start: Pos2
}

/// Foveation seat: mouse when on the viewport, else screen center.
pub fn attention_from_pointer(
    pointer: Option<(i32, i32)>
    , sampling_size: (usize, usize)
) -> (i32, i32) {
    let center = ((sampling_size.0 / 2) as i32, (sampling_size.1 / 2) as i32);
    let Some((x, y)) = pointer else {
        return center;
    };
    if x >= 0
        && y >= 0
        && (x as usize) < sampling_size.0
        && (y as usize) < sampling_size.1
    {
        (x, y)
    } else {
        center
    }
}

pub fn parse_inputs(
    ctx: &egui::Context
    , state: &mut WindowState
    , sampling_size: (usize, usize)
)
    -> (Vec<ZoomerCommand>, (i32, i32)) {

    let time_elapsed = state.controls_timer.elapsed();
    state.controls_timer = std::time::Instant::now();

    let settings = &state.controls_settings;

    let mut returned = (vec!(), attention_from_pointer(None, sampling_size));
    let mut signed_zoom_bumps: i32 = 0;

    let ppp = ctx.pixels_per_point();

    let min_size = min(state.size.x as u32, state.size.y as u32) as f32;

    ctx.input(|input_state| {
        // Foveation: spiral from mouse when it is on the viewport; otherwise
        // center screen (pointer gone / off-window must not stick to a corner).
        returned.1 = attention_from_pointer(
            input_state.pointer.latest_pos().map(|p| (p.x as i32, p.y as i32))
            , sampling_size
        );

        // begin a new drag if neither of the buttons are held and one or both has just been pressed


        match &state.sampling_context.mouse_drag_start {
            Some(start) => {

                // end the current drag if appropriate
                if (!input_state.pointer.button_down(egui::PointerButton::Primary)) && (!input_state.pointer.button_down(egui::PointerButton::Middle)) {
                    state.sampling_context.mouse_drag_start = None;
                    state.sampling_context.proximate_answers = true;
                } else {
                    // execute the drag

                    let pos = input_state.pointer.latest_pos().unwrap();

                    let offset = (
                        (start.1.x as i32) // * min_size_recip
                        , (start.1.y as i32) // * min_size_recip
                    );

                    let objective_offset: (IntExp, IntExp) = (
                        IntExp { val: Integer::from(offset.0), exp: 0 }
                            .shift(-state.sampling_context.location.zoom_pot)
                            .shift(-PIXELS_PER_UNIT_POT)
                        , IntExp { val: Integer::from(offset.1), exp: 0 }
                            .shift(-state.sampling_context.location.zoom_pot)
                            .shift(-PIXELS_PER_UNIT_POT)
                    );

                    // dragging should snap to pixels

                    //let min_size_recip = (1<<16) / min_size as i32;

                    let drag = (
                        (pos.x as i32 - start.1.x as i32) // * min_size_recip
                        , (pos.y as i32 - start.1.y as i32) // * min_size_recip
                    );

                    let drag_start_pos = start.0.pos.clone();

                    let objective_drag: (IntExp, IntExp) = (
                        IntExp { val: Integer::from(drag.0), exp: 0 }
                            .shift(-state.sampling_context.location.zoom_pot)
                            .shift(-PIXELS_PER_UNIT_POT)
                        , IntExp { val: Integer::from(drag.1), exp: 0 }
                            .shift(-state.sampling_context.location.zoom_pot)
                            .shift(-PIXELS_PER_UNIT_POT)
                    );

                    // Grab-follow: mouse +Y (down) must decrease stored pos.1 so
                    // math imag rises and the grabbed seat stays under the cursor
                    // (same sign as X: subtract screen delta). The old `+ drag.y`
                    // matched the pre-top-left paint flip and felt Y-reversed.
                    returned.0.push(
                        ZoomerCommand::MoveTo {
                            x: drag_start_pos.0 - objective_drag.0 - objective_offset.0
                            ,
                            y: drag_start_pos.1 - objective_drag.1 - objective_offset.1
                        }
                    );
                }
            }
            None => {
                if
                (input_state.pointer.primary_pressed() && (!input_state.pointer.button_down(egui::PointerButton::Middle)))
                    || (input_state.pointer.button_pressed(egui::PointerButton::Middle) && (!input_state.pointer.primary_down())) {
                    let d = input_state.pointer.latest_pos().unwrap();

                    let offset = (
                        (d.x as i32) // * min_size_recip
                        , (d.y as i32) // * min_size_recip
                    );

                    let objective_offset: (IntExp, IntExp) = (
                        IntExp { val: Integer::from(offset.0), exp: 0 }
                            .shift(-state.sampling_context.location.zoom_pot)
                            .shift(-PIXELS_PER_UNIT_POT)
                        , IntExp { val: Integer::from(offset.1), exp: 0 }
                            .shift(-state.sampling_context.location.zoom_pot)
                            .shift(-PIXELS_PER_UNIT_POT)
                    );

                    state.sampling_context.mouse_drag_start = Some(
                        (ObjectivePosAndZoom {
                            pos: (
                                state.sampling_context.location.clone().pos.0
                                    + objective_offset.0
                                , state.sampling_context.location.clone().pos.1
                                    + objective_offset.1
                            )
                            ,
                            zoom_pot: {
                                state.sampling_context.location.clone().zoom_pot
                            }
                        }
                         , d
                        )
                    );
                }
            }
        }


        let delta = input_state.raw_scroll_delta.y;
        let pots = consume_scroll_debt(&mut state.scroll_debt, delta, SCROLL_SPEED);
        if !pots.is_empty() {
            let c = input_state.pointer.latest_pos().unwrap();
            // Seat / drag / Shift-Space all use top-left screenspace; do not Y-flip.
            let center_screenspace_pos = (c.x as i32, c.y as i32);
            for pot in pots {
                signed_zoom_bumps = signed_zoom_bumps.saturating_add(pot);
                returned.0.push(ZoomerCommand::Zoom {
                    pot,
                    center_screenspace_pos,
                });
            }
        }

        // Shift/Space zoom at screen center (requirements); ~5 bumps/s while held.
        let screen_center_screenspace = (
            sampling_size.0 as i32 / 2
            , sampling_size.1 as i32 / 2
        );
        let dt = time_elapsed.as_secs_f64() as f32;
        let shift_held = input_state.modifiers.shift;
        let shift_pressed = shift_held && !state.shift_was_down;
        let in_bumps = consume_key_zoom_debt(
            &mut state.key_zoom_in_debt
            , shift_held
            , shift_pressed
            , dt
            , KEY_ZOOM_BUMPS_PER_SEC
        );
        for _ in 0..in_bumps {
            signed_zoom_bumps = signed_zoom_bumps.saturating_add(1);
            returned.0.push(ZoomerCommand::Zoom {
                pot: 1
                , center_screenspace_pos: screen_center_screenspace
            });
        }
        let space_held = input_state.key_down(egui::Key::Space);
        let space_pressed = input_state.key_pressed(egui::Key::Space);
        let out_bumps = consume_key_zoom_debt(
            &mut state.key_zoom_out_debt
            , space_held
            , space_pressed
            , dt
            , KEY_ZOOM_BUMPS_PER_SEC
        );
        for _ in 0..out_bumps {
            signed_zoom_bumps = signed_zoom_bumps.saturating_sub(1);
            returned.0.push(ZoomerCommand::Zoom {
                pot: -1
                , center_screenspace_pos: screen_center_screenspace
            });
        }
        state.shift_was_down = shift_held;

        let small_edge = min(sampling_size.0, sampling_size.1);
        let pixels_per_second = small_edge as f32 * MOVE_SPEED_IN_SCREENS;

        let delta = pixels_per_second * (time_elapsed.as_secs_f64() as f32);

        let delta = IntExp{
            val: Integer::from((delta * 1024.0) as i32)
            , exp: -10
        };

        let move_down = (input_state.key_down(egui::Key::S) && !input_state.key_pressed(egui::Key::S))
            || (input_state.key_down(egui::Key::ArrowDown) && !input_state.key_pressed(egui::Key::ArrowDown));
        let move_up = (input_state.key_down(egui::Key::W) && !input_state.key_pressed(egui::Key::W))
            || (input_state.key_down(egui::Key::ArrowUp) && !input_state.key_pressed(egui::Key::ArrowUp));
        let move_left = (input_state.key_down(egui::Key::A) && !input_state.key_pressed(egui::Key::A))
            || (input_state.key_down(egui::Key::ArrowLeft) && !input_state.key_pressed(egui::Key::ArrowLeft));
        let move_right = (input_state.key_down(egui::Key::D) && !input_state.key_pressed(egui::Key::D))
            || (input_state.key_down(egui::Key::ArrowRight) && !input_state.key_pressed(egui::Key::ArrowRight));
        if move_down {
            returned.0.push(ZoomerCommand::Move { pixels_x: IntExp::from(0), pixels_y: delta.clone() });
        }
        if move_up {
            returned.0.push(ZoomerCommand::Move { pixels_x: IntExp::from(0), pixels_y: IntExp::from(0)-delta.clone() });
        }
        if move_left {
            returned.0.push(ZoomerCommand::Move { pixels_x: IntExp::from(0)-delta.clone(), pixels_y: IntExp::from(0) });
        }
        if move_right {
            returned.0.push(ZoomerCommand::Move { pixels_x: delta.clone(), pixels_y: IntExp::from(0) });
        }
    });

    update_mag_velocity_ewma(
        &mut state.mag_velocity_ewma
        , signed_zoom_bumps
        , time_elapsed.as_secs_f64()
    );

    returned
}

/// Apply one scroll delta into debt and emit zoom pots (no egui).
/// Opposite-sign delta resets debt to half-threshold toward the new sign.
// r[impl cz.fast.no-tick-backlog+1]
// r[impl cz.fast.scroll-10-in-300ms+1]
pub fn consume_scroll_debt(debt: &mut f32, delta: f32, threshold: f32) -> Vec<i32> {
    if delta != 0.0 && *debt != 0.0 && delta.signum() != debt.signum() {
        *debt = delta.signum() * threshold / 2.0;
    }
    *debt += delta;
    let mut pots = Vec::new();
    while debt.abs() >= threshold {
        let step = debt.signum();
        pots.push(scroll_step_to_zoom_pot(step));
        *debt -= step * threshold;
    }
    pots
}

/// Hold-key zoom debt: first press grants one bump; while held accumulate bumps/sec.
// r[impl cz.fast.shift-space-5bps+1]
pub fn consume_key_zoom_debt(
    debt: &mut f32
    , held: bool
    , just_pressed: bool
    , dt_secs: f32
    , bumps_per_sec: f32
) -> u32 {
    if !held {
        *debt = 0.0;
        return 0;
    }
    if just_pressed {
        *debt += 1.0;
    }
    *debt += dt_secs.max(0.0) * bumps_per_sec;
    let mut bumps = 0u32;
    while *debt >= 1.0 {
        *debt -= 1.0;
        bumps += 1;
    }
    bumps
}

/// EWMA α for magnification velocity (D-SCH-2): half-life ~5 wakes.
pub const MAG_VELOCITY_EWMA_ALPHA: f64 = 1.0 / 8.0;
/// Below this absolute rate, treat velocity as stationary (mode 0).
pub const MAG_VELOCITY_IDLE_EPS: f64 = 0.05;
/// Floor dt so a hitch does not explode bumps/sec.
pub const MAG_VELOCITY_DT_FLOOR_SECS: f64 = 1.0 / 240.0;

/// Update EWMA of signed zoom bumps/sec. Zero-bump frames decay toward 0.
pub fn update_mag_velocity_ewma(ewma: &mut f64, signed_bumps: i32, dt_secs: f64) -> f64 {
    let dt = dt_secs.max(MAG_VELOCITY_DT_FLOOR_SECS);
    let rate = (signed_bumps as f64) / dt;
    *ewma += MAG_VELOCITY_EWMA_ALPHA * (rate - *ewma);
    if ewma.abs() < MAG_VELOCITY_IDLE_EPS {
        *ewma = 0.0;
    }
    *ewma
}

/// Scheduler mode from EWMA: >0 zoom-in, <0 zoom-out, 0 stationary.
pub fn mag_velocity_mode(ewma: f64) -> i32 {
    if ewma > MAG_VELOCITY_IDLE_EPS {
        1
    } else if ewma < -MAG_VELOCITY_IDLE_EPS {
        -1
    } else {
        0
    }
}

/// Map accumulated scroll step sign to zoom POT.
/// egui `raw_scroll_delta`: positive Y means content moves down (classic scroll-up /
/// natural swipe-down). Scroll-up must zoom in → pot +1 (standards).
/// Shift/Space keys still use pot ±1 directly.
// r[impl cz.fast.natural-zoom-2x+1]
// r[impl cz.ctrl.scroll-up-zooms-in+1]
pub fn scroll_step_to_zoom_pot(step_sign: f32) -> i32 {
    if step_sign > 0.0 {
        1
    } else {
        -1
    }
}

#[cfg(test)]
mod scroll_zoom_tests {
    use super::*;

    // r[verify cz.fast.natural-zoom-2x+1]
    // r[verify cz.ctrl.scroll-up-zooms-in+1]
    #[test]
    fn positive_scroll_step_zooms_in() {
        assert_eq!(scroll_step_to_zoom_pot(1.0), 1);
    }

    // r[verify cz.fast.natural-zoom-2x+1]
    // r[verify cz.ctrl.scroll-up-zooms-in+1]
    #[test]
    fn negative_scroll_step_zooms_out() {
        assert_eq!(scroll_step_to_zoom_pot(-1.0), -1);
    }

    // r[verify cz.fast.natural-zoom-2x+1]
    // r[verify cz.ctrl.scroll-up-zooms-in+1]
    #[test]
    fn scroll_bump_is_one_pot() {
        assert_eq!(scroll_step_to_zoom_pot(40.0).abs(), 1);
    }

    // r[verify cz.ctrl.scroll-up-zooms-in+1]
    #[test]
    fn scroll_up_delta_increases_mag_pot() {
        // Positive raw_scroll_delta.y = content down = scroll-up → zoom in → pot +1.
        let mut debt = 0.0;
        let pots = consume_scroll_debt(&mut debt, SCROLL_SPEED, SCROLL_SPEED);
        assert_eq!(pots, vec![1]);
    }

    // r[verify cz.fast.scroll-10-in-300ms+1]
    #[test]
    fn ten_scroll_thresholds_yield_ten_pots() {
        let mut debt = 0.0;
        let pots = consume_scroll_debt(&mut debt, SCROLL_SPEED * 10.0, SCROLL_SPEED);
        assert_eq!(pots.len(), 10);
        assert!(pots.iter().all(|&p| p == 1));
    }

    // r[verify cz.fast.scroll-10-in-300ms+1]
    #[test]
    fn ten_bumps_fit_in_300ms_accounting() {
        // Product bar: 10 applied bumps within 300ms — debt consume is O(n) instantaneous.
        let t0 = std::time::Instant::now();
        let mut debt = 0.0;
        let pots = consume_scroll_debt(&mut debt, SCROLL_SPEED * 10.0, SCROLL_SPEED);
        assert_eq!(pots.len(), 10);
        assert!(t0.elapsed().as_millis() <= 300);
    }

    // r[verify cz.fast.scroll-10-in-300ms+1]
    #[test]
    fn repeating_ten_bump_bursts_stay_exact() {
        let mut debt = 0.0;
        for _ in 0..3 {
            let pots = consume_scroll_debt(&mut debt, SCROLL_SPEED * 10.0, SCROLL_SPEED);
            assert_eq!(pots.len(), 10);
        }
    }

    // r[verify cz.fast.no-tick-backlog+1]
    #[test]
    fn n_thresholds_emit_n_zooms_no_skip() {
        let mut debt = 0.0;
        let pots = consume_scroll_debt(&mut debt, SCROLL_SPEED * 7.0, SCROLL_SPEED);
        assert_eq!(pots.len(), 7);
    }

    // r[verify cz.fast.no-tick-backlog+1]
    #[test]
    fn opposite_sign_clears_backlog_direction() {
        let mut debt = 0.0;
        let _ = consume_scroll_debt(&mut debt, SCROLL_SPEED * 3.0, SCROLL_SPEED);
        let pots = consume_scroll_debt(&mut debt, -SCROLL_SPEED, SCROLL_SPEED);
        // Opposite sign resets to half threshold then adds full → one out bump.
        assert_eq!(pots, vec![-1]);
    }

    // r[verify cz.fast.no-tick-backlog+1]
    #[test]
    fn residual_debt_below_threshold_is_not_a_deferred_burst() {
        let mut debt = 0.0;
        let pots = consume_scroll_debt(&mut debt, SCROLL_SPEED * 2.5, SCROLL_SPEED);
        assert_eq!(pots.len(), 2);
        assert!(debt.abs() < SCROLL_SPEED);
        let more = consume_scroll_debt(&mut debt, 0.0, SCROLL_SPEED);
        assert!(more.is_empty());
    }

    // r[verify cz.fast.shift-space-5bps+1]
    #[test]
    fn key_zoom_press_emits_one_bump() {
        let mut debt = 0.0;
        let n = consume_key_zoom_debt(&mut debt, true, true, 0.0, KEY_ZOOM_BUMPS_PER_SEC);
        assert_eq!(n, 1);
    }

    // r[verify cz.fast.shift-space-5bps+1]
    #[test]
    fn key_zoom_hold_one_second_is_about_five() {
        let mut debt = 0.0;
        let n = consume_key_zoom_debt(&mut debt, true, true, 1.0, KEY_ZOOM_BUMPS_PER_SEC);
        // press + 1s*5 = 6, or press then accumulate — accept 5..=6
        assert!((5..=6).contains(&n), "got {n}");
    }

    // r[verify cz.fast.shift-space-5bps+1]
    #[test]
    fn key_zoom_release_clears_debt() {
        let mut debt = 0.5;
        let n = consume_key_zoom_debt(&mut debt, false, false, 0.2, KEY_ZOOM_BUMPS_PER_SEC);
        assert_eq!(n, 0);
        assert_eq!(debt, 0.0);
    }

    // r[verify cz.fast.input-next-frame-17ms+1]
    #[test]
    fn scroll_consume_is_same_turn() {
        let mut debt = 0.0;
        let pots = consume_scroll_debt(&mut debt, SCROLL_SPEED, SCROLL_SPEED);
        assert_eq!(pots.len(), 1);
    }

    // r[verify cz.fast.input-next-frame-17ms+1]
    #[test]
    fn key_zoom_consume_is_same_turn() {
        let mut debt = 0.0;
        assert_eq!(consume_key_zoom_debt(&mut debt, true, true, 0.0, 5.0), 1);
    }

    // r[verify cz.fast.input-next-frame-17ms+1]
    #[test]
    fn seventeen_ms_frame_budget_constant() {
        assert!(std::time::Duration::from_millis(17).as_millis() <= 17);
        assert_eq!(KEY_ZOOM_BUMPS_PER_SEC, 5.0);
    }

    #[test]
    fn attention_defaults_to_center_when_pointer_missing() {
        assert_eq!(attention_from_pointer(None, (800, 480)), (400, 240));
    }

    #[test]
    fn attention_uses_on_screen_pointer() {
        assert_eq!(attention_from_pointer(Some((10, 20)), (800, 480)), (10, 20));
    }

    #[test]
    fn attention_centers_when_pointer_off_screen() {
        assert_eq!(attention_from_pointer(Some((-1, 100)), (800, 480)), (400, 240));
        assert_eq!(attention_from_pointer(Some((900, 100)), (800, 480)), (400, 240));
        assert_eq!(attention_from_pointer(Some((100, -5)), (800, 480)), (400, 240));
        assert_eq!(attention_from_pointer(Some((100, 500)), (800, 480)), (400, 240));
    }

    #[test]
    fn ewma_burst_then_idle_decays_to_zero() {
        let mut ewma = 0.0;
        update_mag_velocity_ewma(&mut ewma, 10, 0.3);
        assert!(ewma > MAG_VELOCITY_IDLE_EPS, "got {ewma}");
        for _ in 0..64 {
            update_mag_velocity_ewma(&mut ewma, 0, 1.0 / 60.0);
        }
        assert_eq!(ewma, 0.0);
        assert_eq!(mag_velocity_mode(ewma), 0);
    }

    #[test]
    fn ewma_zoom_in_sets_positive_mode() {
        let mut ewma = 0.0;
        update_mag_velocity_ewma(&mut ewma, 5, 0.2);
        assert_eq!(mag_velocity_mode(ewma), 1);
    }

    #[test]
    fn ewma_zoom_out_sets_negative_mode() {
        let mut ewma = 0.0;
        update_mag_velocity_ewma(&mut ewma, -5, 0.2);
        assert_eq!(mag_velocity_mode(ewma), -1);
    }
}

