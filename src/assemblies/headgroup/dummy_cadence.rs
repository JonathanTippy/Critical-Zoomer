//! Dummy headgroup for pipeline cadence steady-state tests.
//! Same channel ports as the live window; no egui — drives stencil/settings and
//! records HUD emission RateCounters.

use crate::assemblies::headgroup::window::rolling::RateCounter;
use crate::assemblies::structs::*;
use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
use crate::settings::{Settings, DEFAULT_COLORING_SCRIPT};
use crate::utils::{IntExp, ObjectivePosAndZoom};
use egui::Color32;
use steady_state::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Snapshot of stage rates (events/s over the trailing 1s RateCounter window).
#[derive(Clone, Debug)]
pub struct CadenceReport {
    pub pub_hz: f64,
    pub esc_hz: f64,
    pub col_hz: f64,
    pub packages_dropped: u64,
    pub color_label: &'static str,
    pub escape_label: &'static str,
    pub measure_secs: f64,
    pub first_color_after_ms: f64,
    /// Total stamps observed at the head during the measure window (not only last 1s).
    pub pub_total: u64,
    pub esc_total: u64,
    pub col_total: u64,
}

#[derive(Clone)]
pub struct DummyCadenceConfig {
    pub settings: Settings,
    /// Wall time to sample RateCounters after the first colored frame arrives.
    pub measure_after_first_frame: Duration,
    pub report: Arc<Mutex<Option<CadenceReport>>>,
}

pub struct DummyCadenceState {
    bootstrapped: bool,
    first_color_at: Option<Instant>,
    measure_deadline: Option<Instant>,
    publisher_fps: RateCounter,
    escape_fps: RateCounter,
    color_fps: RateCounter,
    pub_total: u64,
    esc_total: u64,
    col_total: u64,
    last_packages_dropped: u64,
    last_color_label: &'static str,
    last_escape_label: &'static str,
    started_at: Instant,
    done: bool,
}

pub async fn run(
    actor: SteadyActorShadow,
    pixels_in: SteadyRx<View<Color32>>,
    stencil_out: SteadyTx<PointStencil>,
    settings_out: SteadyTxBundle<Settings, 4>,
    attention_out: SteadyTx<Option<(i32, i32)>>,
    state: SteadyState<DummyCadenceState>,
    cfg: DummyCadenceConfig,
) -> Result<(), Box<dyn Error>> {
    internal_behavior(
        actor.into_spotlight(
            [&pixels_in],
            [
                &stencil_out,
                &settings_out[0],
                &settings_out[1],
                &settings_out[2],
                &settings_out[3],
                &attention_out,
            ],
        ),
        pixels_in,
        stencil_out,
        settings_out,
        attention_out,
        state,
        cfg,
    )
    .await
}

