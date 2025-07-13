use steady_state::*;

use crate::actor::window::*;
use crate::actor::computer::*;


const DEFAULT_AVERAGE_COLOR:(u8,u8,u8) = (255, 0, 255);



pub(crate) struct Screen {
    real_center: String
    , imag_center: String
    , zoom: String
    , screen_height: u32
    , screen_width: u32
}

pub(crate) enum JobType {
    Mandelbrot
    , TrackPoint
    , Julia
}

// all jobs should be either completed or cancelled or time out
// The worker will run at a worker clock speed of 20tps or 50mspt
// that means it will split jobs to just under that size so it can be responsive

pub(crate) enum ZoomerJob {
    StartJob {
        job_type: JobType
        , job_id: u64
        , screen: Screen
        , minus_screens: Vec<Screen>
        , timeout: Duration
    }
    , CancelJob {
        job_id: u64
    }
}

pub(crate) struct ZoomerReport {
    pub(crate) actor_start: Instant,
    pub(crate) actor_wake: Instant,
    pub(crate) time_to_xyz: Vec<(String, Duration)>
}
pub(crate) struct PixelsForWindow {
    pub(crate) pixels: Vec<(u8, u8, u8)>
    , pub(crate) report: Option<ZoomerReport>
}

pub(crate) struct ZoomedScreen {
    pixels: Vec<(u8, u8, u8)>
    , zoom_power: i32
}



pub(crate) struct SamplingState {
    pub(crate) average_color: (u8, u8, u8),
    pub(crate) screen_collection: Vec<Vec<(u8, u8, u8)>>,
    pub(crate) viewport_position_real: String,
    pub(crate) viewport_position_imag: String,
    pub(crate) viewport_zoom: String,
    pub(crate) zoom_power_base: u8,
    pub(crate) window_res: (u32, u32)
}

async fn sample(
    commands: ZoomerCommandPackage,
    mut state: &mut Option<SamplingState>
) -> PixelsForWindow {


    // Initialize the actor's state, setting batch_size to half the generator channel's capacity.
    // This ensures that the producer can fill one half while the consumer processes the other.

    match(state) {
        Some(_) => {}
        None => {
            state = &mut Some(SamplingState {
                average_color: DEFAULT_AVERAGE_COLOR,
                screen_collection: vec!(),
                viewport_position_real: String::from("0"),
                viewport_position_imag: String::from("0"),
                viewport_zoom: String::from("1"),
                zoom_power_base: 2,
                window_res: (DEFAULT_WINDOW_RES.0, DEFAULT_WINDOW_RES.0)
            });
        }
    };

                for command in command_package.commands {
                    match command {
                        ZoomerCommand::DemandFrame{} => {
                            will_send_frame = true;
                        }
                        ZoomerCommand::SetAttention{pixel_x, pixel_y} => {
                            // send the jobs and stuff
                        }
                        ZoomerCommand::SetRes{hori, verti} => {
                            state.window_res = (hori, verti);
                        }
                        ZoomerCommand::ZoomClean{factor_power} => {
                        }
                        ZoomerCommand::SetZoomPowerBase{base} => {
                        }
                        ZoomerCommand::ZoomUnclean{factor} => {
                        }
                        ZoomerCommand::SetZoom{factor} => {
                        }
                        ZoomerCommand::MoveClean{pixels_x, pixels_y} => {
                        }
                        ZoomerCommand::SetPos{real, imag} => {
                        }
                        ZoomerCommand::TrackPoint{point_id, point_real, point_imag} => {
                        }
                        ZoomerCommand::UntrackPoint{point_id} => {
                        }
                        ZoomerCommand::UntrackAllPoints{} => {
                        }
                    }
                }

                if will_send_frame {
                    match command_package.bucket {
                        Some(mut bucket) => {

                            // sample frame
                            //bucket.into_iter().for_each(|p| p = state.average_color);
                            
                            for mut i in 0..bucket.len() {
                                bucket[i] = state.average_color;
                            }

                            // send frame
                            let response = PixelsForWindow{
                                pixels: bucket,
                                report: Some(
                                    ZoomerReport{
                                        actor_start: actor_start,
                                        actor_wake: actor_wake,
                                        time_to_xyz: Vec::from([
                                            (String::from("time to notice command:"), noticing_time)
                                            , (String::from("time to process command:"), my_code_start.elapsed())
                                        ])
                                    }
                                )
                            };

                            actor.try_send(&mut pixels_out, response);

                        }
                        None => {panic!("frame demanded, but no bucket provided. cannot follow command.")}
                    }
                }


            }
            None => {}//info!("no command package to process");}
        };
    }
    // Final shutdown log, reporting all statistics.
    info!("Transformer shutting down");
    Ok(())
}