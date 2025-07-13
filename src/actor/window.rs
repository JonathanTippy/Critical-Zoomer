use steady_state::*;
use eframe::{egui, NativeOptions};
//use eframe::Frame::raw_window_handle;
use egui_extras;
use winit::platform::x11::EventLoopBuilderExtX11; // For X11
//use winit::platform::wayland::EventLoopBuilderExtWayland; // For Wayland
//use winit::platform::windows::EventLoopBuilderExtWindows; // For Windows
use winit::event_loop::EventLoopBuilder;
use egui::{Color32, ColorImage, TextureHandle, Vec2, Pos2, ViewportInfo};
use winit::raw_window_handle::HasWindowHandle;
use winit::dpi::PhysicalPosition;
use crate::actor::transformer::*;
use crate::actor::computer::*;
use crate::operation::sampling;

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use std::sync::{Arc, Mutex};


const RECOVER_EGUI_CRASHES:bool = false;
// ^ half implimented; in cases where the window is supposed to
// be minimized or not on top, it might bother the user by restarting.
const MIN_FRAME_RATE:f64 = 20.0;
const MAX_FRAME_TIME:f64 = 1.0 / MIN_FRAME_RATE;
const VSYNC:bool = false;

pub const DEFAULT_WINDOW_RES:(u32, u32) = (800, 480);


#[derive(Debug)]
struct EguiWindowError {
    state: WindowState
}

impl fmt::Display for EguiWindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "egui window stopped unexpectedly")
    }
}

impl Error for EguiWindowError {}

/// State struct for the window actor.



// movements never use deltas as the rate of the window is not unchanging
// they can use start / stop commands or explicit screenspace coordinate setting
// sets use Strings (separation of concerns)
// all commands should be carried out immediately when received
// (or, if that isn't possible, never when debugging and with a delay when in release.)
// commands can't be undone (undo/redo in window)
// commands are packaged into a vector and sent to the controller.
// all commands in the vector are executed every input tick
pub(crate) enum ZoomerCommand {
    SetAttention{pixel_x:u32, pixel_y:u32}
    , SetRes{hori: u32, verti: u32}
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
    , pub(crate) bucket: Option<Vec<(u8, u8, u8)>>
}


#[derive(Clone, Debug)]
pub(crate) struct WindowState {
    size: Vec2
    , location: Option<Pos2>
    , last_frame_period: Option<(Instant, Instant)>
    , free_buffer: Vec<Vec<(u8, u8, u8)>>
    , used_buffer: Vec<Vec<(u8,u8,u8)>>
    , id_counter:u64
    , expecting_frame: bool
}


