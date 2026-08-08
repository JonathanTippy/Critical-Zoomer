// read delivery.md for project context
use crate::assemblies::headgroup::window::sampling::{sample, SamplingContext, ZoomerCommand};

/// Apply viewport commands without sampling pixels (goto / tests).
pub fn transform(
    command_package: Vec<ZoomerCommand>,
    sampling_context: &mut SamplingContext,
) {
    let mut sink = Vec::new();
    // `sample` applies commands even when screen is None (no pixel loop).
    sample(command_package, &mut sink, sampling_context);
}
