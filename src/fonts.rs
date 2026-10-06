//! Installed-font discovery and `SlideFormat` face resolution (M1-05g2). No GPUI.
//!
//! `Catalog::scan`, face loading and `Fonts::resolve` do blocking filesystem
//! work and belong on background workers; `Resolved::bundled` resolves
//! without I/O and is safe anywhere. Bounds are Sela's, not observed
//! EasyWorship ranges: 8 MiB font files, 64-face collections, 8192 catalog
//! faces and a 32 MiB loaded-face cache.
use crate::{
    format::{MAX_FONT_NAME, SlideFormat},
    scene::ContentVersion,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

/// Bundled DejaVu Sans regular and Bold (see `tests/fixtures/README.md`).
/// Provisional default typography until a theme ticket chooses otherwise.
pub const BUNDLED: &[u8] = include_bytes!("../tests/fixtures/DejaVuSans.ttf");
pub const BUNDLED_BOLD: &[u8] = include_bytes!("../tests/fixtures/DejaVuSans-Bold.ttf");
/// Same bound as every other source the preparer accepts.
pub const MAX_FILE: u64 = 8 * 1024 * 1024;
/// Collections beyond this many faces are skipped whole.
pub const MAX_COLLECTION: u32 = 64;
/// Catalog cap; every face beyond it is counted as excluded.
pub const MAX_CATALOG_FACES: usize = 8192;
/// Total bytes of loaded installed faces kept in the cache.
const CACHE_BUDGET: usize = 32 * 1024 * 1024;
/// Directory recursion bound for one scan.
const SCAN_DEPTH: u32 = 6;

const BUNDLED_VERSION: ContentVersion = ContentVersion {
    id: 0x53454c41_44656a61_56755361_6e730001,
    revision: 1,
};
const BUNDLED_BOLD_VERSION: ContentVersion = ContentVersion {
    id: 0x53454c41_44656a61_56755361_6e730002,
    revision: 1,
};

/// One parsed font face. Cheap to clone; the bytes are shared.
#[derive(Clone)]
pub struct Face {
    bytes: Arc<[u8]>,
    index: u32,
    version: ContentVersion,
}

impl Face {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn bytes_arc(&self) -> Arc<[u8]> {
        self.bytes.clone()
    }
    pub fn index(&self) -> u32 {
        self.index
    }
    pub fn version(&self) -> ContentVersion {
        self.version
    }
    /// The identity installed faces get: SHA-256 of the file plus the face
    /// index. Bundled faces keep fixed constants.
    fn installed(bytes: Arc<[u8]>, index: u32) -> Option<Self> {
        ttf_parser::Face::parse(&bytes, index).ok()?;
        let digest = Sha256::digest(&bytes);
        let mut id = [0u8; 16];
        id.copy_from_slice(&digest[..16]);
        let mut revision = [0u8; 8];
        revision.copy_from_slice(&digest[16..24]);
        Some(Self {
            version: ContentVersion {
                id: u128::from_le_bytes(id),
                revision: u64::from_le_bytes(revision) ^ (u64::from(index) << 32),
            },
            bytes,
            index,
        })
    }
}

/// One face choice for a `SlideFormat`: the face, whether weight and slant
/// must be synthesized at raster time, and a user-facing fallback note.
#[derive(Clone)]
pub struct Resolved {
    face: Face,
    synth_bold: bool,
    synth_italic: bool,
    warning: Option<String>,
}

impl Resolved {
    pub fn face(&self) -> &Face {
        &self.face
    }
    pub fn synth_bold(&self) -> bool {
        self.synth_bold
    }
    pub fn synth_italic(&self) -> bool {
        self.synth_italic
    }
    /// Set when the requested family could not be resolved and the bundled
    /// default is shown instead.
    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    /// Resolve against the bundled faces only: the default look, or the same
    /// with a fallback warning for a named family. Pure: no I/O or catalog.
    pub fn bundled(format: &SlideFormat) -> Self {
        let bold = format.bold == Some(true);
        Self {
            face: if bold {
                bundled_bold_face()
            } else {
                bundled_regular_face()
            },
            // The bundled family ships a real Bold face but no italic face.
            synth_bold: false,
            synth_italic: format.italic == Some(true),
            warning: format.font.as_deref().map(|name| {
                format!("Font \u{201c}{name}\u{201d} unavailable \u{b7} showing DejaVu Sans")
            }),
        }
    }
}

fn bundled_regular_face() -> Face {
    static FACE: OnceLock<Face> = OnceLock::new();
    FACE.get_or_init(|| Face {
        bytes: BUNDLED.into(),
        index: 0,
        version: BUNDLED_VERSION,
    })
    .clone()
}

fn bundled_bold_face() -> Face {
    static FACE: OnceLock<Face> = OnceLock::new();
    FACE.get_or_init(|| Face {
        bytes: BUNDLED_BOLD.into(),
        index: 0,
        version: BUNDLED_BOLD_VERSION,
    })
    .clone()
}

/// One catalogued face. `keys` are the lowercased family names (English,
/// typographic and localized) a request may match.
struct CatalogFace {
    family: String,
    keys: Box<[String]>,
    weight: u16,
    italic: bool,
    path: PathBuf,
    index: u32,
}

/// Snapshot of the installed fonts. Scanning is bounded; `excluded` counts
/// every file or face skipped by a bound or a parse failure.
pub struct Catalog {
    faces: Vec<CatalogFace>,
    /// Unique display family names, sorted: the font list M1-05g3 offers.
    families: Vec<String>,
    pub excluded: usize,
}

impl Catalog {
    /// Scan the system font directories. Blocking; background workers only.
    pub fn scan_system() -> Self {
        Self::scan(system_dirs())
    }

    /// Scan explicit directories (recursively, bounded). Blocking.
    pub fn scan<I: IntoIterator<Item = PathBuf>>(dirs: I) -> Self {
        Self::scan_with(dirs, MAX_CATALOG_FACES)
    }

    /// `scan` with an explicit face cap, for tests and small-device profiles.
    pub fn scan_with<I: IntoIterator<Item = PathBuf>>(dirs: I, cap: usize) -> Self {
        let mut scanner = Scanner {
            faces: Vec::new(),
            excluded: 0,
            cap,
        };
        for dir in dirs {
            scanner.walk(&dir, 0);
        }
        let mut faces = scanner.faces;
        faces.sort_by(|a, b| (&a.path, a.index).cmp(&(&b.path, b.index)));
        faces.dedup_by(|a, b| a.path == b.path && a.index == b.index);
        let mut families: Vec<String> = faces.iter().map(|f| f.family.clone()).collect();
        families.sort();
        families.dedup();
        Self {
            faces,
            families,
            excluded: scanner.excluded,
        }
    }

    /// Display family names in the catalog.
    pub fn families(&self) -> &[String] {
        &self.families
    }

    /// Best catalogued face for a family request: the italic flag first, then
    /// the bold tier (weight >= 600 when bold is wanted, else below), then the
    /// closest weight to 700 or 400, then the scan order. Sela's rule, not
    /// observed EasyWorship behavior.
    fn face_for(&self, family: &str, bold: bool, italic: bool) -> Option<&CatalogFace> {
        let key = family.trim().to_lowercase();
        let target: u16 = if bold { 700 } else { 400 };
        self.faces
            .iter()
            .filter(|face| face.keys.contains(&key))
            .min_by_key(|face| {
                (
                    u8::from(face.italic != italic),
                    u8::from(bold && face.weight < 600 || !bold && face.weight >= 600),
                    face.weight.abs_diff(target),
                    face.index,
                )
            })
    }
}

struct Scanner {
    faces: Vec<CatalogFace>,
    excluded: usize,
    cap: usize,
}

impl Scanner {
    fn walk(&mut self, dir: &Path, depth: u32) {
        if depth > SCAN_DEPTH {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                self.walk(&entry.path(), depth + 1);
                continue;
            }
            if !matches!(
                entry.path().extension().and_then(|e| e.to_str()),
                Some("ttf" | "ttc" | "otf" | "otc")
            ) {
                continue;
            }
            self.load(&entry.path());
        }
    }

    fn load(&mut self, path: &Path) {
        let Ok(metadata) = std::fs::metadata(path) else {
            self.excluded += 1;
            return;
        };
        if !metadata.is_file() || metadata.len() > MAX_FILE {
            self.excluded += 1;
            return;
        }
        let Ok(bytes) = std::fs::read(path) else {
            self.excluded += 1;
            return;
        };
        let count = match collection_faces(&bytes) {
            // A collection header that is truncated or out of bounds skips
            // the whole file; the face-count read is bounded by the header.
            None if bytes.get(..4) == Some(b"ttcf") => return self.excluded += 1,
            None => 1,
            Some(count) => count,
        };
        for index in 0..count {
            self.record(path, &bytes, index);
        }
    }

    fn record(&mut self, path: &Path, bytes: &[u8], index: u32) {
        if self.faces.len() >= self.cap {
            self.excluded += 1;
            return;
        }
        let Ok(face) = ttf_parser::Face::parse(bytes, index) else {
            self.excluded += 1;
            return;
        };
        let Some((family, keys)) = family_of(&face) else {
            self.excluded += 1;
            return;
        };
        if family.len() > MAX_FONT_NAME || keys.iter().any(|key| key.len() > MAX_FONT_NAME) {
            self.excluded += 1;
            return;
        }
        self.faces.push(CatalogFace {
            family,
            keys,
            weight: face.weight().to_number(),
            italic: !matches!(face.style(), ttf_parser::Style::Normal),
            path: path.to_owned(),
            index,
        });
    }
}

