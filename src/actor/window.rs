use steady_state::*;
use eframe::{egui, NativeOptions};
//use eframe::Frame::raw_window_handle;
use egui_extras;
use winit::platform::x11::EventLoopBuilderExtX11; // For X11
//use winit::platform::wayland::EventLoopBuilderExtWayland; // For Wayland
//use winit::platform::windows::EventLoopBuilderExtWindows; // For Windows
use winit::event_loop::EventLoopBuilder;
use egui::{Color32, ColorImage, TextureHandle, Vec2, Pos2, ViewportInfo, viewport::*};
use winit::raw_window_handle::HasWindowHandle;
use winit::dpi::PhysicalPosition;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use std::collections::*;


use crate::actor::colorer::*;
use crate::actor::updater::*;
use crate::action::sampling::*;
use crate::action::settings::*;
use crate::action::rolling::*;



const RECOVER_EGUI_CRASHES:bool = false;
// ^ half implimented; in cases where the window is supposed to
// be minimized or not on top, it might bother the user by restarting.
//const MIN_FRAME_RATE:f64 = 20.0;
//const MAX_FRAME_TIME:f64 = 1.0 / MIN_FRAME_RATE;
const VSYNC:bool = false;

pub(crate) const DEFAULT_WINDOW_RES:(u32, u32) = (800, 480);

 pub(crate) const MIN_PIXELS:u32 = 1080; // min_pixels is prioritized over min_fps
pub(crate) const MIN_FPS:f32 = 50.0;

/// State struct for the window actor.

pub(crate) struct ZoomerState {
    pub(crate) settings_window_open: bool
}

pub(crate) struct ZoomerReport {
    pub(crate) actor_start: Instant,
    pub(crate) actor_wake: Instant,
    pub(crate) time_to_xyz: Vec<(String, Duration)>
}

pub(crate) enum ZoomerCommand {
    SetAttention{pixel_x:u32, pixel_y:u32}
    , ZoomClean{factor_power: i8}
    , SetZoomPowerBase{base: u8}
    , ZoomUnclean{factor: f32}
    , SetZoom{factor: String}
    , MoveClean{pixels_x: i32, pixels_y: i32}
    , SetPos{real: String, imag: String}
    , TrackPoint{point_id:u64, point_real: String, point_imag: String}
    , UntrackPoint{point_id:u64}
    , UntrackAllPoints
} pub(crate) const NUMBER_OF_COMMANDS:u16=10;

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
    , pub(crate) sampling_resolution_multiplier: f32
    , pub(crate) timer: Instant
}

