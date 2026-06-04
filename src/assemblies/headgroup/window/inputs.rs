use std::cmp::min;
use eframe::emath::Pos2;
use rug::Integer;
use crate::assemblies::headgroup::window::{WindowState, ZoomerCommand};
use crate::assemblies::workgroup::work_controller::PIXELS_PER_UNIT_POT;
use crate::utils::{IntExp, ObjectivePosAndZoom};

use crate::assemblies::headgroup::window::sampling::*;

#[derive(Clone, Debug)]
pub(crate) struct MouseDragStart {
    pub(crate) objective_drag_start: ObjectivePosAndZoom
    ,
    pub(crate) screenspace_drag_start: Pos2
}

pub(crate) fn parse_inputs(ctx: &egui::Context, state: &mut WindowState, sampling_size: (usize, usize)) -> (Vec<ZoomerCommand>, (i32, i32)) {
    let settings = &state.controls_settings;

    let mut returned = (vec!(), (0, 0));

    let ppp = ctx.pixels_per_point();

    let min_size = min(state.size.x as u32, state.size.y as u32) as f32;

    ctx.input(|input_state| {
        if let Some(pos) = input_state.pointer.latest_pos() {
            returned.1 = (pos.x as i32, pos.y as i32);
        }

        // begin a new drag if neither of the buttons are held and one or both has just been pressed


        match &state.sampling_context.mouse_drag_start {
            Some(start) => {

                // end the current drag if appropriate
                if (!input_state.pointer.button_down(egui::PointerButton::Primary)) && (!input_state.pointer.button_down(egui::PointerButton::Middle)) {
                    state.sampling_context.mouse_drag_start = None;
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


        let scroll = input_state.raw_scroll_delta.y;

        if scroll != 0.0 {

            //info!("scrolling");

            let c = input_state.pointer.latest_pos().unwrap();

            let c = (
                c.x // * (1<<16) as f32 / min_size
                , c.y // * (1<<16) as f32 / min_size
            );

            returned.0.push(
                if scroll > 0.0 {
                    //info!("zooming in");
                    ZoomerCommand::Zoom {
                        pot: 1
                        ,
                        center_screenspace_pos: (c.0 as i32, c.1 as i32)
                    }
                } else {
                    //info!("zooming out");
                    ZoomerCommand::Zoom {
                        pot: -1
                        ,
                        center_screenspace_pos: (c.0 as i32, c.1 as i32)
                    }
                }
            );
        }


        if input_state.key_down(egui::Key::ArrowDown) {
            returned.0.push(ZoomerCommand::Move { pixels_x: 0, pixels_y: 1 });
        }
        if input_state.key_down(egui::Key::ArrowUp) {
            returned.0.push(ZoomerCommand::Move { pixels_x: 0, pixels_y: -1 });
        }
        if input_state.key_down(egui::Key::ArrowLeft) {
            returned.0.push(ZoomerCommand::Move { pixels_x: -1, pixels_y: 0 });
        }
        if input_state.key_down(egui::Key::ArrowRight) {
            returned.0.push(ZoomerCommand::Move { pixels_x: 1, pixels_y: 0 });
        }
    });

    returned
}