/// Face count a `ttcf` header declares, with a bound, without a full parse.
/// `None` for non-collections or headers that are truncated or out of bounds.
pub fn collection_faces(bytes: &[u8]) -> Option<u32> {
    if bytes.get(..4) != Some(b"ttcf") {
        return None;
    }
    let count = u32::from_be_bytes(bytes.get(8..12)?.try_into().ok()?);
    (count > 0 && count <= MAX_COLLECTION).then_some(count)
}

/// Display family plus lowercase match keys, or `None` when the face has no
/// usable Unicode family name. Windows en-US is preferred, then any English
/// platform spelling, then the typographic name in any language.
fn family_of(face: &ttf_parser::Face<'_>) -> Option<(String, Box<[String]>)> {
    let mut display: Option<(u8, String)> = None;
    let mut keys: Vec<String> = Vec::new();
    for name in face.names() {
        // Only Unicode-backed records decode; other encodings cannot match.
        let Some(text) = name.to_string() else {
            continue;
        };
        if !matches!(name.name_id, 1 | 16) {
            continue;
        }
        let rank = match (name.platform_id, name.name_id, name.language_id) {
            (ttf_parser::PlatformId::Windows, 16, 0x409) => 0,
            (ttf_parser::PlatformId::Windows, 1, 0x409) => 1,
            (ttf_parser::PlatformId::Windows, 16, _) => 2,
            (ttf_parser::PlatformId::Windows, 1, _) => 3,
            (_, 16, _) => 4,
            _ => 5,
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        if display.as_ref().is_none_or(|(best, _)| rank < *best) {
            display = Some((rank, text.to_owned()));
        }
        let lower = text.to_lowercase();
        if !lower.is_empty() && !keys.contains(&lower) {
            keys.push(lower);
        }
    }
    let (_, family) = display?;
    if keys.is_empty() {
        return None;
    }
    Some((family, keys.into_boxed_slice()))
}

/// System font directories, mirroring fontdb's platform list. On Linux this
/// is a directory heuristic, not full fontconfig (recorded limitation).
pub fn system_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut dirs = Vec::new();
        let root = std::env::var_os("SYSTEMROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        dirs.push(root.join("Fonts"));
        if let Some(home) = std::env::var_os("USERPROFILE") {
            let home = PathBuf::from(home);
            dirs.push(home.join(r"AppData\Local\Microsoft\Windows\Fonts"));
            dirs.push(home.join(r"AppData\Roaming\Microsoft\Windows\Fonts"));
        }
        dirs
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/Library/Fonts"),
            PathBuf::from("/System/Library/Fonts"),
        ]
    }
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "windows"))))]
    {
        let mut dirs = vec![
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/local/share/fonts"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            dirs.push(home.join(".local/share/fonts"));
            dirs.push(home.join(".fonts"));
        }
        dirs
    }
}

