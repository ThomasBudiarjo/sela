//! Profile image resources and the logo choice. No GPUI types.
//!
//! Images live in `<profile>/Resources/Images/`; the logo choice is the image's
//! file name in `<profile>/logo.txt`. All file work runs on the `Worker` thread
//! or another background thread. Limits and policies: `docs/images.md`.
use crate::{
    background::{self, Aspect, ImageRef},
    scene::{
        self, BackgroundSpec, ContentVersion, Extent, MAX_SCENE_BYTES, MAX_SOURCE_BYTES,
        PrepareError, RendererCapabilities, ResourceRef, SceneSpec,
    },
};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
};

pub const IMAGES: &str = "Resources/Images";
pub const LOGO_FILE: &str = "logo.txt";
const MAX_NAME: usize = 255;
const MAX_IMAGES: usize = 4096;
/// Largest image file accepted, the same bound every preparation read uses.
pub const MAX_FILE_BYTES: usize = MAX_SOURCE_BYTES;
/// Largest stored edge in pixels. Sources are fitted on the CPU, so this may
/// exceed the output texture limit.
pub const MAX_EDGE: u32 = 16384;
/// Decoded straight RGBA budget for one image: 64 MiB, 16 777 216 pixels
/// (4096 × 4096, or 5120 × 3200).
pub const MAX_DECODED_BYTES: usize = MAX_SCENE_BYTES;
/// Write granularity, so a cancelled import stops between chunks.
const COPY_CHUNK: usize = 256 * 1024;
/// "name.png", then "name (2).png" up to this suffix.
const MAX_NAME_ATTEMPTS: u32 = 1000;
/// Bytes of same-stem images compared by one import while looking for an
/// identical copy; past this a new numbered copy is made.
const DEDUP_BUDGET: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Another I/O failure; the kind is shown to the operator.
    Io(io::ErrorKind),
    /// The image is not in the profile's `Resources/Images` folder.
    Missing,
    /// The file chosen for import or relink no longer exists.
    SourceMissing,
    Unsupported,
    /// The file exceeds `MAX_FILE_BYTES`.
    TooLarge,
    /// An edge exceeds `MAX_EDGE` or the decoded size `MAX_DECODED_BYTES`.
    Dimensions,
    /// Animated PNG; only still images are accepted.
    Animated,
    /// The header or pixel data does not decode.
    Corrupt,
    InvalidName,
    /// The chosen file cannot be read.
    ReadDenied,
    /// The profile's image folder cannot be read or changed.
    ProfileDenied,
    DiskFull,
    Cancelled,
    /// Relink: the chosen file's bytes differ from the pinned SHA-256.
    Mismatch,
    /// Relink: a different image already has the expected name.
    NameTaken,
    Busy,
    Disconnected,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Io(kind) => {
                return write!(
                    f,
                    "Image file cannot be read or written ({kind}); check the file and \
                     the profile folder, then try again"
                );
            }
            Self::Missing => "Image file is missing from the profile",
            Self::SourceMissing => {
                "The chosen file no longer exists; it may have been moved or renamed. \
                 Choose it again"
            }
            Self::Unsupported => "Only PNG and JPEG images can be imported",
            Self::TooLarge => "Image file is larger than 8 MiB; save a smaller copy and import it",
            Self::Dimensions => {
                "Image is too large: at most 16384 pixels on a side and 16.7 million \
                 pixels (for example 4096 × 4096). Save a smaller copy and import it"
            }
            Self::Animated => {
                "Animated images are not supported; export a still PNG or JPEG and import it"
            }
            Self::Corrupt => {
                "Image file is damaged or incomplete and cannot be decoded; export it again \
                 and retry"
            }
            Self::InvalidName => "Image file name is not usable",
            Self::ReadDenied => {
                "Sela is not allowed to read this file. Check its permissions, or copy it \
                 to a folder you own and import the copy"
            }
            Self::ProfileDenied => {
                "Sela is not allowed to change the profile's Resources/Images folder. Make \
                 sure it is not read-only and that your account may change it, then try again"
            }
            Self::DiskFull => "The disk is full; free some space, then try again",
            Self::Cancelled => "Import cancelled; nothing was added",
            Self::Mismatch => {
                "The chosen file is a different image (its contents do not match); nothing \
                 was changed. Choose the original file"
            }
            Self::NameTaken => {
                "A different image already uses that name in Resources/Images; nothing was \
                 replaced. Rename or remove that image first, then relink"
            }
            Self::Busy => "Image work is still running; try again",
            Self::Disconnected => "Image worker stopped; restart Sela",
        })
    }
}
impl std::error::Error for Error {}

/// Which side of a copy failed, so the message names what to fix.
#[derive(Clone, Copy)]
enum Side {
    /// The file the operator chose.
    Source,
    /// The profile folder.
    Profile,
}

fn io_at(error: io::Error, side: Side) -> Error {
    use io::ErrorKind as Kind;
    match (error.kind(), side) {
        (Kind::NotFound, Side::Source) => Error::SourceMissing,
        (Kind::NotFound, Side::Profile) => Error::Missing,
        (Kind::PermissionDenied, Side::Source) => Error::ReadDenied,
        (Kind::PermissionDenied | Kind::ReadOnlyFilesystem, Side::Profile) => Error::ProfileDenied,
        (Kind::StorageFull, _) => Error::DiskFull,
        (kind, _) => Error::Io(kind),
    }
}

fn io_error(error: io::Error) -> Error {
    io_at(error, Side::Profile)
}

/// The chosen logo image, hashed so preparation detects later changes.
#[derive(Clone)]
pub struct Logo {
    pub name: String,
    pub resource: ResourceRef,
}

pub fn images_dir(profile: &Path) -> PathBuf {
    profile.join(IMAGES)
}

/// A bare file name with a PNG or JPEG extension; no separators or dot files.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && name.trim() == name
        && Path::new(name).file_name() == Some(name.as_ref())
        && extension(name).is_some()
}

fn extension(name: &str) -> Option<&'static str> {
    let (_, ext) = name.rsplit_once('.')?;
    match ext.to_ascii_lowercase().as_str() {
        "png" => Some("png"),
        "jpg" => Some("jpg"),
        "jpeg" => Some("jpeg"),
        _ => None,
    }
}

/// Image names in the profile, sorted case-insensitively. A missing folder is empty.
pub fn list(profile: &Path) -> Result<Vec<String>, Error> {
    let entries = match fs::read_dir(images_dir(profile)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(error)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(io_error)?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if valid_name(&name) && entry.file_type().is_ok_and(|t| t.is_file()) {
            names.push(name);
            if names.len() == MAX_IMAGES {
                break;
            }
        }
    }
    names.sort_by_key(|name| name.to_lowercase());
    Ok(names)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, Error> {
    read_bounded_at(path, Side::Profile)
}

fn read_bounded_at(path: &Path, side: Side) -> Result<Vec<u8>, Error> {
    let error = |e| io_at(e, side);
    let metadata = fs::metadata(path).map_err(error)?;
    if !metadata.is_file() {
        return Err(Error::Io(io::ErrorKind::InvalidInput));
    }
    if metadata.len() > MAX_FILE_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(error)?
        .take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(Error::TooLarge);
    }
    Ok(bytes)
}

fn check_image(bytes: &[u8]) -> Result<(), Error> {
    let format = image::guess_format(bytes).map_err(|_| Error::Unsupported)?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err(Error::Unsupported);
    }
    image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|_| Error::Unsupported)?;
    Ok(())
}

/// What a header inspection found, before the full decode.
struct Inspected {
    orientation: image::metadata::Orientation,
    icc: bool,
}

