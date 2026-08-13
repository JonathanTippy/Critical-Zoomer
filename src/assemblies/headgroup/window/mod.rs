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

use crate::assemblies::structs::*;
use crate::assemblies::headgroup::window::inputs::*;
use crate::assemblies::headgroup::window::sampling::*;
use crate::assemblies::headgroup::window::coords::*;
use crate::assemblies::headgroup::window::transforms::transform;


pub mod rolling;
pub mod widgetize;
pub mod inputs;
pub mod sampling;
pub mod transforms;
pub mod coords;
pub mod snip;
pub mod gaze;

const RECOVER_EGUI_CRASHES:bool = false;
// ^ half implimented; in cases where the window is supposed to
// be minimized or not on top, it might bother the user by restarting.
//const MIN_FRAME_RATE:f64 = 20.0;
//const MAX_FRAME_TIME:f64 = 1.0 / MIN_FRAME_RATE;
const VSYNC: bool = true; // GL swap Wait. Does not by itself stop 100% CPU
                          // while update() calls bare request_repaint.



 //pub const MIN_PIXELS:u32 = 40; // min_pixels is prioritized over min_fps and should be greater than ~6
//pub const MIN_FPS:f32 = 10.0;

/// State struct for the window actor.

pub struct ZoomerState {
    pub settings_window_open: bool
    , pub position: (String, String)
    , pub zoom: String
}

pub struct ZoomerReport {
    pub actor_start: Instant,
    pub actor_wake: Instant,
    pub time_to_xyz: Vec<(String, Duration)>
}



pub struct ZoomerCommandPackage {
    pub start_time: Instant
    , pub commands: Vec<ZoomerCommand>
}


#[derive(Clone)]
pub struct WindowState {
    pub size: Vec2
    , pub location: Option<Pos2>
    , pub last_frame_period: Option<(Instant, Instant)>
    , pub buffers: Vec<Vec<Color32>>
    , pub id_counter:u64
    , pub sampling_context: SamplingContext
    , pub settings_window_context: Arc<Mutex<SettingsWindowContext>>
    , pub settings_window_open: bool
    , pub controls_settings: ControlsSettings
    , pub rolling_frame_info: (
        VecDeque<(Instant, u64, Duration, Duration)>
        , VecDeque<(Instant, u64, Duration, Duration)>
        , VecDeque<(Instant, u64, Duration, Duration)>
        , Option<Instant>
    )
    , pub texturing_things: Vec<(TextureHandle, ColorImage, Vec<Color32>)>
    //, pub sampling_resolution_multiplier: f32
    , pub timer: Instant
    , pub fps_margin: f32
    , pub timer2: Instant
    , pub controls_timer: Instant
    , pub stencil_serial_number_counter: u64
    , pub scroll_debt: f32
    , pub coord_input: String
    , pub startup_goto_applied: bool
    // r[impl cz.depth.gear-hud+2]
    , pub pps_counter: RateCounter
    , pub ips_counter: RateCounter
    , pub publisher_fps_counter: RateCounter
    , pub escape_fps_counter: RateCounter
    , pub color_fps_counter: RateCounter
    , pub controller_fps_counter: RateCounter
    , pub last_gear_label: &'static str
    , pub last_stack_label: &'static str
    , pub last_mode_label: &'static str
    , pub last_ref_label: &'static str
    , pub last_color_label: &'static str
    , pub last_escape_label: &'static str
    , pub last_packages_dropped: u64
    , pub last_ipp: u32
    , pub last_ipp_final: bool
    // Last auto_vsync_hz value fanned to content actors.
    , pub last_fanned_auto_vsync_hz: f64
    // Fan Settings only when UI/cadence actually changed.
    , pub settings_fanout_needed: bool
    // Cap live settings preview fan-out while the panel is open.
    , pub settings_ui_fan_timer: Instant
    // Reuse GPU texture when no new View arrived this frame.
    , pub display_texture: Option<TextureHandle>
    , pub last_attention: AttentionFocus
    , pub gaze: gaze::GazeSession
    // Last stencil location+res actually sent (skip duplicate Replace).
    , pub last_sent_stencil_key: Option<(ObjectivePosAndZoom, (usize, usize))>
}