/// Entry point for the window actor.
pub async fn run(
    actor: SteadyActorShadow,
    pixels_in: SteadyRx<ZoomerScreen>,
    state_out: SteadyTx<ZoomerUpdate>,
    buckets_out: SteadyTx<Vec<Vec<(u8,u8,u8)>>>,
    state: SteadyState<WindowState>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&pixels_in], [&state_out]),
        pixels_in,
        state_out,
        buckets_out,
        state,
    )
    .await
    // If it's testing, use test behavior instead
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    pixels_in: SteadyRx<ZoomerScreen>,
    state_out: SteadyTx<ZoomerUpdate>,
    buckets_out: SteadyTx<Vec<Vec<(u8,u8,u8)>>>,
    state: SteadyState<WindowState>,
) -> Result<(), Box<dyn Error>> {

    let mut portable_actor = Arc::new(Mutex::new(actor));

    let mut state = state.lock(|| WindowState{
        size: egui::vec2(DEFAULT_WINDOW_RES.0 as f32, DEFAULT_WINDOW_RES.1 as f32)
        , location: None
        , last_frame_period: None
        , buffers: vec!(vec!((Color32::BLACK);(DEFAULT_WINDOW_RES.0*DEFAULT_WINDOW_RES.1) as usize))
        , id_counter: 0
        , sampling_context: SamplingContext {
            used_screen: vec!(
                ZoomerScreen{
                    pixels: vec!((0,0,0);(DEFAULT_WINDOW_RES.0*DEFAULT_WINDOW_RES.1) as usize)
                    , screen_size: (DEFAULT_WINDOW_RES.0, DEFAULT_WINDOW_RES.1)
                    , zoom_factor: "1".to_string()
                    , location: ("0".to_string(), "0".to_string())
                    , state_revision: 0
                }
            ),
            unused_screen: vec!(
                ZoomerScreen{
                    pixels: vec!((0,0,0);(DEFAULT_WINDOW_RES.0*DEFAULT_WINDOW_RES.1) as usize)
                    , screen_size: (DEFAULT_WINDOW_RES.0, DEFAULT_WINDOW_RES.1)
                    , zoom_factor: "1".to_string()
                    , location: ("0".to_string(), "0".to_string())
                    , state_revision: 1
                }
            )
            , sampling_size: (DEFAULT_WINDOW_RES.0, DEFAULT_WINDOW_RES.1)
            /*world: None,
            viewport_position_real: "0",
            viewport_position_imag: "0",
            viewport_zoom: "1",
            zoom_power_base: 2,
            window_res: (DEFAULT_WINDOW_RES.0, DEFAULT_WINDOW_RES.1)*/
        }
        , settings_window_context: Arc::new(Mutex::new(DEFAULT_SETTINGS_WINDOW_CONTEXT))
        , settings_window_open: false
        , controls_settings: ControlsSettings::H
        , rolling_frame_info: (VecDeque::new(), VecDeque::new(), VecDeque::new(), None)
        , texturing_things: vec!()
        , sampling_resolution_multiplier: 1.0
        , timer: Instant::now()
    }).await;

    // with_decorations!!!!
    // with_fullscreen!!!!

    let viewport_options =
        egui::ViewportBuilder::default()
        .with_inner_size(state.size.clone())
            ;

    let mut viewport_options = match state.location {
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

    let mut portable_state = Arc::new(Mutex::new(state));

    let passthrough = EguiWindowPassthrough{
        portable_actor: portable_actor.clone()
        , pixels_in: pixels_in.clone()
        , state_out: state_out.clone()
        , buckets_out: buckets_out.clone()
        , portable_state: portable_state.clone()
    };

    eframe::run_native(
        "Critical Zoomer",
        options,
        Box::new(|_cc| Ok(Box::new(passthrough))),
    ).unwrap();


    let mut actor = portable_actor.lock().unwrap();
    let mut state_out = state_out.try_lock().unwrap();
    let mut pixels_in = pixels_in.try_lock().unwrap();
    let mut state = portable_state.lock().unwrap();

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
    state_out: SteadyTx<ZoomerUpdate>,
    buckets_out: SteadyTx<Vec<Vec<(u8,u8,u8)>>>,
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
        let mut state_out = self.state_out.try_lock().unwrap();
        let mut buckets_out = self.buckets_out.try_lock().unwrap();
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

            match (state.last_frame_period) {
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



            // go fast

            ctx.request_repaint();

            let size = ((state.size.x*state.sampling_resolution_multiplier) as usize, (state.size.y*state.sampling_resolution_multiplier) as usize);
            let pixels = size.0 * size.1;

            //let start = Instant::now();

            /*if state.buffers.len() == 0 {
                state.buffers.push(Vec::with_capacity(pixels));
            }*/

            let mut sampler_buffer = Vec::with_capacity(pixels);

            //info!("bucket length: {}", sampler_buffer.len());

            //info!("took {:.3}ms allocating a new bucket", start.elapsed().as_secs_f64()*1000.0);

            // prepare bucket

            //let start = Instant::now();

            /*if state.buffers[0].len() != pixels {
                state.buffers[0].resize(pixels, (Color32::BLACK));
            }*/

            //info!("took {:.3}ms resizing bucket", start.elapsed().as_secs_f64()*1000.0);

            //let start = Instant::now();

            // send colring bucket to colorer

            if state.sampling_context.unused_screen.len() > 0 {
                actor.try_send(&mut buckets_out, vec!(state.sampling_context.unused_screen.pop().unwrap().pixels));
            }

            //info!("took {:.3}ms sending bucket to colorer", start.elapsed().as_secs_f64()*1000.0);


            //info!("took {:.3}ms before updating sampling state", this_frame_start.elapsed().as_secs_f64()*1000.0);

            // update sampling state

            //let start = Instant::now();

            match actor.try_take(&mut pixels_in) {
                Some(p) => {
                    info!("window recieved pixels");
                    let old = state.sampling_context.used_screen.pop();
                    state.sampling_context.used_screen.push(p);
                    actor.try_send(
                        &mut buckets_out,
                        vec!(old.unwrap().pixels)
                    );
                }
                None => {}
            }
            //info!("took {:.3}ms updating sampling state", start.elapsed().as_secs_f64()*1000.0);
            //let start = Instant::now();
            // sample

            let command_package = ZoomerCommandPackage {
                start_time: Instant::now(),
                commands: vec!(),
            };

            //info!("took {:.3}ms cloning bucket", start.elapsed().as_secs_f64()*1000.0);
            //let start = Instant::now();


            state.sampling_context.sampling_size = (size.0 as u32, size.1 as u32);

            sample(command_package, &mut sampler_buffer, &mut state.sampling_context);

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

            //info!("took {:.3}ms texturing", start.elapsed().as_secs_f64()*1000.0);

            //info!("took {:.3}ms", start.elapsed().as_secs_f64()*1000.0);

            //let start = Instant::now();


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

                //info!("took {:.3}ms", start.elapsed().as_secs_f64()*1000.0);

                // Add a transparent text block in the top-left corner for debug info
                ui.put(
                    egui::Rect::from_min_size(
                        egui::pos2(10.0, 10.0),
                        egui::vec2(1000.0, 1000.0)
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

                                        if state.timer.elapsed().as_secs_f64() > 0.1 {
                                            state.timer = Instant::now();

                                            let fps:f64 = r.0.0 as f64 / 1000000000.0;
                                            let frametime:f64 = r.0.1.as_secs_f64()*1000.0;
                                            if fps < (MIN_FPS) as f64 {
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
                                            } else {
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

                                        }



                                    }
                                    None => {}
                                }

                                match rolling_frame_result.1 {
                                    Some(r) => {
                                        response += format!("fps:{:.1} / 1s low: {:.1}", r.0.0 as f64 / 1000000000.0, 1.0 / r.1.0.as_secs_f64()).as_str();
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
                        }
                        return button_state;
                    }
                );

                if state.settings_window_open {
                    let result = settings(&ctx, state.settings_window_context.clone());
                    state.settings_window_open = !result.will_close;
                }
            });

            // save current window position and size
            ctx.input(|input_state| {
                let info:ViewportInfo = input_state.raw.viewports.get(&input_state.raw.viewport_id).unwrap().clone();
                match info.outer_rect {
                    Some(r) => { state.location = Some(r.min); }
                    None => {}
                }
                match info.inner_rect {
                    Some(r) => {
                        state.size = r.size();
                        //state.sampling_state.window_res.0 = r.size().x as u32;
                        //state.sampling_state.window_res.0 = r.size().y as u32;
                    }
                    None => {}
                }
            });

            state.last_frame_period = Some(  (this_frame_start, Instant::now())  );

            //info!("took {:.3}ms after texturing", start.elapsed().as_secs_f64()*1000.0);
        }
        else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}