/// Header-only checks: format, animation, declared dimensions and metadata.
/// Declared dimensions are bounded here, before any pixel buffer exists.
fn inspect(bytes: &[u8]) -> Result<Inspected, Error> {
    fn read(decoder: &mut impl image::ImageDecoder) -> Result<Inspected, Error> {
        let (width, height) = decoder.dimensions();
        if width == 0 || height == 0 {
            return Err(Error::Corrupt);
        }
        // `total_bytes` is the decoder's own buffer, twice RGBA for 16-bit PNGs.
        if width > MAX_EDGE
            || height > MAX_EDGE
            || u64::from(width) * u64::from(height) * 4 > MAX_DECODED_BYTES as u64
            || decoder.total_bytes() > MAX_DECODED_BYTES as u64
        {
            return Err(Error::Dimensions);
        }
        Ok(Inspected {
            orientation: decoder.orientation().map_err(|_| Error::Corrupt)?,
            icc: decoder.icc_profile().map_err(|_| Error::Corrupt)?.is_some(),
        })
    }
    let limited = |error: image::ImageError| match error {
        image::ImageError::Limits(_) => Error::Dimensions,
        _ => Error::Corrupt,
    };
    match image::guess_format(bytes).map_err(|_| Error::Unsupported)? {
        image::ImageFormat::Png => {
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(MAX_EDGE);
            limits.max_image_height = Some(MAX_EDGE);
            limits.max_alloc = Some(MAX_DECODED_BYTES as u64);
            let mut png = image::codecs::png::PngDecoder::with_limits(Cursor::new(bytes), limits)
                .map_err(limited)?;
            if png.is_apng().map_err(|_| Error::Corrupt)? {
                return Err(Error::Animated);
            }
            read(&mut png)
        }
        image::ImageFormat::Jpeg => {
            read(&mut image::codecs::jpeg::JpegDecoder::new(Cursor::new(bytes)).map_err(limited)?)
        }
        _ => Err(Error::Unsupported),
    }
}

/// A completed import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Imported {
    /// The stored file name in `Resources/Images`.
    pub name: String,
    pub sha256: [u8; 32],
    /// Displayed size, after the Exif orientation.
    pub extent: Extent,
    /// Identical bytes were already stored under this name; nothing was copied.
    pub reused: bool,
    /// The file's Exif orientation was not upright and is applied on display.
    pub oriented: bool,
    /// An embedded ICC color profile is ignored; samples are shown as sRGB.
    pub icc_ignored: bool,
}

impl Imported {
    pub fn image_ref(&self) -> ImageRef {
        ImageRef {
            name: self.name.clone(),
            sha256: self.sha256,
        }
    }
}

/// Copies a PNG/JPEG into the profile without replacing an existing image;
/// a clashing name gets a " (2)" style suffix. Returns the stored name.
pub fn import(profile: &Path, source: &Path) -> Result<String, Error> {
    import_with(profile, source, &mut || false).map(|imported| imported.name)
}

/// `import` with cancellation and the full result. Blocking: background
/// threads only. A cancelled or failed import leaves no file behind.
pub fn import_checked(
    profile: &Path,
    source: &Path,
    cancel: &AtomicBool,
) -> Result<Imported, Error> {
    import_with(profile, source, &mut || cancel.load(Ordering::Acquire))
}

fn numbered(stem: &str, attempt: u32, ext: &str) -> String {
    if attempt == 1 {
        format!("{stem}.{ext}")
    } else {
        format!("{stem} ({attempt}).{ext}")
    }
}

/// `cancel` is polled between steps and copy chunks; once the file has its
/// final name the import is complete and no longer cancellable.
fn import_with(
    profile: &Path,
    source: &Path,
    cancel: &mut dyn FnMut() -> bool,
) -> Result<Imported, Error> {
    let name = source
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(Error::InvalidName)?;
    let ext = extension(name).ok_or(Error::Unsupported)?;
    let (stem, _) = name.rsplit_once('.').ok_or(Error::Unsupported)?;
    let stem: String = stem
        .chars()
        .map(|c| if c.is_control() { '_' } else { c })
        .collect();
    let stem = stem.trim().trim_start_matches('.');
    let stem = if stem.is_empty() { "image" } else { stem };
    let bytes = read_bounded_at(source, Side::Source)?;
    if cancel() {
        return Err(Error::Cancelled);
    }
    let inspected = inspect(&bytes)?;
    // The full bounded decode proves the pixel data is intact, so an imported
    // image never renders black for corruption later.
    let (extent, _) = scene::decode_image(
        &bytes,
        SOURCE_CAPS,
        MAX_DECODED_BYTES,
        &AtomicBool::new(false),
    )
    .map_err(|error| match error {
        PrepareError::TooLarge => Error::Dimensions,
        PrepareError::UnsupportedImage => Error::Unsupported,
        _ => Error::Corrupt,
    })?;
    if cancel() {
        return Err(Error::Cancelled);
    }
    let sha256: [u8; 32] = Sha256::digest(&bytes).into();
    let imported = |name: String, reused: bool| Imported {
        name,
        sha256,
        extent,
        reused,
        oriented: inspected.orientation != image::metadata::Orientation::NoTransforms,
        icc_ignored: inspected.icc,
    };
    let dir = images_dir(profile);
    fs::create_dir_all(&dir).map_err(io_error)?;
    let mut budget = DEDUP_BUDGET;
    let mut free = None;
    for attempt in 1..=MAX_NAME_ATTEMPTS {
        let candidate = numbered(stem, attempt, ext);
        if !valid_name(&candidate) {
            return Err(Error::InvalidName);
        }
        let metadata = match fs::symlink_metadata(dir.join(&candidate)) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                free = Some(attempt);
                break;
            }
            Err(error) => return Err(io_error(error)),
        };
        if metadata.is_file() && metadata.len() == bytes.len() as u64 && metadata.len() <= budget {
            budget -= metadata.len();
            if read_bounded(&dir.join(&candidate)).is_ok_and(|existing| existing == bytes) {
                return Ok(imported(candidate, true));
            }
        }
    }
    let free = free.ok_or(Error::InvalidName)?;
    let temporary = write_temporary(&dir, &bytes, cancel)?;
    for attempt in free..=MAX_NAME_ATTEMPTS {
        let candidate = numbered(stem, attempt, ext);
        if !valid_name(&candidate) {
            return Err(Error::InvalidName);
        }
        match link_new(&temporary.0, &dir.join(&candidate)) {
            Ok(()) => return Ok(imported(candidate, false)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(error)),
        }
    }
    Err(Error::InvalidName)
}

/// A dot-prefixed temporary in the images folder (never listed), removed on
/// drop. After a successful link it is only the second name of the file.
struct Temporary(PathBuf);

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn write_temporary(
    dir: &Path,
    bytes: &[u8],
    cancel: &mut dyn FnMut() -> bool,
) -> Result<Temporary, Error> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut opened = None;
    for _ in 0..16 {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!(".sela-import-{}-{n}.tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                opened = Some((Temporary(path), file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(error)),
        }
    }
    let (temporary, mut file) = opened.ok_or(Error::Io(io::ErrorKind::AlreadyExists))?;
    for chunk in bytes.chunks(COPY_CHUNK) {
        if cancel() {
            return Err(Error::Cancelled);
        }
        file.write_all(chunk).map_err(io_error)?;
    }
    file.sync_all().map_err(io_error)?;
    drop(file);
    if cancel() {
        return Err(Error::Cancelled);
    }
    Ok(temporary)
}

/// Gives `temporary`'s file the name `path` only if `path` is free, so an
/// existing image is never replaced (`AlreadyExists` otherwise).
fn link_new(temporary: &Path, path: &Path) -> io::Result<()> {
    match fs::hard_link(temporary, path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(error),
        // FAT/exFAT volumes and some shares have no hard links: reserve the
        // name with an empty file, then rename over that placeholder.
        Err(_) => {
            OpenOptions::new().write(true).create_new(true).open(path)?;
            fs::rename(temporary, path).inspect_err(|_| {
                let _ = fs::remove_file(path);
            })
        }
    }
}