/// Entry point for the window actor.
pub async fn run(
    actor: SteadyActorShadow,
    pixels_in: SteadyRx<View<Color32>>,
    stencil_out: SteadyTx<(PointStencil)>,
    settings_out: SteadyTxBundle<Settings,4>,
    attention_out: SteadyTx<AttentionFocus>,
    state: SteadyState<WindowState>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&pixels_in], [&stencil_out, &settings_out[0], &settings_out[1], &settings_out[2], &settings_out[3], &attention_out]),
        pixels_in,
        stencil_out,
        settings_out,
        attention_out,
        state,
    )
    .await
    // If it's testing, use test behavior instead
}

async fn internal_behavior<A: SteadyActor>(
    actor: A,
    pixels_in: SteadyRx<View<Color32>>,
    stencil_out: SteadyTx<(PointStencil)>,
    settings_out: SteadyTxBundle<Settings, 4>,
    attention_out: SteadyTx<AttentionFocus>,
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
        , stencil_serial_number_counter: 0
        , scroll_debt: SCROLL_SPEED/2.0
        , coord_input: String::new()
        , startup_goto_applied: false
        , pps_counter: RateCounter::default()
        , ips_counter: RateCounter::default()
        , publisher_fps_counter: RateCounter::default()
        , escape_fps_counter: RateCounter::default()
        , color_fps_counter: RateCounter::default()
        , controller_fps_counter: RateCounter::default()
        , last_gear_label: "naive"
        , last_stack_label: "f64"
        , last_mode_label: "naive"
        , last_ref_label: "NA"
        , last_color_label: Settings::DEFAULT.resolved_color_gear().manual_gear_label()
        , last_escape_label: Settings::DEFAULT.resolved_escape_gear().manual_gear_label()
        , last_packages_dropped: 0
        , last_ipp: 0
        , last_ipp_final: false
        , last_fanned_auto_vsync_hz: Settings::DEFAULT.auto_vsync_hz
        , settings_fanout_needed: true
        , settings_ui_fan_timer: Instant::now()
        , display_texture: None
        , last_attention: AttentionFocus::default()
        , gaze: gaze::GazeSession::new()
        , last_sent_stencil_key: None
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
        , stencil_out: stencil_out.clone()
        , settings_out: settings_out.clone()
        , attention_out: attention_out.clone()
        , portable_state: portable_state.clone()
    };

    eframe::run_native(
        "Critical Zoomer",
        options,
        Box::new(|_cc| Ok(Box::new(passthrough)))
    )?;


    let mut actor = portable_actor.lock().unwrap();
    let sampler_out = stencil_out.try_lock().unwrap();
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
    pixels_in: SteadyRx<View<Color32>>,
    stencil_out: SteadyTx<(PointStencil)>,
    settings_out: SteadyTxBundle<Settings, 4>,
    attention_out: SteadyTx<AttentionFocus>,
    portable_state:Arc<Mutex<StateGuard<'a, WindowState>>>
}

