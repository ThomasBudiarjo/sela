//! Owned editable inputs and immutable, resolved scene snapshots. No GPUI types.
use crate::format::{
    Align, MAX_OPACITY, MAX_OUTLINE_SIZE, MAX_SHADOW_BLUR, MAX_SHADOW_OFFSET, VAlign,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SCENE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContentVersion {
    pub id: u128,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RendererCapabilities {
    pub max_texture_dimension: u32,
}

#[derive(Clone)]
pub struct ResourceRef {
    pub version: ContentVersion,
    pub path: PathBuf,
    pub sha256: [u8; 32],
}

#[derive(Clone)]
pub enum BackgroundSpec {
    Color([u8; 4]),
    Image(ResourceRef),
}

#[derive(Clone)]
pub struct TextSpec {
    pub content: String,
    pub font: ResourceRef,
    pub font_size: u16,
}

/// Resolved per-slide text style (M1-05g2). Values are Sela's units on the
/// 1080-line reference canvas, not observed EasyWorship ranges; the raster
/// scales them to its own extent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextStyle {
    pub color: [u8; 3],
    pub align: Align,
    pub valign: VAlign,
    pub underline: bool,
    /// The format wants italic; `synth_italic` means no italic face exists,
    /// so the slant is synthesized at raster time.
    pub italic: bool,
    pub synth_bold: bool,
    pub synth_italic: bool,
    pub outline: Option<OutlineStyle>,
    pub shadow: Option<ShadowStyle>,
}

impl Default for TextStyle {
    /// Today's look: white, centered both ways, no decoration or effects.
    fn default() -> Self {
        Self {
            color: [255, 255, 255],
            align: Align::Center,
            valign: VAlign::Middle,
            underline: false,
            italic: false,
            synth_bold: false,
            synth_italic: false,
            outline: None,
            shadow: None,
        }
    }
}

impl TextStyle {
    /// Same bounds as `format::SlideFormat`, so a valid format resolves to a
    /// valid style and the wire never widens them.
    pub fn is_valid(&self) -> bool {
        self.outline
            .is_none_or(|o| (1..=MAX_OUTLINE_SIZE).contains(&o.size) && o.opacity <= MAX_OPACITY)
            && self.shadow.is_none_or(|s| {
                s.angle < 360
                    && s.offset <= MAX_SHADOW_OFFSET
                    && s.blur <= MAX_SHADOW_BLUR
                    && s.opacity <= MAX_OPACITY
            })
    }
}

/// EW outline type None/Outer with round join; Center and Inner are open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutlineStyle {
    pub color: [u8; 3],
    /// Reference-canvas points; dilation radius at 1080 lines.
    pub size: u8,
    pub opacity: u8,
}

/// EW shadow with the observed dial convention (315 = down-right).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShadowStyle {
    pub color: [u8; 3],
    pub angle: u16,
    pub offset: u8,
    pub blur: u8,
    pub opacity: u8,
}

/// Owned text as the cue carries it: content, face, pixel size and style.
#[derive(Clone)]
pub struct OwnedText {
    pub content: String,
    pub font_version: ContentVersion,
    pub font: Arc<[u8]>,
    /// Face within a collection; validated by parsing, not a wire range.
    pub face_index: u32,
    pub font_size: u16,
    pub style: TextStyle,
}

/// Editing/selection owns this value. Submit a clone to freeze that revision.
#[derive(Clone)]
pub struct SceneSpec {
    pub version: ContentVersion,
    pub extent: Extent,
    pub background: BackgroundSpec,
    pub text: Option<TextSpec>,
}

// Payloads deliberately lack Debug: ordinary diagnostics must not print lyrics,
// font bytes, pixels or filesystem paths. Public access is read-only.
pub enum PreparedBackground {
    Color([u8; 4]),
    Image {
        version: ContentVersion,
        extent: Extent,
        rgba: Box<[u8]>,
    },
}

pub struct PreparedText {
    content: String,
    font_version: ContentVersion,
    font: Arc<[u8]>,
    face_index: u32,
    font_size: u16,
    style: TextStyle,
}

impl PreparedText {
    pub fn content(&self) -> &str {
        &self.content
    }
    pub fn font(&self) -> &[u8] {
        &self.font
    }
    pub fn font_version(&self) -> ContentVersion {
        self.font_version
    }
    pub fn face_index(&self) -> u32 {
        self.face_index
    }
    pub fn font_size(&self) -> u16 {
        self.font_size
    }
    pub fn style(&self) -> TextStyle {
        self.style
    }
}

pub struct PreparedCue {
    version: ContentVersion,
    extent: Extent,
    background: PreparedBackground,
    text: Option<PreparedText>,
    bytes: usize,
}