/// Outcome of a successful relink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relinked {
    /// The chosen file was copied into the profile under the pinned name.
    Restored,
    /// The pinned name already holds the matching bytes; nothing was copied.
    AlreadyPresent,
}

/// Repairs a missing pinned image from a file the operator located: only a
/// file whose SHA-256 equals the pin is copied in, under the pinned name. A
/// different file at that name is never replaced. Blocking: background
/// threads only.
pub fn relink(
    profile: &Path,
    image: &ImageRef,
    source: &Path,
    cancel: &AtomicBool,
) -> Result<Relinked, Error> {
    relink_with(profile, image, source, &mut || {
        cancel.load(Ordering::Acquire)
    })
}

fn relink_with(
    profile: &Path,
    image: &ImageRef,
    source: &Path,
    cancel: &mut dyn FnMut() -> bool,
) -> Result<Relinked, Error> {
    if !valid_name(&image.name) {
        return Err(Error::InvalidName);
    }
    let bytes = read_bounded_at(source, Side::Source)?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != image.sha256 {
        return Err(Error::Mismatch);
    }
    if cancel() {
        return Err(Error::Cancelled);
    }
    let dir = images_dir(profile);
    let path = dir.join(&image.name);
    let occupant = || match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => match read_bounded(&path) {
            Ok(existing) if existing == bytes => Ok(Some(Relinked::AlreadyPresent)),
            Ok(_) | Err(Error::TooLarge) => Err(Error::NameTaken),
            Err(error) => Err(error),
        },
        Ok(_) => Err(Error::NameTaken),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(error)),
    };
    if let Some(done) = occupant()? {
        return Ok(done);
    }
    fs::create_dir_all(&dir).map_err(io_error)?;
    let temporary = write_temporary(&dir, &bytes, cancel)?;
    match link_new(&temporary.0, &path) {
        Ok(()) => Ok(Relinked::Restored),
        // Something took the name meanwhile; judge it like before.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            occupant()?.ok_or(Error::NameTaken)
        }
        Err(error) => Err(io_error(error)),
    }
}

/// SHA-256 of a profile image without decoding it, read with the usual
/// bounds; preflight compares it with pinned references.
pub fn content_hash(profile: &Path, name: &str) -> Result<[u8; 32], Substitute> {
    if !valid_name(name) {
        return Err(Substitute::Invalid);
    }
    read_bounded(&images_dir(profile).join(name))
        .map(|bytes| Sha256::digest(&bytes).into())
        .map_err(|error| match error {
            Error::Missing => Substitute::Missing,
            Error::TooLarge => Substitute::TooLarge,
            _ => Substitute::Invalid,
        })
}

/// Hashes a profile image for preparation. The version derives from content.
pub fn resource(profile: &Path, name: &str) -> Result<ResourceRef, Error> {
    if !valid_name(name) {
        return Err(Error::InvalidName);
    }
    let path = images_dir(profile).join(name);
    let bytes = read_bounded(&path)?;
    check_image(&bytes)?;
    let sha256: [u8; 32] = Sha256::digest(&bytes).into();
    Ok(ResourceRef {
        version: ContentVersion {
            id: u128::from_le_bytes(sha256[..16].try_into().unwrap()),
            revision: 1,
        },
        path,
        sha256,
    })
}

/// The stored logo name, or None when no logo was chosen.
pub fn read_logo(profile: &Path) -> Result<Option<String>, Error> {
    let path = profile.join(LOGO_FILE);
    let bytes = match read_bounded(&path) {
        Ok(bytes) => bytes,
        Err(Error::Missing) => return Ok(None),
        Err(error) => return Err(error),
    };
    let text = String::from_utf8(bytes).map_err(|_| Error::InvalidName)?;
    let name = text.trim_end_matches(['\r', '\n']);
    if name.is_empty() {
        return Ok(None);
    }
    if !valid_name(name) {
        return Err(Error::InvalidName);
    }
    Ok(Some(name.to_owned()))
}

/// Replaces `logo.txt` through a temporary file so a crash keeps the old choice.
pub fn write_logo(profile: &Path, name: &str) -> Result<(), Error> {
    if !valid_name(name) {
        return Err(Error::InvalidName);
    }
    fs::create_dir_all(profile).map_err(io_error)?;
    let temporary = profile.join(".logo.txt.tmp");
    let result = (|| {
        let mut file = File::create(&temporary)?;
        file.write_all(name.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, profile.join(LOGO_FILE))
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(io_error(error));
    }
    Ok(())
}

fn load(profile: &Path, name: String) -> Result<Logo, Error> {
    Ok(Logo {
        resource: resource(profile, &name)?,
        name,
    })
}

/// One logo cue identity per image and surface extent, so the renderer's
/// acknowledged logo identifies both.
pub fn logo_version(image: ContentVersion, extent: Extent) -> ContentVersion {
    ContentVersion {
        id: image.id,
        revision: (u64::from(extent.width) << 32) | u64::from(extent.height),
    }
}

/// The logo as a scene: the image alone, fitted by the renderer.
pub fn logo_spec(logo: &Logo, extent: Extent) -> SceneSpec {
    SceneSpec {
        version: logo_version(logo.resource.version, extent),
        extent,
        background: BackgroundSpec::Image(logo.resource.clone()),
        text: None,
    }
}

/// Why a slide background image shows black instead: the owner's rule for a
/// missing or changed image (M1-05h), also used for unreadable files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Substitute {
    Missing,
    Changed,
    TooLarge,
    Invalid,
}

impl Substitute {
    pub fn warning(self, name: &str) -> String {
        let why = match self {
            Self::Missing => "is missing",
            Self::Changed => "changed since it was chosen",
            Self::TooLarge => "is too large",
            Self::Invalid => "cannot be read",
        };
        format!("Background image ‘{name}’ {why}; showing black")
    }
}

/// Source images may exceed the output texture because they are fitted on
/// the CPU; the decoded size stays within the 64 MiB scene budget.
const SOURCE_CAPS: RendererCapabilities = RendererCapabilities {
    max_texture_dimension: MAX_EDGE,
};

/// A background image's straight RGBA pixels, only if its bytes still match
/// the hash pinned when it was chosen.
pub fn decode_background(
    profile: &Path,
    image: &ImageRef,
) -> Result<(Extent, Vec<u8>), Substitute> {
    if !valid_name(&image.name) {
        return Err(Substitute::Invalid);
    }
    let bytes =
        read_bounded(&images_dir(profile).join(&image.name)).map_err(|error| match error {
            Error::Missing => Substitute::Missing,
            Error::TooLarge => Substitute::TooLarge,
            _ => Substitute::Invalid,
        })?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != image.sha256 {
        return Err(Substitute::Changed);
    }
    scene::decode_image(
        &bytes,
        SOURCE_CAPS,
        MAX_SCENE_BYTES,
        &AtomicBool::new(false),
    )
    .map_err(|error| match error {
        PrepareError::TooLarge => Substitute::TooLarge,
        _ => Substitute::Invalid,
    })
}

/// A background image decoded and fitted to exactly `extent`.
pub fn fitted_background(
    profile: &Path,
    image: &ImageRef,
    aspect: Aspect,
    extent: Extent,
) -> Fitted {
    if u64::from(extent.width) * u64::from(extent.height) * 4 > MAX_SCENE_BYTES as u64 {
        return Err(Substitute::TooLarge);
    }
    let (from, rgba) = decode_background(profile, image)?;
    background::fit(&rgba, from, extent, aspect)
        .map(Into::into)
        .ok_or(Substitute::Invalid)
}

