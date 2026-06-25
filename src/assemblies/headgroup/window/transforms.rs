use rug::Integer;
use crate::assemblies::headgroup::window::sampling::{SamplingContext, ZoomerCommand};
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::intexp::*;

pub fn transform(
    mut command_package: Vec<ZoomerCommand>,
    sampling_context: &mut SamplingContext
) {
    let context = sampling_context;

    // handle commands

    for command in &mut command_package {
        match command {
            ZoomerCommand::SetFocus { pixel_x, pixel_y } => {}
            ZoomerCommand::Zoom { pot, center_screenspace_pos } => {
                /*let center_centered_pos = (
                    center_screenspace_pos.0 + (context.screen_size.0/2) as i32
                    , center_screenspace_pos.1 + (context.screen_size.1/2) as i32
                );*/

                // adjust position & zoom based on zooming in 3 steps
                // step 1: move to zoom center
                // step 2: zoom
                // step 3: move back so zoom center falls on same screenspace location

                let pixel_width = IntExp { val: Integer::from(1), exp: -context.location.zoom_pot }.shift(-PIXELS_PER_UNIT_POT);

                context.location.pos = (
                    context.location.pos.0.clone()
                        + IntExp { val: Integer::from(center_screenspace_pos.0), exp: -context.location.zoom_pot }.shift(-PIXELS_PER_UNIT_POT)
                        - (pixel_width.clone() >> 1)
                    , context.location.pos.1.clone()
                        + IntExp { val: Integer::from(center_screenspace_pos.1), exp: -context.location.zoom_pot }.shift(-PIXELS_PER_UNIT_POT)
                        - (pixel_width.clone() >> 1)
                );

                context.location.zoom_pot += *pot;

                let pixel_width = IntExp { val: Integer::from(1), exp: -context.location.zoom_pot }.shift(-PIXELS_PER_UNIT_POT);

                context.location.pos = (
                    context.location.pos.0.clone()
                        - IntExp { val: Integer::from(center_screenspace_pos.0), exp: -context.location.zoom_pot }.shift(-PIXELS_PER_UNIT_POT)
                        + (pixel_width.clone() >> 1)
                    , context.location.pos.1.clone()
                        - IntExp { val: Integer::from(center_screenspace_pos.1), exp: -context.location.zoom_pot }.shift(-PIXELS_PER_UNIT_POT)
                        + (pixel_width.clone() >> 1)
                );

                // round position to not be more precise than necessary

                if *pot < 0 {
                    context.location.pos = (
                        context.location.pos.0.clone().round((-*pot) as usize)
                        , context.location.pos.1.clone().round((-*pot) as usize)
                    );
                }


                // reset mouse drag start to the new screenspace location
                // theoretically this is not necessary as objective position
                // of mouse drag start will always remain attached to mouse
                // current position.
                // mouse screenspace position should be invariant under zoom
                // as the mouse's screenspace position is the zoom center.

                /*match &context.mouse_drag_start {
                    Some(d) => {
                        context.mouse_drag_start = Some(
                            (
                                /*ObjectivePosAndZoom{
                                    pos: context.location.pos.clone()
                                    , zoom_pot: context.location.zoom_pot
                                }*/
                                d.0.clone()
                                , egui::Pos2 {
                                x: center_screenspace_pos.0 as f32
                                , y: center_screenspace_pos.1 as f32
                            }
                            ));
                    }
                    None => {}
                }*/


                context.updated = true;
            }
            ZoomerCommand::SetZoom { pot } => {
                context.location.zoom_pot = *pot;
                context.updated = true;
            }
            ZoomerCommand::Move { pixels_x, pixels_y } => {
                context.location.pos = (
                    context.location.pos.0.clone() + pixels_x.clone().shift(-context.location.zoom_pot).shift(-PIXELS_PER_UNIT_POT)
                    , context.location.pos.1.clone() + pixels_y.clone().shift(-context.location.zoom_pot).shift(-PIXELS_PER_UNIT_POT)
                );
                context.updated = true;
            }
            ZoomerCommand::MoveTo { x, y } => {
                context.location.pos =
                    (x.clone(), y.clone());
                context.updated = true;
            }

            ZoomerCommand::SetPos { real, imag } => {}
            ZoomerCommand::TrackPoint { point_id, point_real, point_imag } => {}
            ZoomerCommand::UntrackPoint { point_id } => {}
            ZoomerCommand::UntrackAllPoints {} => {}
        }
    }
}