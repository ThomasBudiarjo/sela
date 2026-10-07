//! Schedule preflight (M1-11): images that are missing or changed since they
//! were chosen, and named fonts that resolve to the bundled fallback. No GPUI
//! types. `check` hashes files and may load fonts, so it runs on `Worker`,
//! never on the UI thread. Policy and limits: `docs/images.md`.
use crate::{
    background::ImageRef,
    fonts::{self, Fonts},
    format::SlideFormat,
    images::{self, Relinked, Substitute},
    storage::Song,
};
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
};

/// Distinct image names hashed by one check (each read is at most 8 MiB).
pub const MAX_IMAGE_CHECKS: usize = 256;
/// Distinct font families resolved by one check.
pub const MAX_FONT_CHECKS: usize = 64;

/// Where a song uses an image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Use {
    /// The song master, behind slides without their own background.
    Master,
    /// The slide's own background; an index into `Song::sections`.
    Slide(usize),
}

/// One pinned image that would render black, with every place it is used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageIssue {
    pub image: ImageRef,
    /// `Missing`, `Changed` (hash mismatch), `TooLarge` or `Invalid`.
    pub problem: Substitute,
    pub uses: Vec<Use>,
}

/// A named family that is not installed (or no longer loads); those slides
/// render in the bundled DejaVu Sans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontIssue {
    /// As first written in the song, trimmed.
    pub family: String,
    /// Indexes into `Song::sections`.
    pub slides: Vec<usize>,
}

/// Problems of one schedule item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemReport {
    /// Position in the schedule.
    pub index: usize,
    pub title: String,
    pub images: Vec<ImageIssue>,
    pub fonts: Vec<FontIssue>,
}

/// The profile's logo choice cannot be shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogoIssue {
    /// `logo.txt` cannot be read or names an unusable file.
    Choice(images::Error),
    /// The chosen image cannot be read. The logo is not pinned to a hash, so
    /// a replaced logo file is not reported.
    Image {
        name: String,
        problem: images::Error,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Only items with at least one issue, in schedule order.
    pub items: Vec<ItemReport>,
    pub logo: Option<LogoIssue>,
    /// False until the shared font catalog has been scanned; fonts are then
    /// not reported rather than all reported missing.
    pub fonts_checked: bool,
    /// Image uses skipped once `MAX_IMAGE_CHECKS` names were hashed.
    pub unchecked_images: usize,
    /// Font uses skipped once `MAX_FONT_CHECKS` families were resolved.
    pub unchecked_fonts: usize,
}

impl Report {
    /// Nothing missing and nothing left unchecked.
    pub fn is_clean(&self) -> bool {
        self.items.is_empty()
            && self.logo.is_none()
            && self.fonts_checked
            && self.unchecked_images == 0
            && self.unchecked_fonts == 0
    }
}

/// Checks a schedule's resolved songs (in item order, as `storage::Reply::
/// Schedule` returns them) against the profile and the font store. Images
/// are hashed, not decoded. `None` when `cancelled` turned true. Blocking.
pub fn check(
    profile: &Path,
    songs: &[Song],
    fonts: &Fonts,
    cancelled: &dyn Fn() -> bool,
) -> Option<Report> {
    let mut report = Report {
        fonts_checked: fonts.catalog().is_some(),
        ..Report::default()
    };
    let mut hashes: HashMap<String, Result<[u8; 32], Substitute>> = HashMap::new();
    let mut families: HashMap<String, bool> = HashMap::new();
    for (index, song) in songs.iter().enumerate() {
        if cancelled() {
            return None;
        }
        let mut item = ItemReport {
            index,
            title: song.title.clone(),
            images: Vec::new(),
            fonts: Vec::new(),
        };
        let uses = song
            .master
            .as_ref()
            .and_then(|b| b.image_ref())
            .map(|image| (Use::Master, image))
            .into_iter()
            .chain(song.sections.iter().enumerate().filter_map(|(i, s)| {
                let image = s.background.as_ref()?.image_ref()?;
                Some((Use::Slide(i), image))
            }));
        for (at, image) in uses {
            if !hashes.contains_key(&image.name) {
                if hashes.len() >= MAX_IMAGE_CHECKS {
                    report.unchecked_images += 1;
                    continue;
                }
                let hash = images::content_hash(profile, &image.name);
                hashes.insert(image.name.clone(), hash);
            }
            let problem = match hashes[&image.name] {
                Ok(hash) if hash == image.sha256 => continue,
                Ok(_) => Substitute::Changed,
                Err(problem) => problem,
            };
            match item.images.iter_mut().find(|i| i.image == *image) {
                Some(issue) => issue.uses.push(at),
                None => item.images.push(ImageIssue {
                    image: image.clone(),
                    problem,
                    uses: vec![at],
                }),
            }
        }
        if report.fonts_checked {
            for (slide, section) in song.sections.iter().enumerate() {
                let Some(family) = section.format.font.as_deref().map(str::trim) else {
                    continue;
                };
                if family.is_empty() {
                    continue;
                }
                let key = family.to_lowercase();
                if !families.contains_key(&key) {
                    if families.len() >= MAX_FONT_CHECKS {
                        report.unchecked_fonts += 1;
                        continue;
                    }
                    if cancelled() {
                        return None;
                    }
                    let format = SlideFormat {
                        font: Some(family.to_owned()),
                        ..SlideFormat::default()
                    };
                    let missing = fonts.resolve(&format).warning().is_some();
                    families.insert(key.clone(), missing);
                }
                if !families[&key] {
                    continue;
                }
                match item
                    .fonts
                    .iter_mut()
                    .find(|f| f.family.to_lowercase() == key)
                {
                    Some(issue) => issue.slides.push(slide),
                    None => item.fonts.push(FontIssue {
                        family: family.to_owned(),
                        slides: vec![slide],
                    }),
                }
            }
        }
        if !item.images.is_empty() || !item.fonts.is_empty() {
            report.items.push(item);
        }
    }
    if cancelled() {
        return None;
    }
    report.logo = match images::read_logo(profile) {
        Ok(None) => None,
        Ok(Some(name)) => images::resource(profile, &name)
            .err()
            .map(|problem| LogoIssue::Image { name, problem }),
        Err(error) => Some(LogoIssue::Choice(error)),
    };
    Some(report)
}

