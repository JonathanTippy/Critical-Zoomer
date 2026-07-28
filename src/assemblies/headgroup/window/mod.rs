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
use transforms::transform;
use crate::assemblies::workgroup::work_controller::*;

use crate::settings::*;
use crate::utils::*; use crate::intexp::*;
use crate::constants::*;
use crate::assemblies::headgroup::window::rolling::*;
use crate::assemblies::headgroup::window::widgetize::*;

use crate::assemblies::structs::*;
use crate::assemblies::headgroup::window::inputs::*;
use crate::assemblies::headgroup::window::sampling::*;
use crate::assemblies::headgroup::window::shade::*;
use crate::assemblies::headgroup::window::gpu_display::*;

use crate::intexp::*;

pub mod rolling;
pub mod widgetize;
pub mod inputs;
pub mod sampling;
pub mod shade;
pub mod gpu_display;
pub mod transforms;
pub mod coords;
pub mod navigate;
pub mod offscreen;

use crate::assemblies::headgroup::window::coords::*;
use crate::assemblies::headgroup::window::navigate::*;
use crate::assemblies::headgroup::window::offscreen::{
    R2ScreenRelation, ViewportComplexRect,
};

// #region agent log
pub fn agent_dbg(hypothesis_id: &str, location: &str, message: &str, data_json: &str) {
    use std::io::Write;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/home/jonathan/git/Critical-Zoomer/.cursor/debug-51c884.log")
    {
        let _ = writeln!(
            f
            , "{{\"sessionId\":\"51c884\",\"hypothesisId\":\"{}\",\"location\":\"{}\",\"message\":\"{}\",\"data\":{},\"timestamp\":{}}}"
            , hypothesis_id
            , location
            , message
            , data_json
            , ts
        );
    }
}
// #endregion

const RECOVER_EGUI_CRASHES:bool = false;
// ^ half implimented; in cases where the window is supposed to
// be minimized or not on top, it might bother the user by restarting.
//const MIN_FRAME_RATE:f64 = 20.0;
//const MAX_FRAME_TIME:f64 = 1.0 / MIN_FRAME_RATE;
// r[impl cz.display.headgroup-60fps+1]
const VSYNC:bool = true;
const TARGET_FRAME_PERIOD: std::time::Duration = std::time::Duration::from_nanos(16_666_667);



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
    , pub startup_goto_applied: bool
    , pub nav_target: Option<(IntExp, IntExp, i32)>
    , pub pending_gpu_tile_origins: HashSet<(usize, usize)>
    , pub gpu_atlas_size: (u32, u32)
    , pub gpu_atlas_clear: bool
    , pub coord_input: String
    , pub atlas_location: Option<ObjectivePosAndZoom>
    , pub shift_was_down: bool
}

/// Entry point for the window actor.
pub async fn run(
    actor: SteadyActorShadow
    , pixels_in: SteadyRx<GPUTile>
    , stencil_out: SteadyTx<(PointStencil)>
    , settings_out: SteadyTxBundle<Settings,2>
    , attention_out: SteadyTx<(i32, i32)>
    , memory_bump_in: SteadyRx<crate::assemblies::workgroup_new::tile_publisher::MemoryBump>
    , state: SteadyState<WindowState>
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight(
            [&pixels_in, &memory_bump_in]
            , [&stencil_out, &settings_out[0], &settings_out[1], &attention_out]
        )
        , pixels_in
        , stencil_out
        , settings_out
        , attention_out
        , memory_bump_in
        , state
    )
    .await
}

