//! Opt-in synchronous offscreen diagnostic, never a live-frame API.
use sela::scene::{Extent, MAX_SCENE_BYTES};
use std::{error::Error, time::Duration};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Copy, Debug)]
pub enum Fit {
    Contain,
    Cover,
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
#[allow(dead_code)] // Used by native_cues; offscreen examples share this module.
pub struct ReadyComposition {
    bindings: wgpu::BindGroup,
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

fn validate(size: Extent, image: &Image<'_>, alpha: &[u8], cap: u32) -> Result<usize> {
    let bytes = byte_len(size, cap)?;
    let image_bytes = byte_len(
        Extent {
            width: image.width,
            height: image.height,
        },
        cap,
    )?;
    if image.rgba.len() != image_bytes || alpha.len() != bytes / 4 {
        return Err("image RGBA or output alpha length mismatch".into());
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
@group(0) @binding(2) var<uniform> rect: vec4f;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    var p = array<vec2f, 3>(vec2f(-1,-1), vec2f(3,-1), vec2f(-1,3));
    return vec4f(p[i], 0, 1);
}
@fragment fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
    let uv = (p.xy - rect.xy) / rect.zw;
    var rgb = vec3f(0);
    if all(uv >= vec2f(0)) && all(uv < vec2f(1)) {
        let dims = textureDimensions(background);
        let at = min(vec2u(uv * vec2f(dims)), dims - vec2u(1));
        let color = textureLoad(background, vec2i(at), 0);
        rgb = color.rgb * color.a;
    }
    let coverage = textureLoad(mask, vec2i(p.xy), 0).r;
    return vec4f(mix(rgb, vec3f(1), coverage), 1);
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
        alpha: &[u8],
        fit: Fit,
    ) -> Result<ReadyComposition> {
        validate(
            size,
            &background,
            alpha,
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
            wgpu::TextureFormat::R8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        self.upload(&image, background.rgba, background.width * 4);
        self.upload(&mask, alpha, size.width);
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: true,
        });
        {
            let mut data = uniform.get_mapped_range_mut(..);
            for (i, value) in rectangle(size, image_size, fit).into_iter().enumerate() {
                data.slice(i * 4..i * 4 + 4)
                    .copy_from_slice(&value.to_ne_bytes());
            }
        }
        uniform.unmap();
        let image_view = image.create_view(&Default::default());
        let mask_view = mask.create_view(&Default::default());
        let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&image_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&mask_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        // write_texture stages until submit. Flush and bound this worker's
        // in-flight upload staging before exposing readiness; NEVER in redraw.
        let upload = self.queue.submit([]);
        self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(upload),
            timeout: Some(Duration::from_secs(2)),
        })?;
        Ok(ReadyComposition { bindings })
    }

    /// Nonblocking encode/submit boundary. No upload, wait, map or readback.
    #[allow(dead_code)] // Native entry point, unused by offscreen examples.
    pub fn submit_native(&self, view: &wgpu::TextureView, ready: &ReadyComposition) {
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &ready.bindings, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Blocking GPU completion/readback, for diagnostics on a worker only.
    /// One call owns its textures/buffer; no persistent unbounded cache/queue.
    pub fn render(
        &self,
        size: Extent,
        background: Image<'_>,
        text_alpha: &[u8],
        fit: Fit,
    ) -> Result<Vec<u8>> {
        let bytes = validate(
            size,
            &background,
            text_alpha,
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
            wgpu::TextureFormat::R8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let target = self.texture(
            size,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        self.upload(&image, background.rgba, background.width * 4);
        self.upload(&mask, text_alpha, size.width);
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: true,
        });
        {
            let mut data = uniform.get_mapped_range_mut(..);
            for (i, value) in rectangle(size, image_size, fit).into_iter().enumerate() {
                data.slice(i * 4..i * 4 + 4)
                    .copy_from_slice(&value.to_ne_bytes());
            }
        }
        uniform.unmap();
        let image_view = image.create_view(&Default::default());
        let mask_view = mask.create_view(&Default::default());
        let target_view = target.create_view(&Default::default());
        let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&image_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&mask_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
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
        let rgba = self.render(
            size,
            Image {
                width: 3,
                height: 2,
                rgba: &source,
            },
            &[0; 6],
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
            &[0, 128, 255, 0, 0, 0],
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
            &[0, 128, 255, 0, 0, 0],
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
        let white = [255, 255, 255, 128];
        let rgba = self.render(
            size,
            Image {
                width: 1,
                height: 1,
                rgba: &white,
            },
            &[0; 6],
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
                &[0; 6],
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
                    &[0; 16],
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
                    &[0; 6],
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
            &[0; 6],
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
        assert!(validate(one, &good, &[0], 8192).is_ok());
        assert!(validate(one, &good, &[], 8192).is_err());
        assert!(
            validate(
                one,
                &Image {
                    rgba: &[0; 3],
                    ..good
                },
                &[0],
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
}
