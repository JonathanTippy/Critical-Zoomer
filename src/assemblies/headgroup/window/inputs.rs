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

pub fn parse_inputs(
    ctx: &egui::Context
    , state: &mut WindowState
    , sampling_size: (usize, usize)
)
    -> (Vec<ZoomerCommand>, (i32, i32)) {

    let time_elapsed = state.controls_timer.elapsed();
    state.controls_timer = std::time::Instant::now();

    let settings = &state.controls_settings;

    let mut returned = (vec!(), (0, 0));

    let ppp = ctx.pixels_per_point();

    let min_size = min(state.size.x as u32, state.size.y as u32) as f32;

    ctx.input(|input_state| {
        if let Some(pos) = input_state.pointer.latest_pos() {
            returned.1 = ((pos.x as i32).clamp(0, sampling_size.0 as i32-1), (pos.y as i32).clamp(0, sampling_size.1 as i32-1));
        }

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

                    returned.0.push(
                        ZoomerCommand::MoveTo {
                            x: drag_start_pos.0 - objective_drag.0 - objective_offset.0
                            ,
                            y: drag_start_pos.1 + objective_drag.1 - objective_offset.1
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


        if delta != 0.0 && state.scroll_debt != 0.0 && delta.signum() != state.scroll_debt.signum() {

            state.scroll_debt = delta.signum() * SCROLL_SPEED/2.0;
        }
        state.scroll_debt += delta;


        let threshold = SCROLL_SPEED;
        while state.scroll_debt.abs() >= threshold {
            let step = state.scroll_debt.signum();


            //info!("scrolling");

            let c = input_state.pointer.latest_pos().unwrap();
            let center_screenspace_pos = (
                c.x as i32
                , (sampling_size.1 as i32 - 1) - (c.y as i32)
            );

            let pot = scroll_step_to_zoom_pot(step);
            returned.0.push(ZoomerCommand::Zoom {
                pot,
                center_screenspace_pos,
            });
            state.scroll_debt -= step * threshold;
            //state.scroll_debt = delta.signum() * SCROLL_SPEED / 2.0;
        }

        let pointer = input_state.pointer.latest_pos()
            .map(|p| (p.x as i32, p.y as i32))
            .unwrap_or((
                sampling_size.0 as i32 / 2
                , sampling_size.1 as i32 / 2
            ));
        let center_screenspace_pos = (
            pointer.0
            , (sampling_size.1 as i32 - 1) - pointer.1
        );
        if input_state.modifiers.shift && !state.shift_was_down {
            eprintln!("cz_key: Shift (zoomin)");
            // #region agent log
            crate::assemblies::headgroup::window::agent_dbg(
                "H-KEY"
                , "inputs.rs:shift"
                , "key_shift_zoomin"
                , "{\"key\":\"Shift\"}"
            );
            // #endregion
            // #region agent log
            crate::assemblies::headgroup::window::agent_dbg(
                "H-ZOOM-Y"
                , "inputs.rs:shift"
                , "zoom_center"
                , &format!(
                    "{{\"dir\":\"in\",\"pointer\":[{},{}],\"center\":[{},{}],\"screen\":[{},{}]}}"
                    , pointer.0
                    , pointer.1
                    , center_screenspace_pos.0
                    , center_screenspace_pos.1
                    , sampling_size.0
                    , sampling_size.1
                )
            );
            // #endregion
            returned.0.push(ZoomerCommand::Zoom {
                pot: 1
                , center_screenspace_pos
            });
        }
        if input_state.key_pressed(egui::Key::Space) {
            eprintln!("cz_key: Space (zoomout)");
            // #region agent log
            crate::assemblies::headgroup::window::agent_dbg(
                "H-KEY"
                , "inputs.rs:space"
                , "key_space_zoomout"
                , "{\"key\":\"Space\"}"
            );
            // #endregion
            returned.0.push(ZoomerCommand::Zoom {
                pot: -1
                , center_screenspace_pos
            });
        }
        if input_state.key_pressed(egui::Key::K) {
            eprintln!("cz_key: K probe");
            // #region agent log
            crate::assemblies::headgroup::window::agent_dbg(
                "H-KEY"
                , "inputs.rs:k"
                , "key_k_probe"
                , "{\"key\":\"K\"}"
            );
            // #endregion
        }
        state.shift_was_down = input_state.modifiers.shift;

        let small_edge = min(sampling_size.0, sampling_size.1);
        let pixels_per_second = small_edge as f32 * MOVE_SPEED_IN_SCREENS;

        let delta = pixels_per_second * (time_elapsed.as_secs_f64() as f32);

        let delta = IntExp{
            val: Integer::from((delta * 1024.0) as i32)
            , exp: -10
        };

        if input_state.key_down(egui::Key::S)
            && !input_state.key_pressed(egui::Key::S) {
            returned.0.push(ZoomerCommand::Move { pixels_x: IntExp::from(0), pixels_y: delta.clone() });
        }
        if input_state.key_down(egui::Key::W)
            && !input_state.key_pressed(egui::Key::W) {
            returned.0.push(ZoomerCommand::Move { pixels_x: IntExp::from(0), pixels_y: IntExp::from(0)-delta.clone() });
        }
        if input_state.key_down(egui::Key::A)
            && !input_state.key_pressed(egui::Key::A) {
            returned.0.push(ZoomerCommand::Move { pixels_x: IntExp::from(0)-delta.clone(), pixels_y: IntExp::from(0) });
        }
        if input_state.key_down(egui::Key::D)
            && !input_state.key_pressed(egui::Key::D) {
            returned.0.push(ZoomerCommand::Move { pixels_x: delta.clone(), pixels_y: IntExp::from(0) });
        }
    });

    returned
}

/// Map accumulated scroll step sign to zoom POT.
/// Empirically (egui raw_scroll_delta on this app): positive debt step was zooming
/// the wrong way relative to user expectation, so the sign is inverted vs naive
/// "positive y → zoom in". Shift/Space keys still use pot ±1 directly.
// r[impl cz.fast.natural-zoom-2x+1]
pub fn scroll_step_to_zoom_pot(step_sign: f32) -> i32 {
    if step_sign > 0.0 {
        -1
    } else {
        1
    }
}

#[cfg(test)]
mod scroll_zoom_tests {
    use super::*;

    // r[verify cz.fast.natural-zoom-2x+1]
    #[test]
    fn positive_scroll_step_zooms_out_on_this_stack() {
        assert_eq!(scroll_step_to_zoom_pot(1.0), -1);
    }

    // r[verify cz.fast.natural-zoom-2x+1]
    #[test]
    fn negative_scroll_step_zooms_in_on_this_stack() {
        assert_eq!(scroll_step_to_zoom_pot(-1.0), 1);
    }

    // r[verify cz.fast.natural-zoom-2x+1]
    #[test]
    fn scroll_bump_is_one_pot() {
        assert_eq!(scroll_step_to_zoom_pot(40.0).abs(), 1);
    }
}

