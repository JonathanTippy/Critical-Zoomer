// Property and example tests binding the craftsmanship inventory
// (docs/assistant/tracey/craftsmanship-rules.md) to the v0.0.9 workgroup code.
// Each test names the rule it verifies.
//
// Seam note: WorkContext's completion buffer is a growable Vec drained LIFO to the collector channel
// (capacity 100000). Tests that build one still use run_big for headroom.

use std::time::{Duration, Instant};
use std::collections::VecDeque;

use proptest::prelude::*;

use super::perturb_floatexp::FloatExpPerturbationKernel;
use super::perturb_kernel::PerturbationKernel;
use super::work_update;
use super::{invalidate_stale_deliveries, telemetry_update, workshift::*};
use crate::assemblies::headgroup::window::rolling::RateCounter;
use crate::assemblies::headgroup::window::sampling::index_from_relative_location;
use crate::assemblies::workgroup::c_generator::{CGenerator, Mandelbrotable};
use crate::assemblies::workgroup::work_collector::{sample_old_values, ResultsPackage};
use crate::assemblies::workgroup::screen_worker::workshift::get_random_mixmap;
use crate::constants::TEST_SCREEN_RES;
use crate::utils::{index_from_pos, pos_from_index, IntExp, ObjectivePosAndZoom};
use crate::floatexp::{ComplexFloatExp, FloatExp};


include!("craft_core.rs");
include!("depth_and_reference.rs");
include!("never_stall_and_faux.rs");
include!("steady_state_and_quality.rs");
