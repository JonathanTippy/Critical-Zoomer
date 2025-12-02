use steady_state::*;
use eframe::{egui, NativeOptions};
//use eframe::Frame::raw_window_handle;
use winit::platform::x11::EventLoopBuilderExtX11; // For X11
//use winit::platform::wayland::EventLoopBuilderExtWayland; // For Wayland
//use winit::platform::windows::EventLoopBuilderExtWindows; // For Windows
use egui::{Color32, ColorImage, TextureHandle, Vec2, Pos2, ViewportInfo};
use std::error::Error;
use std::sync::{Arc, Mutex};

use std::collections::*;
use std::cmp::*;

use rug::*;

use crate::actor::colorer::*;
use crate::actor::updater::*;
use crate::actor::work_controller::*;

use crate::action::sampling::*;
use crate::action::settings::*;
use crate::action::rolling::*;
use crate::action::utils::*;

const RECOVER_EGUI_CRASHES:bool = false;
// ^ half implimented; in cases where the window is supposed to
// be minimized or not on top, it might bother the user by restarting.
//const MIN_FRAME_RATE:f64 = 20.0;
//const MAX_FRAME_TIME:f64 = 1.0 / MIN_FRAME_RATE;
const VSYNC:bool = false;

pub(crate) const DEFAULT_WINDOW_RES:(u32, u32) = (800, 480);

pub(crate) const HOME_POSTION:(i32, i32, i32) = (-2, -2, -2);

 //pub(crate) const MIN_PIXELS:u32 = 40; // min_pixels is prioritized over min_fps and should be greater than ~6
//pub(crate) const MIN_FPS:f32 = 10.0;

/// State struct for the window actor.

pub(crate) struct ZoomerState {
    pub(crate) settings_window_open: bool
    , pub(crate) position: (String, String)
    , pub(crate) zoom: String
}

pub(crate) struct ZoomerReport {
    pub(crate) actor_start: Instant,
    pub(crate) actor_wake: Instant,
    pub(crate) time_to_xyz: Vec<(String, Duration)>
}

#[derive(Clone)]

pub(crate) enum ZoomerCommand {
    SetFocus{pixel_x:u32, pixel_y:u32}
    , SetZoom{pot: i32}
    , Zoom{pot: i32, center_screenspace_pos: (i32, i32)} // zoom in or out
    , Move{pixels_x: i32, pixels_y: i32}
    , MoveTo{x: IntExp, y: IntExp}
    , SetPos{real: IntExp, imag: IntExp}
    , TrackPoint{point_id:u64, point_real: IntExp, point_imag: IntExp}
    , UntrackPoint{point_id:u64}
    , UntrackAllPoints
} pub(crate) const NUMBER_OF_COMMANDS:u16=10;

#[derive(Clone)]

pub(crate) struct ZoomerCommandPackage {
    pub(crate) start_time: Instant
    , pub(crate) commands: Vec<ZoomerCommand>
}


#[derive(Clone)]
pub(crate) struct WindowState {
    pub(crate) size: Vec2
    , pub(crate) location: Option<Pos2>
    , pub(crate) last_frame_period: Option<(Instant, Instant)>
    , pub(crate) buffers: Vec<Vec<Color32>>
    , pub(crate) id_counter:u64
    , pub(crate) sampling_context: SamplingContext
    , pub(crate) settings_window_context: Arc<Mutex<SettingsWindowContext>>
    , pub(crate) settings_window_open: bool
    , pub(crate) controls_settings: ControlsSettings
    , pub(crate) rolling_frame_info: (
        VecDeque<(Instant, u64, Duration, Duration)>
        , VecDeque<(Instant, u64, Duration, Duration)>
        , VecDeque<(Instant, u64, Duration, Duration)>
        , Option<Instant>
    )
    , pub(crate) texturing_things: Vec<(TextureHandle, ColorImage, Vec<Color32>)>
    //, pub(crate) sampling_resolution_multiplier: f32
    , pub(crate) timer: Instant
    , pub(crate) fps_margin: f32
    , pub(crate) timer2: Instant
}

