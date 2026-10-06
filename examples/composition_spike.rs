//! Static composition/readback diagnostic, not the native live renderer.
// Diagnostic subset of the shared modules; the library checks the rest.
#[path = "../src/audience/compositor.rs"]
#[allow(dead_code)]
mod gpu;
#[path = "../src/audience/text.rs"]
#[allow(dead_code)]
mod text;

use image::ImageEncoder;
use sela::{
    preparation::{PreparationEvent, Preparer},
    scene::{
        BackgroundSpec, ContentVersion, Extent, PreparedBackground, PreparedCue,
        RendererCapabilities, ResourceRef, SceneSpec, TextSpec,
    },
};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

const FONT: &[u8] = include_bytes!("../tests/fixtures/DejaVuSans.ttf");
const SIZE: Extent = Extent {
    width: 641,
    height: 360,
};
const MARGIN: u32 = 32;
const COLOR: [u8; 4] = [18, 53, 109, 255];
type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn resource(path: &Path, bytes: &[u8], id: u128) -> Result<ResourceRef> {
    std::fs::write(path, bytes)?;
    Ok(ResourceRef {
        version: ContentVersion { id, revision: 1 },
        path: path.into(),
        sha256: Sha256::digest(bytes).into(),
    })
}

fn prepare(spec: SceneSpec) -> Result<Arc<PreparedCue>> {
    let mut worker = Preparer::new(RendererCapabilities {
        max_texture_dimension: 4096,
    })?;
    worker.request(spec, Instant::now(), Duration::from_secs(5))?;
    loop {
        match worker.poll(Instant::now()) {
            Some(PreparationEvent::Ready { cue, .. }) => return Ok(cue),
            Some(PreparationEvent::Failed { error, .. }) => return Err(error.into()),
            None => std::thread::sleep(Duration::from_millis(1)),
        }
    }
}

fn mask(cue: Arc<PreparedCue>) -> Result<Vec<u8>> {
    // At most one joined raster job in this diagnostic. No UI or live frame
    // runs on this thread; joining here does not establish a production lifecycle.
    let job = std::thread::Builder::new()
        .name("sela-raster".into())
        .spawn(move || {
            let text = cue.text().expect("diagnostic fixture has explicit text");
            text::fill(
                text.font(),
                text.face_index(),
                text.content(),
                SIZE.width - 2 * MARGIN,
                SIZE.height - 2 * MARGIN,
                f32::from(text.font_size()),
            )
        })?;
    let inner = job
        .join()
        .map_err(|_| "raster worker panicked")?
        .map_err(|error| format!("raster rejected: {error:?}"))?;
    let mut alpha = vec![0; (SIZE.width * SIZE.height) as usize];
    for (y, row) in inner
        .as_chunks::<{ (SIZE.width - 2 * MARGIN) as usize }>()
        .0
        .iter()
        .enumerate()
    {
        let start = (y + MARGIN as usize) * SIZE.width as usize + MARGIN as usize;
        alpha[start..start + row.len()].copy_from_slice(row);
    }
    Ok(alpha)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || args.len() > 2 {
        return Err("usage: composition_spike OUTPUT_DIRECTORY [vulkan|dx12|metal]".into());
    }
    let backend = match args.get(1).and_then(|s| s.to_str()).unwrap_or("vulkan") {
        "vulkan" => wgpu::Backends::VULKAN,
        "dx12" => wgpu::Backends::DX12,
        "metal" => wgpu::Backends::METAL,
        _ => return Err("unsupported explicit backend".into()),
    };
    let output = Path::new(&args[0]);
    // Fixture creation is setup, outside preparation and GPU calls. Files are
    // deleted before rendering, checking ownership rather than path reuse.
    let files = tempfile::tempdir()?;
    let font = resource(&files.path().join("font.ttf"), FONT, 2)?;
    let image_bytes = [
        20, 60, 100, 255, 36, 89, 77, 255, 75, 49, 91, 128, 80, 48, 41, 255, 33, 56, 90, 255, 69,
        79, 42, 255,
    ];
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(
        &image_bytes,
        3,
        2,
        image::ExtendedColorType::Rgba8,
    )?;
    let image = resource(&files.path().join("image.png"), &png, 3)?;
    let start = Instant::now();
    let spec = SceneSpec {
        version: ContentVersion { id: 1, revision: 1 },
        extent: SIZE,
        background: BackgroundSpec::Color(COLOR),
        text: Some(TextSpec {
            content: "Sela · static composition\nCafé · e\u{301}\nسلام".into(),
            font,
            font_size: 40,
        }),
    };
    let solid = prepare(spec.clone())?;
    let mut image_spec = spec;
    image_spec.version.revision = 7;
    image_spec.background = BackgroundSpec::Image(image);
    let image_cue = prepare(image_spec)?;
    files.close()?;
    let alpha = mask(solid.clone())?;
    if !alpha.contains(&255) || !alpha.iter().any(|a| *a > 0 && *a < 255) {
        return Err("font fixture lacks opaque and antialiased ink".into());
    }
    // Fill-only coverage (red channel) with the plain white blend.
    let coverage: Vec<u8> = alpha.iter().flat_map(|a| [*a, 0, 0, 255]).collect();
    println!(
        "prepare_and_raster_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.
    );
    let compositor = gpu::Compositor::new(backend)?;
    println!("adapter={:?}", compositor.adapter_info());
    compositor.check_readback()?;
    std::fs::create_dir_all(output)?;
    for (name, cue, fit) in [
        ("color", &solid, gpu::Fit::Cover),
        ("image-contain", &image_cue, gpu::Fit::Contain),
        ("image-cover", &image_cue, gpu::Fit::Cover),
    ] {
        let image = match cue.background() {
            PreparedBackground::Color(color) => gpu::Image {
                width: 1,
                height: 1,
                rgba: color,
            },
            PreparedBackground::Image { extent, rgba, .. } => gpu::Image {
                width: extent.width,
                height: extent.height,
                rgba,
            },
        };
        let start = Instant::now();
        let pixels =
            compositor.render(cue.extent(), image, &coverage, &gpu::Blend::plain(), fit)?;
        println!(
            "{name}_compose_readback_ms={:.3}",
            start.elapsed().as_secs_f64() * 1000.
        );
        for (pixel, coverage) in pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(coverage.as_chunks::<4>().0)
        {
            if pixel[3] != 255 || (coverage[0] == 255 && *pixel != [255; 4]) {
                return Err("actual font mask was not composed as opaque white".into());
            }
            if name == "color"
                && coverage[0] == 0
                && pixel.iter().zip(COLOR).any(|(a, b)| a.abs_diff(b) > 1)
            {
                return Err("uncovered color pixels changed".into());
            }
        }
        image::save_buffer(
            output.join(format!("{name}.png")),
            &pixels,
            SIZE.width,
            SIZE.height,
            image::ColorType::Rgba8,
        )?;
    }
    // Rejected layout produces no upload/presentation. This is a CPU rejection
    // check, not a claim of native live-state or reference restoration semantics.
    if text::fill(FONT, 0, "Overflow fixture", 20, 100, 40.) != Err(text::TextError::Overflow) {
        return Err("overflow fixture was silently clipped".into());
    }
    println!("PASS: GPU pixel checks; actual-font color/contain/cover; overflow rejected");
    Ok(())
}