/// Entry point for the window actor.
pub async fn run(
    actor: SteadyActorShadow,
    pixels_in: SteadyRx<PixelsForWindow>,
    commands_out: SteadyTx<ZoomerCommandPackage>,
    state: SteadyState<WindowState>,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight([&pixels_in], [&commands_out]),
        pixels_in,
        commands_out,
        state,
    )
    .await
    // If it's testing, use test behavior instead
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    pixels_in: SteadyRx<PixelsForWindow>,
    commands_out: SteadyTx<ZoomerCommandPackage>,
    state: SteadyState<WindowState>,
) -> Result<(), Box<dyn Error>> {

    let mut portable_actor = Arc::new(Mutex::new(actor));

    let mut state = state.lock(|| WindowState{
        size: egui::vec2(DEFAULT_WINDOW_RES.0 as f32, DEFAULT_WINDOW_RES.1 as f32),
        location: None,
        last_frame_period: None,
        free_buffer: vec!(vec!((0,0,0);(DEFAULT_WINDOW_RES.0*DEFAULT_WINDOW_RES.1) as usize);2),
        used_buffer: vec!(vec!((0,0,0);(DEFAULT_WINDOW_RES.0*DEFAULT_WINDOW_RES.1) as usize);1), // on window start, what should be displayed?
        id_counter: 0,
        expecting_frame: false
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
        , commands_out: commands_out.clone()
        , portable_state: portable_state.clone()
    };

    eframe::run_native(
        "Critical Zoomer",
        options,
        Box::new(|_cc| Ok(Box::new(passthrough))),
    ).unwrap();


    let mut actor = portable_actor.lock().unwrap();
    let mut pixels_in = pixels_in.try_lock().unwrap();
    let mut commands_out = commands_out.try_lock().unwrap();
    let mut state = portable_state.lock().unwrap();

    //println!("state size final value: {}", state.size);


    if actor.is_running(
        || i!(pixels_in.is_closed_and_empty())
            && i!(commands_out.mark_closed())
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
    pixels_in: SteadyRx<PixelsForWindow>,
    commands_out: SteadyTx<ZoomerCommandPackage>,
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
        let mut commands_out = self.commands_out.try_lock().unwrap();
        let mut state = self.portable_state.lock().unwrap();

        if actor.is_running(
            || i!(pixels_in.is_closed_and_empty())
                && i!(commands_out.mark_closed())
        ) {

            // calculate framerate and frametime

            let timinginfo:Option<(f64, Duration, Duration, Duration)>;

            match (state.last_frame_period) {
                Some(p) => {
                    timinginfo = Some( (
                        1000000000.0 / (this_frame_start-p.0).as_nanos() as f64
                        , this_frame_start-p.0
                        , p.1-p.0
                        , this_frame_start-p.0 - (p.1-p.0)
                    ) );
                }
                None => {timinginfo = None}
            }

            // blit pixels


            let mut tries = 0;
            while state.expecting_frame {

                match actor.try_take(&mut pixels_in) {

                    Some(pixels) => {
                        state.expecting_frame = false;
                        //info!("command response loop took {:.2}ms", pixels.command_processing_duration.as_secs_f64()*1000.0);
                        //info!("some pixels, swapping buffers");
                        let b = state.used_buffer.pop().unwrap();
                        state.free_buffer.push(b);
                        state.used_buffer.push(pixels.pixels);

                        if tries!=0 {

                            let mut message = format!("retried {} times", tries);

                            match pixels.report {
                                Some(r) => {
                                    for dur in r.time_to_xyz {
                                        message = message + format!("\n{} {:.3}ms", dur.0, dur.1.as_secs_f64()*1000.0).as_str();
                                    }
                                }
                                None => {}
                            }

                            info!("{}", message);
                        }
                    }
                    None => {tries+=1; std::thread::sleep(Duration::from_millis(1))}
                };

            }


            // Convert Vec<u8> to ColorImage
            let pixels_rgba: Vec<Color32> = state.used_buffer[0].clone().into_iter()
                .map(|chunk| Color32::from_rgba_premultiplied(chunk.0, chunk.1, chunk.2, 255))
                .collect();

            // Create or update texture
            let image = ColorImage {
                size: [state.size.x as usize, state.size.y as usize],
                pixels: pixels_rgba,
                source_size: egui::vec2(state.size.x, state.size.y)
            };

            let texture = ctx.load_texture(
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

                ui.image((texture.id(), available_size));

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
                                format!("debug\nfps: {:04.2}\nframetime:{:04.2}ms\n    from me:{:04.2}ms\n    from egui: {:04.2}ms\n", t.0, t.1.as_secs_f64()*1000.0, t.2.as_secs_f64()*1000.0, t.3.as_secs_f64()*1000.0)
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
                            // Gear button action (empty for now)
                        }
                        return button_state;
                    }
                );
            });

            // save current window position and size
            if !RECOVER_EGUI_CRASHES {;} else {
                ctx.input(|input_state| {
                    let info:ViewportInfo = input_state.raw.viewports.get(&input_state.raw.viewport_id).unwrap().clone();
                    match info.outer_rect {
                        Some(r) => { state.location = Some(r.min); }
                        None => {}
                    }
                    match info.inner_rect {
                        Some(r) => { state.size = r.size(); }
                        None => {}
                    }
                });
            }


            let command_package = ZoomerCommandPackage {
                command_package_id: state.id_counter,
                start_time: Instant::now(),
                commands: vec!(ZoomerCommand::DemandFrame{}),
                bucket: state.free_buffer.pop()
            };

            actor.try_send(&mut commands_out, command_package);
            state.expecting_frame = true;
            ctx.request_repaint();


            state.last_frame_period = Some(  (this_frame_start, Instant::now())  );
        }
        else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}