/// A profile image pinned to its current bytes, for choosing it as a slide
/// background.
pub fn image_ref(profile: &Path, name: &str) -> Result<ImageRef, Error> {
    let resource = resource(profile, name)?;
    Ok(ImageRef {
        name: name.to_owned(),
        sha256: resource.sha256,
    })
}

/// A media picker thumbnail: the image zoomed to cover `extent`, as RGBA.
pub fn thumbnail(profile: &Path, name: &str, extent: Extent) -> Result<Vec<u8>, Error> {
    if !valid_name(name) {
        return Err(Error::InvalidName);
    }
    let bytes = read_bounded(&images_dir(profile).join(name))?;
    let (from, rgba) = scene::decode_image(
        &bytes,
        SOURCE_CAPS,
        MAX_SCENE_BYTES,
        &AtomicBool::new(false),
    )
    .map_err(|error| match error {
        PrepareError::TooLarge => Error::TooLarge,
        _ => Error::Unsupported,
    })?;
    background::fit(&rgba, from, extent, Aspect::Zoom).ok_or(Error::Unsupported)
}

/// Prepared-background identity: the image content and its fit.
pub fn background_version(image: &ImageRef, aspect: Aspect) -> ContentVersion {
    ContentVersion {
        id: u128::from_le_bytes(image.sha256[..16].try_into().unwrap()),
        revision: aspect as u64,
    }
}

pub type BackgroundKey = (ImageRef, Aspect, Extent);
pub type Fitted = Result<Arc<[u8]>, Substitute>;
/// About eleven 1080p backgrounds.
pub const BACKGROUND_BUDGET: usize = 96 * 1024 * 1024;
/// Kept even over budget (4K): the live slide and both neighbours.
pub const MIN_BACKGROUNDS: usize = 3;

/// Fitted backgrounds, least recently used first. Substitutes cost no bytes.
/// A dozen entries at most, so lookups are linear.
pub struct BackgroundCache {
    entries: VecDeque<(BackgroundKey, Fitted)>,
    budget: usize,
}

impl BackgroundCache {
    pub fn new(budget: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            budget,
        }
    }

    /// Looks up and marks the entry as most recently used.
    pub fn get(&mut self, key: &BackgroundKey) -> Option<Fitted> {
        let at = self.entries.iter().position(|(k, _)| k == key)?;
        let entry = self.entries.remove(at)?;
        let value = entry.1.clone();
        self.entries.push_back(entry);
        Some(value)
    }

    pub fn peek(&self, key: &BackgroundKey) -> Option<&Fitted> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn insert(&mut self, key: BackgroundKey, value: Fitted) {
        self.entries.retain(|(k, _)| *k != key);
        self.entries.push_back((key, value));
        while self.bytes() > self.budget && self.entries.len() > MIN_BACKGROUNDS {
            self.entries.pop_front();
        }
    }

    pub fn bytes(&self) -> usize {
        self.entries
            .iter()
            .map(|(_, v)| v.as_ref().map_or(0, |rgba| rgba.len()))
            .sum()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many backgrounds of `extent` fit the budget, never below the minimum;
    /// prefetching more than this would evict its own entries.
    pub fn capacity(&self, extent: Extent) -> usize {
        let each = (extent.width as usize * extent.height as usize * 4).max(1);
        (self.budget / each).max(MIN_BACKGROUNDS)
    }

    /// A substitute may be stale once the profile's images change.
    pub fn forget_substitutes(&mut self) {
        self.entries.retain(|(_, v)| v.is_ok());
    }
}

pub enum Job {
    Scan,
    Import(PathBuf),
    /// Validate, hash and persist an image as the logo.
    UseAsLogo(String),
    /// Read `logo.txt` and hash the image it names.
    LoadLogo,
    /// Decode, verify and fit a slide background for one output extent.
    Background(BackgroundKey),
}

pub enum Reply {
    Images(Result<Vec<String>, Error>),
    Imported(Result<String, Error>),
    Logo(Result<Option<Logo>, Error>),
    Background(BackgroundKey, Fitted),
}

/// One thread, two queued jobs and two buffered replies; never blocks the caller.
pub struct Worker {
    jobs: SyncSender<Job>,
    replies: Receiver<Reply>,
    /// Import jobs accepted so far, numbered from 1 in queue order.
    imports: AtomicU64,
    /// Imports numbered up to this are cancelled.
    cancelled: Arc<AtomicU64>,
}