/// Entry point for the window actor.
pub async fn run(
    actor: SteadyActorShadow,
    pixels_in: SteadyRx<ZoomerScreen>,
    sampler_out: SteadyTx<(ObjectivePosAndZoom, (u32, u32))>,
    settings_out: SteadyTx<ZoomerSettingsUpdate>,
    state: SteadyState<WindowState>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&pixels_in], [&sampler_out, &settings_out]),
        pixels_in,
        sampler_out,
        settings_out,
        state,
    )
    .await
    // If it's testing, use test behavior instead
}

async fn internal_behavior<A: SteadyActor>(
    actor: A,
    pixels_in: SteadyRx<ZoomerScreen>,
    sampler_out: SteadyTx<(ObjectivePosAndZoom, (u32, u32))>,
    settings_out: SteadyTx<ZoomerSettingsUpdate>,
    state: SteadyState<WindowState>,
) -> Result<(), Box<dyn Error>> {

    let portable_actor = Arc::new(Mutex::new(actor));

    let state = state.lock(|| WindowState{
        size: egui::vec2(DEFAULT_WINDOW_RES.0 as f32, DEFAULT_WINDOW_RES.1 as f32)
        , location: None
        , last_frame_period: None
        , buffers: vec!(vec!(Color32::BLACK;(DEFAULT_WINDOW_RES.0*DEFAULT_WINDOW_RES.1) as usize))
        , id_counter: 0
        , sampling_context: SamplingContext {
            screens: vec!()
            , screen_size: (DEFAULT_WINDOW_RES.0, DEFAULT_WINDOW_RES.1)
            , location: ObjectivePosAndZoom {
                pos: (IntExp::from(HOME_POSTION.0), IntExp::from(HOME_POSTION.1))
                , zoom_pot: HOME_POSTION.2
            }
            , updated: true
            , mouse_drag_start:None
        }
        , settings_window_context: Arc::new(Mutex::new(DEFAULT_SETTINGS_WINDOW_CONTEXT))
        , settings_window_open: false
        , controls_settings: ControlsSettings::H
        , rolling_frame_info: (VecDeque::new(), VecDeque::new(), VecDeque::new(), None)
        , texturing_things: vec!()
        //, sampling_resolution_multiplier: 1.0
        , timer: Instant::now()
        , fps_margin: 0.0

        , timer2: Instant::now()

    }).await;

    // with_decorations!!!!
    // with_fullscreen!!!!

    let viewport_options =
        egui::ViewportBuilder::default()
        .with_inner_size(state.size.clone())
            ;

    let viewport_options = match state.location {
        Some(l) => {viewport_options.with_position(l)}
        None => {viewport_options}
    };

    let options = eframe::NativeOptions {
        event_loop_builder: Some(Box::new(|builder| {
            // Enable any_thread for X11 or Wayland
            #[cfg(target_os = "linux")]
            { builder.with_any_thread(true); }

        })),
        viewport: viewport_options,
        vsync: VSYNC,
        ..NativeOptions::default()


    };

    let portable_state = Arc::new(Mutex::new(state));

    let passthrough = EguiWindowPassthrough{
        portable_actor: portable_actor.clone()
        , pixels_in: pixels_in.clone()
        , sampler_out: sampler_out.clone()
        , settings_out: settings_out.clone()
        , portable_state: portable_state.clone()
    };

    eframe::run_native(
        "Critical Zoomer",
        options,
        Box::new(|_cc| Ok(Box::new(passthrough))),
    ).unwrap();


    let mut actor = portable_actor.lock().unwrap();
    let sampler_out = sampler_out.try_lock().unwrap();
    let pixels_in = pixels_in.try_lock().unwrap();
    let state = portable_state.lock().unwrap();

    //println!("state size final value: {}", state.size);


    if actor.is_running(
        || i!(true)
    ) {
        //warn!("Egui window loop stopped unexpectedly");
        //return Err((Box::from(EguiWindowError{})));
        if RECOVER_EGUI_CRASHES {
        panic!("Egui window loop stopped unexpectedly");
        } else {
            actor.request_shutdown().await;
        }
    }
    info!("Window shutting down");
    Ok(())
}


