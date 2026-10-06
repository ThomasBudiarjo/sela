//! Profile image resources and the logo choice. No GPUI types.
//!
//! Images live in `<profile>/Resources/Images/`; the logo choice is the image's
//! file name in `<profile>/logo.txt`. All file work runs on the `Worker` thread.
use crate::scene::{
    BackgroundSpec, ContentVersion, Extent, MAX_SOURCE_BYTES, ResourceRef, SceneSpec,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
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

pub enum Job {
    Scan,
    Import(PathBuf),
    /// Validate, hash and persist an image as the logo.
    UseAsLogo(String),
    /// Read `logo.txt` and hash the image it names.
    LoadLogo,
}

pub enum Reply {
    Images(Result<Vec<String>, Error>),
    Imported(Result<String, Error>),
    Logo(Result<Option<Logo>, Error>),
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