pub enum Request {
    /// Check a schedule's resolved songs, in item order.
    Check(Vec<Song>),
    /// Copy a located file in as `ImageRef` if, and only if, its hash matches.
    Relink(ImageRef, PathBuf),
}

pub enum Reply {
    /// The latest check's generation and report; superseded checks are
    /// cancelled and do not reply.
    Checked(u64, Report),
    Relinked(ImageRef, Result<Relinked, images::Error>),
}

/// One thread, two queued requests and two buffered replies; never blocks
/// the caller. A newer check cancels an older one still running.
pub struct Worker {
    requests: SyncSender<(u64, Request)>,
    replies: Receiver<Reply>,
    /// Generation of the newest accepted check.
    latest: Arc<AtomicU64>,
    next: u64,
}

impl Worker {
    pub fn start(profile: PathBuf) -> io::Result<Self> {
        Self::start_with(profile, fonts::shared())
    }

    /// `start` with an explicit font store (tests, other catalogs).
    pub fn start_with(profile: PathBuf, fonts: &'static Fonts) -> io::Result<Self> {
        let (requests, incoming) = mpsc::sync_channel::<(u64, Request)>(2);
        let (outgoing, replies) = mpsc::sync_channel(2);
        let latest = Arc::new(AtomicU64::new(0));
        let newest = latest.clone();
        std::thread::Builder::new()
            .name("sela-preflight".into())
            .spawn(move || {
                while let Ok((generation, request)) = incoming.recv() {
                    let reply = match request {
                        Request::Check(songs) => {
                            // Greater, not unequal: the caller publishes a
                            // generation after queueing it, so a check may
                            // start before `latest` reaches it.
                            let superseded = || newest.load(Ordering::Acquire) > generation;
                            match check(&profile, &songs, fonts, &superseded) {
                                Some(report) => Reply::Checked(generation, report),
                                None => continue,
                            }
                        }
                        Request::Relink(image, source) => {
                            let result =
                                images::relink(&profile, &image, &source, &AtomicBool::new(false));
                            Reply::Relinked(image, result)
                        }
                    };
                    if outgoing.send(reply).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests,
            replies,
            latest,
            next: 0,
        })
    }

    fn submit(&mut self, request: Request) -> Result<u64, images::Error> {
        let generation = self.next + 1;
        self.requests
            .try_send((generation, request))
            .map_err(|error| match error {
                TrySendError::Full(_) => images::Error::Busy,
                TrySendError::Disconnected(_) => images::Error::Disconnected,
            })?;
        self.next = generation;
        Ok(generation)
    }

    /// Queues a check; its reply carries the returned generation.
    pub fn check(&mut self, songs: Vec<Song>) -> Result<u64, images::Error> {
        let generation = self.submit(Request::Check(songs))?;
        self.latest.store(generation, Ordering::Release);
        Ok(generation)
    }

    pub fn relink(&mut self, image: ImageRef, source: PathBuf) -> Result<(), images::Error> {
        self.submit(Request::Relink(image, source)).map(|_| ())
    }

    pub fn poll(&self) -> Option<Result<Reply, images::Error>> {
        match self.replies.try_recv() {
            Ok(reply) => Some(Ok(reply)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(images::Error::Disconnected)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{arrangement::SectionId, background::Background, fonts::Catalog, storage::Section};
    use image::ImageEncoder;
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        time::{Duration, Instant},
    };

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut encoded = Vec::new();
        let pixels = vec![90u8; (width * height * 4) as usize];
        image::codecs::png::PngEncoder::new(&mut encoded)
            .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
            .unwrap();
        encoded
    }

    fn pin(name: &str, bytes: &[u8]) -> ImageRef {
        ImageRef {
            name: name.into(),
            sha256: Sha256::digest(bytes).into(),
        }
    }

    fn slide(background: Option<&ImageRef>, font: Option<&str>) -> Section {
        Section {
            id: SectionId::allocate(),
            label: String::new(),
            lyrics: "Line".into(),
            format: SlideFormat {
                font: font.map(Into::into),
                ..SlideFormat::default()
            },
            background: background.map(|image| Background::image(image.clone())),
        }
    }

    fn song(title: &str, master: Option<Background>, sections: Vec<Section>) -> Song {
        Song {
            title: title.into(),
            authors: String::new(),
            copyright: String::new(),
            license: String::new(),
            sections,
            variants: Vec::new(),
            master,
        }
    }

    /// A catalog holding only the bundled DejaVu Sans file, copied to `dir`.
    fn dejavu(dir: &Path) -> (Fonts, PathBuf) {
        let fonts_dir = dir.join("fonts");
        fs::create_dir_all(&fonts_dir).unwrap();
        let file = fonts_dir.join("DejaVuSans.ttf");
        fs::write(&file, fonts::BUNDLED).unwrap();
        let store = Fonts::new();
        store.install_catalog(Catalog::scan([fonts_dir]));
        (store, file)
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        profile: PathBuf,
        songs: Vec<Song>,
        moved: ImageRef,
        edited: ImageRef,
        /// Where the moved image now lives, outside the profile.
        moved_to: PathBuf,
        fonts: Fonts,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let images = images::images_dir(&profile);
        fs::create_dir_all(&images).unwrap();
        let present = png(4, 2);
        fs::write(images.join("Present.png"), &present).unwrap();
        // Edited in place after it was chosen.
        let chosen = png(3, 3);
        fs::write(images.join("Edited.png"), png(5, 5)).unwrap();
        // Moved out of the profile after it was chosen.
        let moved_bytes = png(6, 2);
        let moved_to = dir.path().join("elsewhere.png");
        fs::write(&moved_to, &moved_bytes).unwrap();
        let present = pin("Present.png", &present);
        let edited = pin("Edited.png", &chosen);
        let moved = pin("Moved.png", &moved_bytes);
        let songs = vec![
            song(
                "Amazing",
                Some(Background::image(present.clone())),
                vec![slide(None, None), slide(None, Some("DejaVu Sans"))],
            ),
            song(
                "Grace",
                Some(Background::image(moved.clone())),
                vec![
                    slide(Some(&edited), None),
                    slide(Some(&moved), Some("dejavu sans")),
                    slide(None, Some("Imaginary Sans")),
                    slide(None, Some(" imaginary SANS ")),
                ],
            ),
            song("Plain", Some(Background::color([10, 20, 30])), vec![]),
            song(
                "Second Grace",
                None,
                vec![slide(Some(&edited), None), slide(Some(&present), None)],
            ),
        ];
        let (fonts, _) = dejavu(dir.path());
        Fixture {
            profile,
            songs,
            moved,
            edited,
            moved_to,
            fonts,
            _dir: dir,
        }
    }

    fn expected(f: &Fixture) -> Report {
        Report {
            items: vec![
                ItemReport {
                    index: 1,
                    title: "Grace".into(),
                    images: vec![
                        ImageIssue {
                            image: f.moved.clone(),
                            problem: Substitute::Missing,
                            uses: vec![Use::Master, Use::Slide(1)],
                        },
                        ImageIssue {
                            image: f.edited.clone(),
                            problem: Substitute::Changed,
                            uses: vec![Use::Slide(0)],
                        },
                    ],
                    fonts: vec![FontIssue {
                        family: "Imaginary Sans".into(),
                        slides: vec![2, 3],
                    }],
                },
                ItemReport {
                    index: 3,
                    title: "Second Grace".into(),
                    images: vec![ImageIssue {
                        image: f.edited.clone(),
                        problem: Substitute::Changed,
                        uses: vec![Use::Slide(0)],
                    }],
                    fonts: vec![],
                },
            ],
            logo: None,
            fonts_checked: true,
            unchecked_images: 0,
            unchecked_fonts: 0,
        }
    }

    #[test]
    fn preflight_reports_missing_and_changed_images_and_missing_fonts() {
        let f = fixture();
        let report = check(&f.profile, &f.songs, &f.fonts, &|| false).unwrap();
        assert_eq!(report, expected(&f));
        assert!(!report.is_clean());
        let clean = check(&f.profile, &f.songs[..1], &f.fonts, &|| false).unwrap();
        assert!(clean.is_clean(), "{clean:?}");
        assert_eq!(check(&f.profile, &f.songs, &f.fonts, &|| true), None);
    }

    #[test]
    fn preflight_reports_the_logo_and_unscanned_fonts() {
        let f = fixture();
        fs::write(f.profile.join(images::LOGO_FILE), "Logo.png\n").unwrap();
        let report = check(&f.profile, &f.songs[..1], &f.fonts, &|| false).unwrap();
        assert_eq!(
            report.logo,
            Some(LogoIssue::Image {
                name: "Logo.png".into(),
                problem: images::Error::Missing,
            })
        );
        fs::write(f.profile.join(images::LOGO_FILE), "..\\Logo.png\n").unwrap();
        let report = check(&f.profile, &f.songs[..1], &f.fonts, &|| false).unwrap();
        assert_eq!(
            report.logo,
            Some(LogoIssue::Choice(images::Error::InvalidName))
        );
        fs::write(f.profile.join(images::LOGO_FILE), "Present.png\n").unwrap();
        let report = check(&f.profile, &f.songs[..1], &f.fonts, &|| false).unwrap();
        assert_eq!(report.logo, None);
        // Without a scanned catalog fonts are unchecked, not all missing.
        let unscanned = Fonts::new();
        let report = check(&f.profile, &f.songs, &unscanned, &|| false).unwrap();
        assert!(!report.fonts_checked && !report.is_clean());
        assert!(report.items.iter().all(|item| item.fonts.is_empty()));
    }

    #[test]
    fn preflight_reports_a_catalogued_font_whose_file_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let (fonts, file) = dejavu(dir.path());
        fs::remove_file(file).unwrap();
        let songs = [song("Hymn", None, vec![slide(None, Some("DejaVu Sans"))])];
        let report = check(dir.path(), &songs, &fonts, &|| false).unwrap();
        assert_eq!(
            report.items[0].fonts,
            [FontIssue {
                family: "DejaVu Sans".into(),
                slides: vec![0],
            }]
        );
    }

    #[test]
    fn preflight_work_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let (fonts, _) = dejavu(dir.path());
        let names: Vec<ImageRef> = (0..=MAX_IMAGE_CHECKS)
            .map(|n| pin(&format!("gone {n}.png"), &[n as u8]))
            .collect();
        let families: Vec<String> = (0..=MAX_FONT_CHECKS)
            .map(|n| format!("Absent {n}"))
            .collect();
        let sections = names
            .iter()
            .zip(families.iter().map(Some).chain(std::iter::repeat(None)))
            .map(|(image, family)| slide(Some(image), family.map(String::as_str)))
            .collect();
        let report = check(dir.path(), &[song("Long", None, sections)], &fonts, &|| {
            false
        })
        .unwrap();
        assert_eq!(report.items[0].images.len(), MAX_IMAGE_CHECKS);
        assert_eq!(report.items[0].fonts.len(), MAX_FONT_CHECKS);
        assert_eq!((report.unchecked_images, report.unchecked_fonts), (1, 1));
        assert!(!report.is_clean());
    }

