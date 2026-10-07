//! Profile image resources and the logo choice. No GPUI types.
//!
//! Images live in `<profile>/Resources/Images/`; the logo choice is the image's
//! file name in `<profile>/logo.txt`. All file work runs on the `Worker` thread.
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
        atomic::AtomicBool,
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
};

pub const IMAGES: &str = "Resources/Images";
pub const LOGO_FILE: &str = "logo.txt";
const MAX_NAME: usize = 255;
const MAX_IMAGES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Io(io::ErrorKind),
    Missing,
    Unsupported,
    TooLarge,
    InvalidName,
    Busy,
    Disconnected,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Io(_) => "Image file cannot be read or written; check permissions",
            Self::Missing => "Image file is missing from the profile",
            Self::Unsupported => "Only PNG and JPEG images can be imported",
            Self::TooLarge => "Image file is larger than 8 MiB",
            Self::InvalidName => "Image file name is not usable",
            Self::Busy => "Image work is still running; try again",
            Self::Disconnected => "Image worker stopped; restart Sela",
        })
    }
}
impl std::error::Error for Error {}

fn io_error(error: io::Error) -> Error {
    if error.kind() == io::ErrorKind::NotFound {
        Error::Missing
    } else {
        Error::Io(error.kind())
    }
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
    let metadata = fs::metadata(path).map_err(io_error)?;
    if !metadata.is_file() {
        return Err(Error::Io(io::ErrorKind::InvalidInput));
    }
    if metadata.len() > MAX_SOURCE_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(io_error)?
        .take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > MAX_SOURCE_BYTES {
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

/// Copies a PNG/JPEG into the profile without replacing an existing image;
/// a clashing name gets a " (2)" style suffix. Returns the stored name.
pub fn import(profile: &Path, source: &Path) -> Result<String, Error> {
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
    let bytes = read_bounded(source)?;
    check_image(&bytes)?;
    let dir = images_dir(profile);
    fs::create_dir_all(&dir).map_err(io_error)?;
    for attempt in 1..=1000 {
        let candidate = if attempt == 1 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem} ({attempt}).{ext}")
        };
        if !valid_name(&candidate) {
            return Err(Error::InvalidName);
        }
        let path = dir.join(&candidate);
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(error)),
        };
        if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(io_error(error));
        }
        return Ok(candidate);
    }
    Err(Error::InvalidName)
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
    max_texture_dimension: 16384,
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
}

impl Worker {
    pub fn start(profile: PathBuf) -> io::Result<Self> {
        let (jobs, incoming) = mpsc::sync_channel::<Job>(2);
        let (outgoing, replies) = mpsc::sync_channel(2);
        std::thread::Builder::new()
            .name("sela-images".into())
            .spawn(move || {
                while let Ok(job) = incoming.recv() {
                    let reply = match job {
                        Job::Scan => Reply::Images(list(&profile)),
                        Job::Import(source) => Reply::Imported(import(&profile, &source)),
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
        Ok(Self { jobs, replies })
    }

    pub fn submit(&self, job: Job) -> Result<(), Error> {
        self.jobs.try_send(job).map_err(|error| match error {
            TrySendError::Full(_) => Error::Busy,
            TrySendError::Disconnected(_) => Error::Disconnected,
        })
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
            Err(Error::Missing)
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
}