async fn internal_behavior<A: SteadyActor>(
    actor: A
    , pixels_in: SteadyRx<GPUTile>
    , stencil_out: SteadyTx<(PointStencil)>
    , settings_out: SteadyTxBundle<Settings, 2>
    , attention_out: SteadyTx<(i32, i32)>
    , memory_bump_in: SteadyRx<crate::assemblies::workgroup_new::tile_publisher::MemoryBump>
    , state: SteadyState<WindowState>
) -> Result<(), Box<dyn Error>> {

    let portable_actor = Arc::new(Mutex::new(actor));

    let state = state.lock(|| WindowState{
        size: egui::vec2(DEFAULT_WINDOW_RES.0 as f32, DEFAULT_WINDOW_RES.1 as f32)
        , location: None
        , last_frame_period: None
        , buffers: vec!(vec!(Color32::BLACK;(DEFAULT_WINDOW_RES.0*DEFAULT_WINDOW_RES.1) as usize))
        , id_counter: 0
        , sampling_context: SamplingContext {
            tiles: HashMap::new()
            , tile_gpu_ids: HashMap::new()
            , pending_tile_uploads: Vec::new()
            , next_tile_gpu_id: 1
            , reset_gpu_tile_slots: false
            , color_screen: None
            , proximate_answers: true
            , unsent_answers: true
            , screen_size: (DEFAULT_WINDOW_RES.0, DEFAULT_WINDOW_RES.1)
            , location: ObjectivePosAndZoom {
                pos: (IntExp::from(HOME_POSITION.0), IntExp::from(HOME_POSITION.1))
                , zoom_pot: HOME_POSITION.2
            }
            , updated: true
            , mouse_drag_start: None
            , memory_limit_bytes: 1_000_000_000
            , last_memory_bump: None
        }
        , atlas_location: None
        , shift_was_down: false
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
        , startup_goto_applied: false
        , nav_target: None
        , pending_gpu_tile_origins: HashSet::new()
        , gpu_atlas_size: (DEFAULT_WINDOW_RES.0, DEFAULT_WINDOW_RES.1)
        , gpu_atlas_clear: true
        , coord_input: String::new()
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
            .with_icon(
                eframe::icon_data::from_png_bytes(
                    include_bytes!("../../../../icons/assembly_chain_crosshair.png"),
                )
                    .expect("failed to load app icon"),
                //  ^ cannot happen during runtime due to file paths; image is baked in
            )
        ;
    let viewport_options = match state.location {
        Some(l) => viewport_options.with_position(l),
        None => viewport_options,
    };

    let options = eframe::NativeOptions {
        event_loop_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "linux")]
            { builder.with_any_thread(true); }

        })),
        viewport: viewport_options,
        vsync: VSYNC,
        renderer: eframe::Renderer::Wgpu,
        ..NativeOptions::default()
    };

    let portable_state = Arc::new(Mutex::new(state));

    let passthrough = EguiWindowPassthrough{
        portable_actor: portable_actor.clone()
        , pixels_in: pixels_in.clone()
        , stencil_out: stencil_out.clone()
        , settings_out: settings_out.clone()
        , attention_out: attention_out.clone()
        , memory_bump_in: memory_bump_in.clone()
        , portable_state: portable_state.clone()
    };

    match eframe::run_native(
        "Critical Zoomer"
        , options
        , Box::new(|cc| {
            if let Some(render_state) = &cc.wgpu_render_state {
                ensure_resources(render_state);
            }
            Ok(Box::new(passthrough))
        })
    ) {
        Ok(()) => {}
        Err(err) => {
            // Under xvfb / after a prior event-loop attempt, winit can return
            // RecreationAttempt. Panicking here restarts the actor into a loop;
            // shut down cleanly instead so e2e harnesses can exit.
            error!("eframe event loop ended: {err}");
        }
    }


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
    portable_actor: Arc<Mutex<A>>
    , pixels_in: SteadyRx<GPUTile>
    , stencil_out: SteadyTx<(PointStencil)>
    , settings_out: SteadyTxBundle<Settings, 2>
    , attention_out: SteadyTx<(i32, i32)>
    , memory_bump_in: SteadyRx<crate::assemblies::workgroup_new::tile_publisher::MemoryBump>
    , portable_state:Arc<Mutex<StateGuard<'a, WindowState>>>
}