/// A loaded face plus its last-use stamp for the bounded cache.
struct Cached {
    face: Face,
    used: u64,
}

#[derive(Default)]
struct FaceCache {
    entries: HashMap<(PathBuf, u32), Cached>,
    used: u64,
    bytes: usize,
}

impl FaceCache {
    fn get(&mut self, key: &(PathBuf, u32)) -> Option<Face> {
        self.used += 1;
        let entry = self.entries.get_mut(key)?;
        entry.used = self.used;
        Some(entry.face.clone())
    }

    fn insert(&mut self, key: (PathBuf, u32), face: Face, budget: usize) {
        let size = face.bytes().len();
        if size > budget {
            return;
        }
        if let Some(old) = self.entries.remove(&key) {
            self.bytes -= old.face.bytes().len();
        }
        self.used += 1;
        self.bytes += size;
        let used = self.used;
        self.entries.insert(key, Cached { face, used });
        while self.bytes > budget {
            let evict = self
                .entries
                .iter()
                .min_by_key(|(_, cached)| cached.used)
                .map(|(key, _)| key.clone());
            match evict.and_then(|key| self.entries.remove(&key)) {
                Some(cached) => self.bytes -= cached.face.bytes().len(),
                None => break,
            }
        }
    }
}

/// Shareable font store: an installed catalog plus a bounded face cache.
/// `resolve` may read files; call it from background workers only.
#[derive(Default)]
pub struct Fonts {
    catalog: Mutex<Option<Arc<Catalog>>>,
    cache: Mutex<FaceCache>,
}