impl PreparedCue {
    /// Revalidate an owned transport snapshot on a worker. No paths, decoding,
    /// system font lookup or renderer readiness is implied by this constructor.
    pub fn from_owned(
        version: ContentVersion,
        extent: Extent,
        background: PreparedBackground,
        text: Option<OwnedText>,
        caps: RendererCapabilities,
    ) -> Result<Self, PrepareError> {
        check_extent(extent, caps)?;
        let mut bytes = 0usize;
        match &background {
            PreparedBackground::Color(color) if color[3] != 255 => {
                return Err(PrepareError::InvalidScene);
            }
            PreparedBackground::Image { extent, rgba, .. } => {
                check_extent(*extent, caps)?;
                let expected = extent.width as usize * extent.height as usize * 4;
                if rgba.len() != expected {
                    return Err(PrepareError::InvalidImage);
                }
                bytes = expected;
            }
            PreparedBackground::Color(_) => {}
        }
        let text = text
            .map(|text| {
                if text.content.is_empty()
                    || text.font_size == 0
                    || text.font_size > 512
                    || !text.style.is_valid()
                {
                    return Err(PrepareError::InvalidScene);
                }
                if text.content.len() > MAX_TEXT_BYTES || text.font.len() > MAX_SOURCE_BYTES {
                    return Err(PrepareError::TooLarge);
                }
                // Also validates the face index against the collection.
                ttf_parser::Face::parse(&text.font, text.face_index)
                    .map_err(|_| PrepareError::InvalidFont)?;
                bytes += text.content.len() + text.font.len();
                Ok(PreparedText {
                    content: text.content,
                    font_version: text.font_version,
                    font: text.font,
                    face_index: text.face_index,
                    font_size: text.font_size,
                    style: text.style,
                })
            })
            .transpose()?;
        if bytes > MAX_SCENE_BYTES {
            return Err(PrepareError::TooLarge);
        }
        Ok(Self {
            version,
            extent,
            background,
            text,
            bytes,
        })
    }

    /// Resource-free diagnostic snapshot. No file work or renderer readiness is
    /// implied; opaque color is the only native transport capability in M0-06b.
    pub fn diagnostic_color(
        version: ContentVersion,
        extent: Extent,
        color: [u8; 4],
        caps: RendererCapabilities,
    ) -> Result<Self, PrepareError> {
        check_extent(extent, caps)?;
        if color[3] != 255 {
            return Err(PrepareError::InvalidScene);
        }
        Ok(Self {
            version,
            extent,
            background: PreparedBackground::Color(color),
            text: None,
            bytes: 0,
        })
    }

    pub fn version(&self) -> ContentVersion {
        self.version
    }
    pub fn extent(&self) -> Extent {
        self.extent
    }
    pub fn background(&self) -> &PreparedBackground {
        &self.background
    }
    pub fn text(&self) -> Option<&PreparedText> {
        self.text.as_ref()
    }
    pub fn resource_bytes(&self) -> usize {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareError {
    InvalidScene,
    TooLarge,
    MissingResource(ContentVersion),
    ResourceIo(ContentVersion, std::io::ErrorKind),
    ChangedResource(ContentVersion),
    UnsupportedImage,
    InvalidImage,
    InvalidFont,
    Cancelled,
    TimedOut,
    Busy,
    Disconnected,
}

impl std::fmt::Display for PrepareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidScene => "Invalid scene size, text or font size; correct it and retry",
            Self::TooLarge => "Resource exceeds the preparation budget; reduce its size",
            Self::MissingResource(_) => "Referenced resource is missing; locate it before retrying",
            Self::ResourceIo(_, _) => "Resource cannot be read; check its type and permissions",
            Self::ChangedResource(_) => "Resource bytes changed; explicitly refresh the snapshot",
            Self::UnsupportedImage => {
                "Only static PNG and JPEG images are supported by this preparer"
            }
            Self::InvalidImage => "Image cannot be decoded; replace the corrupt resource",
            Self::InvalidFont => "Font face 0 cannot be parsed; choose a valid font file",
            Self::Cancelled => "Preparation was cancelled",
            Self::TimedOut => "Preparation deadline elapsed; current output is unchanged",
            Self::Busy => "Preparation queue is full; retry after polling completion",
            Self::Disconnected => "Preparation worker disconnected; recreate it before retrying",
        };
        f.write_str(message)
    }
}
impl std::error::Error for PrepareError {}

fn check_extent(extent: Extent, caps: RendererCapabilities) -> Result<(), PrepareError> {
    if extent.width == 0 || extent.height == 0 {
        return Err(PrepareError::InvalidScene);
    }
    if extent.width > caps.max_texture_dimension
        || extent.height > caps.max_texture_dimension
        || u64::from(extent.width) * u64::from(extent.height) > (MAX_SCENE_BYTES / 4) as u64
    {
        return Err(PrepareError::TooLarge);
    }
    Ok(())
}