async fn internal_behavior<A: SteadyActor>(
    mut actor: A,
    pixels_in: SteadyRx<View<Color32>>,
    stencil_out: SteadyTx<PointStencil>,
    settings_out: SteadyTxBundle<Settings, 4>,
    attention_out: SteadyTx<Option<(i32, i32)>>,
    state: SteadyState<DummyCadenceState>,
    cfg: DummyCadenceConfig,
) -> Result<(), Box<dyn Error>> {
    let mut pixels_in = pixels_in.lock().await;
    let mut stencil_out = stencil_out.lock().await;
    let mut settings_channels = [
        settings_out[0].lock().await,
        settings_out[1].lock().await,
        settings_out[2].lock().await,
        settings_out[3].lock().await,
    ];
    let mut attention_out = attention_out.lock().await;

    let mut state = state
        .lock(|| DummyCadenceState {
            bootstrapped: false,
            first_color_at: None,
            measure_deadline: None,
            publisher_fps: RateCounter::default(),
            escape_fps: RateCounter::default(),
            color_fps: RateCounter::default(),
            pub_total: 0,
            esc_total: 0,
            col_total: 0,
            last_packages_dropped: 0,
            last_color_label: "?",
            last_escape_label: "?",
            started_at: Instant::now(),
            done: false,
        })
        .await;

    let content_period = cfg.settings.resolved_content_period();
    // Failsafe: never hang the graph if no frames arrive.
    let absolute_deadline = Instant::now() + cfg.measure_after_first_frame + Duration::from_secs(90);

    while actor.is_running(|| i!(true)) {
        if state.done {
            break;
        }

        await_for_any!(
            actor.wait_periodic(content_period),
            actor.wait_avail(&mut pixels_in, 1),
        );

        if !state.bootstrapped {
            let snap = cfg.settings.clone();
            for ch in settings_channels.iter_mut() {
                let _ = actor.try_send(ch, snap.clone());
            }
            let home = ObjectivePosAndZoom {
                pos: (
                    IntExp::from(HOME_POSITION.0),
                    IntExp::from(HOME_POSITION.1),
                ),
                zoom_pot: HOME_POSITION.2,
            };
            if !actor.is_full(&mut stencil_out) {
                let _ = actor.try_send(
                    &mut stencil_out,
                    PointStencil {
                        location: (
                            home.pos.0.clone(),
                            IntExp::ZERO - home.pos.1.clone(),
                            home.zoom_pot,
                        ),
                        resolution: (
                            DEFAULT_WINDOW_RES.0 as usize,
                            DEFAULT_WINDOW_RES.1 as usize,
                        ),
                        serial_number: 0,
                    },
                );
            }
            if !actor.is_full(&mut attention_out) {
                let _ = actor.try_send(&mut attention_out, None);
            }
            state.bootstrapped = true;
        }

        let avail = actor.avail_units(&mut pixels_in);
        if avail > 0 {
            let (drops, take) =
                crate::assemblies::shadergroup::escaper::take_newest_plan(avail);
            for _ in 0..drops {
                let _ = actor.try_take(&mut pixels_in);
            }
            if take {
                if let Some(s) = actor.try_take(&mut pixels_in) {
                    let now = Instant::now();
                    if let Some(at) = s.hud.publisher_emitted_at {
                        state.publisher_fps.record(1, at);
                        if state.first_color_at.is_some() {
                            state.pub_total = state.pub_total.saturating_add(1);
                        }
                    }
                    if let Some(at) = s.hud.escape_emitted_at {
                        state.escape_fps.record(1, at);
                        if state.first_color_at.is_some() {
                            state.esc_total = state.esc_total.saturating_add(1);
                        }
                    }
                    if let Some(at) = s.hud.color_emitted_at {
                        state.color_fps.record(1, at);
                        if state.first_color_at.is_some() {
                            state.col_total = state.col_total.saturating_add(1);
                        }
                    }
                    state.last_packages_dropped = s.hud.packages_dropped;
                    state.last_color_label = s.hud.color.hud_label();
                    state.last_escape_label = s.hud.escape.hud_label();
                    if state.first_color_at.is_none() {
                        state.first_color_at = Some(now);
                        state.measure_deadline =
                            Some(now + cfg.measure_after_first_frame);
                        // Measure-window-only escaper counters (drop TTFP warmup).
                        crate::debug_agent::reset_escape_rca();
                        // Count this first frame too.
                        if s.hud.publisher_emitted_at.is_some() {
                            state.pub_total = 1;
                        }
                        if s.hud.escape_emitted_at.is_some() {
                            state.esc_total = 1;
                        }
                        if s.hud.color_emitted_at.is_some() {
                            state.col_total = 1;
                        }
                    }
                    let _ = now;
                }
            }
        }

        let now = Instant::now();
        let finished = match state.measure_deadline {
            Some(deadline) if now >= deadline => true,
            _ if now >= absolute_deadline => true,
            _ => false,
        };
        if finished {
            let report = CadenceReport {
                pub_hz: state.publisher_fps.rate(now),
                esc_hz: state.escape_fps.rate(now),
                col_hz: state.color_fps.rate(now),
                packages_dropped: state.last_packages_dropped,
                color_label: state.last_color_label,
                escape_label: state.last_escape_label,
                measure_secs: cfg.measure_after_first_frame.as_secs_f64(),
                first_color_after_ms: state
                    .first_color_at
                    .map(|t| t.duration_since(state.started_at).as_secs_f64() * 1e3)
                    .unwrap_or(-1.0),
                pub_total: state.pub_total,
                esc_total: state.esc_total,
                col_total: state.col_total,
            };
            *cfg.report.lock().unwrap_or_else(|e| e.into_inner()) = Some(report);
            state.done = true;
            actor.request_shutdown().await;
            break;
        }
    }

    Ok(())
}

/// Cadence lab settings: fixed 60 Hz content, coloring script installed, gears set.
/// Forces CPU DirectKernel so shade cadence is not confounded by naive-GPU init.
pub fn cadence_lab_settings(escape: EscaperMode, color: ColorerMode) -> Settings {
    let mut s = Settings::DEFAULT;
    s.coloring_script = Some(DEFAULT_COLORING_SCRIPT.to_vec());
    s.content_refresh_automatic = false;
    s.content_refresh_hz = 60.0;
    s.auto_vsync_hz = 60.0;
    s.manual_escape_gear_enabled = true;
    s.manual_escape_gear = escape;
    s.manual_color_gear_enabled = true;
    s.manual_color_gear = color;
    s.manual_gear_enabled = true;
    s.manual_gear = crate::assemblies::structs::KernelMode::Naive;
    s
}
