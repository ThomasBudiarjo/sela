//! Opt-in synchronous offscreen diagnostic, never a live-frame API.
use sela::scene::{Extent, MAX_SCENE_BYTES, TextStyle};
use std::{error::Error, time::Duration};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Copy, Debug)]
pub enum Fit {
    Contain,
    Cover,
}

/// One blended text layer: a linear-light color and an opacity (0..1).
#[derive(Clone, Copy)]
pub struct Layer {
    pub rgb: [f32; 3],
    pub opacity: f32,
}

impl Layer {
    /// Linear-light white at full opacity.
    pub const WHITE: Layer = Layer {
        rgb: [1., 1., 1.],
        opacity: 1.,
    };
    /// An inactive layer: nothing blends over the background.
    pub const OFF: Layer = Layer {
        rgb: [0., 0., 0.],
        opacity: 0.,
    };
}

/// Shadow, outline and fill blend parameters, back to front. The mask's
/// coverage channels carry no color; colors live only here.
#[derive(Clone, Copy)]
pub struct Blend {
    pub shadow: Layer,
    pub outline: Layer,
    pub fill: Layer,
}

impl Blend {
    /// White text with no outline or shadow: the pre-style look.
    pub fn plain() -> Self {
        Self {
            shadow: Layer::OFF,
            outline: Layer::OFF,
            fill: Layer::WHITE,
        }
    }

    /// Linear-light colors and opacities from a cue style. Absent effects
    /// blend nothing even if a stale mask channel had coverage.
    pub fn from_style(style: &TextStyle) -> Self {
        let effect = |color: [u8; 3], opacity: u8| Layer {
            rgb: [linear(color[0]), linear(color[1]), linear(color[2])],
            opacity: f32::from(opacity) / 100.,
        };
        Self {
            shadow: style
                .shadow
                .map_or(Layer::OFF, |s| effect(s.color, s.opacity)),
            outline: style
                .outline
                .map_or(Layer::OFF, |o| effect(o.color, o.opacity)),
            fill: effect(style.color, 100),
        }
    }
}

/// sRGB byte to linear light, matching how the sRGB background texture decodes.
fn linear(value: u8) -> f32 {
    let c = f32::from(value) / 255.;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
/// Linear light to an sRGB byte, matching how the target texture encodes.
fn srgb(light: f32) -> u8 {
    let c = if light <= 0.003_130_8 {
        light * 12.92
    } else {
        1.055 * light.powf(1. / 2.4) - 0.055
    };
    (c.clamp(0., 1.) * 255.).round() as u8
}
/// CPU twin of the shader's blend, for editor previews and thumbnails: the
/// same three mask channels, colors, opacities, shadow-outline-fill order and
/// linear-light compositing, over one background color. Returns BGRA bytes
/// in the GPUI `RenderImage` order.
pub fn blend_pixels(background: [u8; 4], coverage: &[u8], blend: &Blend) -> Vec<u8> {
    let base = [
        linear(background[0]),
        linear(background[1]),
        linear(background[2]),
    ];
    blend_with(coverage, blend, |_| base)
}

/// `blend_pixels` over a background image already fitted to the output size,
/// as the shader samples it 1:1: straight alpha composites over black. `None`
/// when the image and coverage sizes differ.
pub fn blend_over(background: &[u8], coverage: &[u8], blend: &Blend) -> Option<Vec<u8>> {
    if background.len() != coverage.len() {
        return None;
    }
    let lut: [f32; 256] = std::array::from_fn(|value| linear(value as u8));
    let pixels = background.as_chunks::<4>().0;
    Some(blend_with(coverage, blend, |index| {
        let [r, g, b, a] = pixels[index];
        let alpha = f32::from(a) / 255.;
        [
            lut[usize::from(r)] * alpha,
            lut[usize::from(g)] * alpha,
            lut[usize::from(b)] * alpha,
        ]
    }))
}

fn blend_with(coverage: &[u8], blend: &Blend, base: impl Fn(usize) -> [f32; 3]) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(coverage.len());
    for (index, pixel) in coverage.as_chunks::<4>().0.iter().enumerate() {
        let mut rgb = base(index);
        for (mask, layer) in [
            (pixel[2], &blend.shadow),
            (pixel[1], &blend.outline),
            (pixel[0], &blend.fill),
        ] {
            let alpha = f32::from(mask) / 255. * layer.opacity;
            for (channel, color) in rgb.iter_mut().zip(layer.rgb) {
                *channel = *channel * (1. - alpha) + color * alpha;
            }
        }
        pixels.extend([srgb(rgb[2]), srgb(rgb[1]), srgb(rgb[0]), 255]);
    }
    pixels
}

