//! Audience renderer process; stdin/stdout are binary protocol, never logs.
//! Bounded worker preparation and native submission. No operator/mask policy.
// Offscreen readback helpers are shared with the diagnostic examples.
#[allow(dead_code)]
#[path = "audience/compositor.rs"]
pub mod compositor;
#[allow(dead_code)]
#[path = "audience/text.rs"]
pub mod text;
use sela::{
    delivery::{Command, DeliveryError, Epoch, Outcome, RendererSession, Stamp},
    scene::{Extent, PreparedBackground, RendererCapabilities},
    transport::{Frame, PipeWorkers},
};
use std::{
    collections::VecDeque,
    io,
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Monitor {
    Index(usize),
    /// First monitor that is not the primary one; never falls back to primary.
    FirstSecondary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Window { width: u32, height: u32 },
    Fullscreen(Monitor),
}

pub struct Config {
    pub epoch: Epoch,
    pub backend: wgpu::Backends,
    pub title: &'static str,
    pub placement: Placement,
    /// Diagnostic self-deadline plus a hard process-exit watchdog one second later.
    pub lifetime: Option<Duration>,
    /// Send `Frame::surface` after Ready and whenever the surface extent changes.
    pub report_surface: bool,
    pub centered_text: bool,
    /// Keep presenting the applied scene after the controller's pipes fail.
    /// New commands are never admitted or presented once detached.
    pub retain_on_disconnect: bool,
}

pub fn parse_backend(name: &str) -> Option<wgpu::Backends> {
    Some(match name {
        "gl" => wgpu::Backends::GL,
        "vulkan" => wgpu::Backends::VULKAN,
        "dx12" => wgpu::Backends::DX12,
        "metal" => wgpu::Backends::METAL,
        _ => return None,
    })
}

pub fn default_backend() -> &'static str {
    if cfg!(target_os = "windows") {
        "dx12"
    } else if cfg!(target_os = "macos") {
        "metal"
    } else {
        "vulkan"
    }
}

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}
impl Gpu {
    // No readback, device wait, file work, decoding, rasterization or logging.
    fn clear(&self, frame: wgpu::SurfaceTexture) {
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            // Unconfirmed startup diagnostic black, not Black mask semantics.
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        frame.present();
    }
    fn extent(&self) -> Extent {
        Extent {
            width: self.config.width,
            height: self.config.height,
        }
    }
}
struct Audience {
    config: Config,
    gpu: Option<Gpu>,
    pipes: PipeWorkers,
    session: RendererSession,
    start: Instant,
    next: Instant,
    compositor: Option<Arc<compositor::Compositor>>,
    preparation: Option<SyncSender<Frame>>,
    waiting: Option<Frame>,
    completions: Option<Receiver<Completion>>,
    ready: VecDeque<(Stamp, compositor::ReadyComposition)>,
    applied_ready: Option<compositor::ReadyComposition>,
    detached: bool,
    reported: Option<Extent>,
    resized_at: Instant,
    failure: Option<String>,
}
const SURFACE_SETTLE: Duration = Duration::from_millis(250);
struct Completion {
    stamp: Stamp,
    result: Result<(Command, compositor::ReadyComposition), PreparationFailure>,
}
enum PreparationFailure {
    Resource,
    Upload,
}
// One bounded backpressure slot; never admit/reject a later stamp ahead of
// already queued preparation. Retaining Frame also retains its original deadline.
fn forward_preparation(
    jobs: &SyncSender<Frame>,
    waiting: &mut Option<Frame>,
    mut poll: impl FnMut() -> io::Result<Option<Frame>>,
) -> io::Result<bool> {
    let frame = match waiting.take() {
        Some(frame) => frame,
        None => match poll()? {
            Some(frame) => frame,
            None => return Ok(false),
        },
    };
    frame.command_stamp()?;
    match jobs.try_send(frame) {
        Ok(()) => Ok(true),
        Err(mpsc::TrySendError::Full(frame)) => {
            *waiting = Some(frame);
            Ok(false)
        }
        Err(mpsc::TrySendError::Disconnected(_)) => Err(io::ErrorKind::BrokenPipe.into()),
    }
}
fn prepare_frame(
    frame: Frame,
    caps: RendererCapabilities,
    compositor: &compositor::Compositor,
    centered: bool,
) -> Result<(Command, compositor::ReadyComposition), PreparationFailure> {
    let command = frame
        .into_command(Instant::now(), caps)
        .map_err(|_| PreparationFailure::Resource)?;
    let cue = command.cue();
    let size = cue.extent();
    // Same 32px inset, explicit font, no-wrap policy as composition_spike.
    let mut alpha = vec![0; size.width as usize * size.height as usize];
    if let Some(t) = cue.text() {
        let width = size
            .width
            .checked_sub(64)
            .filter(|w| *w > 0)
            .ok_or(PreparationFailure::Resource)?;
        let height = size
            .height
            .checked_sub(64)
            .filter(|h| *h > 0)
            .ok_or(PreparationFailure::Resource)?;
        let raster = text::raster_aligned(
            t.font(),
            t.content(),
            width,
            height,
            f32::from(t.font_size()),
            centered,
        )
        .map_err(|_| PreparationFailure::Resource)?;
        for row in 0..height as usize {
            let start = (row + 32) * size.width as usize + 32;
            alpha[start..start + width as usize]
                .copy_from_slice(&raster[row * width as usize..(row + 1) * width as usize]);
        }
    }
    let background = match cue.background() {
        PreparedBackground::Color(rgba) => compositor::Image {
            width: 1,
            height: 1,
            rgba,
        },
        PreparedBackground::Image { extent, rgba, .. } => compositor::Image {
            width: extent.width,
            height: extent.height,
            rgba,
        },
    };
    let ready = compositor
        .prepare_native(
            size,
            background,
            &alpha,
            match cue.background() {
                PreparedBackground::Color(_) => compositor::Fit::Cover,
                PreparedBackground::Image { .. } => compositor::Fit::Contain,
            },
        )
        .map_err(|_| PreparationFailure::Upload)?;
    Ok((command, ready))
}
impl Audience {
    fn fail(&mut self, el: &ActiveEventLoop, message: String) {
        self.failure = Some(message);
        el.exit();
    }
    /// Lost controller: exit, or (if configured) keep the applied scene and
    /// stop admitting anything, including already accepted pending commands.
    fn lose_controller(&mut self, el: &ActiveEventLoop) {
        if self.config.retain_on_disconnect {
            self.detached = true;
            self.ready.clear();
            self.waiting = None;
        } else {
            el.exit();
        }
    }
    fn send(&mut self, el: &ActiveEventLoop, frame: Frame) {
        if !self.detached && self.pipes.try_send(frame).is_err() {
            self.lose_controller(el);
        }
    }
    /// Reports only a settled extent: entering fullscreen emits transient
    /// resizes on Windows, and a cue built for one would be RenderFailed.
    fn report_surface(&mut self, el: &ActiveEventLoop) {
        let Some(extent) = self.gpu.as_ref().map(Gpu::extent) else {
            return;
        };
        if self.config.report_surface
            && self.reported != Some(extent)
            && self.resized_at.elapsed() >= SURFACE_SETTLE
        {
            self.reported = Some(extent);
            self.send(el, Frame::surface(self.config.epoch, extent));
        }
    }
    fn window_attributes(
        &self,
        el: &ActiveEventLoop,
    ) -> Result<
        (
            winit::window::WindowAttributes,
            Option<winit::monitor::MonitorHandle>,
        ),
        String,
    > {
        let attributes = Window::default_attributes().with_title(self.config.title);
        Ok(match self.config.placement {
            Placement::Window { width, height } => (
                attributes.with_inner_size(winit::dpi::PhysicalSize::new(width, height)),
                None,
            ),
            Placement::Fullscreen(monitor) => {
                let monitors: Vec<_> = el.available_monitors().collect();
                let primary = el.primary_monitor();
                let target = match monitor {
                    Monitor::Index(index) => monitors.get(index).cloned(),
                    Monitor::FirstSecondary => monitors
                        .iter()
                        .find(|m| primary.as_ref() != Some(*m))
                        .cloned(),
                }
                .ok_or_else(|| match monitor {
                    Monitor::Index(index) => {
                        format!(
                            "audience monitor {index} not found ({} available)",
                            monitors.len()
                        )
                    }
                    Monitor::FirstSecondary => "no secondary audience monitor".into(),
                })?;
                // Startup-only provenance on stderr; stdout stays binary.
                eprintln!(
                    "audience monitor {:?} {}x{} at {},{}",
                    target.name(),
                    target.size().width,
                    target.size().height,
                    target.position().x,
                    target.position().y
                );
                // Operator keeps keyboard focus. winit's set_fullscreen activates
                // the window on Windows, so the output is a non-activated
                // borderless window covering the monitor instead.
                (
                    attributes
                        .with_position(target.position())
                        .with_inner_size(target.size())
                        .with_decorations(false)
                        .with_resizable(false)
                        .with_active(false),
                    Some(target),
                )
            }
        })
    }
}
impl ApplicationHandler for Audience {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() || self.failure.is_some() {
            return;
        }
        let (attributes, target) = match self.window_attributes(el) {
            Ok(attributes) => attributes,
            Err(message) => return self.fail(el, message),
        };
        let window = match el.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => return self.fail(el, format!("cannot create audience window: {error}")),
        };
        // winit sizes the window before moving it, so a target monitor with a
        // different scale factor rescales it on arrival; size it again there.
        if let Some(target) = target {
            window.set_outer_position(target.position());
            let _ = window.request_inner_size(target.size());
        }
        // Blocking adapter/device initialization is confined to startup, before
        // Ready or any cue can enter the live session. Supervisor bounds startup.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: self.config.backend,
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(el.owned_display_handle()))
        });
        let surface = match instance.create_surface(window.clone()) {
            Ok(surface) => surface,
            Err(error) => return self.fail(el, format!("cannot create surface: {error}")),
        };
        let adapter =
            match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::from_env().unwrap_or_default(),
                compatible_surface: Some(&surface),
                ..Default::default()
            })) {
                Ok(adapter) => adapter,
                Err(error) => return self.fail(el, format!("no compatible adapter: {error}")),
            };
        // Startup-only provenance, before Ready/live frames; stdout stays binary.
        eprintln!("audience adapter {:?}", adapter.get_info());
        let (device, queue) =
            match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())) {
                Ok(pair) => pair,
                Err(error) => return self.fail(el, format!("cannot create device: {error}")),
            };
        let size = window.inner_size();
        let Some(mut config) =
            surface.get_default_config(&adapter, size.width.max(1), size.height.max(1))
        else {
            return self.fail(el, "surface unsupported by adapter".into());
        };
        let Some(format) = surface
            .get_capabilities(&adapter)
            .formats
            .into_iter()
            .find(wgpu::TextureFormat::is_srgb)
        else {
            return self.fail(el, "native composition requires an sRGB surface".into());
        };
        config.format = format;
        surface.configure(&device, &config);
        let max = device.limits().max_texture_dimension_2d.min(8192);
        let compositor = Arc::new(compositor::Compositor::from_device(
            device.clone(),
            queue.clone(),
            adapter.get_info(),
            config.format,
        ));
        let (jobs, work) = mpsc::sync_channel::<Frame>(1);
        let (results, completions) = mpsc::sync_channel(1);
        let worker = compositor.clone();
        let centered = self.config.centered_text;
        let spawned = std::thread::Builder::new()
            .name("audience-prepare".into())
            .spawn(move || {
                while let Ok(frame) = work.recv() {
                    let Ok(stamp) = frame.command_stamp() else {
                        break;
                    };
                    let result = prepare_frame(
                        frame,
                        RendererCapabilities {
                            max_texture_dimension: max,
                        },
                        &worker,
                        centered,
                    );
                    let fatal = matches!(result, Err(PreparationFailure::Upload));
                    if results.send(Completion { stamp, result }).is_err() {
                        break;
                    }
                    if fatal {
                        break;
                    } // Never accumulate uploads after a failed GPU wait.
                }
            });
        if spawned.is_err() {
            return self.fail(el, "cannot start preparation worker".into());
        }
        self.compositor = Some(compositor);
        self.preparation = Some(jobs);
        self.completions = Some(completions);
        self.gpu = Some(Gpu {
            window,
            surface,
            device,
            queue,
            config,
        });
        self.resized_at = Instant::now();
        self.send(el, Frame::ready(self.config.epoch, max));
    }
    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(g) = self.gpu.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) if s.width > 0 && s.height > 0 => {
                g.config.width = s.width;
                g.config.height = s.height;
                g.surface.configure(&g.device, &g.config);
                self.resized_at = Instant::now();
            }
            WindowEvent::RedrawRequested => {
                let size = g.window.inner_size();
                if size.width == 0 || size.height == 0 {
                    return;
                }
                let (frame, reconfigure) = match g.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) => (t, false),
                    wgpu::CurrentSurfaceTexture::Suboptimal(t) => (t, true),
                    wgpu::CurrentSurfaceTexture::Outdated => {
                        g.surface.configure(&g.device, &g.config);
                        return;
                    }
                    wgpu::CurrentSurfaceTexture::Timeout
                    | wgpu::CurrentSurfaceTexture::Occluded => return,
                    _ => {
                        el.exit();
                        return;
                    }
                };
                if !self.detached && self.session.pending().is_some() {
                    let stamp = self.session.pending().unwrap().stamp();
                    let Some((prepared_stamp, ready)) = self.ready.pop_front() else {
                        el.exit();
                        return;
                    };
                    if stamp != prepared_stamp {
                        el.exit();
                        return;
                    }
                    let compositor = self.compositor.as_ref().unwrap();
                    if let Some(ack) = self.session.present(Instant::now(), |cue| {
                        if cue.extent().width != g.config.width
                            || cue.extent().height != g.config.height
                        {
                            return Err(DeliveryError::RenderFailed);
                        }
                        compositor
                            .submit_native(&frame.texture.create_view(&Default::default()), &ready);
                        g.window.pre_present_notify();
                        frame.present();
                        Ok(())
                    }) {
                        if ack.outcome == Outcome::Applied {
                            self.applied_ready = Some(ready);
                        }
                        let reconfigure_after = reconfigure;
                        self.send(el, Frame::acknowledgment(ack));
                        if reconfigure_after && let Some(g) = &self.gpu {
                            g.surface.configure(&g.device, &g.config);
                        }
                        return;
                    }
                } else if let Some(ready) = &self.applied_ready {
                    // Do not sample an old fixed-extent mask after resize.
                    let cue = self.session.applied().unwrap();
                    if cue.extent().width == g.config.width
                        && cue.extent().height == g.config.height
                    {
                        self.compositor
                            .as_ref()
                            .unwrap()
                            .submit_native(&frame.texture.create_view(&Default::default()), ready);
                        g.window.pre_present_notify();
                        frame.present();
                    }
                } else {
                    g.clear(frame);
                }
                if reconfigure {
                    g.surface.configure(&g.device, &g.config);
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self
            .config
            .lifetime
            .is_some_and(|lifetime| self.start.elapsed() >= lifetime)
        {
            el.exit();
            return;
        }
        if self.gpu.is_none() {
            return;
        }
        if !self.detached && self.poll_session(el).is_err() {
            return;
        }
        self.report_surface(el);
        let now = Instant::now();
        if now >= self.next {
            self.gpu.as_ref().unwrap().window.request_redraw();
            self.next = now + Duration::from_micros(16_667);
        }
        el.set_control_flow(ControlFlow::WaitUntil(self.next));
    }
}
impl Audience {
    /// Err means the event loop is exiting.
    fn poll_session(&mut self, el: &ActiveEventLoop) -> Result<(), ()> {
        // Only completed uploads cross into the native session; failures never
        // supply a replacement. Worker FIFO plus session ordering filters stale.
        for _ in 0..2 {
            match self.completions.as_ref().unwrap().try_recv() {
                Ok(completion) => {
                    let ack = match completion.result {
                        Ok((command, ready)) => {
                            let ack = self.session.accept(command, Instant::now());
                            if ack.outcome == Outcome::Accepted
                                && !self.ready.iter().any(|(s, _)| *s == ack.stamp)
                            {
                                self.ready.push_back((ack.stamp, ready));
                            }
                            ack
                        }
                        Err(PreparationFailure::Resource) => {
                            self.session.reject_preparation(completion.stamp)
                        }
                        Err(PreparationFailure::Upload) => {
                            el.exit();
                            return Err(()); // GPU uncertainty retires session, no inferred Applied.
                        }
                    };
                    self.send(el, Frame::acknowledgment(ack));
                    if self.detached {
                        return Ok(());
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    el.exit();
                    return Err(());
                }
            }
        }
        // Hard per-tick work limit; pipe flood cannot monopolize frame scheduling.
        for _ in 0..2 {
            match forward_preparation(
                self.preparation.as_ref().unwrap(),
                &mut self.waiting,
                || self.pipes.poll(),
            ) {
                Ok(true) => {}
                Ok(false) => break,
                Err(_) => {
                    self.lose_controller(el);
                    return if self.detached { Ok(()) } else { Err(()) };
                }
            }
        }
        Ok(())
    }
}

pub fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    if config.epoch.0 == 0 {
        return Err("epoch must be nonzero and never reused".into());
    }
    let start = Instant::now();
    if let Some(lifetime) = config.lifetime {
        // Hard independent diagnostic lifetime, including stuck startup/device calls.
        // No logging or graceful GPU teardown on this watchdog path. Supervisor must
        // mark any pending delivery unknown and reap; never automatically replay.
        std::thread::Builder::new()
            .name("audience-lifetime".into())
            .spawn(move || {
                std::thread::sleep(lifetime + Duration::from_secs(1));
                std::process::exit(124);
            })?;
    }
    let pipes = PipeWorkers::new(io::stdin(), io::stdout())?;
    let mut app = Audience {
        session: RendererSession::new(config.epoch),
        config,
        gpu: None,
        pipes,
        start,
        next: start,
        compositor: None,
        preparation: None,
        waiting: None,
        completions: None,
        ready: VecDeque::with_capacity(2),
        applied_ready: None,
        detached: false,
        reported: None,
        resized_at: start,
        failure: None,
    };
    EventLoop::new()?.run_app(&mut app)?;
    match app.failure {
        Some(message) => Err(message.into()),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sela::{
        delivery::{Delivery, Lane},
        scene::{ContentVersion, PreparedCue},
    };

    fn frame(delivery: &mut Delivery, lane: Lane, revision: u64) -> Frame {
        let now = Instant::now();
        let cue = PreparedCue::diagnostic_color(
            ContentVersion { id: 17, revision },
            Extent {
                width: 641,
                height: 360,
            },
            [37, 59, 83, 255],
            RendererCapabilities {
                max_texture_dimension: 4096,
            },
        )
        .unwrap();
        delivery
            .submit(Arc::new(cue), lane, now, now + Duration::from_secs(3))
            .unwrap();
        let mut wire = Vec::new();
        Frame::command(&delivery.take_next().unwrap(), now)
            .unwrap()
            .write(&mut wire)
            .unwrap();
        Frame::read(wire.as_slice()).unwrap()
    }

    #[test]
    fn operator_slide_cues_fit_the_audience_text_preparer() {
        let caps = RendererCapabilities {
            max_texture_dimension: 8192,
        };
        let texts = [
            "One".to_string(),
            "First original line\nSecond original line".to_string(),
            "W".repeat(48),
            "WWWW MMMM\n".repeat(31) + "WWWW MMMM",
            "Café a\u{301} déjà vu — \u{201c}quoted\u{201d}".to_string(),
        ];
        for (width, height) in [
            (640, 360),
            (1280, 720),
            (1920, 1080),
            (2560, 1600),
            (4160, 2160),
        ] {
            let extent = Extent { width, height };
            for text in &texts {
                let slide = sela::slides::Slide {
                    label: String::new(),
                    text: text.clone(),
                };
                let version = ContentVersion { id: 1, revision: 1 };
                let cue = sela::slides::cue(version, &slide, extent, caps).unwrap();
                let t = cue.text().unwrap();
                text::raster_aligned(
                    t.font(),
                    t.content(),
                    width - 64,
                    height - 64,
                    f32::from(t.font_size()),
                    true,
                )
                .unwrap_or_else(|e| panic!("{width}x{height} {text:?}: {e:?}"));
            }
        }
    }

    #[test]
    fn backend_names_are_explicit() {
        assert_eq!(parse_backend("dx12"), Some(wgpu::Backends::DX12));
        assert_eq!(parse_backend("vulkan"), Some(wgpu::Backends::VULKAN));
        assert_eq!(parse_backend("auto"), None);
        assert!(parse_backend(default_backend()).is_some());
    }

    #[test]
    fn backpressured_preparation_keeps_earlier_cue_and_safety_in_order() {
        for lanes in [[Lane::Cue, Lane::Safety], [Lane::Safety, Lane::Cue]] {
            let mut delivery = Delivery::new(Epoch(83));
            let mut source = VecDeque::from([
                frame(&mut delivery, lanes[0], 7),
                frame(&mut delivery, lanes[1], 29),
            ]);
            let mut renderer = RendererSession::new(Epoch(83));
            let (jobs, worker) = mpsc::sync_channel(1);
            let mut waiting = None;
            // Deliberately withhold the worker receive: second legal lane must
            // backpressure, not advance consumed past the queued first stamp.
            assert!(forward_preparation(&jobs, &mut waiting, || Ok(source.pop_front())).unwrap());
            assert!(!forward_preparation(&jobs, &mut waiting, || Ok(source.pop_front())).unwrap());
            assert_eq!(
                waiting.as_ref().unwrap().command_stamp().unwrap().sequence,
                2
            );
            for _ in 0..10 {
                assert!(
                    !forward_preparation(&jobs, &mut waiting, || panic!(
                        "must not drain pipe while full"
                    ))
                    .unwrap()
                );
            }
            for revision in [7, 29] {
                let command = worker
                    .try_recv()
                    .unwrap()
                    .into_command(
                        Instant::now(),
                        RendererCapabilities {
                            max_texture_dimension: 4096,
                        },
                    )
                    .unwrap();
                let accepted = renderer.accept(command, Instant::now());
                assert_eq!(accepted.outcome, Outcome::Accepted);
                assert!(delivery.acknowledge(accepted, Instant::now()));
                let applied = renderer
                    .present(Instant::now(), |cue| {
                        assert_eq!(cue.version().revision, revision);
                        Ok(())
                    })
                    .unwrap();
                assert!(delivery.acknowledge(applied, Instant::now()));
                if revision == 7 {
                    assert!(
                        forward_preparation(&jobs, &mut waiting, || panic!(
                            "retained frame comes first"
                        ))
                        .unwrap()
                    );
                }
            }
            assert!(waiting.is_none());
            assert_eq!(renderer.applied().unwrap().version().revision, 29);
            assert_eq!(
                delivery.live(),
                sela::delivery::LiveState::Confirmed(ContentVersion {
                    id: 17,
                    revision: 29
                })
            );
        }
    }

    #[test]
    fn retained_preparation_keeps_deadline_and_worker_disconnect_is_fatal() {
        let mut delivery = Delivery::new(Epoch(83));
        let first = frame(&mut delivery, Lane::Cue, 7);
        let second = frame(&mut delivery, Lane::Safety, 29);
        let (jobs, worker) = mpsc::sync_channel(1);
        jobs.try_send(first).ok().unwrap();
        let mut waiting = Some(second);
        assert!(!forward_preparation(&jobs, &mut waiting, || panic!("no new input")).unwrap());
        worker.try_recv().unwrap();
        assert!(forward_preparation(&jobs, &mut waiting, || panic!("no new input")).unwrap());
        let later = Instant::now() + Duration::from_secs(6);
        let command = worker
            .try_recv()
            .unwrap()
            .into_command(
                later,
                RendererCapabilities {
                    max_texture_dimension: 4096,
                },
            )
            .unwrap();
        let mut renderer = RendererSession::new(Epoch(83));
        assert_eq!(
            renderer.accept(command, later).outcome,
            Outcome::Rejected(DeliveryError::TimedOut)
        );
        assert!(renderer.pending().is_none());
        drop(worker);
        let mut other = Delivery::new(Epoch(97));
        waiting = Some(frame(&mut other, Lane::Cue, 41));
        assert_eq!(
            forward_preparation(&jobs, &mut waiting, || panic!("no new input"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}