impl Worker {
    pub fn start(profile: PathBuf) -> io::Result<Self> {
        let (jobs, incoming) = mpsc::sync_channel::<Job>(2);
        let (outgoing, replies) = mpsc::sync_channel(2);
        let cancelled = Arc::new(AtomicU64::new(0));
        let cancel = cancelled.clone();
        std::thread::Builder::new()
            .name("sela-images".into())
            .spawn(move || {
                let mut imports = 0;
                while let Ok(job) = incoming.recv() {
                    let reply = match job {
                        Job::Scan => Reply::Images(list(&profile)),
                        Job::Import(source) => {
                            imports += 1;
                            let mut cancelled = || cancel.load(Ordering::Acquire) >= imports;
                            Reply::Imported(
                                import_with(&profile, &source, &mut cancelled).map(|i| i.name),
                            )
                        }
                        Job::UseAsLogo(name) => Reply::Logo(
                            load(&profile, name)
                                .and_then(|logo| write_logo(&profile, &logo.name).map(|()| logo))
                                .map(Some),
                        ),
                        Job::LoadLogo => Reply::Logo(
                            read_logo(&profile)
                                .and_then(|name| name.map(|n| load(&profile, n)).transpose()),
                        ),
                        Job::Background(key) => {
                            let fitted = fitted_background(&profile, &key.0, key.1, key.2);
                            Reply::Background(key, fitted)
                        }
                    };
                    if outgoing.send(reply).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            jobs,
            replies,
            imports: AtomicU64::new(0),
            cancelled,
        })
    }

    pub fn submit(&self, job: Job) -> Result<(), Error> {
        let import = matches!(job, Job::Import(_));
        self.jobs.try_send(job).map_err(|error| match error {
            TrySendError::Full(_) => Error::Busy,
            TrySendError::Disconnected(_) => Error::Disconnected,
        })?;
        if import {
            self.imports.fetch_add(1, Ordering::AcqRel);
        }
        Ok(())
    }

    /// Cancels every import submitted so far, running or queued; each replies
    /// `Imported(Err(Cancelled))` unless its file already has its final name.
    /// Later imports are unaffected.
    pub fn cancel_imports(&self) {
        self.cancelled
            .fetch_max(self.imports.load(Ordering::Acquire), Ordering::AcqRel);
    }

    pub fn poll(&self) -> Option<Result<Reply, Error>> {
        match self.replies.try_recv() {
            Ok(reply) => Some(Ok(reply)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(Error::Disconnected)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;
    use std::time::{Duration, Instant};

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut encoded = Vec::new();
        let pixels = vec![200u8; (width * height * 4) as usize];
        image::codecs::png::PngEncoder::new(&mut encoded)
            .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
            .unwrap();
        encoded
    }

    #[test]
    fn names_are_bare_png_or_jpeg_files() {
        for good in ["logo.png", "Logo Final.JPG", "a.jpeg", "café.png"] {
            assert!(valid_name(good), "{good}");
        }
        for bad in [
            "",
            ".png",
            "logo",
            "logo.gif",
            "../logo.png",
            "a/b.png",
            "a\\b.png",
            "c:x.png",
            " logo.png",
            ".hidden.png",
        ] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn import_copies_without_replacing_and_rejects_non_images() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let source = dir.path().join("Church Logo.PNG");
        fs::write(&source, png(4, 2)).unwrap();
        assert_eq!(import(&profile, &source).unwrap(), "Church Logo.png");
        // Different bytes under the same name get the next free name.
        fs::write(&source, png(5, 2)).unwrap();
        assert_eq!(import(&profile, &source).unwrap(), "Church Logo (2).png");
        assert_eq!(
            fs::read(images_dir(&profile).join("Church Logo (2).png")).unwrap(),
            fs::read(&source).unwrap()
        );
        let fake = dir.path().join("fake.png");
        fs::write(&fake, b"not an image").unwrap();
        assert_eq!(import(&profile, &fake), Err(Error::Unsupported));
        let gif = dir.path().join("anim.gif");
        fs::write(&gif, png(1, 1)).unwrap();
        assert_eq!(import(&profile, &gif), Err(Error::Unsupported));
        assert_eq!(
            import(&profile, &dir.path().join("missing.png")),
            Err(Error::SourceMissing)
        );
        fs::write(images_dir(&profile).join("notes.txt"), b"x").unwrap();
        assert_eq!(
            list(&profile).unwrap(),
            ["Church Logo (2).png", "Church Logo.png"]
        );
        assert_eq!(
            list(&dir.path().join("none")).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn logo_choice_round_trips_and_ignores_unsafe_names() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path();
        assert!(read_logo(profile).unwrap().is_none());
        write_logo(profile, "logo.png").unwrap();
        assert_eq!(read_logo(profile).unwrap().as_deref(), Some("logo.png"));
        assert!(!profile.join(".logo.txt.tmp").exists());
        assert_eq!(write_logo(profile, "../x.png"), Err(Error::InvalidName));
        fs::write(profile.join(LOGO_FILE), "..\\evil.png\n").unwrap();
        assert_eq!(read_logo(profile), Err(Error::InvalidName));
        fs::write(profile.join(LOGO_FILE), "\n").unwrap();
        assert!(read_logo(profile).unwrap().is_none());
    }

    #[test]
    fn resource_hashes_content_and_logo_versions_differ_per_extent() {
        let dir = tempfile::tempdir().unwrap();
        let images = images_dir(dir.path());
        fs::create_dir_all(&images).unwrap();
        fs::write(images.join("a.png"), png(2, 2)).unwrap();
        fs::write(images.join("b.png"), png(3, 2)).unwrap();
        let a = resource(dir.path(), "a.png").unwrap();
        let b = resource(dir.path(), "b.png").unwrap();
        assert_ne!(a.version, b.version);
        assert_eq!(resource(dir.path(), "a.png").unwrap().version, a.version);
        assert_eq!(resource(dir.path(), "c.png").err(), Some(Error::Missing));
        let small = Extent {
            width: 640,
            height: 360,
        };
        let large = Extent {
            width: 1920,
            height: 1080,
        };
        assert_ne!(
            logo_version(a.version, small),
            logo_version(a.version, large)
        );
        let logo = Logo {
            name: "a.png".into(),
            resource: a,
        };
        let spec = logo_spec(&logo, small);
        assert_eq!(spec.extent, small);
        assert!(spec.text.is_none());
        assert!(matches!(spec.background, BackgroundSpec::Image(_)));
    }

    fn pinned(profile: &Path, name: &str, bytes: &[u8]) -> ImageRef {
        let images = images_dir(profile);
        fs::create_dir_all(&images).unwrap();
        fs::write(images.join(name), bytes).unwrap();
        ImageRef {
            name: name.into(),
            sha256: Sha256::digest(bytes).into(),
        }
    }

    #[test]
    fn background_decode_substitutes_missing_changed_and_corrupt_images() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path();
        let good = pinned(profile, "wide.png", &png(4, 2));
        let (extent, rgba) = decode_background(profile, &good).unwrap();
        assert_eq!((extent.width, extent.height, rgba.len()), (4, 2, 32));
        let out = Extent {
            width: 6,
            height: 6,
        };
        let fitted = fitted_background(profile, &good, Aspect::Maintain, out).unwrap();
        assert_eq!(fitted.len(), 6 * 6 * 4);
        assert_eq!(fitted[..4], [0, 0, 0, 255], "letterbox bar");

        let missing = ImageRef {
            name: "gone.png".into(),
            ..good.clone()
        };
        assert_eq!(
            decode_background(profile, &missing).err(),
            Some(Substitute::Missing)
        );
        fs::write(images_dir(profile).join("wide.png"), png(2, 2)).unwrap();
        assert_eq!(
            decode_background(profile, &good).err(),
            Some(Substitute::Changed)
        );
        // Hash-pinned garbage and a GIF named .png are unreadable, not shown.
        let corrupt = pinned(profile, "corrupt.png", b"\x89PNG\r\n\x1a\nnot really");
        assert_eq!(
            decode_background(profile, &corrupt).err(),
            Some(Substitute::Invalid)
        );
        let gif = pinned(profile, "anim.png", b"GIF89a\x01\x00\x01\x00\x00\x00\x00;");
        assert_eq!(
            fitted_background(profile, &gif, Aspect::Zoom, out).err(),
            Some(Substitute::Invalid)
        );
        let unsafe_name = ImageRef {
            name: "../wide.png".into(),
            ..good.clone()
        };
        assert_eq!(
            decode_background(profile, &unsafe_name).err(),
            Some(Substitute::Invalid)
        );
        let huge = Extent {
            width: 8192,
            height: 8192,
        };
        assert_eq!(
            fitted_background(profile, &corrupt, Aspect::Zoom, huge).err(),
            Some(Substitute::TooLarge)
        );
        assert_eq!(
            Substitute::Missing.warning("Sunrise.jpg"),
            "Background image ‘Sunrise.jpg’ is missing; showing black"
        );
    }

    #[test]
    fn picker_pins_the_current_bytes_and_thumbnails_cover_the_extent() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path();
        let good = pinned(profile, "wide.png", &png(4, 2));
        assert_eq!(image_ref(profile, "wide.png").unwrap(), good);
        assert_eq!(decode_background(profile, &good).unwrap().0.width, 4);
        let thumb = Extent {
            width: 3,
            height: 3,
        };
        let rgba = thumbnail(profile, "wide.png", thumb).unwrap();
        assert_eq!(rgba.len(), 3 * 3 * 4);
        assert_eq!(rgba[..4], [200, 200, 200, 200], "zoomed, no bars");
        assert_eq!(image_ref(profile, "gone.png").err(), Some(Error::Missing));
        assert_eq!(
            thumbnail(profile, "../wide.png", thumb).err(),
            Some(Error::InvalidName)
        );
        pinned(profile, "corrupt.png", b"\x89PNG\r\n\x1a\nnot really");
        assert_eq!(
            thumbnail(profile, "corrupt.png", thumb).err(),
            Some(Error::Unsupported)
        );
    }

    #[test]
    fn background_cache_keeps_the_budget_but_never_fewer_than_three() {
        let key = |n: u8, w: u32| {
            (
                ImageRef {
                    name: format!("{n}.png"),
                    sha256: [n; 32],
                },
                Aspect::Zoom,
                Extent {
                    width: w,
                    height: 1,
                },
            )
        };
        let rgba = |len: usize| -> Fitted { Ok(vec![0; len].into()) };
        let mut cache = BackgroundCache::new(100);
        for n in 0..4 {
            cache.insert(key(n, 10), rgba(40));
        }
        // 160 bytes over a 100-byte budget: only the oldest goes.
        assert_eq!((cache.len(), cache.bytes()), (3, 120));
        assert!(cache.peek(&key(0, 10)).is_none());
        // A lookup refreshes recency, so 2 is evicted next instead of 1.
        assert!(cache.get(&key(1, 10)).is_some());
        cache.insert(key(4, 10), rgba(40));
        assert!(cache.peek(&key(1, 10)).is_some());
        assert!(cache.peek(&key(2, 10)).is_none());
        // Same image at another extent is a different entry. Substitutes cost
        // nothing, so the budget alone keeps four entries.
        cache.insert(key(1, 20), Err(Substitute::Missing));
        assert_eq!((cache.len(), cache.bytes()), (3, 80));
        cache.insert(key(5, 10), rgba(10));
        assert_eq!((cache.len(), cache.bytes()), (4, 90));
        cache.forget_substitutes();
        assert!(cache.peek(&key(1, 20)).is_none());
        let hd = Extent {
            width: 1920,
            height: 1080,
        };
        let uhd = Extent {
            width: 3840,
            height: 2160,
        };
        let real = BackgroundCache::new(BACKGROUND_BUDGET);
        assert_eq!(real.capacity(hd), 12);
        assert_eq!(real.capacity(uhd), MIN_BACKGROUNDS);
    }

    #[test]
    fn worker_fits_backgrounds_and_reports_substitutes() {
        let dir = tempfile::tempdir().unwrap();
        let good = pinned(dir.path(), "a.png", &png(2, 2));
        let worker = Worker::start(dir.path().to_path_buf()).unwrap();
        let out = Extent {
            width: 3,
            height: 2,
        };
        let missing = ImageRef {
            name: "b.png".into(),
            ..good.clone()
        };
        worker
            .submit(Job::Background((good.clone(), Aspect::Stretch, out)))
            .unwrap();
        worker
            .submit(Job::Background((missing.clone(), Aspect::Zoom, out)))
            .unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let mut replies = Vec::new();
        while replies.len() < 2 {
            if let Some(reply) = worker.poll() {
                replies.push(reply.unwrap());
            }
            assert!(Instant::now() < end, "bounded wait");
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(matches!(&replies[0],
            Reply::Background((image, Aspect::Stretch, _), Ok(rgba)) if *image == good && rgba.len() == 24));
        assert!(matches!(&replies[1],
            Reply::Background((image, _, _), Err(Substitute::Missing)) if *image == missing));
    }

    #[test]
    fn worker_imports_persists_and_reloads_the_logo() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let source = dir.path().join("logo.png");
        fs::write(&source, png(2, 2)).unwrap();
        let worker = Worker::start(profile.clone()).unwrap();
        let wait = || {
            let end = Instant::now() + Duration::from_secs(10);
            loop {
                if let Some(reply) = worker.poll() {
                    return reply.unwrap();
                }
                assert!(Instant::now() < end, "bounded wait");
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        worker.submit(Job::LoadLogo).unwrap();
        assert!(matches!(wait(), Reply::Logo(Ok(None))));
        worker.submit(Job::Import(source)).unwrap();
        assert!(matches!(wait(), Reply::Imported(Ok(name)) if name == "logo.png"));
        worker.submit(Job::UseAsLogo("logo.png".into())).unwrap();
        assert!(matches!(wait(), Reply::Logo(Ok(Some(logo))) if logo.name == "logo.png"));
        worker.submit(Job::LoadLogo).unwrap();
        assert!(matches!(wait(), Reply::Logo(Ok(Some(logo))) if logo.name == "logo.png"));
        fs::remove_file(images_dir(&profile).join("logo.png")).unwrap();
        worker.submit(Job::LoadLogo).unwrap();
        assert!(matches!(wait(), Reply::Logo(Err(Error::Missing))));
        worker.submit(Job::UseAsLogo("../x.png".into())).unwrap();
        assert!(matches!(wait(), Reply::Logo(Err(Error::InvalidName))));
    }

    // Fixtures below are assembled byte by byte from the PNG, JPEG and Exif
    // layouts, so expected results never come from the code under test.

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut covered = kind.to_vec();
        covered.extend_from_slice(data);
        out.extend_from_slice(&crc32(&covered).to_be_bytes());
        out
    }

    /// The signature (8 bytes) and IHDR (25) come first.
    const AFTER_IHDR: usize = 33;

    fn with_chunk(png: &[u8], extra: Vec<u8>) -> Vec<u8> {
        let mut out = png[..AFTER_IHDR].to_vec();
        out.extend(extra);
        out.extend_from_slice(&png[AFTER_IHDR..]);
        out
    }

    /// A one-pixel PNG whose IHDR declares `width` × `height`.
    fn declaring(width: u32, height: u32) -> Vec<u8> {
        declaring_depth(width, height, 8)
    }

    /// The same, with an RGBA bit depth of 8 or 16.
    fn declaring_depth(width: u32, height: u32, depth: u8) -> Vec<u8> {
        let mut bytes = png(1, 1);
        assert_eq!(&bytes[12..16], b"IHDR");
        assert_eq!(bytes[25], 6, "RGBA");
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        bytes[24] = depth;
        let crc = crc32(&bytes[12..29]);
        bytes[29..33].copy_from_slice(&crc.to_be_bytes());
        bytes
    }

    /// Little-endian TIFF header and IFD0 with one SHORT Orientation (0x0112).
    fn exif_orientation(value: u16) -> Vec<u8> {
        let mut tiff = b"II*\0".to_vec();
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&value.to_le_bytes());
        tiff.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        tiff
    }

    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    /// 2 × 1: red on the left, blue on the right.
    fn red_blue_png() -> Vec<u8> {
        let mut encoded = Vec::new();
        image::codecs::png::PngEncoder::new(&mut encoded)
            .write_image(&[RED, BLUE].concat(), 2, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        encoded
    }

    /// 32 × 16: red left half, blue right half, Exif orientation in APP1.
    fn red_blue_jpeg(orientation: u16) -> Vec<u8> {
        let mut rgb = Vec::new();
        for _ in 0..16 {
            for x in 0..32 {
                rgb.extend_from_slice(if x < 16 { &[255, 0, 0] } else { &[0, 0, 255] });
            }
        }
        let mut encoded = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 95)
            .write_image(&rgb, 32, 16, image::ExtendedColorType::Rgb8)
            .unwrap();
        // APP1 goes after SOI and the JFIF APP0 segment, if any.
        let mut at = 2;
        if encoded[2..4] == [0xFF, 0xE0] {
            at += 2 + usize::from(u16::from_be_bytes([encoded[4], encoded[5]]));
        }
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend(exif_orientation(orientation));
        let mut out = encoded[..at].to_vec();
        out.extend_from_slice(&[0xFF, 0xE1]);
        out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        out.extend(payload);
        out.extend_from_slice(&encoded[at..]);
        out
    }

    /// Incompressible RGBA, so the PNG spans several copy chunks.
    fn noise_png(side: u32) -> Vec<u8> {
        let mut state = 0x2545_f491_u32;
        let pixels: Vec<u8> = (0..side * side * 4)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 24) as u8
            })
            .collect();
        let mut encoded = Vec::new();
        image::codecs::png::PngEncoder::new(&mut encoded)
            .write_image(&pixels, side, side, image::ExtendedColorType::Rgba8)
            .unwrap();
        encoded
    }

    fn pixel(rgba: &[u8], extent: Extent, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * extent.width + x) * 4) as usize;
        rgba[at..at + 4].try_into().unwrap()
    }

    fn near(actual: [u8; 4], expected: [u8; 4]) -> bool {
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| a.abs_diff(e) <= 40)
    }

    /// Every directory entry, including dot files; empty if the folder is absent.
    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = match fs::read_dir(dir) {
            Ok(entries) => entries
                .map(|e| e.unwrap().file_name().into_string().unwrap())
                .collect(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => panic!("{error}"),
        };
        names.sort();
        names
    }

    #[test]
    fn import_reuses_identical_bytes_and_keeps_each_version_of_a_name() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let source = dir.path().join("Sunrise.png");
        let never = AtomicBool::new(false);
        let (first, second) = (png(4, 2), png(2, 4));
        fs::write(&source, &first).unwrap();
        let a = import_checked(&profile, &source, &never).unwrap();
        assert_eq!((a.name.as_str(), a.reused), ("Sunrise.png", false));
        assert_eq!(a.sha256, <[u8; 32]>::from(Sha256::digest(&first)));
        assert_eq!(
            a.extent,
            Extent {
                width: 4,
                height: 2
            }
        );
        assert!(!a.oriented && !a.icc_ignored);
        let again = import_checked(&profile, &source, &never).unwrap();
        assert_eq!((again.name.as_str(), again.reused), ("Sunrise.png", true));
        // Same name, different bytes: the original stays and both are pinned.
        fs::write(&source, &second).unwrap();
        let b = import_checked(&profile, &source, &never).unwrap();
        assert_eq!((b.name.as_str(), b.reused), ("Sunrise (2).png", false));
        assert_eq!(b.sha256, <[u8; 32]>::from(Sha256::digest(&second)));
        let images = images_dir(&profile);
        assert_eq!(fs::read(images.join("Sunrise.png")).unwrap(), first);
        assert_eq!(
            image_ref(&profile, "Sunrise (2).png").unwrap(),
            b.image_ref()
        );
        assert_eq!(
            decode_background(&profile, &a.image_ref()).unwrap().0,
            a.extent
        );
        assert_eq!(
            decode_background(&profile, &b.image_ref()).unwrap().0,
            b.extent
        );
        let b_again = import_checked(&profile, &source, &never).unwrap();
        assert_eq!(
            (b_again.name.as_str(), b_again.reused),
            ("Sunrise (2).png", true)
        );
        assert_eq!(entries(&images), ["Sunrise (2).png", "Sunrise.png"]);
    }

    #[test]
    fn import_refuses_oversized_files_and_decompression_bombs() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let huge = dir.path().join("huge.png");
        let mut bytes = png(1, 1);
        bytes.resize(MAX_FILE_BYTES + 1, 0);
        fs::write(&huge, &bytes).unwrap();
        assert_eq!(import(&profile, &huge), Err(Error::TooLarge));
        // Tiny files declaring enormous images: refused from the header, before
        // any pixel buffer is allocated.
        for (width, height) in [
            (100_000, 100_000),
            (16_385, 1),
            (1, 16_385),
            (4_097, 4_096),
            (16_000, 16_000),
        ] {
            let bomb = dir.path().join(format!("bomb-{width}x{height}.png"));
            let bytes = declaring(width, height);
            assert!(bytes.len() < 128);
            fs::write(&bomb, bytes).unwrap();
            assert_eq!(
                import(&profile, &bomb),
                Err(Error::Dimensions),
                "{width}x{height}"
            );
        }
        // Exactly the 16 777 216-pixel budget passes the header check, but one
        // pixel of data cannot fill it, so the full decode refuses it.
        let short = dir.path().join("short.png");
        fs::write(&short, declaring(4_096, 4_096)).unwrap();
        assert_eq!(import(&profile, &short), Err(Error::Corrupt));
        // 16-bit samples need 8 bytes a pixel while decoding: 3000 × 3000 is
        // 72 000 000 bytes, over the 67 108 864-byte budget.
        let deep = dir.path().join("deep.png");
        fs::write(&deep, declaring_depth(3_000, 3_000, 16)).unwrap();
        assert_eq!(import(&profile, &deep), Err(Error::Dimensions));
        assert!(entries(&images_dir(&profile)).is_empty());
        // The renderer path refuses the same file if one is placed by hand.
        let placed = pinned(&profile, "bomb.png", &declaring(100_000, 100_000));
        assert_eq!(
            decode_background(&profile, &placed).err(),
            Some(Substitute::TooLarge)
        );
    }

    #[test]
    fn import_refuses_animated_png() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        // acTL: one frame, endless plays. The control chunk alone marks the
        // file as an APNG.
        let mut control = 1u32.to_be_bytes().to_vec();
        control.extend_from_slice(&0u32.to_be_bytes());
        let source = dir.path().join("spinner.png");
        fs::write(&source, with_chunk(&png(2, 2), chunk(b"acTL", &control))).unwrap();
        assert_eq!(import(&profile, &source), Err(Error::Animated));
        assert!(entries(&images_dir(&profile)).is_empty());
    }

    #[test]
    fn import_and_decode_apply_exif_orientation() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let never = AtomicBool::new(false);
        let tall = Extent {
            width: 1,
            height: 2,
        };
        let wide = Extent {
            width: 2,
            height: 1,
        };
        // Exif 6 turns the stored image 90° clockwise for display: its left
        // (red) column becomes the top row. 8 turns it counter-clockwise, 3
        // half a turn, 1 is upright.
        for (value, extent, first, second) in [
            (6, tall, RED, BLUE),
            (8, tall, BLUE, RED),
            (3, wide, BLUE, RED),
            (1, wide, RED, BLUE),
        ] {
            let source = dir.path().join(format!("turn{value}.png"));
            let tagged = with_chunk(&red_blue_png(), chunk(b"eXIf", &exif_orientation(value)));
            fs::write(&source, tagged).unwrap();
            let imported = import_checked(&profile, &source, &never).unwrap();
            assert_eq!(imported.extent, extent, "Exif {value}");
            assert_eq!(imported.oriented, value != 1, "Exif {value}");
            let (decoded, rgba) = decode_background(&profile, &imported.image_ref()).unwrap();
            assert_eq!(decoded, extent, "Exif {value}");
            let last = (extent.width - 1, extent.height - 1);
            assert_eq!(pixel(&rgba, decoded, 0, 0), first, "Exif {value}");
            assert_eq!(
                pixel(&rgba, decoded, last.0, last.1),
                second,
                "Exif {value}"
            );
        }
        let thumb = thumbnail(&profile, "turn6.png", tall).unwrap();
        assert_eq!(
            thumb,
            [RED, BLUE].concat(),
            "thumbnails follow the orientation"
        );

        let source = dir.path().join("photo.jpg");
        fs::write(&source, red_blue_jpeg(6)).unwrap();
        let photo = import_checked(&profile, &source, &never).unwrap();
        let portrait = Extent {
            width: 16,
            height: 32,
        };
        assert_eq!((photo.extent, photo.oriented), (portrait, true));
        let (decoded, rgba) = decode_background(&profile, &photo.image_ref()).unwrap();
        assert_eq!(decoded, portrait);
        assert!(
            near(pixel(&rgba, decoded, 8, 8), RED),
            "top is the red half"
        );
        assert!(
            near(pixel(&rgba, decoded, 8, 24), BLUE),
            "bottom is the blue half"
        );
    }

    #[test]
    fn cancelled_import_leaves_no_file_even_mid_copy() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let images = images_dir(&profile);
        let source = dir.path().join("noise.png");
        let bytes = noise_png(512);
        assert!(bytes.len() > 2 * COPY_CHUNK);
        fs::write(&source, &bytes).unwrap();
        // Cancel as soon as part of the copy is on disk.
        let partial = std::cell::Cell::new(false);
        let mut cancel = || {
            let written = entries(&images)
                .iter()
                .any(|name| fs::metadata(images.join(name)).is_ok_and(|m| m.len() > 0));
            partial.set(written);
            written
        };
        assert_eq!(
            import_with(&profile, &source, &mut cancel),
            Err(Error::Cancelled)
        );
        assert!(partial.get(), "cancelled while copying");
        assert!(entries(&images).is_empty());
        assert_eq!(
            import_checked(&profile, &source, &AtomicBool::new(true)),
            Err(Error::Cancelled)
        );
        assert!(entries(&images).is_empty());
        assert_eq!(import(&profile, &source).unwrap(), "noise.png");
        assert_eq!(fs::read(images.join("noise.png")).unwrap(), bytes);
        assert_eq!(entries(&images), ["noise.png"]);
    }

    #[test]
    fn worker_cancels_submitted_imports_only() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let source = dir.path().join("a.png");
        fs::write(&source, png(2, 2)).unwrap();
        let worker = Worker::start(profile.clone()).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let submit = |job: fn(&Path) -> Job| loop {
            match worker.submit(job(&source)) {
                Ok(()) => break,
                Err(Error::Busy) => {
                    assert!(Instant::now() < end, "bounded wait");
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!("{error}"),
            }
        };
        let wait = || loop {
            if let Some(reply) = worker.poll() {
                return reply.unwrap();
            }
            assert!(Instant::now() < end, "bounded wait");
            std::thread::sleep(Duration::from_millis(2));
        };
        // Three unpolled scan replies overfill the two reply slots, so the
        // thread cannot start the import until a reply is polled.
        for _ in 0..3 {
            submit(|_| Job::Scan);
        }
        submit(|source| Job::Import(source.to_owned()));
        worker.cancel_imports();
        for _ in 0..3 {
            assert!(matches!(wait(), Reply::Images(Ok(_))));
        }
        assert!(matches!(wait(), Reply::Imported(Err(Error::Cancelled))));
        assert!(entries(&images_dir(&profile)).is_empty());
        submit(|source| Job::Import(source.to_owned()));
        assert!(matches!(wait(), Reply::Imported(Ok(name)) if name == "a.png"));
    }

    #[test]
    fn relink_restores_only_the_pinned_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let images = images_dir(&profile);
        let never = AtomicBool::new(false);
        let (original, other) = (png(4, 2), png(2, 4));
        let image = ImageRef {
            name: "Cross.png".into(),
            sha256: Sha256::digest(&original).into(),
        };
        // The file was moved out of the profile and renamed.
        let moved = dir.path().join("backup").join("cross copy.png");
        fs::create_dir_all(moved.parent().unwrap()).unwrap();
        fs::write(&moved, &original).unwrap();
        let unrelated = dir.path().join("unrelated.png");
        fs::write(&unrelated, &other).unwrap();

        assert_eq!(
            relink(&profile, &image, &unrelated, &never),
            Err(Error::Mismatch)
        );
        assert!(entries(&images).is_empty());
        assert_eq!(
            relink(&profile, &image, &dir.path().join("gone.png"), &never),
            Err(Error::SourceMissing)
        );
        assert_eq!(
            relink(&profile, &image, &moved, &AtomicBool::new(true)),
            Err(Error::Cancelled)
        );
        assert!(entries(&images).is_empty());
        let unsafe_name = ImageRef {
            name: "../Cross.png".into(),
            ..image.clone()
        };
        assert_eq!(
            relink(&profile, &unsafe_name, &moved, &never),
            Err(Error::InvalidName)
        );

        assert_eq!(
            relink(&profile, &image, &moved, &never),
            Ok(Relinked::Restored)
        );
        assert_eq!(fs::read(images.join("Cross.png")).unwrap(), original);
        assert!(decode_background(&profile, &image).is_ok());
        assert_eq!(
            relink(&profile, &image, &moved, &never),
            Ok(Relinked::AlreadyPresent)
        );
        // A changed file under the pinned name is another image: kept as is.
        fs::write(images.join("Cross.png"), &other).unwrap();
        assert_eq!(
            relink(&profile, &image, &moved, &never),
            Err(Error::NameTaken)
        );
        assert_eq!(fs::read(images.join("Cross.png")).unwrap(), other);
        assert_eq!(entries(&images), ["Cross.png"]);
    }

    #[test]
    fn io_failures_name_the_side_and_the_recovery() {
        use io::ErrorKind as Kind;
        let at = |kind: Kind, side| io_at(io::Error::from(kind), side);
        assert_eq!(at(Kind::PermissionDenied, Side::Source), Error::ReadDenied);
        assert_eq!(
            at(Kind::PermissionDenied, Side::Profile),
            Error::ProfileDenied
        );
        assert_eq!(
            at(Kind::ReadOnlyFilesystem, Side::Profile),
            Error::ProfileDenied
        );
        assert_eq!(at(Kind::StorageFull, Side::Profile), Error::DiskFull);
        assert_eq!(at(Kind::NotFound, Side::Source), Error::SourceMissing);
        assert_eq!(at(Kind::NotFound, Side::Profile), Error::Missing);
        assert_eq!(at(Kind::TimedOut, Side::Profile), Error::Io(Kind::TimedOut));
        assert!(Error::ProfileDenied.to_string().contains("not read-only"));
        assert!(Error::ReadDenied.to_string().contains("import the copy"));
        assert!(Error::Io(Kind::TimedOut).to_string().contains("timed out"));
        assert!(Error::Mismatch.to_string().contains("nothing was changed"));
    }

    #[cfg(unix)]
    #[test]
    fn permission_denied_is_reported_without_partial_files() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let images = images_dir(&profile);
        fs::create_dir_all(&images).unwrap();
        let source = dir.path().join("a.png");
        fs::write(&source, png(2, 2)).unwrap();
        let mode = |path: &Path, mode| fs::set_permissions(path, fs::Permissions::from_mode(mode));
        mode(&images, 0o555).unwrap();
        if File::create(images.join("probe")).is_ok() {
            let _ = fs::remove_file(images.join("probe"));
            mode(&images, 0o755).unwrap();
            eprintln!("skipped: this user bypasses file permissions");
            return;
        }
        assert_eq!(import(&profile, &source), Err(Error::ProfileDenied));
        mode(&images, 0o755).unwrap();
        assert!(entries(&images).is_empty());
        mode(&source, 0o000).unwrap();
        assert_eq!(import(&profile, &source), Err(Error::ReadDenied));
        mode(&source, 0o644).unwrap();
        assert_eq!(import(&profile, &source).unwrap(), "a.png");
    }

    /// Windows cannot make a folder unwritable without editing its ACL, so
    /// this test adds and removes deny entries with `icacls`. Run it with
    /// `cargo test --lib images::tests::permission -- --ignored`.
    #[cfg(windows)]
    #[test]
    #[ignore = "edits ACLs with icacls"]
    fn permission_denied_is_reported_without_partial_files_on_windows() {
        use std::process::{Command, Stdio};
        const EVERYONE: &str = "*S-1-1-0";
        struct Deny(PathBuf);
        impl Drop for Deny {
            fn drop(&mut self) {
                let _ = Command::new("icacls")
                    .arg(&self.0)
                    .args(["/remove:d", EVERYONE])
                    .stdout(Stdio::null())
                    .status();
            }
        }
        let deny = |path: &Path, rights: &str| {
            let status = Command::new("icacls")
                .arg(path)
                .args(["/deny", &format!("{EVERYONE}:{rights}")])
                .stdout(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "icacls failed");
            Deny(path.to_owned())
        };
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let images = images_dir(&profile);
        fs::create_dir_all(&images).unwrap();
        let source = dir.path().join("a.png");
        fs::write(&source, png(2, 2)).unwrap();
        {
            let _deny = deny(&images, "(W)");
            assert_eq!(import(&profile, &source), Err(Error::ProfileDenied));
        }
        assert!(entries(&images).is_empty());
        {
            let _deny = deny(&source, "(R)");
            assert_eq!(import(&profile, &source), Err(Error::ReadDenied));
        }
        assert_eq!(import(&profile, &source).unwrap(), "a.png");
    }
}
