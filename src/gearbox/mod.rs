//! Gearbox: compute-gear policy and test-only Oracle.
//!
//! Production delta stepping remains in [`crate::delta_gear`] until fully
//! migrated. This module owns vocabulary, HUD labels, admission policy, and
//! the FloatExp absolute Oracle kernel (never a live app path).

pub mod oracle;
pub mod policy;

pub use crate::delta_gear::ComputeGear;
pub use oracle::{OracleAnswer, OracleKernel, iterate_oracle_bout};
pub use policy::{
    best_pps_kernel, hud_label, legal_kernels, view_gear_from_relative_admission,
    PPS_PROBE_SHIFTS_PER_CANDIDATE, PPS_REEVAL_INTERVAL,
};