impl<A: SteadyActor> eframe::App for EguiWindowPassthrough<'_, A> {

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let _cpu = crate::debug_agent::busy_window();
        let this_frame_start = Instant::now();

        // min framerate
        //ctx.request_repaint_after(Duration::from_secs_f64(MAX_FRAME_TIME));

        // init hybrid actor
        let mut actor = self.portable_actor.lock().unwrap();
        let mut pixels_in = self.pixels_in.try_lock().unwrap();
        let mut stencil_out = self.stencil_out.try_lock().unwrap();
        let settings_out = [
            self.settings_out[0].try_lock().unwrap()
            ,self.settings_out[1].try_lock().unwrap()
            ,self.settings_out[2].try_lock().unwrap()
            ,self.settings_out[3].try_lock().unwrap()
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


            // Bare request_repaint (351afdf). Head ~100% CPU still open.
            // Do not treat this as vsync pacing.
            ctx.request_repaint();

            let mut got_new_view = false;

            let size = (state.size.x as usize, state.size.y as usize);
            let pixels = size.0 * size.1;

            let mut sampler_buffer = Vec::with_capacity(pixels);

            // Drain-to-newest: window is not the collector — keep the tip only.
            let avail_views = actor.avail_units(&mut pixels_in);
            if avail_views > 0 {
                let (drops, take) =
                    crate::assemblies::shadergroup::escaper::take_newest_plan(avail_views);
                for _ in 0..drops {
                    let _ = actor.try_take(&mut pixels_in);
                }
                if take {
                    match actor.try_take(&mut pixels_in) {
                        Some(s) => {
                            got_new_view = true;
                            let now = Instant::now();
                            state.pps_counter.record(s.hud.points_delta, now);
                            state.ips_counter.record(s.hud.iterations_delta, now);
                            if let Some(at) = s.hud.publisher_emitted_at {
                                state.publisher_fps_counter.record(1, at);
                            }
                            if let Some(at) = s.hud.escape_emitted_at {
                                state.escape_fps_counter.record(1, at);
                            }
                            if let Some(at) = s.hud.color_emitted_at {
                                state.color_fps_counter.record(1, at);
                            }
                            if let Some(at) = s.hud.controller_emitted_at {
                                state.controller_fps_counter.record(1, at);
                            }
                            state.last_gear_label = s.hud.mode.hud_label();
                            state.last_stack_label = s.hud.stack.hud_label();
                            state.last_mode_label = s.hud.mode.hud_label();
                            state.last_ref_label = s.hud.ref_hud_label();
                            state.last_color_label = s.hud.color.hud_label();
                            state.last_escape_label = s.hud.escape.hud_label();
                            state.last_packages_dropped = s.hud.packages_dropped;
                            state.last_ipp = s.hud.ipp;
                            state.last_ipp_final = s.hud.ipp_final;
                            update_sampling_context(&mut state.sampling_context, s);
                        }
                        None => {}
                    }
                }
            }

            if state.sampling_context.updated
            {
                let key = (
                    state.sampling_context.location.clone(),
                    (state.size.x as usize, state.size.y as usize),
                );
                // Duplicate stencils (same loc/res) must not Replace — that restarts
                // work. Exact key only; continuum still flows Views/attention/settings.
                if state.last_sent_stencil_key.as_ref() != Some(&key) {
                    actor.try_send(&mut stencil_out, PointStencil{
                        location: (state.sampling_context.location.pos.0.clone()
                        , IntExp::ZERO-state.sampling_context.location.pos.1.clone()
                        , state.sampling_context.location.zoom_pot.clone()
                        )
                        , resolution: key.1
                        , serial_number: state.stencil_serial_number_counter
                    });
                    state.stencil_serial_number_counter +=1;
                    state.last_sent_stencil_key = Some(key);
                }
                state.sampling_context.updated = false;
            }

            // sample

            let (eye_on, request_cal) = if let Ok(mut ctx_settings) =
                state.settings_window_context.try_lock()
            {
                let on = ctx_settings.settings.eye_tracking_enabled;
                let req = ctx_settings.settings.request_gaze_calibrate;
                if req {
                    ctx_settings.settings.request_gaze_calibrate = false;
                }
                (on, req)
            } else {
                (false, false)
            };
            state.gaze.set_enabled(eye_on);
            if eye_on && request_cal {
                state.gaze.begin_calibrate();
            }

            let (mut command_package, pointer) = parse_inputs(&ctx, &mut state, size);
            let gaze = state.gaze.tick((size.0 as f32, size.1 as f32));
            let attention = AttentionFocus { pointer, gaze };
            // Same attention value need not resend; changing attention still flows.
            if attention != state.last_attention {
                actor.try_send(&mut attention_out, attention);
                state.last_attention = attention;
            }

            if !state.startup_goto_applied {
                if let Ok(line) = std::env::var("CZ_GOTO") {
                    if !line.trim().is_empty() {
                        if let Some(cmds) = commands_from_goto_line(&line) {
                            command_package.extend(cmds);
                        }
                    }
                }
                state.startup_goto_applied = true;
            }
            let goto_path = std::env::var("CZ_GOTOFILE")
                .unwrap_or_else(|_| "/tmp/cz_ctl.goto".to_string());
            if let Ok(line) = std::fs::read_to_string(&goto_path) {
                let _ = std::fs::remove_file(&goto_path);
                if let Some(cmds) = commands_from_goto_line(&line) {
                    command_package.extend(cmds);
                }
            }

            state.sampling_context.screen_size = (size.0 as u32, size.1 as u32);

            let need_resample = got_new_view
                || !command_package.is_empty()
                || state.display_texture.is_none()
                || state.sampling_context.screen.is_none();

            if need_resample {
                if state.sampling_context.screen.is_some() {
                    sample(command_package, &mut sampler_buffer, &mut state.sampling_context);
                } else if !command_package.is_empty() {
                    transform(command_package, &mut state.sampling_context);
                } else if state.sampling_context.screen.is_none() {
                    for _ in 0..pixels {
                        sampler_buffer.push(Color32::PURPLE);
                    }
                }

                crate::assemblies::headgroup::window::snip::maybe_write_viewport_snip(
                    size,
                    &sampler_buffer,
                );

                let image = ColorImage {
                    size: [size.0, size.1],
                    pixels: sampler_buffer,
                    source_size: egui::vec2(size.0 as f32, size.1 as f32),
                };
                state.display_texture = Some(ctx.load_texture(
                    "pixel_texture",
                    image,
                    egui::TextureOptions::NEAREST,
                ));
            } else if !command_package.is_empty() {
                transform(command_package, &mut state.sampling_context);
            }

            let handle = state
                .display_texture
                .clone()
                .expect("display texture set above");


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
                        egui::vec2(560.0, 200.0)
                    ),
                    |ui: &mut egui::Ui| {
                        // Set transparent background
                        ui.style_mut().visuals.panel_fill = egui::Color32::TRANSPARENT;

                        // Increase text size
                        ui.style_mut().text_styles.get_mut(&egui::TextStyle::Body).unwrap().size = 16.0;



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
                                        let now = Instant::now();
                                        let pps = state.pps_counter.rate(now);
                                        let ips = state.ips_counter.rate(now);
                                        let pub_1s = state.publisher_fps_counter.rate(now);
                                        let pub_10s = state.publisher_fps_counter.rate_10s(now);
                                        let esc_1s = state.escape_fps_counter.rate(now);
                                        let esc_10s = state.escape_fps_counter.rate_10s(now);
                                        let col_1s = state.color_fps_counter.rate(now);
                                        let col_10s = state.color_fps_counter.rate_10s(now);
                                        let ctrl_1s = state.controller_fps_counter.rate(now);
                                        let ctrl_10s = state.controller_fps_counter.rate_10s(now);
                                        let ipp_txt = if state.last_ipp_final {
                                            format!("ipp:{}", state.last_ipp)
                                        } else {
                                            format!("ipp:~{}", state.last_ipp)
                                        };
                                        let ten_s = rolling_frame_result.0.map(|r| {
                                            1.0 / r.1.0.as_secs_f64()
                                        });
                                        // r[impl cz.depth.gear-hud+2]
                                        let ref_hud = if state.last_mode_label == "pert" {
                                            Some(state.last_ref_label)
                                        } else {
                                            None
                                        };
                                        response += format_hud_overlay(
                                            1.0 / r.1.0.as_secs_f64(),
                                            ten_s,
                                            pub_1s,
                                            pub_10s,
                                            esc_1s,
                                            esc_10s,
                                            col_1s,
                                            col_10s,
                                            ctrl_1s,
                                            ctrl_10s,
                                            state.last_gear_label,
                                            state.last_stack_label,
                                            ref_hud,
                                            state.last_color_label,
                                            state.last_escape_label,
                                            pps,
                                            ips,
                                            &ipp_txt,
                                        )
                                        .as_str();
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

                if let Some(toast) = state.gaze.toast_text() {
                    let sampling = (size.0 as f32, size.1 as f32);
                    let calibrating = matches!(state.gaze.phase, gaze::GazePhase::Calibrating { .. });
                    egui::Area::new(egui::Id::new("gaze_toast"))
                        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 12.0))
                        .order(egui::Order::Foreground)
                        .show(ctx, |ui| {
                            egui::Frame::popup(ui.style())
                                .inner_margin(egui::Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.label(toast);
                                    if calibrating {
                                        if ui.button("Yup, doing it").clicked() {
                                            state.gaze.confirm_pose(sampling);
                                        }
                                    }
                                });
                        });
                }
                if let Some(corner) = state.gaze.active_corner() {
                    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), state.size);
                    let painter = ctx.layer_painter(egui::LayerId::new(
                        egui::Order::Foreground,
                        egui::Id::new("gaze_corner"),
                    ));
                    paint_gaze_corner_mark(&painter, rect, corner);
                }

                egui::Area::new(egui::Id::new("coord_bar"))
                    .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-8.0, -8.0))
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style())
                            .inner_margin(egui::Margin::symmetric(8, 6))
                            .show(ui, |ui| {
                                ui.set_min_width(520.0);
                                let screen = (
                                    state.size.x.max(1.0) as u32
                                    , state.size.y.max(1.0) as u32
                                );
                                let (cre, cim) = viewport_center(
                                    &state.sampling_context.location
                                    , screen
                                );
                                let center_text = format_location_readout(
                                    &cre
                                    , &cim
                                    , state.sampling_context.location.zoom_pot
                                );
                                ui.horizontal(|ui| {
                                    ui.label("location");
                                    let mut readonly = center_text.clone();
                                    ui.add(
                                        egui::TextEdit::singleline(&mut readonly)
                                            .desired_width(320.0)
                                            .interactive(true)
                                    );
                                    if ui.button("Copy").clicked() {
                                        write_location_clipboard(&center_text);
                                        ui.ctx().copy_text(center_text);
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("goto");
                                    let response = ui.add(
                                        egui::TextEdit::singleline(&mut state.coord_input)
                                            .desired_width(240.0)
                                            .hint_text("re, im  or  a+bi")
                                    );
                                    if response.gained_focus() && state.coord_input.is_empty() {
                                        if let Some(text) = read_location_clipboard() {
                                            let trimmed = text.trim().to_string();
                                            if goto_line_is_valid(&trimmed) {
                                                state.coord_input = trimmed;
                                            }
                                        }
                                    }
                                    if ui.button("Paste").clicked() {
                                        if let Some(text) = read_location_clipboard() {
                                            state.coord_input = text.trim().to_string();
                                        }
                                    }
                                    let valid = goto_line_is_valid(&state.coord_input);
                                    let apply = ui.add_enabled(valid, egui::Button::new("Apply"));
                                    let go = apply.clicked()
                                        || (valid
                                            && response.lost_focus()
                                            && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                                    if go {
                                        if let Some(cmds) = commands_from_goto_line(&state.coord_input) {
                                            transform(cmds, &mut state.sampling_context);
                                            state.sampling_context.updated = true;
                                        }
                                    }
                                });
                            });
                    });

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
                    let closing = settings(&ctx, state.settings_window_context.clone());
                    state.settings_window_open = !closing;
                    // Preview fan ≤10 Hz while open; always fan on close.
                    if closing
                        || state.settings_ui_fan_timer.elapsed()
                            >= Duration::from_millis(100)
                    {
                        state.settings_fanout_needed = true;
                        state.settings_ui_fan_timer = Instant::now();
                    }
                }
                // Aim Automatic content cadence at egui's predicted vsync period.
                // Never rewrite from measured present FPS — that jittered timers.
                // Never create a second winit EventLoop to probe the monitor —
                // that poisons eframe with WinitEventLoop(RecreationAttempt).
                // Hysteresis ≥2 Hz vs last fan: predicted_dt ±1 Hz noise must not
                // rewrite content timers every present.
                let mut snap_to_fan = None;
                let mut mark_fanout = false;
                let predicted_dt = ctx.input(|i| i.predicted_dt);
                let egui_hz = Settings::resolve_auto_vsync_hz(predicted_dt, None);
                if (egui_hz - state.last_fanned_auto_vsync_hz).abs() >= 2.0 {
                    mark_fanout = true;
                }
                if let Ok(mut ctx_settings) = state.settings_window_context.try_lock() {
                    if mark_fanout {
                        ctx_settings.settings.auto_vsync_hz = egui_hz;
                    }
                    if state.settings_fanout_needed || mark_fanout {
                        snap_to_fan = Some(ctx_settings.settings.clone());
                    }
                }
                if mark_fanout {
                    state.settings_fanout_needed = true;
                }
                if let Some(snap) = snap_to_fan {
                    state.last_fanned_auto_vsync_hz = snap.auto_vsync_hz;
                    state.settings_fanout_needed = false;
                    for mut channel in settings_out {
                        actor.try_send(&mut channel, snap.clone());
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

fn paint_gaze_corner_mark(
    painter: &egui::Painter,
    rect: egui::Rect,
    corner: gaze::GazeCorner,
) {
    let len = 18.0;
    let stroke = egui::Stroke::new(2.0, Color32::from_rgb(255, 220, 80));
    let (h0, h1, v0, v1) = match corner {
        gaze::GazeCorner::TopLeft => {
            let p = rect.left_top();
            (p, p + egui::vec2(len, 0.0), p, p + egui::vec2(0.0, len))
        }
        gaze::GazeCorner::TopRight => {
            let p = rect.right_top();
            (p, p + egui::vec2(-len, 0.0), p, p + egui::vec2(0.0, len))
        }
        gaze::GazeCorner::BottomRight => {
            let p = rect.right_bottom();
            (p, p + egui::vec2(-len, 0.0), p, p + egui::vec2(0.0, -len))
        }
        gaze::GazeCorner::BottomLeft => {
            let p = rect.left_bottom();
            (p, p + egui::vec2(len, 0.0), p, p + egui::vec2(0.0, -len))
        }
    };
    painter.line_segment([h0, h1], stroke);
    painter.line_segment([v0, v1], stroke);
}