impl Fonts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Publish a scan result; it replaces any previous catalog.
    pub fn install_catalog(&self, catalog: Catalog) {
        *self.catalog.lock().unwrap() = Some(Arc::new(catalog));
    }

    /// The installed catalog, once a scan has been published.
    pub fn catalog(&self) -> Option<Arc<Catalog>> {
        self.catalog.lock().unwrap().clone()
    }

    /// Resolve `format` to a face. `None` format fields keep the default
    /// look; a named family needs the catalog and may read its file.
    pub fn resolve(&self, format: &SlideFormat) -> Resolved {
        let Some(name) = format.font.as_deref().map(str::trim) else {
            return Resolved::bundled(format);
        };
        if name.is_empty() {
            return Resolved::bundled(format);
        }
        let bold = format.bold == Some(true);
        let italic = format.italic == Some(true);
        let synth = |face: &CatalogFace| (bold && face.weight < 600, italic && !face.italic);
        let Some(catalog) = self.catalog() else {
            return Resolved::bundled(format);
        };
        let Some(target) = catalog.face_for(name, bold, italic) else {
            return Resolved::bundled(format);
        };
        let key = (target.path.clone(), target.index);
        if let Some(face) = self.cache.lock().unwrap().get(&key) {
            let (synth_bold, synth_italic) = synth(target);
            return Resolved {
                face,
                synth_bold,
                synth_italic,
                warning: None,
            };
        }
        // Re-check the size bound: the file may have changed since the scan.
        let loaded = std::fs::metadata(&target.path)
            .ok()
            .filter(|m| m.is_file() && m.len() <= MAX_FILE)
            .and_then(|_| std::fs::read(&target.path).ok())
            .and_then(|bytes| Face::installed(bytes.into(), target.index));
        match loaded {
            Some(face) => {
                let (synth_bold, synth_italic) = synth(target);
                self.cache
                    .lock()
                    .unwrap()
                    .insert(key, face.clone(), CACHE_BUDGET);
                Resolved {
                    face,
                    synth_bold,
                    synth_italic,
                    warning: None,
                }
            }
            // The file vanished or stopped parsing since the scan: the
            // catalog is a snapshot, so fall back with the usual warning.
            None => Resolved::bundled(format),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::SlideFormat;

    /// A synthetic valid collection: two face indexes over one regular font,
    /// with the embedded font's table offsets rebased to the collection header
    /// (ttf-parser resolves table records as offsets within the whole file).
    fn collection() -> Vec<u8> {
        const HEADER: usize = 20; // tag, version, count, two offset slots.
        let mut ttc = Vec::with_capacity(HEADER + BUNDLED.len());
        ttc.extend_from_slice(b"ttcf");
        ttc.extend_from_slice(&[0, 1, 0, 0]); // version 0x00010000
        ttc.extend_from_slice(&2u32.to_be_bytes());
        ttc.extend_from_slice(&(HEADER as u32).to_be_bytes()); // face 0 offset
        ttc.extend_from_slice(&(HEADER as u32).to_be_bytes()); // face 1 offset
        ttc.extend_from_slice(BUNDLED);
        let tables = u16::from_be_bytes([BUNDLED[4], BUNDLED[5]]);
        for record in 0..usize::from(tables) {
            // Table records begin after the 12-byte face header; +8 is the
            // offset field of each 16-byte record.
            let at = HEADER + 12 + record * 16 + 8;
            let offset = u32::from_be_bytes(ttc[at..at + 4].try_into().unwrap());
            ttc[at..at + 4].copy_from_slice(&(offset + HEADER as u32).to_be_bytes());
        }
        ttc
    }

    fn dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn write(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn bundled_resolution_matches_the_format_flags() {
        let plain = Resolved::bundled(&SlideFormat::default());
        assert_eq!(plain.face().bytes(), BUNDLED);
        assert_eq!(plain.face().index(), 0);
        assert!(!plain.synth_bold() && !plain.synth_italic());
        assert_eq!(plain.warning(), None);

        let bold = Resolved::bundled(&SlideFormat {
            bold: Some(true),
            ..Default::default()
        });
        assert_eq!(bold.face().bytes(), BUNDLED_BOLD);
        assert!(!bold.synth_bold() && !bold.synth_italic());

        let italic = Resolved::bundled(&SlideFormat {
            italic: Some(true),
            ..Default::default()
        });
        assert_eq!(italic.face().bytes(), BUNDLED);
        assert!(italic.synth_italic() && !italic.synth_bold());

        let named = Resolved::bundled(&SlideFormat {
            font: Some("Tahoma".into()),
            bold: Some(true),
            ..Default::default()
        });
        assert_eq!(named.face().bytes(), BUNDLED_BOLD);
        assert_eq!(
            named.warning(),
            Some("Font \u{201c}Tahoma\u{201d} unavailable \u{b7} showing DejaVu Sans")
        );
    }

    #[test]
    fn catalog_scan_bounds_files_and_records_faces() {
        let files = dir();
        write(&files, "family.ttf", BUNDLED);
        write(&files, "family-bold.ttf", BUNDLED_BOLD);
        write(&files, "ignored.txt", b"not a font");
        write(&files, "huge.ttf", &vec![0u8; MAX_FILE as usize + 1]);
        let mut too_many = collection();
        too_many[8..12].copy_from_slice(&65u32.to_be_bytes());
        write(&files, "too-many.ttc", &too_many);
        let mut truncated = collection();
        truncated.truncate(10);
        write(&files, "truncated.ttc", &truncated);
        write(&files, "not-a-font.ttf", b"OTTOgarbage");
        write(&files, "collection.ttc", &collection());

        let catalog = Catalog::scan([files.path().to_path_buf()]);
        assert_eq!(
            catalog.faces.len(),
            4,
            "regular, bold and two collection faces"
        );
        // huge, too-many, truncated header and the garbage face are excluded.
        assert_eq!(catalog.excluded, 4);
        assert_eq!(catalog.families(), ["DejaVu Sans".to_owned()]);
        let bold: Vec<_> = catalog
            .faces
            .iter()
            .filter(|f| f.weight == 700)
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(bold, ["family-bold.ttf"]);
        let collection_faces: Vec<u32> = catalog
            .faces
            .iter()
            .filter(|f| f.path.extension() == Some("ttc".as_ref()))
            .map(|f| f.index)
            .collect();
        assert_eq!(collection_faces, [0, 1]);

        // The catalog cap keeps the first faces and counts the rest excluded.
        let capped = Catalog::scan_with([files.path().to_path_buf()], 3);
        assert_eq!(capped.faces.len(), 3);
        assert_eq!(capped.excluded, 5);
    }

    #[test]
    fn collection_face_headers_are_bounded() {
        assert_eq!(collection_faces(&collection()), Some(2));
        assert_eq!(collection_faces(BUNDLED), None);
        assert_eq!(collection_faces(b"ttcf"), None);
        let mut zero = collection();
        zero[8..12].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(collection_faces(&zero), None);
        let mut many = collection();
        many[8..12].copy_from_slice(&64u32.to_be_bytes());
        assert_eq!(collection_faces(&many), Some(64));
        let mut overflow = collection();
        overflow[8..12].copy_from_slice(&65u32.to_be_bytes());
        assert_eq!(collection_faces(&overflow), None);
    }

    #[test]
    fn installed_face_identity_covers_the_collection_index() {
        let bytes: Arc<[u8]> = collection().into();
        let first = Face::installed(bytes.clone(), 0).unwrap();
        let second = Face::installed(bytes, 1).unwrap();
        assert_ne!(first.version(), second.version());
        assert_eq!(
            Face::installed(second.bytes_arc(), 1).unwrap().version(),
            second.version()
        );
        assert!(Face::installed(first.bytes_arc(), 2).is_none());
    }

    #[test]
    fn fonts_resolve_from_the_catalog_and_fall_back() {
        let files = dir();
        write(&files, "family.ttf", BUNDLED);
        write(&files, "family-bold.ttf", BUNDLED_BOLD);
        let fonts = Fonts::new();
        let bold_format = SlideFormat {
            font: Some("DejaVu Sans".into()),
            bold: Some(true),
            ..Default::default()
        };
        // No catalog installed yet: bundled fallback with a warning.
        let early = fonts.resolve(&bold_format);
        assert_eq!(early.face().bytes(), BUNDLED_BOLD);
        assert!(early.warning().is_some());

        fonts.install_catalog(Catalog::scan([files.path().to_path_buf()]));
        let resolved = fonts.resolve(&bold_format);
        assert_eq!(resolved.face().bytes(), BUNDLED_BOLD);
        assert!(!resolved.synth_bold() && !resolved.synth_italic());
        assert_eq!(resolved.warning(), None);
        // A repeat resolve (differently cased) reuses the cached allocation.
        let repeat = fonts.resolve(&SlideFormat {
            font: Some("  DEJAVU sans ".into()),
            bold: Some(true),
            ..Default::default()
        });
        assert!(Arc::ptr_eq(
            &repeat.face().bytes_arc(),
            &resolved.face().bytes_arc()
        ));

        // A real italic face is absent from the catalog: slant is synthesized.
        let italic = fonts.resolve(&SlideFormat {
            font: Some("DejaVu Sans".into()),
            italic: Some(true),
            ..Default::default()
        });
        assert!(italic.synth_italic() && !italic.synth_bold());

        // A missing family falls back with the warning.
        let missing = fonts.resolve(&SlideFormat {
            font: Some("Nope".into()),
            ..Default::default()
        });
        assert_eq!(missing.face().bytes(), BUNDLED);
        assert!(missing.warning().is_some());

        // So does a file that disappears before its first resolve (a cold
        // cache): the catalog is a snapshot, not a live filesystem watch.
        let gone = dir();
        let path = write(&gone, "family.ttf", BUNDLED);
        let cold = Fonts::new();
        cold.install_catalog(Catalog::scan([gone.path().to_path_buf()]));
        std::fs::remove_file(&path).unwrap();
        let vanished = cold.resolve(&SlideFormat {
            font: Some("DejaVu Sans".into()),
            ..Default::default()
        });
        assert_eq!(vanished.face().bytes(), BUNDLED);
        assert!(vanished.warning().is_some());
    }

    #[test]
    fn cache_serves_repeats_and_evicts_over_budget() {
        let mut cache = FaceCache::default();
        let a = Face::installed(BUNDLED.into(), 0).unwrap();
        let b = Face::installed(BUNDLED_BOLD.into(), 0).unwrap();
        let c = Face::installed(collection().into(), 0).unwrap();
        // a and b fit; c only fits next to whichever was used last.
        let budget = a.bytes().len() + c.bytes().len() + 1;
        let key = |name: &str| (PathBuf::from(name), 0);
        cache.insert(key("a"), a.clone(), budget);
        cache.insert(key("b"), b.clone(), budget);
        assert!(cache.get(&key("a")).is_some(), "a is now most recent");
        cache.insert(key("c"), c, budget);
        assert_eq!(cache.entries.len(), 2);
        assert!(cache.get(&key("b")).is_none(), "b was least recent");
        assert!(cache.get(&key("a")).is_some());
        assert!(cache.bytes <= budget);
        // Replacing a key must not double-count its bytes.
        let before = cache.bytes;
        let fresh = Face::installed(BUNDLED.into(), 0).unwrap();
        cache.insert(key("a"), fresh, budget);
        assert_eq!(cache.bytes, before);
    }
}
