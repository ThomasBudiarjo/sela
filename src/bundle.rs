//! Portable bundles (M1-13): standalone schedules and whole-profile backups.
//! No GPUI types. Format, limits and recovery steps: `docs/bundle.md`.
//!
//! - A **standalone schedule** (`.selapackage`, File › Export Standalone
//!   Schedule…, EW8-OBS-053) is the `.selaschedule` document with its song
//!   and theme snapshots plus, optionally, the profile images it uses.
//!   Fonts are listed, never embedded; scripture items whose translation may
//!   not be shared are left out and listed. Opening one (File › Open
//!   Schedule…) stores its images in the profile without replacing any
//!   other image and writes the schedule into `<profile>/Schedules/`; the
//!   normal open and preflight then report what this machine lacks.
//! - A **profile backup** (`.selabackup`, Profiles › Back Up Profile…) holds
//!   a verified copy of `library.sqlite`, the settings (recent schedules made
//!   profile-relative), the logo choice, profile images, saved schedules
//!   and imported translations. Restore (Profiles › Restore Profile…) builds
//!   a new profile folder next to the chosen place and renames it into
//!   place only after every file was verified; it never writes into an
//!   existing folder.
//!
//! Every bundle is a strict ZIP ([`archive`]) with a versioned manifest
//! ([`manifest`]). Reading validates the whole archive and every checksum
//! before anything in a profile changes. All functions block: run them on
//! [`Worker`].
pub mod archive;
pub mod manifest;

use crate::{
    background::{Background, Fill, ImageRef},
    fonts::Fonts,
    format::SlideFormat,
    images::{self, Substitute},
    schedule_file::{self, ReadError, ScheduleFile},
    settings,
    storage::{self, Repository},
};
use archive::{Archive, ArchiveError, Limits, Writer};
use manifest::{FileRecord, FontRecord, ImageRecord, Kind, Manifest, ManifestError, Omitted, Role};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
};

/// Standalone schedule files.
pub const SCHEDULE_EXTENSION: &str = "selapackage";
/// Profile backup files.
pub const BACKUP_EXTENSION: &str = "selabackup";
/// Images one standalone schedule may carry.
pub const MAX_PACKAGE_IMAGES: usize = 512;
/// Largest library copy in a backup (the storage backup bound).
pub const MAX_LIBRARY_BYTES: u64 = 256 * 1024 * 1024;
/// Largest imported translation file in a backup.
pub const MAX_SCRIPTURE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_LOGO_BYTES: u64 = 4096;
/// Files a backup walks through in `Schedules/` before it stops looking.
const MAX_WALK: usize = 10_000;
/// Folder depth below `Schedules/` that is backed up.
const MAX_SCHEDULE_DEPTH: usize = 4;
/// Names listed in a summary's skipped list; the rest are only counted.
pub const MAX_LISTED: usize = 50;

pub const SCHEDULE_LIMITS: Limits = Limits {
    entries: 2 + MAX_PACKAGE_IMAGES,
    entry_bytes: images::MAX_FILE_BYTES as u64,
    total_bytes: 1024 * 1024 * 1024,
};

pub const PROFILE_LIMITS: Limits = Limits {
    entries: 16_384,
    entry_bytes: MAX_LIBRARY_BYTES,
    total_bytes: 3 * 1024 * 1024 * 1024,
};

/// Largest size an entry of `role` may declare.
fn role_limit(role: Role) -> u64 {
    match role {
        Role::Schedule => schedule_file::MAX_FILE_BYTES as u64,
        Role::Image => images::MAX_FILE_BYTES as u64,
        Role::Library => MAX_LIBRARY_BYTES,
        Role::Settings => settings::MAX_FILE_BYTES,
        Role::Logo => MAX_LOGO_BYTES,
        Role::Scripture => MAX_SCRIPTURE_BYTES,
    }
}

/// Why a bundle could not be written or read. Nothing in the profile (or at
/// the restore destination) has changed when one is returned, except where
/// a variant says otherwise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a Sela bundle: not a ZIP archive, or no manifest.
    NotBundle,
    /// A bundle of the other kind (a backup opened as a schedule, ...).
    WrongKind(Kind),
    /// Written by a newer Sela: its manifest version.
    NewerFormat(u64),
    /// The backup's library schema is newer than this Sela opens.
    NewerLibrary(i64),
    /// The schedule inside was written by a newer Sela.
    NewerSchedule(u64),
    /// Refused because it could write outside its place or overwrite other
    /// files: unsafe or absolute path, `..`, link, duplicate or conflicting
    /// names, encryption, ZIP64. Says what.
    Unsafe(String),
    /// Damaged or inconsistent: checksum mismatch, truncated data, a
    /// malformed manifest or one that disagrees with the archive.
    Damaged(String),
    /// Larger than Sela accepts (an entry, the whole bundle, or a file that
    /// inflates past its declared size). Names the entry when there is one.
    TooLarge(String),
    /// Restore never writes into an existing file or folder.
    DestinationExists,
    /// The profile's library could not be copied or the backup's library is
    /// not a usable Sela library.
    Library(storage::Error),
    /// An embedded image is unusable (checked before anything is stored), or
    /// could not be stored; images stored before it stay.
    Image(String, images::Error),
    /// The schedule has no items to export.
    Empty,
    DiskFull,
    PermissionDenied,
    Io(io::ErrorKind),
    Cancelled,
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::StorageFull => Self::DiskFull,
            io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
                Self::PermissionDenied
            }
            kind => Self::Io(kind),
        }
    }
}

