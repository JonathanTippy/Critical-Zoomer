#![allow(warnings)]

use steady_state::*;
use arg::MainArg;
use clap::Parser;
mod arg;

use rug::*;

use std::thread;
use assemblies::pipeline::{self, HeadKind, STACK_SIZE};
use assemblies::{headgroup, shadergroup, workgroup};
use settings::Settings;

pub mod actor {}
pub mod settings;

pub mod utils;
pub mod range;
pub mod constants;
pub mod floatexp;
pub mod reference;
pub mod series;
pub mod delta_gear;
pub mod gearbox;
pub mod perturb;
pub mod assemblies;
pub mod debug_agent;

fn main() {
    #[cfg(target_os = "linux")]
    if std::env::var("WINIT_X11_SCALE_FACTOR").is_err() {
        std::env::set_var("WINIT_X11_SCALE_FACTOR", "1");
    }

    debug_agent::init_cpu_profile_from_env();

    let builder = thread::Builder::new()
        .name("worker-thread".into())
        .stack_size(STACK_SIZE);

    let handler = builder
        .spawn(|| {
            let cli_args = MainArg::parse();
            init_logging(LogLevel::Info, None);
            let mut graph = GraphBuilder::default()
                .with_telemtry_production_rate_ms(40)
                .with_default_actor_stack_size(STACK_SIZE)
                .build(cli_args);
            pipeline::build_pipeline(&mut graph, HeadKind::LiveWindow);
            graph.start();
            graph.block_until_stopped(Duration::from_millis(100));
        })
        .expect("Failed to spawn thread");

    handler.join().expect("Thread panicked");
}
