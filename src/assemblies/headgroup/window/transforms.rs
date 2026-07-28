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

                context.location.zoom_pot += *pot; // r[impl cz.fast.natural-zoom-2x+1]
                // r[impl cz.ctrl.zoom-in-homothety+1]

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

                // Keep objective drag bookmark; resync screenspace so zoom-back works.
                // Screen Y matches seat Y (top-left origin); do not imag-flip.
                if let Some((objective, _)) = context.mouse_drag_start.clone() {
                    context.mouse_drag_start = Some((
                        objective
                        , egui::Pos2 {
                            x: center_screenspace_pos.0 as f32
                            , y: center_screenspace_pos.1 as f32
                        }
                    ));
                }

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

            ZoomerCommand::SetPos { real, imag } => {
                // Requirements: field location is viewport center.
                let screen = context.screen_size;
                let zoom = context.location.zoom_pot;
                context.location = crate::assemblies::headgroup::window::coords::ul_for_center(
                    real.clone()
                    , imag.clone()
                    , zoom
                    , screen
                );
                context.mouse_drag_start = None;
                context.updated = true;
            }
            ZoomerCommand::NavigateTo { .. } => {}
            ZoomerCommand::TrackPoint { point_id, point_real, point_imag } => {}
            ZoomerCommand::UntrackPoint { point_id } => {}
            ZoomerCommand::UntrackAllPoints {} => {}
        }
    }
}

#[cfg(test)]
mod zoom_tests {
    use super::*;
    use crate::assemblies::headgroup::window::sampling::SamplingContext;
    use crate::utils::ObjectivePosAndZoom;
    use std::collections::HashMap;

    fn ctx_at(zoom: i32) -> SamplingContext {
        SamplingContext {
            tiles: HashMap::new(),
            tile_gpu_ids: HashMap::new(),
            pending_tile_uploads: Vec::new(),
            next_tile_gpu_id: 0,
            reset_gpu_tile_slots: false,
            proximate_answers: true,
            unsent_answers: true,
            screen_size: (800, 480),
            location: ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO),
                zoom_pot: zoom,
            },
            updated: false,
            mouse_drag_start: None,
            memory_limit_bytes: 1_000_000_000,
            last_memory_bump: None,
            handle_filled: HashMap::new(),
        }
    }

    // r[verify cz.fast.natural-zoom-2x+1]
    #[test]
    fn one_bump_changes_zoom_pot_by_one() {
        let mut ctx = ctx_at(0);
        transform(
            vec![ZoomerCommand::Zoom {
                pot: 1,
                center_screenspace_pos: (400, 240),
            }],
            &mut ctx,
        );
        assert_eq!(ctx.location.zoom_pot, 1);
    }

    // r[verify cz.ctrl.zoom-in-homothety+1]
    #[test]
    fn zoom_in_increments_magnification_pot_by_one() {
        let mut ctx = ctx_at(2);
        transform(
            vec![ZoomerCommand::Zoom {
                pot: 1,
                center_screenspace_pos: (100, 50),
            }],
            &mut ctx,
        );
        assert_eq!(ctx.location.zoom_pot, 3);
    }

    // r[verify cz.ctrl.zoom-in-homothety+1]
    #[test]
    fn zoom_in_keeps_pointer_complex_fixed() {
        // Pointer-fixed: zoom in then out at the same screenspace point restores UL.
        let mut ctx = ctx_at(0);
        let pointer = (100, 50);
        let before = (
            ctx.location.pos.0.clone(),
            ctx.location.pos.1.clone(),
            ctx.location.zoom_pot,
        );
        transform(
            vec![
                ZoomerCommand::Zoom {
                    pot: 1,
                    center_screenspace_pos: pointer,
                },
                ZoomerCommand::Zoom {
                    pot: -1,
                    center_screenspace_pos: pointer,
                },
            ],
            &mut ctx,
        );
        assert_eq!(ctx.location.zoom_pot, before.2);
        assert_eq!(ctx.location.pos.0.round(2).val, before.0.round(2).val);
        assert_eq!(ctx.location.pos.1.round(2).val, before.1.round(2).val);
    }

    // r[verify cz.ctrl.zoom-in-homothety+1]
    #[test]
    fn zoom_out_is_inverse_of_zoom_in_at_center() {
        let mut ctx = ctx_at(0);
        let center = (400, 240);
        transform(
            vec![
                ZoomerCommand::Zoom { pot: 1, center_screenspace_pos: center },
                ZoomerCommand::Zoom { pot: -1, center_screenspace_pos: center },
            ],
            &mut ctx,
        );
        assert_eq!(ctx.location.zoom_pot, 0);
    }

    #[test]
    fn zoom_out_bump_decrements_pot() {
        let mut ctx = ctx_at(3);
        transform(
            vec![ZoomerCommand::Zoom {
                pot: -1,
                center_screenspace_pos: (400, 240),
            }],
            &mut ctx,
        );
        assert_eq!(ctx.location.zoom_pot, 2);
    }

    #[test]
    fn two_in_bumps_are_four_x() {
        let mut ctx = ctx_at(0);
        transform(
            vec![
                ZoomerCommand::Zoom {
                    pot: 1,
                    center_screenspace_pos: (400, 240),
                },
                ZoomerCommand::Zoom {
                    pot: 1,
                    center_screenspace_pos: (400, 240),
                },
            ],
            &mut ctx,
        );
        assert_eq!(ctx.location.zoom_pot, 2);
    }
}