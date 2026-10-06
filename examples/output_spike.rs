//! Original diagnostic prototype; intentionally opt-in and bounded to 25 seconds.
use gpui::{prelude::*, *};
use std::{
    sync::{
        Arc, OnceLock,
        mpsc::{SyncSender, sync_channel},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window as NativeWindow, WindowId},
};

static TELEMETRY: OnceLock<SyncSender<String>> = OnceLock::new();
fn stamp(event: &str) {
    let mut line = format!(
        "{} {} {}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros(),
        event
    );
    let mut boundary = line.len().min(1024);
    while !line.is_char_boundary(boundary) {
        boundary -= 1;
    }
    line.truncate(boundary);
    // Drop telemetry under pressure, never block the frame/UI path on log I/O.
    if let Some(tx) = TELEMETRY.get() {
        let _ = tx.try_send(line);
    }
}
actions!(spike, [Stall100, Stall500, Stall2000, PrepareDelayed, Exit]);
fn stall(ms: u64) {
    stamp(&format!("stall_begin {ms}"));
    std::thread::sleep(Duration::from_millis(ms));
    stamp(&format!("stall_end {ms}"));
}
struct Operator {
    focus: FocusHandle,
    preparation: Option<Task<()>>,
}
impl Operator {
    fn prepare_delayed(&mut self, cx: &mut Context<Self>) {
        if self.preparation.is_some() {
            stamp("preparation_busy");
            return;
        }
        // Controlled latency injection only, not a decoder or prepared scene.
        // One in-flight worker; repeated input is rejected, never queued.
        let worker = cx.background_executor().spawn(async {
            stamp("preparation_begin");
            std::thread::sleep(Duration::from_secs(2));
            stamp("preparation_end");
        });
        self.preparation = Some(cx.spawn(async move |this, cx| {
            worker.await;
            let _ = this.update(cx, |this, cx| {
                this.preparation = None;
                stamp("preparation_observed");
                cx.notify();
            });
        }));
        cx.notify();
    }
}
impl Render for Operator {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().key_context("Spike").track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &Stall100, _, _| stall(100)))
            .on_action(cx.listener(|_, _: &Stall500, _, _| stall(500)))
            .on_action(cx.listener(|_, _: &Stall2000, _, _| stall(2000)))
            .on_action(cx.listener(|this, _: &PrepareDelayed, _, cx| this.prepare_delayed(cx)))
            .on_action(cx.listener(|_, _: &Exit, _, cx| { stamp("operator_exit"); cx.quit(); }))
            .size_full().p_8().bg(rgb(0xf4f6f9)).text_color(rgb(0x182536))
            .flex().flex_col().gap_4()
            .child("M0-04 diagnostic operator (not service UI)")
            .child("Ctrl+1 / Ctrl+2 / Ctrl+3: stall UI 100 / 500 / 2000 ms")
            .child(div().id("stall100").on_click(cx.listener(|_, _, _, _| stall(100))).child("Stall 100 ms"))
            .child(div().id("stall500").on_click(cx.listener(|_, _, _, _| stall(500))).child("Stall 500 ms"))
            .child(div().id("stall2000").on_click(cx.listener(|_, _, _, _| stall(2000))).child("Stall 2000 ms"))
            .child(div().id("prepare").on_click(cx.listener(|this, _, _, cx| this.prepare_delayed(cx))).child(
                if self.preparation.is_some() { "Preparing on worker · repeated requests rejected" }
                else { "Prepare with 2s delay · Ctrl+P" }
            ))
            .child(div().id("exit").on_click(cx.listener(|_, _, _, cx| { stamp("operator_exit"); cx.quit(); })).child("Exit operator · Ctrl+Q"))
            .child("Audience: close its titlebar to stop; automatic deadline (25s default). Driver owns both processes.")
    }
}
struct Gpu {
    window: Arc<NativeWindow>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
}
struct Audience {
    gpu: Option<Gpu>,
    start: Instant,
    next: Instant,
    frames: u64,
    deadline: Duration,
    monitor: Option<String>,
}
fn describe(m: &winit::monitor::MonitorHandle) -> String {
    let (p, s) = (m.position(), m.size());
    // Name last: platform monitor names may contain spaces.
    format!(
        "{}x{}+{}+{} scale {} refresh_mhz {} {}",
        s.width,
        s.height,
        p.x,
        p.y,
        m.scale_factor(),
        m.refresh_rate_millihertz().unwrap_or(0),
        m.name().unwrap_or_default()
    )
}
impl Audience {
    fn note_monitor(&mut self) {
        let Some(g) = &self.gpu else { return };
        let Some(m) = g.window.current_monitor() else {
            return;
        };
        let name = m.name();
        if name != self.monitor {
            stamp(&format!("monitor_current {}", describe(&m)));
            self.monitor = name;
        }
    }
}
impl ApplicationHandler for Audience {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let monitors: Vec<_> = el.available_monitors().collect();
        for (i, m) in monitors.iter().enumerate() {
            stamp(&format!("monitor {i} {}", describe(m)));
        }
        // Explicit opt-in placement; no silent fallback to another display.
        let target = std::env::var("SELA_SPIKE_MONITOR").ok().map(|v| {
            let index: usize = v.parse().expect("SELA_SPIKE_MONITOR must be an index");
            monitors
                .get(index)
                .cloned()
                .expect("SELA_SPIKE_MONITOR out of range")
        });
        let fullscreen = std::env::var_os("SELA_SPIKE_FULLSCREEN").is_some();
        let attributes = NativeWindow::default_attributes()
            .with_title("Sela audience spike")
            .with_inner_size(winit::dpi::LogicalSize::new(640., 360.));
        let attributes = match target {
            Some(m) if fullscreen => {
                attributes.with_fullscreen(Some(winit::window::Fullscreen::Borderless(Some(m))))
            }
            Some(m) => attributes.with_position(winit::dpi::PhysicalPosition::new(
                m.position().x + 100,
                m.position().y + 100,
            )),
            None if fullscreen => panic!("SELA_SPIKE_FULLSCREEN requires SELA_SPIKE_MONITOR"),
            None => attributes.with_position(winit::dpi::LogicalPosition::new(800., 100.)),
        };
        let window = Arc::new(el.create_window(attributes).unwrap());
        let backend = match std::env::var("SELA_SPIKE_BACKEND")
            .as_deref()
            .unwrap_or("gl")
        {
            "gl" => wgpu::Backends::GL,
            "vulkan" => wgpu::Backends::VULKAN,
            "dx12" => wgpu::Backends::DX12,
            other => panic!("unsupported backend {other}"),
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: backend,
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(el.owned_display_handle()))
        });
        let power_preference = wgpu::PowerPreference::from_env().unwrap_or_default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .unwrap();
        stamp(&format!("adapter {:?}", adapter.get_info()));
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let s = window.inner_size();
        let config = surface
            .get_default_config(&adapter, s.width.max(1), s.height.max(1))
            .unwrap();
        surface.configure(&device, &config);
        stamp(&format!(
            "surface {:?} {:?} {:?} latency {}",
            config.format,
            config.present_mode,
            config.alpha_mode,
            config.desired_maximum_frame_latency
        ));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl("@vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4f { var p=array<vec2f,3>(vec2f(-1,-1),vec2f(1,-1),vec2f(0,1)); return vec4f(p[i],0,1); } @fragment fn fs()->@location(0) vec4f { return vec4f(1,0.7,0.1,1); }".into()) });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(config.format.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        self.gpu = Some(Gpu {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
        });
        self.note_monitor();
    }
    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match &event {
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                stamp(&format!("scale_factor {scale_factor}"));
                self.note_monitor();
            }
            WindowEvent::Moved(_) => self.note_monitor(),
            _ => {}
        }
        let Some(g) = self.gpu.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => {
                stamp("audience_close");
                el.exit();
            }
            WindowEvent::Resized(s) => {
                stamp(&format!("resize {} {}", s.width, s.height));
                if s.width > 0 && s.height > 0 {
                    g.config.width = s.width;
                    g.config.height = s.height;
                    g.surface.configure(&g.device, &g.config);
                }
            }
            WindowEvent::RedrawRequested => {
                let s = g.window.inner_size();
                if s.width == 0 || s.height == 0 {
                    return;
                }
                let mut reconfigure = false;
                let frame = match g.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) => t,
                    wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                        reconfigure = true;
                        t
                    }
                    wgpu::CurrentSurfaceTexture::Outdated => {
                        g.surface.configure(&g.device, &g.config);
                        stamp("surface_outdated");
                        return;
                    }
                    wgpu::CurrentSurfaceTexture::Timeout
                    | wgpu::CurrentSurfaceTexture::Occluded => {
                        stamp("surface_skip");
                        return;
                    }
                    other => {
                        stamp(&format!("surface_fatal {other:?}"));
                        el.exit();
                        return;
                    }
                };
                let view = frame.texture.create_view(&Default::default());
                let mut encoder = g.device.create_command_encoder(&Default::default());
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.03,
                                    g: 0.08,
                                    b: 0.2,
                                    a: 1.,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        ..Default::default()
                    });
                    pass.set_pipeline(&g.pipeline);
                    let width = (g.config.width as f32 / 5.).max(1.);
                    let x = motion(self.start.elapsed().as_secs_f32())
                        * (g.config.width as f32 - width);
                    pass.set_viewport(x, 0., width, g.config.height as f32, 0., 1.);
                    pass.draw(0..3, 0..1);
                }
                g.queue.submit([encoder.finish()]);
                g.window.pre_present_notify();
                frame.present();
                drop(view);
                if reconfigure {
                    g.surface.configure(&g.device, &g.config);
                }
                self.frames += 1;
                stamp(&format!(
                    "present_call {} {}",
                    self.frames,
                    self.start.elapsed().as_micros()
                ));
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.start.elapsed() >= self.deadline {
            stamp("audience_deadline");
            el.exit();
            return;
        }
        let now = Instant::now();
        if now >= self.next {
            if let Some(g) = &self.gpu {
                g.window.request_redraw();
            }
            self.next = now + Duration::from_micros(16_667);
        }
        el.set_control_flow(ControlFlow::WaitUntil(self.next));
    }
}
fn motion(seconds: f32) -> f32 {
    (seconds * 2.).sin() * 0.5 + 0.5
}
fn main() {
    let (tx, rx) = sync_channel::<String>(2048);
    TELEMETRY.set(tx).unwrap();
    let logger = std::thread::spawn(move || {
        while let Ok(line) = rx.recv() {
            if line == "__done" {
                break;
            }
            println!("{line}");
        }
    });
    if std::env::args().any(|a| a == "--audience") {
        // Longer soaks are opt-in and still self-terminate; 600s bounds a lost supervisor.
        let seconds = std::env::var("SELA_SPIKE_SECONDS")
            .map(|v| {
                v.parse::<u64>()
                    .expect("SELA_SPIKE_SECONDS must be an integer")
            })
            .unwrap_or(25)
            .clamp(1, 600);
        let now = Instant::now();
        EventLoop::new()
            .unwrap()
            .run_app(&mut Audience {
                gpu: None,
                start: now,
                next: now,
                frames: 0,
                deadline: Duration::from_secs(seconds),
                monitor: None,
            })
            .unwrap();
    } else {
        gpui_platform::application().run(|cx: &mut App| {
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.bind_keys([
                KeyBinding::new("ctrl-1", Stall100, Some("Spike")),
                KeyBinding::new("ctrl-2", Stall500, Some("Spike")),
                KeyBinding::new("ctrl-3", Stall2000, Some("Spike")),
                KeyBinding::new("ctrl-p", PrepareDelayed, Some("Spike")),
                KeyBinding::new("ctrl-q", Exit, Some("Spike")),
            ]);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(30.), px(80.)),
                        size(px(700.), px(480.)),
                    ))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Sela output operator spike".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| Operator {
                        focus: cx.focus_handle(),
                        preparation: None,
                    });
                    view.read(cx).focus.clone().focus(window, cx);
                    view
                },
            )
            .unwrap();
            cx.activate(true);
        });
    }
    // Joining is outside UI/render execution; normal exit flushes bounded telemetry.
    let _ = TELEMETRY.get().unwrap().send("__done".into());
    let _ = logger.join();
}
#[cfg(test)]
mod tests {
    #[test]
    fn motion_is_bounded_and_changes() {
        for i in 0..10000 {
            assert!((0.0..=1.0).contains(&super::motion(i as f32 / 60.)));
        }
        assert_ne!(super::motion(0.), super::motion(0.5));
    }
}