impl From<ArchiveError> for Error {
    fn from(error: ArchiveError) -> Self {
        match error {
            ArchiveError::Io(kind) => io::Error::from(kind).into(),
            ArchiveError::NotArchive => Self::NotBundle,
            ArchiveError::Malformed(what) => Self::Damaged(what.into()),
            ArchiveError::Unsupported(what) => Self::Unsafe(what.into()),
            ArchiveError::UnsafeName(name) => Self::Unsafe(format!("unsafe file name “{name}”")),
            ArchiveError::DuplicateName(name) => {
                Self::Unsafe(format!("two files named “{name}” (ignoring case)"))
            }
            ArchiveError::TooManyEntries => Self::TooLarge("too many files".into()),
            ArchiveError::TooLarge(name) => Self::TooLarge(name),
            ArchiveError::Checksum(name) => Self::Damaged(format!("“{name}” fails its checksum")),
            ArchiveError::Cancelled => Self::Cancelled,
        }
    }
}

impl From<ManifestError> for Error {
    fn from(error: ManifestError) -> Self {
        match error {
            ManifestError::Newer(version) => Self::NewerFormat(version),
            ManifestError::Malformed(what) => Self::Damaged(format!("manifest: {what}")),
            ManifestError::Path(path) => Self::Unsafe(format!("manifest path “{path}”")),
            ManifestError::Duplicate(path) => {
                Self::Unsafe(format!("manifest lists “{path}” twice"))
            }
        }
    }
}

fn library_error(error: storage::Error) -> Error {
    match error {
        storage::Error::Canceled => Error::Cancelled,
        error => Error::Library(error),
    }
}

fn generator() -> String {
    format!("Sela {}", env!("CARGO_PKG_VERSION"))
}

/// `path` with `extension`, added only when missing.
pub fn with_extension(path: &Path, extension: &str) -> PathBuf {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(extension))
    {
        return path.to_path_buf();
    }
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".");
    name.push(extension);
    path.with_file_name(name)
}

/// A file the Open dialog should hand to [`import_schedule`] instead of the
/// schedule reader: either bundle extension (a backup is refused there with
/// `WrongKind`, which tells the operator where it belongs).
pub fn is_bundle(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case(SCHEDULE_EXTENSION) || e.eq_ignore_ascii_case(BACKUP_EXTENSION)
    })
}

// ---------------------------------------------------------------------------
// Small file helpers.