pub(crate) fn validate(spec: &SceneSpec, caps: RendererCapabilities) -> Result<(), PrepareError> {
    check_extent(spec.extent, caps)?;
    if let Some(text) = &spec.text {
        if text.font_size == 0 || text.font_size > 512 || text.content.is_empty() {
            return Err(PrepareError::InvalidScene);
        }
        if text.content.len() > MAX_TEXT_BYTES || text.font.path.as_os_str().len() > 4096 {
            return Err(PrepareError::TooLarge);
        }
    }
    if let BackgroundSpec::Image(resource) = &spec.background
        && resource.path.as_os_str().len() > 4096
    {
        return Err(PrepareError::TooLarge);
    }
    Ok(())
}

fn cancelled(flag: &AtomicBool) -> Result<(), PrepareError> {
    if flag.load(Ordering::Acquire) {
        Err(PrepareError::Cancelled)
    } else {
        Ok(())
    }
}

fn read_resource(resource: &ResourceRef, cancel: &AtomicBool) -> Result<Vec<u8>, PrepareError> {
    cancelled(cancel)?;
    let io_error = |error: std::io::Error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PrepareError::MissingResource(resource.version)
        } else {
            PrepareError::ResourceIo(resource.version, error.kind())
        }
    };
    // Reject known non-regular inputs before open (e.g. FIFO). OS path races or
    // stalled filesystem calls still require process-level isolation to interrupt.
    let metadata = std::fs::metadata(&resource.path).map_err(io_error)?;
    if !metadata.is_file() {
        return Err(PrepareError::ResourceIo(
            resource.version,
            std::io::ErrorKind::InvalidInput,
        ));
    }
    if metadata.len() > MAX_SOURCE_BYTES as u64 {
        return Err(PrepareError::TooLarge);
    }
    let file = File::open(&resource.path).map_err(io_error)?;
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    cancelled(cancel)?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(PrepareError::TooLarge);
    }
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != resource.sha256 {
        return Err(PrepareError::ChangedResource(resource.version));
    }
    Ok(bytes)
}

pub(crate) fn prepare(
    spec: SceneSpec,
    caps: RendererCapabilities,
    cancel: &AtomicBool,
) -> Result<PreparedCue, PrepareError> {
    validate(&spec, caps)?;
    cancelled(cancel)?;
    let text = spec
        .text
        .map(|text| {
            let font = read_resource(&text.font, cancel)?;
            ttf_parser::Face::parse(&font, 0).map_err(|_| PrepareError::InvalidFont)?;
            Ok(OwnedText {
                content: text.content,
                font_version: text.font.version,
                font: font.into(),
                face_index: 0,
                font_size: text.font_size,
                // The spec path resolves the default look; styled cues are
                // built by `slides::cue` with a resolved face.
                style: TextStyle::default(),
            })
        })
        .transpose()?;
    let mut bytes = text.as_ref().map_or(0, |t| t.content.len() + t.font.len());
    let background = match spec.background {
        BackgroundSpec::Color(color) => PreparedBackground::Color(color),
        BackgroundSpec::Image(resource) => {
            let source = read_resource(&resource, cancel)?;
            let format =
                image::guess_format(&source).map_err(|_| PrepareError::UnsupportedImage)?;
            if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
                return Err(PrepareError::UnsupportedImage);
            }
            let (width, height) = image::ImageReader::with_format(Cursor::new(&source), format)
                .into_dimensions()
                .map_err(|_| PrepareError::InvalidImage)?;
            let extent = Extent { width, height };
            check_extent(extent, caps)?;
            bytes += width as usize * height as usize * 4;
            if bytes > MAX_SCENE_BYTES {
                return Err(PrepareError::TooLarge);
            }
            cancelled(cancel)?;
            let mut reader = image::ImageReader::with_format(Cursor::new(&source), format);
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(caps.max_texture_dimension);
            limits.max_image_height = Some(caps.max_texture_dimension);
            limits.max_alloc = Some(MAX_SCENE_BYTES as u64);
            if format == image::ImageFormat::Png {
                let png = image::codecs::png::PngDecoder::with_limits(
                    Cursor::new(&source),
                    limits.clone(),
                )
                .map_err(|_| PrepareError::InvalidImage)?;
                if png.is_apng().map_err(|_| PrepareError::InvalidImage)? {
                    return Err(PrepareError::UnsupportedImage);
                }
            }
            reader.limits(limits);
            let rgba = reader
                .decode()
                .map_err(|_| PrepareError::InvalidImage)?
                .into_rgba8()
                .into_raw();
            PreparedBackground::Image {
                version: resource.version,
                extent,
                rgba: rgba.into_boxed_slice(),
            }
        }
    };
    cancelled(cancel)?;
    // Same revalidation an owned transport snapshot gets on a worker.
    PreparedCue::from_owned(spec.version, spec.extent, background, text, caps)
}