pub struct Image<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

pub struct Compositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    info: wgpu::AdapterInfo,
}

/// Renderer-owned bindings uploaded on the preparation worker, not read back.
/// `background_only` shares the textures with text coverage off.
#[allow(dead_code)] // Used by native_cues; offscreen examples share this module.
pub struct ReadyComposition {
    bindings: wgpu::BindGroup,
    background_only: wgpu::BindGroup,
}

/// What one presented frame shows: the full scene (no mask), its background
/// without text (Clear, or the logo), or plain black.
#[allow(dead_code)] // Used by native_cues; offscreen examples share this module.
#[derive(Clone, Copy)]
pub enum Draw<'a> {
    Scene(&'a ReadyComposition),
    Background(&'a ReadyComposition),
    Black,
}

fn byte_len(size: Extent, cap: u32) -> Result<usize> {
    if size.width == 0
        || size.height == 0
        || size.width > cap.min(8192)
        || size.height > cap.min(8192)
    {
        return Err("nonzero dimensions within device and 8192-edge limits required".into());
    }
    let bytes = u64::from(size.width) * u64::from(size.height) * 4;
    if bytes > MAX_SCENE_BYTES as u64 {
        return Err("RGBA payload exceeds 64MiB scene budget".into());
    }
    Ok(bytes as usize)
}

fn validate(size: Extent, image: &Image<'_>, coverage: &[u8], cap: u32) -> Result<usize> {
    let bytes = byte_len(size, cap)?;
    let image_bytes = byte_len(
        Extent {
            width: image.width,
            height: image.height,
        },
        cap,
    )?;
    // Coverage is RGBA: r=fill, g=outline, b=shadow, a unused.
    if image.rgba.len() != image_bytes || coverage.len() != bytes {
        return Err("image RGBA or text coverage length mismatch".into());
    }
    Ok(bytes)
}

// Physical pixels, top-left origin. Rectangle can extend beyond output for cover.
fn rectangle(size: Extent, image: Extent, fit: Fit) -> [f32; 4] {
    let x = size.width as f32 / image.width as f32;
    let y = size.height as f32 / image.height as f32;
    let scale = match fit {
        Fit::Contain => x.min(y),
        Fit::Cover => x.max(y),
    };
    let w = image.width as f32 * scale;
    let h = image.height as f32 * scale;
    [
        (size.width as f32 - w) / 2.,
        (size.height as f32 - h) / 2.,
        w,
        h,
    ]
}

const SHADER: &str = r#"
@group(0) @binding(0) var background: texture_2d<f32>;
@group(0) @binding(1) var mask: texture_2d<f32>;
struct Placement { rect: vec4f, fill: vec4f, outline: vec4f, shadow: vec4f }
@group(0) @binding(2) var<uniform> placement: Placement;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    var p = array<vec2f, 3>(vec2f(-1,-1), vec2f(3,-1), vec2f(-1,3));
    return vec4f(p[i], 0, 1);
}
@fragment fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
    let rect = placement.rect;
    let uv = (p.xy - rect.xy) / rect.zw;

    var rgb = vec3f(0);
    if all(uv >= vec2f(0)) && all(uv < vec2f(1)) {
        let dims = textureDimensions(background);
        let at = min(vec2u(uv * vec2f(dims)), dims - vec2u(1));
        let color = textureLoad(background, vec2i(at), 0);
        rgb = color.rgb * color.a;
    }
    // Coverage mask: r=fill, g=outline, b=shadow. Shadow blends under
    // outline under fill; each layer's own color and opacity apply here.
    let m = textureLoad(mask, vec2i(p.xy), 0);
    rgb = mix(rgb, placement.shadow.rgb, m.b * placement.shadow.a);
    rgb = mix(rgb, placement.outline.rgb, m.g * placement.outline.a);
    rgb = mix(rgb, placement.fill.rgb, m.r * placement.fill.a);

    return vec4f(rgb, 1);
}
"#;