fn parent_of(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn nonce() -> String {
    use std::sync::atomic::AtomicU64;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!(
        "{}-{:x}-{}",
        std::process::id(),
        time,
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

fn private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Opens a profile file for reading without following a symbolic link at
/// its own name, and only if it is a regular file of at most `max` bytes.
fn open_regular(path: &Path, max: u64) -> Result<File, Skip> {
    let metadata = fs::symlink_metadata(path).map_err(|_| Skip::Unreadable)?;
    if metadata.file_type().is_symlink() {
        return Err(Skip::Link);
    }
    if !metadata.is_file() {
        return Err(Skip::Unusable);
    }
    if metadata.len() > max {
        return Err(Skip::TooLarge);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|_| Skip::Unreadable)?;
    match file.metadata() {
        Ok(m) if m.is_file() && m.len() <= max => Ok(file),
        Ok(m) if m.is_file() => Err(Skip::TooLarge),
        _ => Err(Skip::Unusable),
    }
}

/// A temporary file beside the destination; removed unless published.
struct TempFile {
    path: PathBuf,
    published: bool,
}

impl TempFile {
    fn create(destination: &Path, prefix: &str) -> Result<(Self, File), Error> {
        let path = parent_of(destination).join(format!("{prefix}{}.partial", nonce()));
        let file = private_file(&path)?;
        Ok((
            Self {
                path,
                published: false,
            },
            file,
        ))
    }

    /// Atomically replaces `destination` (the Save dialog already asked
    /// before an existing file is replaced).
    fn publish(mut self, destination: &Path) -> Result<(), Error> {
        fs::rename(&self.path, destination)?;
        self.published = true;
        Ok(())
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// A private working folder, removed with everything in it unless kept.
struct Staging {
    path: PathBuf,
    keep: bool,
}

impl Staging {
    fn create(parent: &Path, prefix: &str) -> Result<Self, Error> {
        let path = parent.join(format!("{prefix}{}", nonce()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path)?;
        Ok(Self { path, keep: false })
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// `root` joined with a validated `/`-separated bundle path.
fn under(root: &Path, path: &str) -> PathBuf {
    path.split('/')
        .fold(root.to_path_buf(), |dir, part| dir.join(part))
}

fn cancelled(cancel: &AtomicBool) -> impl Fn() -> bool + '_ {
    move || cancel.load(Ordering::Acquire)
}

// ---------------------------------------------------------------------------
// Reading any bundle.

/// An opened bundle whose manifest agrees with its archive.
struct Opened {
    archive: Archive,
    manifest: Manifest,
    /// Archive entry index of each manifest file record, in record order.
    indexes: Vec<usize>,
}

fn open_bundle(path: &Path, limits: Limits, cancel: &AtomicBool) -> Result<Opened, Error> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::NotBundle);
    }
    let mut archive = Archive::open(file, limits)?;
    let index = archive.find(manifest::NAME).ok_or(Error::NotBundle)?;
    if archive.entries()[index].size > manifest::MAX_BYTES as u64 {
        return Err(Error::TooLarge(manifest::NAME.into()));
    }
    let (bytes, _) = archive.read(index, &cancelled(cancel))?;
    let manifest = manifest::decode(&bytes)?;
    // The manifest lists every other entry exactly, with its size, and each
    // size is within its role's bound.
    let mut indexes = Vec::with_capacity(manifest.files.len());
    for record in &manifest.files {
        let found = archive
            .find(&record.path)
            .ok_or_else(|| Error::Damaged(format!("“{}” is listed but missing", record.path)))?;
        let entry = &archive.entries()[found];
        if entry.size != record.size {
            return Err(Error::Damaged(format!(
                "“{}” is not the size the manifest says",
                record.path
            )));
        }
        if record.size > role_limit(record.role) {
            return Err(Error::TooLarge(record.path.clone()));
        }
        indexes.push(found);
    }
    if archive.entries().len() != manifest.files.len() + 1 {
        let extra = archive
            .entries()
            .iter()
            .find(|e| e.name != manifest::NAME && !manifest.files.iter().any(|f| f.path == e.name))
            .map(|e| e.name.clone())
            .unwrap_or_default();
        return Err(Error::Damaged(format!("“{extra}” is not in the manifest")));
    }
    Ok(Opened {
        archive,
        manifest,
        indexes,
    })
}

impl Opened {
    /// The entry's bytes, checked against the manifest's SHA-256.
    fn read(&mut self, record: usize, cancel: &AtomicBool) -> Result<Vec<u8>, Error> {
        let (bytes, digest) = self
            .archive
            .read(self.indexes[record], &cancelled(cancel))?;
        self.check(record, digest.sha256)?;
        Ok(bytes)
    }

    fn extract_to(
        &mut self,
        record: usize,
        out: &mut dyn Write,
        cancel: &AtomicBool,
    ) -> Result<(), Error> {
        let digest = self
            .archive
            .extract(self.indexes[record], out, &cancelled(cancel))?;
        self.check(record, digest.sha256)
    }

    fn check(&self, record: usize, sha256: [u8; 32]) -> Result<(), Error> {
        let file = &self.manifest.files[record];
        if sha256 != file.sha256 {
            return Err(Error::Damaged(format!(
                "“{}” does not match its SHA-256",
                file.path
            )));
        }
        Ok(())
    }
}

/// What a bundle says about itself, without extracting anything else.
pub fn inspect(path: &Path) -> Result<Manifest, Error> {
    open_bundle(path, PROFILE_LIMITS, &AtomicBool::new(false)).map(|opened| opened.manifest)
}

// ---------------------------------------------------------------------------
// Standalone schedules.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportOptions {
    /// Carry the profile images the schedule uses (the default). Without
    /// them the bundle lists the images, and the receiving profile needs its
    /// own copies (preflight reports them, Locate… repairs).
    pub embed_images: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self { embed_images: true }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportSummary {
    /// Items written, songs and scripture together.
    pub items: usize,
    pub embedded: Vec<String>,
    /// Images the schedule uses that were not included: missing, changed
    /// since they were chosen, or unreadable in this profile.
    pub not_embedded: Vec<(String, Substitute)>,
    /// Images left out because `embed_images` was off.
    pub listed_only: Vec<String>,
    /// Named fonts the receiving machine needs installed.
    pub fonts: Vec<FontRecord>,
    pub omitted: Vec<Omitted>,
    /// Size of the written file.
    pub bytes: u64,
}

/// Every background of the file's songs and theme snapshots.
fn backgrounds_mut(file: &mut ScheduleFile) -> impl Iterator<Item = &mut Background> {
    file.items.iter_mut().flat_map(|item| {
        let song = item.song.master.iter_mut().chain(
            item.song
                .sections
                .iter_mut()
                .filter_map(|s| s.background.as_mut()),
        );
        song.chain(
            item.theme
                .iter_mut()
                .filter_map(|theme| theme.theme.background.as_mut()),
        )
    })
}

fn backgrounds(file: &ScheduleFile) -> Vec<&Background> {
    file.items
        .iter()
        .flat_map(|item| {
            item.song
                .master
                .iter()
                .chain(
                    item.song
                        .sections
                        .iter()
                        .filter_map(|s| s.background.as_ref()),
                )
                .chain(
                    item.theme
                        .iter()
                        .filter_map(|theme| theme.theme.background.as_ref()),
                )
        })
        .collect()
}

/// Distinct pinned images in first-use order.
pub fn image_refs(file: &ScheduleFile) -> Vec<ImageRef> {
    let mut seen = HashSet::new();
    backgrounds(file)
        .into_iter()
        .filter_map(Background::image_ref)
        .filter(|image| seen.insert((image.name.clone(), image.sha256)))
        .cloned()
        .collect()
}

/// Distinct named font families (ignoring case) in first-use order.
pub fn font_families(file: &ScheduleFile) -> Vec<String> {
    let mut seen = HashSet::new();
    let formats = file.items.iter().flat_map(|item| {
        item.song
            .sections
            .iter()
            .map(|s| &s.format)
            .chain(item.theme.iter().map(|t| &t.theme.format))
    });
    formats
        .filter_map(|format| format.font.as_deref().map(str::trim))
        .filter(|family| !family.is_empty())
        .filter(|family| seen.insert(family.to_lowercase()))
        .map(str::to_owned)
        .collect()
}

/// The file without scripture items whose translation may not be shared;
/// later scripture positions move up so the order is kept.
fn without_restricted(file: &ScheduleFile) -> (ScheduleFile, Vec<Omitted>) {
    let mut out = ScheduleFile {
        title: file.title.clone(),
        items: file.items.clone(),
        scripture: Vec::new(),
    };
    let mut omitted = Vec::new();
    for item in &file.scripture {
        if item.snapshot.redistributable {
            let mut kept = item.clone();
            kept.position -= omitted.len();
            out.scripture.push(kept);
        } else {
            omitted.push(Omitted {
                title: item.title.clone(),
                translation: item.snapshot.translation.clone(),
            });
        }
    }
    (out, omitted)
}

fn installed(fonts: &Fonts, family: &str) -> Option<bool> {
    fonts.catalog()?;
    let format = SlideFormat {
        font: Some(family.to_owned()),
        ..SlideFormat::default()
    };
    Some(fonts.resolve(&format).warning().is_none())
}

/// Writes `file` as a standalone schedule at `destination` (replacing an
/// existing file only once the new one is complete). Blocking.
pub fn export_schedule(
    profile: &Path,
    file: &ScheduleFile,
    destination: &Path,
    fonts: &Fonts,
    options: ExportOptions,
    cancel: &AtomicBool,
) -> Result<ExportSummary, Error> {
    let (file, omitted) = without_restricted(file);
    if file.items.is_empty() && file.scripture.is_empty() {
        return Err(Error::Empty);
    }
    let document = schedule_file::encode(&file).map_err(|error| match error {
        schedule_file::WriteError::TooLarge => Error::TooLarge(manifest::SCHEDULE_PATH.into()),
        _ => Error::Damaged("the schedule cannot be written".into()),
    })?;
    let refs = image_refs(&file);
    if refs.len() > MAX_PACKAGE_IMAGES {
        return Err(Error::TooLarge("too many images".into()));
    }
    let font_records: Vec<FontRecord> = font_families(&file)
        .into_iter()
        .map(|family| FontRecord {
            installed: installed(fonts, &family),
            family,
        })
        .collect();
    let (temp, out) = TempFile::create(destination, ".sela-export-")?;
    let mut writer = Writer::new(out);
    let mut files = Vec::new();
    let digest = writer.add_bytes(manifest::SCHEDULE_PATH, true, &document)?;
    files.push(FileRecord {
        path: manifest::SCHEDULE_PATH.into(),
        size: digest.size,
        sha256: digest.sha256,
        role: Role::Schedule,
    });
    let mut summary = ExportSummary {
        items: file.items.len() + file.scripture.len(),
        fonts: font_records.clone(),
        omitted: omitted.clone(),
        ..ExportSummary::default()
    };
    let mut records = Vec::new();
    let mut embedded_names = HashSet::new();
    for image in refs {
        if cancel.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        let mut embedded = false;
        if !options.embed_images {
            summary.listed_only.push(image.name.clone());
        } else if !embedded_names.contains(&image.name.to_lowercase()) {
            match images::content_hash(profile, &image.name) {
                Ok(hash) if hash == image.sha256 => {
                    let path = images::images_dir(profile).join(&image.name);
                    let mut source = open_regular(&path, images::MAX_FILE_BYTES as u64)
                        .map_err(|_| Error::Image(image.name.clone(), images::Error::Missing))?;
                    let digest = writer.add(
                        &manifest::image_path(&image.name),
                        false,
                        &mut source,
                        images::MAX_FILE_BYTES as u64,
                        &cancelled(cancel),
                    )?;
                    // The file changed between the check and the copy.
                    if digest.sha256 != image.sha256 {
                        return Err(Error::Image(image.name.clone(), images::Error::Mismatch));
                    }
                    files.push(FileRecord {
                        path: manifest::image_path(&image.name),
                        size: digest.size,
                        sha256: digest.sha256,
                        role: Role::Image,
                    });
                    embedded_names.insert(image.name.to_lowercase());
                    summary.embedded.push(image.name.clone());
                    embedded = true;
                }
                Ok(_) => summary
                    .not_embedded
                    .push((image.name.clone(), Substitute::Changed)),
                Err(problem) => summary.not_embedded.push((image.name.clone(), problem)),
            }
        } else {
            // Same name, other bytes than the copy already embedded.
            summary
                .not_embedded
                .push((image.name.clone(), Substitute::Changed));
        }
        records.push(ImageRecord {
            name: image.name,
            sha256: image.sha256,
            embedded,
        });
    }
    let manifest = Manifest {
        kind: Kind::Schedule,
        generator: generator(),
        title: file.title.clone(),
        files,
        images: records,
        fonts: font_records,
        omitted,
        library_schema: None,
    };
    writer.add_bytes(manifest::NAME, true, &manifest::encode(&manifest))?;
    let out = writer.finish()?;
    summary.bytes = out.metadata()?.len();
    drop(out);
    temp.publish(destination)?;
    Ok(summary)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    /// The schedule file written into `<profile>/Schedules/`.
    pub schedule: PathBuf,
    pub title: String,
    pub items: usize,
    /// Images copied into the profile under their own name.
    pub added: Vec<String>,
    /// Images the profile already had, byte for byte.
    pub reused: Vec<String>,
    /// (bundle name, stored name): a different image already used the name.
    pub renamed: Vec<(String, String)>,
    /// Images the schedule uses that the bundle does not carry.
    pub not_included: Vec<String>,
    pub fonts: Vec<FontRecord>,
    pub omitted: Vec<Omitted>,
}

/// A file name for a schedule title: the characters every system accepts.
fn file_stem(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .take(80)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_owned();
    if cleaned.is_empty() || !archive::safe_name(&format!("{cleaned}.selaschedule")) {
        "Schedule".into()
    } else {
        cleaned
    }
}

/// Writes `bytes` as `<dir>/<stem>.selaschedule`, or "<stem> (n)" when that
/// name is taken; never replaces a file.
fn write_new_schedule(dir: &Path, stem: &str, bytes: &[u8]) -> Result<PathBuf, Error> {
    fs::create_dir_all(dir)?;
    let temp = dir.join(format!(".sela-import-{}.partial", nonce()));
    let mut out = private_file(&temp)?;
    let result = (|| -> Result<PathBuf, Error> {
        out.write_all(bytes)?;
        out.sync_all()?;
        for attempt in 1..=1000u32 {
            let name = if attempt == 1 {
                format!("{stem}.{}", schedule_file::EXTENSION)
            } else {
                format!("{stem} ({attempt}).{}", schedule_file::EXTENSION)
            };
            let path = dir.join(name);
            match fs::hard_link(&temp, &path) {
                Ok(()) => return Ok(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(_) => {
                    // No hard links (FAT, some shares): reserve, then rename.
                    match private_file(&path) {
                        Ok(_) => {
                            fs::rename(&temp, &path).inspect_err(|_| {
                                let _ = fs::remove_file(&path);
                            })?;
                            return Ok(path);
                        }
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                        Err(error) => return Err(error.into()),
                    }
                }
            }
        }
        Err(Error::Io(io::ErrorKind::AlreadyExists))
    })();
    let _ = fs::remove_file(&temp);
    result
}

/// Opens a standalone schedule into `profile`: validates the whole bundle
/// first, then stores its images (never replacing another image) and
/// writes its schedule as a new file in `<profile>/Schedules/`. Blocking.
pub fn import_schedule(
    profile: &Path,
    package: &Path,
    cancel: &AtomicBool,
) -> Result<ImportSummary, Error> {
    let mut opened = open_bundle(package, SCHEDULE_LIMITS, cancel)?;
    if opened.manifest.kind != Kind::Schedule {
        return Err(Error::WrongKind(opened.manifest.kind));
    }
    let schedule_record = opened
        .manifest
        .files
        .iter()
        .position(|f| f.role == Role::Schedule)
        .expect("the manifest decoder requires one");
    let document = opened.read(schedule_record, cancel)?;
    let mut file = schedule_file::decode(&document).map_err(|error| match error {
        ReadError::Newer(version) => Error::NewerSchedule(version),
        ReadError::TooLarge => Error::TooLarge(manifest::SCHEDULE_PATH.into()),
        _ => Error::Damaged("the schedule inside is damaged".into()),
    })?;
    let images: Vec<usize> = (0..opened.manifest.files.len())
        .filter(|&i| opened.manifest.files[i].role == Role::Image)
        .collect();
    // Validation before mutation: every embedded image reads back with its
    // checksum and passes the image header checks.
    for &record in &images {
        let bytes = opened.read(record, cancel)?;
        let name = image_name(&opened.manifest.files[record]);
        images::check_embedded(&bytes).map_err(|error| Error::Image(name, error))?;
    }
    if cancel.load(Ordering::Acquire) {
        return Err(Error::Cancelled);
    }
    let mut summary = ImportSummary {
        title: file.title.clone(),
        items: file.items.len() + file.scripture.len(),
        fonts: opened.manifest.fonts.clone(),
        omitted: opened.manifest.omitted.clone(),
        ..ImportSummary::default()
    };
    let mut renames: HashMap<(String, [u8; 32]), String> = HashMap::new();
    for &record in &images {
        let bytes = opened.read(record, cancel)?;
        let name = image_name(&opened.manifest.files[record]);
        let adopted =
            images::adopt(profile, &name, &bytes, cancel).map_err(|error| match error {
                images::Error::Cancelled => Error::Cancelled,
                error => Error::Image(name.clone(), error),
            })?;
        match (adopted.reused, adopted.name == name) {
            (true, _) => summary.reused.push(name.clone()),
            (false, true) => summary.added.push(name.clone()),
            (false, false) => {}
        }
        if adopted.name != name {
            summary.renamed.push((name.clone(), adopted.name.clone()));
            renames.insert((name, opened.manifest.files[record].sha256), adopted.name);
        }
    }
    for background in backgrounds_mut(&mut file) {
        if let Fill::Media(Some(image)) = &mut background.fill
            && let Some(stored) = renames.get(&(image.name.clone(), image.sha256))
        {
            image.name = stored.clone();
        }
    }
    let carried: HashSet<(String, [u8; 32])> = opened
        .manifest
        .images
        .iter()
        .filter(|i| i.embedded)
        .map(|i| (i.name.clone(), i.sha256))
        .collect();
    summary.not_included = image_refs(&file)
        .into_iter()
        .filter(|image| {
            !carried.contains(&(image.name.clone(), image.sha256))
                && !renames.values().any(|stored| *stored == image.name)
        })
        .map(|image| image.name)
        .collect();
    let bytes = schedule_file::encode(&file)
        .map_err(|_| Error::Damaged("the schedule inside is damaged".into()))?;
    let stem = file_stem(if file.title.is_empty() {
        &opened.manifest.title
    } else {
        &file.title
    });
    summary.schedule = write_new_schedule(&profile.join("Schedules"), &stem, &bytes)?;
    Ok(summary)
}

fn image_name(record: &FileRecord) -> String {
    record
        .path
        .strip_prefix("images/")
        .unwrap_or(&record.path)
        .to_owned()
}

// ---------------------------------------------------------------------------
// Profile backups.

/// Why a profile file was not backed up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    /// A symbolic link: it could point outside the profile.
    Link,
    TooLarge,
    /// Not a regular file, or its name is unusable on another system.
    Unusable,
    Unreadable,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackupSummary {
    pub files: usize,
    /// Size of the written file.
    pub bytes: u64,
    /// Profile-relative paths left out and why (at most `MAX_LISTED`).
    pub skipped: Vec<(String, Skip)>,
    /// Further skipped paths beyond the listed ones.
    pub more_skipped: usize,
    /// Recent-schedule entries outside the profile, which a backup does not
    /// carry (they name another place on this machine).
    pub recent_dropped: usize,
    pub library_schema: i64,
}

impl BackupSummary {
    fn skip(&mut self, path: String, why: Skip) {
        if self.skipped.len() < MAX_LISTED {
            self.skipped.push((path, why));
        } else {
            self.more_skipped += 1;
        }
    }
}

/// `path` relative to `profile`, `/`-separated, if it is inside it and safe.
fn relative(profile: &Path, path: &Path) -> Option<String> {
    let rest = path.strip_prefix(profile).ok()?;
    let parts: Option<Vec<&str>> = rest
        .components()
        .map(|c| match c {
            std::path::Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    let joined = parts?.join("/");
    archive::safe_name(&joined).then_some(joined)
}

/// The settings with recent schedules inside the profile made relative and
/// the others left out (they name places on this machine only).
fn portable_settings(profile: &Path, text: &str) -> Option<(String, usize)> {
    let mut parsed = settings::parse(text).ok()?;
    let before = parsed.recent_schedules.len();
    parsed.recent_schedules = parsed
        .recent_schedules
        .iter()
        .filter_map(|path| relative(profile, path).map(PathBuf::from))
        .collect();
    let dropped = before - parsed.recent_schedules.len();
    Some((settings::render(&parsed), dropped))
}

/// Bundled translations are rebuilt by Sela itself; backups skip them.
fn bundled_translation(name: &str) -> bool {
    crate::scripture::bundled::shipped()
        .iter()
        .any(|bundle| name.eq_ignore_ascii_case(&format!("{}.selabible", bundle.id)))
}

/// Candidate files under `Schedules/`, depth-bounded, without following
/// links.
fn walk_schedules(profile: &Path, summary: &mut BackupSummary) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    let mut pending = vec![(profile.join("Schedules"), 0usize)];
    let mut seen = 0usize;
    while let Some((dir, depth)) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            seen += 1;
            if seen > MAX_WALK {
                summary.more_skipped += 1;
                continue;
            }
            let path = entry.path();
            let Some(rel) = relative(profile, &path) else {
                summary.skip(path.to_string_lossy().into_owned(), Skip::Unusable);
                continue;
            };
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                summary.skip(rel, Skip::Unreadable);
                continue;
            };
            if metadata.file_type().is_symlink() {
                summary.skip(rel, Skip::Link);
            } else if metadata.is_dir() {
                if depth < MAX_SCHEDULE_DEPTH {
                    pending.push((path, depth + 1));
                } else {
                    summary.skip(rel, Skip::Unusable);
                }
            } else if manifest::allowed(Kind::Profile, Role::Schedule, &rel) {
                found.push((rel, path));
            } else if !entry.file_name().to_string_lossy().starts_with('.') {
                summary.skip(rel, Skip::Unusable);
            }
        }
    }
    found.sort();
    found
}

/// The files of a profile backup, in archive order, with their roles.
fn profile_files(profile: &Path, summary: &mut BackupSummary) -> Vec<(String, PathBuf, Role)> {
    let mut files = Vec::new();
    if profile.join(images::LOGO_FILE).exists() {
        files.push((
            manifest::LOGO_PATH.to_owned(),
            profile.join(images::LOGO_FILE),
            Role::Logo,
        ));
    }
    let listed = |dir: &Path| -> Vec<(String, PathBuf)> {
        let mut entries: Vec<(String, PathBuf)> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
            .collect();
        entries.sort();
        entries
    };
    for (name, path) in listed(&images::images_dir(profile)) {
        let rel = format!("{}/{name}", images::IMAGES);
        if name.starts_with('.') {
            continue;
        }
        if !manifest::allowed(Kind::Profile, Role::Image, &rel) {
            summary.skip(rel, Skip::Unusable);
            continue;
        }
        files.push((rel, path, Role::Image));
    }
    for (rel, path) in walk_schedules(profile, summary) {
        files.push((rel, path, Role::Schedule));
    }
    for (name, path) in listed(&profile.join("Scripture")) {
        let rel = format!("Scripture/{name}");
        if name.starts_with('.') || bundled_translation(&name) {
            continue;
        }
        if !manifest::allowed(Kind::Profile, Role::Scripture, &rel) {
            summary.skip(rel, Skip::Unusable);
            continue;
        }
        files.push((rel, path, Role::Scripture));
    }
    files
}

/// Writes a backup of `profile` to `destination` (replacing an existing
/// file only once the new one is complete). The library is copied through
/// SQLite's backup API and verified first. Blocking.
pub fn backup_profile(
    profile: &Path,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<BackupSummary, Error> {
    let library = profile.join(manifest::LIBRARY_PATH);
    if !library.is_file() {
        return Err(Error::Library(storage::Error::Missing));
    }
    let staging = Staging::create(parent_of(destination), ".sela-backup-")?;
    let copy = staging.path.join(manifest::LIBRARY_PATH);
    Repository::restore_new(&library, &copy, cancel).map_err(library_error)?;
    let mut summary = BackupSummary {
        library_schema: storage::library_schema(&copy).map_err(library_error)?,
        ..BackupSummary::default()
    };
    let (temp, out) = TempFile::create(destination, ".sela-backup-")?;
    let mut writer = Writer::new(out);
    let mut records = Vec::new();
    let mut add = |writer: &mut Writer,
                   path: &str,
                   role: Role,
                   deflate: bool,
                   source: &mut dyn Read|
     -> Result<(), Error> {
        let digest = writer.add(path, deflate, source, role_limit(role), &cancelled(cancel))?;
        records.push(FileRecord {
            path: path.to_owned(),
            size: digest.size,
            sha256: digest.sha256,
            role,
        });
        Ok(())
    };
    let mut library_file = File::open(&copy)?;
    add(
        &mut writer,
        manifest::LIBRARY_PATH,
        Role::Library,
        true,
        &mut library_file,
    )?;
    let settings_path = settings::path_in(profile);
    if settings_path.exists() {
        let mut text = String::new();
        let read = open_regular(&settings_path, settings::MAX_FILE_BYTES)
            .and_then(|mut f| f.read_to_string(&mut text).map_err(|_| Skip::Unusable));
        match read.ok().and_then(|_| portable_settings(profile, &text)) {
            Some((portable, dropped)) => {
                summary.recent_dropped = dropped;
                add(
                    &mut writer,
                    manifest::SETTINGS_PATH,
                    Role::Settings,
                    true,
                    &mut portable.as_bytes(),
                )?;
            }
            None => summary.skip(settings::FILE_NAME.into(), Skip::Unusable),
        }
    }
    for (rel, path, role) in profile_files(profile, &mut summary) {
        if cancel.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        match open_regular(&path, role_limit(role)) {
            Ok(mut source) => {
                // PNG and JPEG are already compressed.
                let deflate = role != Role::Image;
                match add(&mut writer, &rel, role, deflate, &mut source) {
                    Ok(()) => {}
                    // Grown past its bound since the check.
                    Err(Error::TooLarge(_)) => return Err(Error::TooLarge(rel)),
                    Err(error) => return Err(error),
                }
            }
            Err(why) => summary.skip(rel, why),
        }
    }
    summary.files = records.len();
    let manifest = Manifest {
        kind: Kind::Profile,
        generator: generator(),
        title: profile
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_control())
            .take(manifest::MAX_TEXT / 4)
            .collect(),
        files: records,
        images: Vec::new(),
        fonts: Vec::new(),
        omitted: Vec::new(),
        library_schema: Some(summary.library_schema),
    };
    writer.add_bytes(manifest::NAME, true, &manifest::encode(&manifest))?;
    let out = writer.finish()?;
    summary.bytes = out.metadata()?.len();
    drop(out);
    temp.publish(destination)?;
    drop(staging);
    Ok(summary)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RestoreSummary {
    /// The new profile folder.
    pub destination: PathBuf,
    pub files: usize,
    pub library_schema: i64,
    /// The backup's settings could not be read; defaults apply.
    pub settings_skipped: bool,
}

/// The library file of a restored profile, for `--operator-library`.
pub fn library_in(profile: &Path) -> PathBuf {
    profile.join(manifest::LIBRARY_PATH)
}

/// The backup's settings with profile-relative recent schedules placed
/// under `destination` (absolute ones are dropped).
fn restored_settings(destination: &Path, text: &str) -> Option<String> {
    let mut parsed = settings::parse(text).ok()?;
    parsed.recent_schedules = parsed
        .recent_schedules
        .iter()
        .filter_map(|path| {
            let text = path.to_str()?.replace('\\', "/");
            archive::safe_name(&text).then(|| under(destination, &text))
        })
        .collect();
    Some(settings::render(&parsed))
}

/// Restores a backup as the new profile folder `destination`, which must not
/// exist. Everything is written and verified in a private folder beside it
/// first; only the final rename makes it appear. An interrupted restore
/// leaves at most that private `.sela-restore-*` folder behind. Blocking.
pub fn restore_profile(
    backup: &Path,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<RestoreSummary, Error> {
    match fs::symlink_metadata(destination) {
        Ok(_) => return Err(Error::DestinationExists),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = parent_of(destination);
    if !parent.is_dir() {
        return Err(Error::Io(io::ErrorKind::NotFound));
    }
    let mut opened = open_bundle(backup, PROFILE_LIMITS, cancel)?;
    if opened.manifest.kind != Kind::Profile {
        return Err(Error::WrongKind(opened.manifest.kind));
    }
    if let Some(schema) = opened.manifest.library_schema
        && schema > storage::newest_schema()
    {
        return Err(Error::NewerLibrary(schema));
    }
    let staging = Staging::create(parent, ".sela-restore-")?;
    let mut summary = RestoreSummary {
        destination: destination.to_path_buf(),
        ..RestoreSummary::default()
    };
    let mut order: Vec<usize> = (0..opened.manifest.files.len()).collect();
    // The library first: the most likely to be refused.
    order.sort_by_key(|&i| opened.manifest.files[i].role != Role::Library);
    for (done, record) in order.into_iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        #[cfg(test)]
        if std::env::var("SELA_ABORT_RESTORE_AFTER").ok() == Some(done.to_string()) {
            std::process::abort();
        }
        let _ = done;
        let file = opened.manifest.files[record].clone();
        match file.role {
            Role::Library => {
                let unverified = staging.path.join(".library.unverified");
                let mut out = private_file(&unverified)?;
                opened.extract_to(record, &mut out, cancel)?;
                out.sync_all()?;
                drop(out);
                let library = staging.path.join(manifest::LIBRARY_PATH);
                Repository::restore_new(&unverified, &library, cancel).map_err(library_error)?;
                fs::remove_file(&unverified)?;
            }
            Role::Settings => {
                let bytes = opened.read(record, cancel)?;
                let text = String::from_utf8(bytes).ok();
                match text.and_then(|t| restored_settings(destination, &t)) {
                    Some(text) => {
                        let mut out = private_file(&staging.path.join(manifest::SETTINGS_PATH))?;
                        out.write_all(text.as_bytes())?;
                        out.sync_all()?;
                    }
                    None => {
                        summary.settings_skipped = true;
                        continue;
                    }
                }
            }
            _ => {
                let target = under(&staging.path, &file.path);
                if let Some(dir) = target.parent() {
                    fs::create_dir_all(dir)?;
                }
                let mut out = private_file(&target)?;
                opened.extract_to(record, &mut out, cancel)?;
                out.sync_all()?;
            }
        }
        summary.files += 1;
    }
    let library = staging.path.join(manifest::LIBRARY_PATH);
    summary.library_schema = storage::library_schema(&library).map_err(library_error)?;
    if cancel.load(Ordering::Acquire) {
        return Err(Error::Cancelled);
    }
    // A rename never replaces a non-empty folder or a file (and on Windows
    // never replaces anything), so data created there meanwhile is kept.
    match fs::rename(&staging.path, destination) {
        Ok(()) => {
            let mut staging = staging;
            staging.keep = true;
            Ok(summary)
        }
        Err(_) if fs::symlink_metadata(destination).is_ok() => Err(Error::DestinationExists),
        Err(error) => Err(error.into()),
    }
}

// ---------------------------------------------------------------------------
// Worker.

pub enum Job {
    ExportSchedule {
        file: Box<ScheduleFile>,
        destination: PathBuf,
        options: ExportOptions,
    },
    ImportSchedule {
        package: PathBuf,
    },
    BackupProfile {
        destination: PathBuf,
    },
    RestoreProfile {
        backup: PathBuf,
        destination: PathBuf,
    },
}

#[derive(Debug)]
pub enum Done {
    Exported(PathBuf, Result<ExportSummary, Error>),
    Imported(PathBuf, Result<ImportSummary, Error>),
    BackedUp(PathBuf, Result<BackupSummary, Error>),
    Restored(PathBuf, Result<RestoreSummary, Error>),
}

/// One thread running one bundle job at a time for one profile. `submit`
/// never blocks: it refuses while a job is queued or running.
pub struct Worker {
    jobs: SyncSender<(Job, Arc<AtomicBool>)>,
    replies: Receiver<Done>,
    running: Arc<AtomicBool>,
    cancel: Option<Arc<AtomicBool>>,
}

impl Worker {
    pub fn start(profile: PathBuf) -> io::Result<Self> {
        Self::start_with(profile, crate::fonts::shared())
    }

    pub fn start_with(profile: PathBuf, fonts: &'static Fonts) -> io::Result<Self> {
        Self::start_gated(profile, fonts, None)
    }

    /// `start_with` whose thread waits while `hold` is true before each job
    /// (tests catch a job while it runs, without sleeping in the worker).
    #[doc(hidden)]
    pub fn start_gated(
        profile: PathBuf,
        fonts: &'static Fonts,
        hold: Option<Arc<AtomicBool>>,
    ) -> io::Result<Self> {
        let (jobs, incoming) = mpsc::sync_channel::<(Job, Arc<AtomicBool>)>(1);
        let (outgoing, replies) = mpsc::sync_channel(1);
        let running = Arc::new(AtomicBool::new(false));
        let busy = running.clone();
        std::thread::Builder::new()
            .name("sela-bundle".into())
            .spawn(move || {
                while let Ok((job, cancel)) = incoming.recv() {
                    while hold.as_ref().is_some_and(|h| h.load(Ordering::Acquire)) {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                    let done = match job {
                        Job::ExportSchedule {
                            file,
                            destination,
                            options,
                        } => {
                            let result = export_schedule(
                                &profile,
                                &file,
                                &destination,
                                fonts,
                                options,
                                &cancel,
                            );
                            Done::Exported(destination, result)
                        }
                        Job::ImportSchedule { package } => {
                            let result = import_schedule(&profile, &package, &cancel);
                            Done::Imported(package, result)
                        }
                        Job::BackupProfile { destination } => {
                            let result = backup_profile(&profile, &destination, &cancel);
                            Done::BackedUp(destination, result)
                        }
                        Job::RestoreProfile {
                            backup,
                            destination,
                        } => {
                            let result = restore_profile(&backup, &destination, &cancel);
                            Done::Restored(destination, result)
                        }
                    };
                    let failed = outgoing.send(done).is_err();
                    busy.store(false, Ordering::Release);
                    if failed {
                        break;
                    }
                }
            })?;
        Ok(Self {
            jobs,
            replies,
            running,
            cancel: None,
        })
    }

    /// Whether a job is queued or running (its reply not yet polled).
    pub fn busy(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Queues `job`; false (and nothing queued) while another is running.
    pub fn submit(&mut self, job: Job) -> bool {
        if self.busy() {
            return false;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.running.store(true, Ordering::Release);
        if self.jobs.try_send((job, cancel.clone())).is_err() {
            self.running.store(false, Ordering::Release);
            return false;
        }
        self.cancel = Some(cancel);
        true
    }

    /// Asks the running job to stop at its next check; it replies
    /// `Cancelled` and leaves nothing half-written behind.
    pub fn cancel(&self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Release);
        }
    }

    pub fn poll(&mut self) -> Option<Done> {
        match self.replies.try_recv() {
            Ok(done) => {
                self.cancel = None;
                Some(done)
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}

#[cfg(test)]
mod tests;
