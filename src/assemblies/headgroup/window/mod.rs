use steady_state::*;
use eframe::{egui, NativeOptions};
//use eframe::Frame::raw_window_handle;
use winit::platform::x11::EventLoopBuilderExtX11; // For X11
//use winit::platform::wayland::EventLoopBuilderExtWayland; // For Wayland
//use winit::platform::windows::EventLoopBuilderExtWindows; // For Windows
use egui::{Color32, ColorImage, Pos2, TextureHandle, Vec2, ViewportInfo};
use std::error::Error;
use std::sync::{Arc, Mutex};

use std::collections::*;
use std::cmp::*;

use rug::*;

use crate::assemblies::shadergroup::colorer::*;
use crate::assemblies::workgroup::work_controller::*;

use crate::settings::*;
use crate::utils::*;
use crate::constants::*;
use crate::assemblies::headgroup::window::rolling::*;
use crate::assemblies::headgroup::window::widgetize::*;

use crate::assemblies::headgroup::window::inputs::*;
use crate::assemblies::headgroup::window::sampling::*;


pub(crate) mod rolling;
pub(crate) mod widgetize;
pub(crate) mod inputs;
pub(crate) mod sampling;
pub mod transforms;

const RECOVER_EGUI_CRASHES:bool = false;
// ^ half implimented; in cases where the window is supposed to
// be minimized or not on top, it might bother the user by restarting.
//const MIN_FRAME_RATE:f64 = 20.0;
//const MAX_FRAME_TIME:f64 = 1.0 / MIN_FRAME_RATE;
const VSYNC:bool = false;



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
    , pub(crate) controls_timer: Instant
}

/// Entry point for the window actor.
pub async fn run(
    actor: SteadyActorShadow,
    pixels_in: SteadyRx<ZoomerScreen>,
    sampler_out: SteadyTx<(ObjectivePosAndZoom, (u32, u32))>,
    settings_out: SteadyTxBundle<Settings,2>,
    attention_out: SteadyTx<(i32, i32)>,
    state: SteadyState<WindowState>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&pixels_in], [&sampler_out, &settings_out[0], &settings_out[1], &attention_out]),
        pixels_in,
        sampler_out,
        settings_out,
        attention_out,
        state,
    )
    .await
    // If it's testing, use test behavior instead
}

async fn internal_behavior<A: SteadyActor>(
    actor: A,
    pixels_in: SteadyRx<ZoomerScreen>,
    sampler_out: SteadyTx<(ObjectivePosAndZoom, (u32, u32))>,
    settings_out: SteadyTxBundle<Settings, 2>,
    attention_out: SteadyTx<(i32, i32)>,
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
            screen: None
            , screen_size: (DEFAULT_WINDOW_RES.0, DEFAULT_WINDOW_RES.1)
            , location: ObjectivePosAndZoom {
                pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1))
                , zoom_pot: HOME_POSITION.2
            }
            , updated: true
            , mouse_drag_start:None
        }
        , settings_window_context: Arc::new(Mutex::new(DEFAULT_SETTINGS_WINDOW_CONTEXT))
        , settings_window_open: false
        , controls_settings: ControlsSettings::H
        , rolling_frame_info: (VecDeque::new(), VecDeque::new(), VecDeque::new(), None)
        , texturing_things: vec!()
        , timer: Instant::now()
        , fps_margin: 0.0
        , timer2: Instant::now()
        , controls_timer: Instant::now()

    }).await;

    {
        let mut settings_state = state.settings_window_context.try_lock().unwrap();
        if settings_state.settings.coloring_script.is_none() {
            settings_state.settings.coloring_script = Some(DEFAULT_COLORING_SCRIPT.into());
        }
    }


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
        , attention_out: attention_out.clone()
        , portable_state: portable_state.clone()
    };

    eframe::run_native(
        "Critical Zoomer",
        options,
        Box::new(|_cc| Ok(Box::new(passthrough)))
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
    settings_out: SteadyTxBundle<Settings, 2>,
    attention_out: SteadyTx<(i32, i32)>,
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
        let settings_out = [
            self.settings_out[0].try_lock().unwrap()
            ,self.settings_out[1].try_lock().unwrap()
        ];
        let mut attention_out = self.attention_out.try_lock().unwrap();
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
                    update_sampling_context(&mut state.sampling_context, s);

                }
                None => {}
            }

            if state.sampling_context.screen.is_none() {
                for _ in 0..pixels {sampler_buffer.push(Color32::PURPLE)};
                //actor.try_send(&mut sampler_out, (state.sampling_context.relative_transforms.clone(), (state.size.x as u32, state.size.y as u32)));
            }

            if state.sampling_context.updated
            {
                actor.try_send(&mut sampler_out, (state.sampling_context.location.clone(), (state.size.x as u32, state.size.y as u32)));
                state.sampling_context.updated = false;
            }

            // sample

            let (command_package, attention) = parse_inputs(&ctx, &mut state, size);
            actor.try_send(&mut attention_out, attention);

            state.sampling_context.screen_size = (size.0 as u32, size.1 as u32);

            if state.sampling_context.screen.is_some() {
                sample(command_package, &mut sampler_buffer, &mut state.sampling_context);
            }

            let start = Instant::now();

            let image = ColorImage {
                size: [size.0, size.1],
                pixels: sampler_buffer,
                source_size: egui::vec2(size.0 as f32, size.1 as f32)
            };

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
                                match rolling_frame_result.2 {
                                    Some(r) => {
                                    }
                                    None => {}
                                }

                                match rolling_frame_result.1 {
                                    Some(r) => {
                                        response += format!("fps:{:.0} / 1s low: {:.1}", r.0.0 as f64 / 1000000000.0, 1.0 / r.1.0.as_secs_f64()).as_str();

                                    }
                                    None => {}
                                }

                                match rolling_frame_result.0 {
                                    Some(r) => {

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
                                pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1))
                                , zoom_pot: HOME_POSITION.2
                            };
                            state.sampling_context.updated = true;
                        }
                        return button_state;
                    }
                );



                if state.settings_window_open {
                    let result = settings(&ctx, state.settings_window_context.clone());
                    state.settings_window_open = !result.will_close;
                    for mut channel in settings_out {
                        actor.try_send(&mut channel, result.settings.clone());
                    }
                } else {
                    for mut channel in settings_out {
                        actor.try_send(&mut channel, state.settings_window_context.try_lock().unwrap().settings.clone());
                    }
                }
            });


            state.last_frame_period = Some(  (this_frame_start, Instant::now())  );

        }
        else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

