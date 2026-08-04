// read delivery.md for project context
//! The one wgpu context the whole app shares.
//!
//! Requirements: no GPU toggle; GPU acceleration always on when a device exists.
//! Architecture (architecture.md): the workgroup completes tiles resident to the
//! GPU and hands the headgroup gpu-native answers, bypassing upload. That is only
//! possible when compute and display address the same device, so the context is
//! built once here, before the window, and handed to eframe rather than the other
//! way around.
// r[impl cz.seamless.gpu-preferred+1]

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Wake, Waker};

/// Shared device handle. `None` means no adapter was available anywhere on this
/// machine, in which case every GPU-preferred path falls back to its CPU twin.
pub type SharedGpu = Option<Arc<GpuContext>>;

/// Instance, adapter, device and queue, all cloneable Arc handles internally.
///
/// One device means one queue: display and compute submissions serialize against
/// each other. Callers on the compute side must budget their submissions so a
/// dispatch cannot delay the next present past the frame deadline.
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Whether the device was created with timestamp queries, which the
    /// submission budget uses to calibrate itself when the adapter allows it.
    pub timestamps_supported: bool,
}

impl GpuContext {
    /// Build the shared context, preferring a high-performance adapter and
    /// accepting a fallback one rather than giving up on the GPU entirely.
    ///
    /// No surface exists yet at this point, so the adapter is chosen with
    /// `compatible_surface: None`. On a multi-GPU machine that can in principle
    /// select an adapter which cannot present; callers must treat a failure to
    /// present as a reason to let eframe create its own device instead.
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = request_adapter(&instance)?;

        // Timestamps are how the submission budget learns how long a dispatch
        // actually took; they are a diagnostic, never a requirement.
        let timestamps_supported = adapter
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY);
        let required_features = if timestamps_supported {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        };

        // The tile atlas grows with the user's memory limit, which has no fixed
        // ceiling, so take the adapter's real limits rather than the conservative
        // defaults that would cap the atlas far below the VRAM budget.
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("critical_zoomer_shared"),
            required_features,
            required_limits: adapter.limits(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| format!("gpu device: {e:?}"))?;

        Ok(GpuContext {
            instance,
            adapter,
            device,
            queue,
            timestamps_supported,
        })
    }

    /// The process-wide context, built on first use and reused forever after.
    ///
    /// There is exactly one GPU context per process by construction, which is
    /// what lets the workgroup hand the headgroup gpu-native answers. `main`
    /// calls this before building the graph so the device exists before the
    /// window; every later caller receives that same device.
    pub fn shared() -> SharedGpu {
        static SHARED: OnceLock<SharedGpu> = OnceLock::new();
        SHARED
            .get_or_init(|| match GpuContext::new() {
                Ok(context) => Some(Arc::new(context)),
                Err(err) => {
                    // Not fatal: every GPU path has a CPU twin, so the app runs.
                    steady_state::warn!(
                        "no shared gpu context ({err}); falling back to cpu paths"
                    );
                    None
                }
            })
            .clone()
    }

    /// Whether this machine has a usable GPU at all.
    pub fn available() -> bool {
        GpuContext::shared().is_some()
    }
}

fn request_adapter(instance: &wgpu::Instance) -> Result<wgpu::Adapter, String> {
    if let Ok(adapter) = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    })) {
        return Ok(adapter);
    }
    block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: true,
    }))
    .map_err(|e| format!("gpu adapter: {e:?}"))
}

struct SpinWaker;
impl Wake for SpinWaker {
    fn wake(self: Arc<Self>) {}
}

/// wgpu's setup calls are async but resolve without a reactor, so a spin poll is
/// enough and avoids pulling an executor into startup.
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::from(Arc::new(SpinWaker));
    let mut context = Context::from_waker(&waker);
    let mut future = Pin::from(Box::new(future));
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cz.seamless.gpu-preferred+1]
    #[test]
    fn shared_context_serves_compute_and_render_from_one_device() {
        let Some(context) = GpuContext::shared() else {
            // No adapter here; CPU fallback is the specified behaviour.
            return;
        };
        // The point of the shared context is that a buffer written by compute is
        // addressable by display work. Both usages must come off one device.
        let buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shared_context_probe"),
            size: 256,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        context.queue.write_buffer(&buffer, 0, &[0u8; 256]);
        context
            .device
            .poll(wgpu::PollType::Wait)
            .expect("shared device must service its own queue");
    }

    #[test]
    fn block_on_resolves_a_ready_future() {
        assert_eq!(block_on(async { 7u32 }), 7);
    }

    // r[verify cz.seamless.gpu-preferred+1]
    #[test]
    fn repeated_calls_hand_back_one_context() {
        let (Some(first), Some(second)) = (GpuContext::shared(), GpuContext::shared()) else {
            // No adapter here; both calls must agree on that too.
            assert!(GpuContext::shared().is_none());
            return;
        };
        assert!(
            Arc::ptr_eq(&first, &second)
            , "the app must never hold two GPU contexts; upload bypass depends on it"
        );
    }
}