struct EguiWindowPassthrough<'a, A> {
    portable_actor: Arc<Mutex<A>>,
    pixels_in: SteadyRx<ZoomerScreen>,
    sampler_out: SteadyTx<(ObjectivePosAndZoom, (u32, u32))>,
    settings_out: SteadyTx<ZoomerSettingsUpdate>,
    portable_state:Arc<Mutex<StateGuard<'a, WindowState>>>
}

impl<A: SteadyActor> eframe::App for EguiWindowPassthrough<'_, A> {

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let this_frame_start = Instant::now();

        // min framerate
        //ctx.request_repaint_after(Duration::from_secs_f64(MAX_FRAME_TIME));

        // init hybrid actor
        let mut actor = self.portable_actor.lock().unwrap();
        let mut pixels_in = self.pixels_in.try_lock().unwrap();
        let mut sampler_out = self.sampler_out.try_lock().unwrap();
        let settings_out = self.settings_out.try_lock().unwrap();
        let mut state = self.portable_state.lock().unwrap();


        if actor.is_running(
            || i!(true)
        ) {

            // calculate framerate and frametime

            let timinginfo:Option<(Instant, u64, Duration, Duration)>;

            match state.rolling_frame_info.3 {
                Some(_) => {}
                None => {
                    state.rolling_frame_info.3 = Some(Instant::now());
                }
            }

            match state.last_frame_period {
                Some(p) => {
                    timinginfo = Some( (
                        p.0
                        , (1000000000*1000000000) / (this_frame_start-p.0).as_nanos() as u64
                        , this_frame_start-p.0
                        , p.1-p.0
                    ) );
                }
                None => {timinginfo = None}
            }

            // update rolling data & obtain rolling results


            let rolling_frame_result = rolling_frame_calc(
                &mut state.rolling_frame_info
                , timinginfo
            );


            // make sure we run every frame
            ctx.request_repaint();

            let size = (state.size.x as usize, state.size.y as usize);
            let pixels = size.0 * size.1;

            let mut sampler_buffer = Vec::with_capacity(pixels);

            match actor.try_take(&mut pixels_in) {
                Some(s) => {
                    //info!("window recieved pixels");
                    if s.pixels.len() == pixels {
                        update_sampling_context(&mut state.sampling_context, s);
                    } else {
                        info!("pixel length mismatch. expected {} got {}", pixels, s.pixels.len())
                    }
                }
                None => {}
            }

            if state.sampling_context.screens.len() == 0 {
                for _ in 0..pixels {sampler_buffer.push(Color32::PURPLE)};
                //actor.try_send(&mut sampler_out, (state.sampling_context.relative_transforms.clone(), (state.size.x as u32, state.size.y as u32)));
            }

            if state.sampling_context.updated
            {
                actor.try_send(&mut sampler_out, (state.sampling_context.location.clone(), (state.size.x as u32, state.size.y as u32)));
                state.sampling_context.updated = false;
            }

            // sample

            let command_package = parse_inputs(&ctx, &mut state, size);

            state.sampling_context.screen_size = (size.0 as u32, size.1 as u32);

            if state.sampling_context.screens.len() > 0 {
                sample(command_package, &mut sampler_buffer, &mut state.sampling_context);
            } /*else {
                for _ in 0..pixels {
                    sampler_buffer.push(Color32::PURPLE);
                }
            }*/



            /*if state.sampling_context.screens[0].complete
                && (
                    state.sampling_context.relative_transforms.pos != (0, 0)
                    || state.sampling_context.relative_transforms.zoom_pot != 0
                )
            {
                //info!("screen is complete, sending transform updates");
                if state.timer2.elapsed().as_secs_f64() > 0.1 {
                    actor.try_send(&mut sampler_out, state.sampling_context.relative_transforms.clone());
                    state.timer2 = Instant::now();
                }

            }*/

            //info!("took {:.3}ms sampling", start.elapsed().as_secs_f64()*1000.0);

            let start = Instant::now();

            let image = ColorImage {
                size: [size.0, size.1],
                pixels: sampler_buffer,
                source_size: egui::vec2(size.0 as f32, size.1 as f32)
            };

            //info!("took {:.3}ms color imaging", start.elapsed().as_secs_f64()*1000.0);

            //let start = Instant::now();

            let handle = ctx.load_texture(
                "pixel_texture",
                image,
                egui::TextureOptions::NEAREST,
            );


            egui::CentralPanel::default()
            .frame(egui::Frame {
                inner_margin: egui::Margin::same(0), // Remove margins
                //fill: egui::Color32::TRANSPARENT, // Transparent background
                ..Default::default()
            })
            .show(ctx, |ui|
            {

                ui.visuals_mut().override_text_color = Some(Color32::WHITE);

                let available_size = ui.available_size();

                //let start = Instant::now();

                ui.image((handle.id(), available_size));

                state.size = available_size;

                //info!("took {:.3}ms", start.elapsed().as_secs_f64()*1000.0);

                // Add a transparent text block in the top-left corner for debug info
                ui.put(
                    egui::Rect::from_min_size(
                        egui::pos2(10.0, 10.0),
                        egui::vec2(300.0, 240.0)
                    ),
                    |ui: &mut egui::Ui| {
                        // Set transparent background
                        ui.style_mut().visuals.panel_fill = egui::Color32::TRANSPARENT;

                        // Increase text size
                        ui.style_mut().text_styles.get_mut(&egui::TextStyle::Body).unwrap().size = 18.0;



                        let debug_text = match timinginfo {
                            Some(t) => {
                                let mut response = format!("");
                                /*let mut response = format!("debug\nframe data:\nfps: {:.2}\nframetime:{:.2}ms\n    from me:{:.2}ms\n    from egui: {:.2}ms\n"
                                        , t.1 as f64 / 1000000000.0
                                        , t.2.as_secs_f64()*1000.0
                                        , t.3.as_secs_f64()*1000.0
                                        , t.2.as_secs_f64()*1000.0-t.3.as_secs_f64()*1000.0
                                );*/


                                match rolling_frame_result.2 {
                                    Some(r) => {
                                        /*response += format!("100ms avg data:\nfps: {:.2}\nframetime:{:.2}ms\n    from me:{:.2}ms\n    from egui: {:.2}ms\n\n"
                                            , r.0.0 as f64 / 1000000000.0
                                            , r.0.1.as_secs_f64()*1000.0
                                            , r.0.2.as_secs_f64()*1000.0
                                            , r.0.1.as_secs_f64()*1000.0 - r.0.2.as_secs_f64()*1000.0
                                        ).as_str();*/
                                        /*response += format!("100ms low:\nfps: {:.2}\nframetime:{:.2}ms\n    from me:{:.2}ms\n    from egui: {:.2}ms\n"
                                                            , 1.0 / r.1.0.as_secs_f64()
                                                            , r.1.0.as_secs_f64()*1000.0
                                                            , r.1.1.as_secs_f64()*1000.0
                                                            , r.1.0.as_secs_f64()*1000.0 - r.1.1.as_secs_f64()*1000.0
                                        ).as_str();*/


                                        /*if state.timer.elapsed().as_secs_f64() > 0.1 {
                                            state.timer = Instant::now();

                                            let fps:f64 = r.0.0 as f64 / 1000000000.0;
                                            let frametime:f64 = r.0.1.as_secs_f64()*1000.0;
                                            if fps < (MIN_FPS - state.fps_margin) as f64 {
                                                let excess_frametime:f64 = frametime - ((1000.0/MIN_FPS) as f64);

                                                let size = ((state.size.x*state.sampling_resolution_multiplier) as usize, (state.size.y*state.sampling_resolution_multiplier) as usize);
                                                let pixels = (size.0 * size.1) as f64;

                                                let time_per_pixel:f64 = frametime/pixels;

                                                let excess_pixels:f64 = excess_frametime/time_per_pixel;

                                                let target_pixels:f64 = pixels - excess_pixels;

                                                let aspect_ratio:f64 = state.size.x as f64 / state.size.y as f64;

                                                let ypixels:f64 = (target_pixels/aspect_ratio).sqrt();

                                                let res_multiplier:f32 = (ypixels / (state.size.y as f64)) as f32;

                                                state.sampling_resolution_multiplier = res_multiplier;
                                            } else if fps > (MIN_FPS + state.fps_margin ) as f64 {
                                                let extra_frametime:f64 = ((1000.0/MIN_FPS) as f64)-frametime;

                                                let size = ((state.size.x*state.sampling_resolution_multiplier) as usize, (state.size.y*state.sampling_resolution_multiplier) as usize);
                                                let pixels = (size.0 * size.1) as f64;

                                                let time_per_pixel:f64 = frametime/pixels;

                                                let new_pixels:f64 = extra_frametime/time_per_pixel;///2.0; // extra slow when adding new pixels

                                                let target_pixels:f64 = pixels + new_pixels;

                                                let aspect_ratio:f64 = state.size.x as f64 / state.size.y as f64;

                                                let ypixels:f64 = (target_pixels/aspect_ratio).sqrt();

                                                let res_multiplier:f32 = (ypixels / (state.size.y as f64)) as f32;

                                                state.sampling_resolution_multiplier = res_multiplier;
                                            }

                                            let min_res_mult = std::cmp::max(5, MIN_PIXELS) as f32/std::cmp::min(state.size.x as u32, state.size.y as u32) as f32;

                                            if state.sampling_resolution_multiplier < min_res_mult {
                                                //info!("PROBLEM: sampling res can't be less than 0.001");
                                                state.sampling_resolution_multiplier = min_res_mult;
                                            }
                                            if state.sampling_resolution_multiplier > 1.0 {
                                                //info!("PROBLEM: sampling res can't be greater than 1.0");
                                                state.sampling_resolution_multiplier = 1.0;
                                            }

                                        }*/


                                    }
                                    None => {}
                                }

                                match rolling_frame_result.1 {
                                    Some(r) => {
                                        response += format!("fps:{:.0} / 1s low: {:.1}", r.0.0 as f64 / 1000000000.0, 1.0 / r.1.0.as_secs_f64()).as_str();




                                        /*
                                        response += format!("1s avg data:\nfps: {:.1}\nframetime:{:.1}ms\n    from me:{:.1}ms\n    from egui: {:.1}ms\n\n"
                                                            , r.0.0 as f64 / 1000000000.0
                                                            , r.0.1.as_secs_f64()*1000.0
                                                            , r.0.2.as_secs_f64()*1000.0
                                                            , r.0.1.as_secs_f64()*1000.0 - r.0.2.as_secs_f64()*1000.0
                                        ).as_str();
                                        */
                                        /*response += format!("1s low:\nfps: {:.2}\nframetime:{:.2}ms\n    from me:{:.2}ms\n    from egui: {:.2}ms\n"
                                                            , 1.0 / r.1.0.as_secs_f64()
                                                            , r.1.0.as_secs_f64()*1000.0
                                                            , r.1.1.as_secs_f64()*1000.0
                                                            , r.1.0.as_secs_f64()*1000.0 - r.1.1.as_secs_f64()*1000.0
                                        ).as_str();*/





                                        /*
                                        if r.0.0 as f64 / 1000000000.0 < (MIN_FPS).into() {
                                            state.sampling_resolution_multiplier
                                                = state.sampling_resolution_multiplier/1.001;
                                        }
                                        if r.0.0 as f64 / 1000000000.0 > (MIN_FPS).into() {
                                            state.sampling_resolution_multiplier
                                                = state.sampling_resolution_multiplier*1.001;
                                        }


                                        if r.0.0 as f64 / 1000000000.0 < (MIN_FPS-10.0).into() {
                                            state.sampling_resolution_multiplier
                                            = state.sampling_resolution_multiplier/1.01;
                                        } else {
                                            if r.0.0 as f64 / 1000000000.0 > (MIN_FPS+10.0).into() {
                                                state.sampling_resolution_multiplier
                                                = state.sampling_resolution_multiplier*1.01;
                                            }
                                        }*/

                                    }
                                    None => {}
                                }

                                match rolling_frame_result.0 {
                                    Some(r) => {
                                        //state.fps_margin = 10.0;
                                        /*response += format!("10s data:\nfps: {:.2}\nframetime:{:.2}ms\n    from me:{:.2}ms\n    from egui: {:.2}ms\n"
                                                            , r.0.0 as f64 / 1000000000.0
                                                            , r.0.1.as_secs_f64()*1000.0
                                                            , r.0.2.as_secs_f64()*1000.0
                                                            , r.0.1.as_secs_f64()*1000.0 - r.0.2.as_secs_f64()*1000.0
                                        ).as_str();*/
                                        /*response += format!("10s low:\nfps: {:.1}\nframetime:{:.1}ms\n    from me:{:.1}ms\n    from egui: {:.1}ms\n"
                                                            , 1.0 / r.1.0.as_secs_f64()
                                                            , r.1.0.as_secs_f64()*1000.0
                                                            , r.1.1.as_secs_f64()*1000.0
                                                            , r.1.0.as_secs_f64()*1000.0 - r.1.1.as_secs_f64()*1000.0
                                        ).as_str();*/
                                        response += format!(" / 10s low: {:.1}", 1.0 / r.1.0.as_secs_f64()).as_str();
                                    }
                                    None => {}
                                }




                                response
                            }
                            None => {
                                format!("debug\n")
                            }
                        };

                        // Create the debug text at the correct location and return the result
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            return ui.label(debug_text);
                        }).inner
                    }
                );

                // Add a gear icon button in the top-right corner
                ui.put(
                    egui::Rect::from_min_size(
                        egui::pos2(ui.available_width() - 40.0, 0.0),
                        egui::vec2(40.0, 40.0)
                    ),
                    |ui: &mut egui::Ui| {
                        // create button and get its state
                        let button_state = ui.button("⚙");
                        if button_state.clicked() {
                            state.settings_window_open = true;
                        }
                        return button_state;
                    }
                );

                // Add a home icon button in the top-right corner
                ui.put(
                    egui::Rect::from_min_size(
                        egui::pos2(ui.available_width() - 80.0, 0.0),
                        egui::vec2(40.0, 40.0)
                    ),
                    |ui: &mut egui::Ui| {
                        // create button and get its state
                        let button_state = ui.button("🏠");
                        if button_state.clicked() {
                            state.sampling_context.location = ObjectivePosAndZoom {
                                pos: (IntExp::from(HOME_POSTION.0), IntExp::from(HOME_POSTION.1))
                                , zoom_pot: HOME_POSTION.2
                            };
                            state.sampling_context.updated = true;
                        }
                        return button_state;
                    }
                );

                if state.settings_window_open {
                    let result = settings(&ctx, state.settings_window_context.clone());
                    state.settings_window_open = !result.will_close;
                }
            });
            //println!("hi 1");
            // save current window position and size
            /*
            ctx.input(|input_state| {
                //println!("hi 2");
                let info:ViewportInfo = input_state.raw.viewports.get(&input_state.raw.viewport_id).unwrap().clone();
                match info.outer_rect {
                    Some(r) => {
                        println!("hi 11");
                        state.location = Some(r.min);
                    }
                    None => {}
                }
                match info.inner_rect {
                    Some(r) => {
                        println!("hi 3");
                        state.size = r.size();
                        state.sampling_context.screen_size.0 = r.size().x as u32;
                        state.sampling_context.screen_size.1 = r.size().y as u32;
                        state.sampling_context.updated = true;
                        println!("changed res to {}x{}", r.size().x, r.size().y);
                    }
                    None => {
                        //println!("hi 4");
                    }
                }
                /*match info.minimized {
                    Some(m) => {
                        info!("minimized: {}", m);
                    }
                    None => {}
                }
                match info.focused {
                    Some(f) => {
                        info!("focused: {}", f);
                    }
                    None => {}
                }
                match info.maximized {
                    Some(f) => {
                        info!("maximized: {}", f);
                    }
                    None => {}
                }
                match info.fullscreen {
                    Some(f) => {
                        info!("fullscreen: {}", f);
                    }
                    None => {}
                }*/
            });*/

            state.last_frame_period = Some(  (this_frame_start, Instant::now())  );

            //info!("took {:.3}ms after texturing", start.elapsed().as_secs_f64()*1000.0);
        }
        else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MouseDragStart {
    pub(crate) objective_drag_start: ObjectivePosAndZoom
    , pub(crate) screenspace_drag_start: Pos2
}