impl Compositor {
    pub fn new(backends: wgpu::Backends) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::from_env().unwrap_or_default(),
            ..Default::default()
        }))?;
        let info = adapter.get_info();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        Ok(Self::from_device(
            device,
            queue,
            info,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ))
    }

    pub fn from_device(
        device: wgpu::Device,
        queue: wgpu::Queue,
        info: wgpu::AdapterInfo,
        format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("static composition"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("static composition"),
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
                targets: &[Some(format.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            device,
            queue,
            pipeline,
            info,
        }
    }

    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.info
    }

    fn texture(
        &self,
        size: Extent,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }

    fn upload(&self, texture: &wgpu::Texture, bytes: &[u8], stride: u32) {
        self.queue.write_texture(
            texture.as_image_copy(),
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(texture.height()),
            },
            texture.size(),
        );
    }

    /// Validation/allocation/upload only; invoke on a bounded worker. No readback.
    #[allow(dead_code)] // Native entry point, unused by offscreen examples.
    pub fn prepare_native(
        &self,
        size: Extent,
        background: Image<'_>,
        coverage: &[u8],
        blend: &Blend,
        fit: Fit,
    ) -> Result<ReadyComposition> {
        validate(
            size,
            &background,
            coverage,
            self.device.limits().max_texture_dimension_2d,
        )?;
        let image_size = Extent {
            width: background.width,
            height: background.height,
        };
        let image = self.texture(
            image_size,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let mask = self.texture(
            size,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        self.upload(&image, background.rgba, background.width * 4);
        self.upload(&mask, coverage, size.width * 4);
        let image_view = image.create_view(&Default::default());
        let mask_view = mask.create_view(&Default::default());
        let rect = rectangle(size, image_size, fit);

        let bindings = self.bind(&image_view, &mask_view, rect, blend, 1.0);

        // write_texture stages until submit. Flush and bound this worker's
        // in-flight upload staging before exposing readiness; NEVER in redraw.
        let upload = self.queue.submit([]);
        self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(upload),
            timeout: Some(Duration::from_secs(2)),
        })?;
        let background_only = self.bind(&image_view, &mask_view, rect, blend, 0.0);
        Ok(ReadyComposition {
            bindings,
            background_only,
        })
    }

    fn bind(
        &self,
        image: &wgpu::TextureView,
        mask: &wgpu::TextureView,
        rect: [f32; 4],
        blend: &Blend,
        master: f32,
    ) -> wgpu::BindGroup {
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: true,
        });
        {
            let mut data = uniform.get_mapped_range_mut(..);
            let mut write = |index: usize, value: f32| {
                data.slice(index * 4..index * 4 + 4)
                    .copy_from_slice(&value.to_ne_bytes());
            };
            for (index, value) in rect.into_iter().enumerate() {
                write(index, value);
            }
            // rect (vec4), then fill, outline and shadow (rgb + opacity).
            for (layer, base) in [blend.fill, blend.outline, blend.shadow]
                .into_iter()
                .zip([4, 8, 12])
            {
                for (offset, value) in layer
                    .rgb
                    .into_iter()
                    .chain([layer.opacity * master])
                    .enumerate()
                {
                    write(base + offset, value);
                }
            }
        }
        uniform.unmap();
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(image),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(mask),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        })
    }

    /// Nonblocking encode/submit boundary. No upload, wait, map or readback.
    #[allow(dead_code)] // Native entry point, unused by offscreen examples.
    pub fn submit_native(&self, view: &wgpu::TextureView, draw: Draw<'_>) {
        let bindings = match draw {
            Draw::Scene(ready) => Some(&ready.bindings),
            Draw::Background(ready) => Some(&ready.background_only),
            Draw::Black => None,
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            if let Some(bindings) = bindings {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, bindings, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Blocking GPU completion/readback, for diagnostics on a worker only.
    /// One call owns its textures/buffer; no persistent unbounded cache/queue.
    pub fn render(
        &self,
        size: Extent,
        background: Image<'_>,
        coverage: &[u8],
        blend: &Blend,
        fit: Fit,
    ) -> Result<Vec<u8>> {
        let bytes = validate(
            size,
            &background,
            coverage,
            self.device.limits().max_texture_dimension_2d,
        )?;
        let stride = (size.width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let read_size = u64::from(stride) * u64::from(size.height);
        if read_size > self.device.limits().max_buffer_size {
            return Err("padded readback exceeds device buffer limit".into());
        }
        let image_size = Extent {
            width: background.width,
            height: background.height,
        };
        let image = self.texture(
            image_size,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let mask = self.texture(
            size,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let target = self.texture(
            size,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        self.upload(&image, background.rgba, background.width * 4);
        self.upload(&mask, coverage, size.width * 4);
        let image_view = image.create_view(&Default::default());
        let mask_view = mask.create_view(&Default::default());
        let rect = rectangle(size, image_size, fit);

        let target_view = target.create_view(&Default::default());
        let bindings = self.bind(&image_view, &mask_view, rect, blend, 1.0);

        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: read_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(size.height),
                },
            },
            target.size(),
        );
        let submission = self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        readback.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = tx.send(result);
        });
        self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(10)),
        })?;
        rx.recv_timeout(Duration::from_secs(1))??;
        let mapped = readback.get_mapped_range(..);
        let mut rgba = Vec::with_capacity(bytes);
        for row in mapped.chunks_exact(stride as usize) {
            rgba.extend_from_slice(&row[..size.width as usize * 4]);
        }
        drop(mapped);
        readback.unmap();
        Ok(rgba)
    }

    /// Repeatable opt-in real GPU checks. Ordinary unit tests do not request an adapter.
    pub fn check_readback(&self) -> Result<()> {
        let size = Extent {
            width: 3,
            height: 2,
        };
        let source = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255, 0, 255, 255, 255,
            255, 0, 255, 255,
        ];
        let plain = Blend::plain();
        let rgba = self.render(
            size,
            Image {
                width: 3,
                height: 2,
                rgba: &source,
            },
            &[0; 24],
            &plain,
            Fit::Contain,
        )?;
        if rgba != source {
            return Err("asymmetric orientation/channel/row-padding check failed".into());
        }
        let midtone = [128, 64, 32, 255];
        let rgba = self.render(
            size,
            Image {
                width: 1,
                height: 1,
                rgba: &midtone,
            },
            // Fill coverage 0, 128 and 255 across the top row.
            &[
                0, 0, 0, 255, 128, 0, 0, 255, 255, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0,
                255,
            ],
            &plain,
            Fit::Cover,
        )?;
        for (pixel, expected) in [(0, midtone), (1, [205, 192, 189, 255])] {
            if rgba[pixel * 4..pixel * 4 + 4]
                .iter()
                .zip(expected)
                .any(|(a, b)| a.abs_diff(b) > 1)
            {
                return Err("sRGB decode/linear composite/encode check failed".into());
            }
        }
        let black = [0, 0, 0, 255];
        let rgba = self.render(
            size,
            Image {
                width: 1,
                height: 1,
                rgba: &black,
            },
            &[
                0, 0, 0, 255, 128, 0, 0, 255, 255, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0,
                255,
            ],
            &plain,
            Fit::Cover,
        )?;
        if rgba[..4] != black
            || rgba[8..12] != [255; 4]
            || !(187..=189).contains(&rgba[4])
            || rgba[4..7] != [rgba[4]; 3]
            || rgba[7] != 255
        {
            return Err("linear white coverage check failed".into());
        }
        // Colored layers blend in order, each with its own opacity: fill
        // blue over outline red over a half-opacity green shadow.
        let colored = Blend {
            fill: Layer {
                rgb: [0., 0., 1.],
                opacity: 1.,
            },
            outline: Layer {
                rgb: [1., 0., 0.],
                opacity: 1.,
            },
            shadow: Layer {
                rgb: [0., 1., 0.],
                opacity: 0.5,
            },
        };
        let wide = Extent {
            width: 4,
            height: 1,
        };
        let coverage = [
            0, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let rgba = self.render(
            wide,
            Image {
                width: 1,
                height: 1,
                rgba: &black,
            },
            &coverage,
            &colored,
            Fit::Cover,
        )?;
        for (pixel, expected) in [
            (0, [0, 0, 0, 255]),
            (1, [255, 0, 0, 255]),
            (3, [0, 0, 255, 255]),
        ] {
            if rgba[pixel * 4..pixel * 4 + 4] != expected {
                return Err("colored layer blend order check failed".into());
            }
        }
        // Shadow at half opacity over black is half linear green.
        if rgba[8] != 0 || !(187..=189).contains(&rgba[9]) || rgba[10] != 0 || rgba[11] != 255 {
            return Err("colored layer opacity check failed".into());
        }
        // A slide background fitted to the output composites 1:1, and the
        // editor's CPU twin matches it: varied colors, straight alpha and
        // partial coverage of every layer.
        let fitted = [
            200, 30, 90, 255, 12, 180, 240, 128, 255, 255, 255, 255, 60, 60, 60, 255, 0, 120, 30,
            0, 250, 200, 10, 255,
        ];
        let coverage = [
            0, 0, 0, 255, 128, 0, 0, 255, 0, 200, 0, 255, 0, 0, 90, 255, 64, 64, 64, 255, 255, 0,
            0, 255,
        ];
        let gpu = self.render(
            size,
            Image {
                width: 3,
                height: 2,
                rgba: &fitted,
            },
            &coverage,
            &colored,
            Fit::Contain,
        )?;
        let cpu = blend_over(&fitted, &coverage, &colored).ok_or("blend_over sizes")?;
        // The CPU twin returns BGRA for GPUI images.
        for (gpu, cpu) in gpu.as_chunks::<4>().0.iter().zip(cpu.as_chunks::<4>().0) {
            let cpu = [cpu[2], cpu[1], cpu[0], cpu[3]];
            if gpu.iter().zip(cpu).any(|(a, b)| a.abs_diff(b) > 1) {
                return Err(format!("CPU blend over image {cpu:?} != GPU {gpu:?}").into());
            }
        }
        let white = [255, 255, 255, 128];
        let rgba = self.render(
            size,
            Image {
                width: 1,
                height: 1,
                rgba: &white,
            },
            &[0; 24],
            &plain,
            Fit::Cover,
        )?;
        if rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| !(187..=189).contains(&p[0]) || p[..3] != [p[0]; 3] || p[3] != 255)
        {
            return Err("straight background alpha over black check failed".into());
        }
        let hidden = [255, 0, 255, 0];
        if self
            .render(
                size,
                Image {
                    width: 1,
                    height: 1,
                    rgba: &hidden,
                },
                &[0; 24],
                &plain,
                Fit::Cover,
            )?
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| *p != black)
        {
            return Err("transparent color leaked".into());
        }
        // Both image aspect axes: two center columns/rows are green, outer are red.
        let square = Extent {
            width: 4,
            height: 4,
        };
        for vertical in [false, true] {
            let (w, h) = if vertical { (2, 4) } else { (4, 2) };
            let mut pixels = Vec::new();
            for y in 0..h {
                for x in 0..w {
                    let axis = if vertical { y } else { x };
                    pixels.extend_from_slice(if axis == 0 || axis == 3 {
                        &[255, 0, 0, 255]
                    } else {
                        &[0, 255, 0, 255]
                    });
                }
            }
            for fit in [Fit::Contain, Fit::Cover] {
                let rgba = self.render(
                    square,
                    Image {
                        width: w,
                        height: h,
                        rgba: &pixels,
                    },
                    &[0; 64],
                    &plain,
                    fit,
                )?;
                for y in 0..4 {
                    for x in 0..4 {
                        let axis = if vertical { x } else { y };
                        let expected = match fit {
                            Fit::Cover => [0, 255, 0, 255],
                            Fit::Contain if axis == 0 || axis == 3 => black,
                            Fit::Contain => {
                                if (if vertical { y } else { x }) == 0
                                    || (if vertical { y } else { x }) == 3
                                {
                                    [255, 0, 0, 255]
                                } else {
                                    [0, 255, 0, 255]
                                }
                            }
                        };
                        let at = (y * 4 + x) * 4;
                        if rgba[at..at + 4] != expected {
                            return Err("contain/cover boundary check failed".into());
                        }
                    }
                }
            }
        }
        if self
            .render(
                size,
                Image {
                    width: 1,
                    height: 1,
                    rgba: &black,
                },
                &[],
                &plain,
                Fit::Cover,
            )
            .is_ok()
            || self
                .render(
                    size,
                    Image {
                        width: 0,
                        height: 1,
                        rgba: &[],
                    },
                    &[0; 24],
                    &plain,
                    Fit::Cover,
                )
                .is_ok()
            || self
                .render(
                    Extent {
                        width: 8193,
                        height: 1,
                    },
                    Image {
                        width: 1,
                        height: 1,
                        rgba: &black,
                    },
                    &[],
                    &plain,
                    Fit::Cover,
                )
                .is_ok()
        {
            return Err("render input rejection check failed".into());
        }
        // Failed validation must not poison the device for the next call.
        self.render(
            size,
            Image {
                width: 1,
                height: 1,
                rgba: &black,
            },
            &[0; 24],
            &plain,
            Fit::Cover,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geometry_both_axes() {
        let wide = Extent {
            width: 8,
            height: 4,
        };
        let square = Extent {
            width: 4,
            height: 4,
        };
        assert_eq!(rectangle(square, wide, Fit::Contain), [0., 1., 4., 2.]);
        assert_eq!(rectangle(square, wide, Fit::Cover), [-2., 0., 8., 4.]);
        assert_eq!(rectangle(wide, square, Fit::Contain), [2., 0., 4., 4.]);
        assert_eq!(rectangle(wide, square, Fit::Cover), [0., -2., 8., 8.]);
    }
    #[test]
    fn rejection_without_gpu() {
        let one = Extent {
            width: 1,
            height: 1,
        };
        let good = Image {
            width: 1,
            height: 1,
            rgba: &[0; 4],
        };
        assert!(validate(one, &good, &[0; 4], 8192).is_ok());
        assert!(validate(one, &good, &[], 8192).is_err());
        assert!(
            validate(
                one,
                &Image {
                    rgba: &[0; 3],
                    ..good
                },
                &[0; 4],
                8192
            )
            .is_err()
        );
        for size in [
            Extent {
                width: 0,
                height: 1,
            },
            Extent {
                width: 8193,
                height: 1,
            },
            Extent {
                width: u32::MAX,
                height: u32::MAX,
            },
            Extent {
                width: 8192,
                height: 8192,
            },
        ] {
            assert!(byte_len(size, 8192).is_err());
        }
        assert!(
            byte_len(
                Extent {
                    width: 3,
                    height: 1
                },
                2
            )
            .is_err()
        );
        assert_eq!(
            byte_len(
                Extent {
                    width: 4096,
                    height: 4096
                },
                8192
            )
            .unwrap(),
            MAX_SCENE_BYTES
        );
    }

    #[test]
    fn blend_over_matches_a_uniform_color_and_composites_alpha_over_black() {
        let coverage = [0, 0, 0, 255, 128, 0, 0, 255, 255, 255, 255, 255];
        let plain = Blend::plain();
        let color = [128, 64, 32, 255];
        assert_eq!(
            blend_over(&color.repeat(3), &coverage, &plain).unwrap(),
            blend_pixels(color, &coverage, &plain)
        );
        let clear = [255, 255, 255, 0].repeat(3);
        let over = blend_over(&clear, &coverage, &plain).unwrap();
        assert_eq!(over[..4], [0, 0, 0, 255]);
        let half = blend_over(&[255, 255, 255, 128], &[0; 4], &plain).unwrap();
        assert!((187..=189).contains(&half[0]), "{half:?}");
        assert!(blend_over(&[0; 8], &coverage, &plain).is_none());
    }

    #[test]
    fn style_blend_converts_to_linear_light_and_opacity() {
        use sela::scene::{OutlineStyle, ShadowStyle};
        let style = TextStyle {
            color: [255, 0, 0],
            outline: Some(OutlineStyle {
                color: [128, 128, 128],
                size: 7,
                opacity: 100,
            }),
            shadow: Some(ShadowStyle {
                color: [10, 10, 10],
                angle: 315,
                offset: 18,
                blur: 9,
                opacity: 90,
            }),
            ..TextStyle::default()
        };
        let blend = Blend::from_style(&style);
        assert_eq!(blend.fill.rgb, [1., 0., 0.]);
        assert_eq!(blend.fill.opacity, 1.);
        // 128 is linear 0.2158, the same value the sRGB background decodes to.
        assert!((blend.outline.rgb[0] - 0.2158).abs() < 0.001);
        assert_eq!(blend.outline.opacity, 1.);
        assert!((blend.shadow.opacity - 0.9).abs() < 1e-6);
        let plain = Blend::plain();
        assert_eq!(plain.fill.rgb, [1., 1., 1.]);
        assert_eq!((plain.outline.opacity, plain.shadow.opacity), (0., 0.));
    }
}