impl<A: SteadyActor> eframe::App for EguiWindowPassthrough<'_, A> {

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
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
        ];
        let mut attention_out = self.attention_out.try_lock().unwrap();
        let mut memory_bump_in = self.memory_bump_in.try_lock().unwrap();
        let mut state = self.portable_state.lock().unwrap();


        if actor.is_running(
            || i!(true)
        ) {

            // Drain workgroup memory bumps → raise slider floor + sampling limit.
            while actor.avail_units(&mut memory_bump_in) > 0 {
                if let Some(bump) = actor.try_take(&mut memory_bump_in) {
                    apply_ui_memory_bump(&mut state, bump.needed_bytes);
                }
            }

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


            // Cap headgroup wakeups near 60fps (design/headgroup.md).
            ctx.request_repaint_after(TARGET_FRAME_PERIOD);

            let size = (state.size.x as usize, state.size.y as usize);
            let pixels = size.0 * size.1;

            //let mut sampler_buffer = vec!();//Vec::with_capacity(pixels);

            while actor.avail_units(&mut pixels_in) > 0 {
                match actor.try_take(&mut pixels_in) {
                    Some(tile) => {
                        state.sampling_context.ingest_gpu_tile(tile);
                    }
                    None => { break; }
                }
            }

            if state.sampling_context.updated
            {
                actor.try_send(&mut stencil_out, PointStencil{
                    homothety: (state.sampling_context.location.pos.0.clone()
                                , IntExp::ZERO-state.sampling_context.location.pos.1.clone()
                                , state.sampling_context.location.zoom_pot.clone()
                    )
                    , resolution: (state.size.x as usize, state.size.y as usize)
                    , serial_number: state.stencil_serial_number_counter
                    , focus: None
                    , hover: None
                });
                state.stencil_serial_number_counter +=1;
                state.sampling_context.updated = false;
            }

            // sample

            let (mut command_package, attention) = parse_inputs(&ctx, &mut state, size);
            actor.try_send(&mut attention_out, attention);

            if !state.startup_goto_applied {
                if let Ok(line) = std::env::var("CZ_GOTO") {
                    if !line.trim().is_empty() {
                        let cmds = if std::env::var("CZ_NAV").is_ok() {
                            commands_from_navigate_line(&line)
                        } else {
                            commands_from_goto_line(&line)
                        };
                        if let Some(cmds) = cmds {
                            if std::env::var("CZ_NAV").is_ok() {
                                if let ZoomerCommand::NavigateTo { real, imag, pot } = &cmds[0] {
                                    state.nav_target = Some((real.clone(), imag.clone(), *pot));
                                }
                            } else {
                                command_package.extend(cmds);
                            }
                        }
                    }
                }
                state.startup_goto_applied = true;
            }
            if let Ok(line) = std::fs::read_to_string("/tmp/cz_ctl.goto") {
                let _ = std::fs::remove_file("/tmp/cz_ctl.goto");
                if let Some(cmds) = commands_from_goto_line(&line) {
                    command_package.extend(cmds);
                    state.sampling_context.clear_tiles();
                    state.nav_target = None;
                }
            }
            if let Ok(line) = std::fs::read_to_string("/tmp/cz_ctl.navigate") {
                let _ = std::fs::remove_file("/tmp/cz_ctl.navigate");
                if let Some(cmds) = commands_from_navigate_line(&line) {
                    if let ZoomerCommand::NavigateTo { real, imag, pot } = &cmds[0] {
                        state.nav_target = Some((real.clone(), imag.clone(), *pot));
                    }
                }
            }
            if let Some(target) = state.nav_target.clone() {
                if let Some(nav_cmds) = advance_navigation(
                    &target
                    , &state.sampling_context.location
                    , size
                ) {
                    command_package.extend(nav_cmds);
                } else {
                    state.nav_target = None;
                }
            }

            state.sampling_context.screen_size = (size.0 as u32, size.1 as u32);

            transform(command_package, &mut state.sampling_context);

            let dragging = state.sampling_context.mouse_drag_start.is_some();
            let upload_n = state.sampling_context.pending_tile_uploads.len();
            // #region agent log
            if upload_n > 0 || state.sampling_context.updated {
                agent_dbg(
                    "H-PAN-A"
                    , "mod.rs:upload_gate"
                    , "upload_gate"
                    , &format!(
                        "{{\"uploads\":{},\"dragging\":{},\"tiles\":{},\"zoom\":{}}}"
                        , upload_n
                        , dragging
                        , state.sampling_context.tile_count()
                        , state.sampling_context.location.zoom_pot
                    )
                );
            }
            // #endregion

            state.atlas_location = Some(state.sampling_context.location.clone());
            let mut settings_for_shade = state.settings_window_context.try_lock()
                .ok()
                .map(|guard| guard.settings.clone())
                .unwrap_or_else(|| Settings::DEFAULT);
            // Keep sampling budget aligned with the settings slider before prune.
            state.sampling_context.memory_limit_bytes =
                settings_for_shade.memory_limit_bytes;
            let blit_frame = build_shade_frame(
                &mut state.sampling_context
                , &mut settings_for_shade
            );
            // Local prune may have bumped; raise the settings slider floor.
            if let Some(needed) = state.sampling_context.last_memory_bump.take() {
                apply_ui_memory_bump(&mut state, needed);
                settings_for_shade.memory_limit_bytes =
                    state.sampling_context.memory_limit_bytes;
            }
            if let Ok(mut guard) = state.settings_window_context.try_lock() {
                guard.settings = settings_for_shade;
            }
            state.sampling_context.unsent_answers = false;
            state.sampling_context.proximate_answers = false;
            state.gpu_atlas_size = state.sampling_context.screen_size;

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
                let paint_rect = ui.available_rect_before_wrap();
                paint_central_panel(ui, paint_rect, blit_frame);

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

                                // r[impl cz.display.offscreen-r2-circle+1]
                                let screen_stencil = PointStencil {
                                    homothety: (
                                        state.sampling_context.location.pos.0.clone(),
                                        IntExp::ZERO
                                            - state.sampling_context.location.pos.1.clone(),
                                        state.sampling_context.location.zoom_pot,
                                    ),
                                    resolution: (
                                        state.size.x.max(1.0) as usize,
                                        state.size.y.max(1.0) as usize,
                                    ),
                                    serial_number: 0,
                                    focus: None,
                                    hover: None,
                                };
                                let view = ViewportComplexRect::from_stencil(&screen_stencil);
                                // Red arrows drawn below via painter; keep debug free of unicode HUD.
                                let _ = view.needs_red_arrows();

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

                // Red off-screen / too-small guidance arrows (requirements).
                {
                    let screen_stencil = PointStencil {
                        homothety: (
                            state.sampling_context.location.pos.0.clone(),
                            IntExp::ZERO
                                - state.sampling_context.location.pos.1.clone(),
                            state.sampling_context.location.zoom_pot,
                        ),
                        resolution: (
                            state.size.x.max(1.0) as usize,
                            state.size.y.max(1.0) as usize,
                        ),
                        serial_number: 0,
                        focus: None,
                        hover: None,
                    };
                    let view = ViewportComplexRect::from_stencil(&screen_stencil);
                    if view.needs_red_arrows() {
                        let painter = ui.painter();
                        let rect = ui.max_rect();
                        let red = Color32::from_rgb(220, 40, 40);
                        let stroke = egui::Stroke::new(3.0, red);
                        let cx = rect.center().x;
                        let cy = rect.center().y;
                        let arm = 28.0;
                        match view.classify_r2() {
                            R2ScreenRelation::OffScreen | R2ScreenRelation::MostlyOffScreen => {
                                // Point toward origin: arrows on edge facing the set.
                                let set_cx = 0.0;
                                let set_cy = 0.0;
                                let view_cx = (view.real_min + view.real_max) * 0.5;
                                let view_cy = (view.imag_min + view.imag_max) * 0.5;
                                let dx = set_cx - view_cx;
                                let dy = set_cy - view_cy;
                                if dx.abs() >= dy.abs() {
                                    let x = if dx > 0.0 { rect.right() - 40.0 } else { rect.left() + 40.0 };
                                    let dir = if dx > 0.0 { 1.0 } else { -1.0 };
                                    painter.line_segment(
                                        [egui::pos2(x - dir * arm, cy), egui::pos2(x, cy)],
                                        stroke,
                                    );
                                    painter.line_segment(
                                        [egui::pos2(x, cy), egui::pos2(x - dir * 12.0, cy - 10.0)],
                                        stroke,
                                    );
                                    painter.line_segment(
                                        [egui::pos2(x, cy), egui::pos2(x - dir * 12.0, cy + 10.0)],
                                        stroke,
                                    );
                                } else {
                                    let y = if dy > 0.0 { rect.top() + 40.0 } else { rect.bottom() - 40.0 };
                                    // +imag is up on screen roughly when dy>0 toward higher imag
                                    let dir = if dy > 0.0 { -1.0 } else { 1.0 };
                                    painter.line_segment(
                                        [egui::pos2(cx, y - dir * arm), egui::pos2(cx, y)],
                                        stroke,
                                    );
                                    painter.line_segment(
                                        [egui::pos2(cx, y), egui::pos2(cx - 10.0, y - dir * 12.0)],
                                        stroke,
                                    );
                                    painter.line_segment(
                                        [egui::pos2(cx, y), egui::pos2(cx + 10.0, y - dir * 12.0)],
                                        stroke,
                                    );
                                }
                            }
                            R2ScreenRelation::TooSmall | R2ScreenRelation::MostlyTooSmall => {
                                // Up and down arrows: zoomed out too far.
                                for (y, dir) in [(rect.top() + 36.0, -1.0), (rect.bottom() - 36.0, 1.0)] {
                                    painter.line_segment(
                                        [egui::pos2(cx, y), egui::pos2(cx, y + dir * arm)],
                                        stroke,
                                    );
                                    painter.line_segment(
                                        [egui::pos2(cx, y), egui::pos2(cx - 10.0, y + dir * 12.0)],
                                        stroke,
                                    );
                                    painter.line_segment(
                                        [egui::pos2(cx, y), egui::pos2(cx + 10.0, y + dir * 12.0)],
                                        stroke,
                                    );
                                }
                            }
                            R2ScreenRelation::OnScreen => {}
                        }
                    }
                }

                egui::Area::new(egui::Id::new("coord_bar"))
                    .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 8.0))
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
                                let center_text = format!("{} + {}i", cre, cim);
                                ui.horizontal(|ui| {
                                    ui.label("location");
                                    let mut readonly = center_text.clone();
                                    ui.add(
                                        egui::TextEdit::singleline(&mut readonly)
                                            .desired_width(320.0)
                                            .interactive(true)
                                    );
                                    if ui.button("Copy").clicked() {
                                        ui.ctx().copy_text(center_text);
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("goto");
                                    let response = ui.add(
                                        egui::TextEdit::singleline(&mut state.coord_input)
                                            .desired_width(280.0)
                                            .hint_text("re, im  or  a+bi")
                                    );
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

                ui.put(
                    egui::Rect::from_min_size(
                        egui::pos2(ui.available_width() - 40.0, 0.0),
                        egui::vec2(40.0, 40.0)
                    ),
                    |ui: &mut egui::Ui| {
                        let button_state = ui.button("⚙");
                        if button_state.clicked() {
                            state.settings_window_open = true;
                        }
                        return button_state;
                    }
                );

                // Home → viewport center at (0+0i); keep framed startup zoom.
                ui.put(
                    egui::Rect::from_min_size(
                        egui::pos2(ui.available_width() - 80.0, 0.0),
                        egui::vec2(40.0, 40.0)
                    ),
                    |ui: &mut egui::Ui| {
                        let button_state = ui.button("🏠");
                        if button_state.clicked() {
                            let screen = (
                                state.size.x.max(1.0) as u32
                                , state.size.y.max(1.0) as u32
                            );
                            state.sampling_context.location = ul_for_center(
                                IntExp::ZERO
                                , IntExp::ZERO
                                , HOME_POSITION.2
                                , screen
                            );
                            state.sampling_context.clear_tiles();
                            state.sampling_context.mouse_drag_start = None;
                            state.atlas_location = None;
                            state.pending_gpu_tile_origins.clear();
                            state.gpu_atlas_clear = true;
                            state.nav_target = None;
                            state.sampling_context.updated = true;
                        }
                        return button_state;
                    }
                );



                if state.settings_window_open {
                    let before_script = state.settings_window_context.try_lock()
                        .ok()
                        .map(|g| format!("{:?}", g.settings.coloring_script));
                    let before_bailout = state.settings_window_context.try_lock()
                        .ok()
                        .map(|g| (
                            g.settings.bailout_radius.value
                            , g.settings.bailout_max_additional_iterations
                            , g.settings.estimate_extra_iterations
                        ));
                    let result = settings(&ctx, state.settings_window_context.clone());
                    state.settings_window_open = !result.will_close;
                    let after_script = format!("{:?}", result.settings.coloring_script);
                    let after_bailout = (
                        result.settings.bailout_radius.value
                        , result.settings.bailout_max_additional_iterations
                        , result.settings.estimate_extra_iterations
                    );
                    if before_script.as_deref() != Some(after_script.as_str())
                        || before_bailout != Some(after_bailout)
                    {
                        state.sampling_context.proximate_answers = true;
                    }
                    state.sampling_context.memory_limit_bytes =
                        result.settings.memory_limit_bytes;
                    for mut channel in settings_out {
                        actor.try_send(&mut channel, result.settings.clone());
                    }
                } else if let Some((limit, settings)) = state
                    .settings_window_context
                    .try_lock()
                    .ok()
                    .map(|guard| (guard.settings.memory_limit_bytes, guard.settings.clone()))
                {
                    state.sampling_context.memory_limit_bytes = limit;
                    for mut channel in settings_out {
                        actor.try_send(&mut channel, settings.clone());
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

fn apply_ui_memory_bump(state: &mut WindowState, needed: usize) {
    use crate::assemblies::workgroup_new::tile_manager::apply_memory_bump;
    state.sampling_context.memory_limit_bytes =
        apply_memory_bump(state.sampling_context.memory_limit_bytes, needed);
    if let Ok(mut guard) = state.settings_window_context.try_lock() {
        guard.settings.memory_limit_bytes =
            apply_memory_bump(guard.settings.memory_limit_bytes, needed);
    }
}

