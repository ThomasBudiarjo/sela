//! Opt-in audience child; stdin/stdout are binary protocol, never logs.
//! Bounded worker preparation and native submission. No operator/mask policy.
#[allow(dead_code)] // Shared offscreen helper also exposes diagnostic-only readback.
#[path = "composition/gpu.rs"]
mod composition;
#[path = "composition/text.rs"]
mod text;
use sela::{
    delivery::{Command, DeliveryError, Epoch, Outcome, RendererSession, Stamp},
    scene::{PreparedBackground, PreparedCue, RendererCapabilities},
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

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}
impl Gpu {
    // No readback, device wait, file work, decoding, rasterization or logging.
    fn submit(
        &self,
        frame: wgpu::SurfaceTexture,
        cue: Option<&PreparedCue>,
    ) -> Result<(), DeliveryError> {
        let color = match cue {
            Some(cue) => {
                if cue.text().is_some()
                    || cue.extent().width != self.config.width
                    || cue.extent().height != self.config.height
                {
                    return Err(DeliveryError::RenderFailed);
                }
                let PreparedBackground::Color(c) = cue.background() else {
                    return Err(DeliveryError::RenderFailed);
                };
                if c[3] != 255 {
                    return Err(DeliveryError::RenderFailed);
                }
                *c
            }
            None => [0, 0, 0, 255], // Unconfirmed startup diagnostic, not Black semantics.
        };
        let channel = |c: u8| {
            let c = f64::from(c) / 255.;
            if self.config.format.is_srgb() {
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            } else {
                c
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: channel(color[0]),
                            g: channel(color[1]),
                            b: channel(color[2]),
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        frame.present();
        Ok(()) // Submission/present call only; not completion or physical scanout.
    }
}
struct Audience {
    gpu: Option<Gpu>,
    pipes: PipeWorkers,
    session: RendererSession,
    epoch: Epoch,
    backend: wgpu::Backends,
    start: Instant,
    next: Instant,
    compositor: Option<Arc<composition::Compositor>>,
    preparation: Option<SyncSender<Frame>>,
    waiting: Option<Frame>,
    completions: Option<Receiver<Completion>>,
    ready: VecDeque<(Stamp, composition::ReadyComposition)>,
    applied_ready: Option<composition::ReadyComposition>,
}
struct Completion {
    stamp: Stamp,
    result: Result<(Command, composition::ReadyComposition), PreparationFailure>,
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
    compositor: &composition::Compositor,
) -> Result<(Command, composition::ReadyComposition), PreparationFailure> {
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
        let raster = text::raster(
            t.font(),
            t.content(),
            width,
            height,
            f32::from(t.font_size()),
        )
        .map_err(|_| PreparationFailure::Resource)?;
        for row in 0..height as usize {
            let start = (row + 32) * size.width as usize + 32;
            alpha[start..start + width as usize]
                .copy_from_slice(&raster[row * width as usize..(row + 1) * width as usize]);
        }
    }
    let background = match cue.background() {
        PreparedBackground::Color(rgba) => composition::Image {
            width: 1,
            height: 1,
            rgba,
        },
        PreparedBackground::Image { extent, rgba, .. } => composition::Image {
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
                PreparedBackground::Color(_) => composition::Fit::Cover,
                PreparedBackground::Image { .. } => composition::Fit::Contain,
            },
        )
        .map_err(|_| PreparationFailure::Upload)?;
    Ok((command, ready))
}
impl ApplicationHandler for Audience {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let window = Arc::new(
            el.create_window(
                Window::default_attributes()
                    .with_title("Sela M0-06b native cue diagnostic")
                    .with_inner_size(winit::dpi::PhysicalSize::new(641, 360)),
            )
            .unwrap(),
        );
        // Blocking adapter/device initialization is confined to startup, before
        // Ready or any cue can enter the live session. Supervisor bounds startup.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: self.backend,
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(el.owned_display_handle()))
        });
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .unwrap();
        // Startup-only provenance, before Ready/live frames; stdout stays binary.
        eprintln!("M0-07d adapter {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .unwrap();
        config.format = surface
            .get_capabilities(&adapter)
            .formats
            .into_iter()
            .find(wgpu::TextureFormat::is_srgb)
            .expect("native composition requires sRGB surface");
        surface.configure(&device, &config);
        let max = device.limits().max_texture_dimension_2d.min(8192);
        let compositor = Arc::new(composition::Compositor::from_device(
            device.clone(),
            queue.clone(),
            adapter.get_info(),
            config.format,
        ));
        let (jobs, work) = mpsc::sync_channel::<Frame>(1);
        let (results, completions) = mpsc::sync_channel(1);
        let worker = compositor.clone();
        std::thread::Builder::new()
            .name("native-cue-prepare".into())
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
                    );
                    let fatal = matches!(result, Err(PreparationFailure::Upload));
                    if results.send(Completion { stamp, result }).is_err() {
                        break;
                    }
                    if fatal {
                        break;
                    } // Never accumulate uploads after a failed GPU wait.
                }
            })
            .unwrap();
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
        if self.pipes.try_send(Frame::ready(self.epoch, max)).is_err() {
            el.exit();
        }
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
                if self.session.pending().is_some() {
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
                        if self.pipes.try_send(Frame::acknowledgment(ack)).is_err() {
                            el.exit();
                        }
                    }
                } else {
                    if let Some(ready) = &self.applied_ready {
                        // Do not sample an old fixed-extent mask after resize.
                        let cue = self.session.applied().unwrap();
                        if cue.extent().width == g.config.width
                            && cue.extent().height == g.config.height
                        {
                            self.compositor.as_ref().unwrap().submit_native(
                                &frame.texture.create_view(&Default::default()),
                                ready,
                            );
                            g.window.pre_present_notify();
                            frame.present();
                        }
                    } else {
                        let _ = g.submit(frame, None);
                    }
                }
                if reconfigure {
                    g.surface.configure(&g.device, &g.config);
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.start.elapsed() >= Duration::from_secs(24) {
            el.exit();
            return;
        }
        let Some(g) = &self.gpu else {
            return;
        };
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
                            return; // GPU uncertainty retires session, no inferred Applied.
                        }
                    };
                    if self.pipes.try_send(Frame::acknowledgment(ack)).is_err() {
                        el.exit();
                        return;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    el.exit();
                    return;
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
                    el.exit();
                    return;
                }
            }
        }
        let now = Instant::now();
        if now >= self.next {
            g.window.request_redraw();
            self.next = now + Duration::from_micros(16_667);
        }
        el.set_control_flow(ControlFlow::WaitUntil(self.next));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: native_cues <fresh-nonzero-epoch-hex> <gl|vulkan|dx12|metal>".into());
    }
    let epoch = Epoch(u128::from_str_radix(&args[1], 16)?);
    if epoch.0 == 0 {
        return Err("epoch must be nonzero and never reused".into());
    }
    let backend = match args[2].as_str() {
        "gl" => wgpu::Backends::GL,
        "vulkan" => wgpu::Backends::VULKAN,
        "dx12" => wgpu::Backends::DX12,
        "metal" => wgpu::Backends::METAL,
        _ => return Err("unsupported backend; no fallback".into()),
    };
    let start = Instant::now();
    // Hard independent diagnostic lifetime, including stuck startup/device calls.
    // No logging or graceful GPU teardown on this watchdog path. Supervisor must
    // mark any pending delivery unknown and reap; never automatically replay.
    std::thread::Builder::new()
        .name("cue-lifetime".into())
        .spawn(|| {
            std::thread::sleep(Duration::from_secs(25));
            std::process::exit(124);
        })?;
    let pipes = PipeWorkers::new(io::stdin(), io::stdout())?;
    let mut app = Audience {
        gpu: None,
        pipes,
        session: RendererSession::new(epoch),
        epoch,
        backend,
        start,
        next: start,
        compositor: None,
        preparation: None,
        waiting: None,
        completions: None,
        ready: VecDeque::with_capacity(2),
        applied_ready: None,
    };
    EventLoop::new()?.run_app(&mut app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sela::{
        delivery::{Delivery, Lane},
        scene::{ContentVersion, Extent},
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