fn parse_inputs(ctx:&egui::Context, state: &mut WindowState, sampling_size: (usize, usize)) -> Vec<ZoomerCommand> {


    let settings = &state.controls_settings;

    let mut returned = vec!();

    let ppp = ctx.pixels_per_point();

    let min_size = min(state.size.x as u32, state.size.y as u32) as f32;

    ctx.input(|input_state| {


        // begin a new drag if neither of the buttons are held and one or both has just been pressed
        if
        (input_state.pointer.primary_pressed() && (! input_state.pointer.button_down(egui::PointerButton::Middle)))
        || (input_state.pointer.button_pressed(egui::PointerButton::Middle) && (! input_state.pointer.primary_down())) {
            let d = input_state.pointer.latest_pos().unwrap();
            state.sampling_context.mouse_drag_start = Some(
                (state.sampling_context.location.clone()
                 , d
                )
            );
        }

        match &state.sampling_context.mouse_drag_start {
            Some(start) => {

                // end the current drag if appropriate
                if (!input_state.pointer.button_down(egui::PointerButton::Primary)) && (!input_state.pointer.button_down(egui::PointerButton::Middle)) {
                    state.sampling_context.mouse_drag_start = None;
                } else {
                    // execute the drag

                    let pos = input_state.pointer.latest_pos().unwrap();

                    // dragging should snap to pixels

                    //let min_size_recip = (1<<16) / min_size as i32;

                    let drag = (
                        (pos.x as i32 - start.1.x as i32)// * min_size_recip
                        , (pos.y as i32 - start.1.y as i32)// * min_size_recip
                    );

                    let drag_start_pos = start.0.pos.clone();

                    let objective_drag:(IntExp, IntExp) = (
                        IntExp{val:Integer::from(drag.0), exp:0}
                            .shift(-state.sampling_context.location.zoom_pot)
                            .shift(-PIXELS_PER_UNIT_POT)
                        , IntExp{val:Integer::from(drag.1), exp:0}
                            .shift(-state.sampling_context.location.zoom_pot)
                            .shift(-PIXELS_PER_UNIT_POT)
                        );

                    returned.push(
                        ZoomerCommand::MoveTo{
                            x: drag_start_pos.0 - objective_drag.0
                            , y: drag_start_pos.1 - objective_drag.1
                        }
                    );
                }
            }
            None => {}
        }

        let scroll = input_state.raw_scroll_delta.y;

        if scroll != 0.0 {

            //info!("scrolling");

            let c = input_state.pointer.latest_pos().unwrap();

            let c = (
                c.x// * (1<<16) as f32 / min_size
                , c.y// * (1<<16) as f32 / min_size
            );

            returned.push(
                if scroll > 0.0 {
                    //info!("zooming in");
                    ZoomerCommand::Zoom{
                        pot: 1
                        , center_screenspace_pos: (c.0 as i32, c.1 as i32)
                    }
                } else {
                    //info!("zooming out");
                    ZoomerCommand::Zoom{
                        pot: -1
                        , center_screenspace_pos: (c.0 as i32, c.1 as i32)
                    }
                }

            );
        }


        if input_state.key_down(egui::Key::ArrowDown) {
            returned.push(ZoomerCommand::Move{pixels_x: 0, pixels_y: 1});
        }
        if input_state.key_down(egui::Key::ArrowUp) {
            returned.push(ZoomerCommand::Move{pixels_x: 0, pixels_y: -1});
        }
        if input_state.key_down(egui::Key::ArrowLeft) {
            returned.push(ZoomerCommand::Move{pixels_x: -1, pixels_y: 0});
        }
        if input_state.key_down(egui::Key::ArrowRight) {
            returned.push(ZoomerCommand::Move{pixels_x: 1, pixels_y: 0});
        }
    });

    returned
}