    #[test]
    fn worker_checks_then_relinks_only_a_matching_file() {
        let f = fixture();
        let fonts: &'static Fonts = Box::leak(Box::new(Fonts::new()));
        fonts.install(f.fonts.catalog().unwrap());
        let mut worker = Worker::start_with(f.profile.clone(), fonts).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let wait = |worker: &Worker| loop {
            if let Some(reply) = worker.poll() {
                return reply.unwrap();
            }
            assert!(Instant::now() < end, "bounded wait");
            std::thread::sleep(Duration::from_millis(2));
        };
        let first = worker.check(f.songs.clone()).unwrap();
        assert!(matches!(wait(&worker),
            Reply::Checked(generation, report) if generation == first && report == expected(&f)));
        // The changed image's current file is another image: refused.
        worker.relink(f.edited.clone(), f.moved_to.clone()).unwrap();
        assert!(matches!(wait(&worker),
            Reply::Relinked(image, Err(images::Error::Mismatch)) if image == f.edited));
        worker.relink(f.moved.clone(), f.moved_to.clone()).unwrap();
        assert!(matches!(wait(&worker),
            Reply::Relinked(image, Ok(Relinked::Restored)) if image == f.moved));
        let second = worker.check(f.songs.clone()).unwrap();
        assert!(second > first);
        let Reply::Checked(generation, report) = wait(&worker) else {
            panic!("expected a check");
        };
        assert_eq!(generation, second);
        let grace = &report.items[0];
        assert_eq!(grace.index, 1);
        assert!(grace.images.iter().all(|issue| issue.image != f.moved));
        assert_eq!(grace.images.len(), 1);
    }
}
