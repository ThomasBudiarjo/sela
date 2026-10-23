//! `manifest.json`: the versioned description of a bundle (docs/bundle.md
//! "Manifest"). It names every other entry with its size, SHA-256 and role,
//! lists the images and fonts the content refers to and what was left out.
//! A reader refuses a manifest whose `version` it does not know, and any
//! entry the manifest does not list.
use super::archive::{MAX_ENTRIES, safe_name};
use crate::format::MAX_FONT_NAME;
use crate::images;
use serde_json::{Map, Value, json};

pub const NAME: &str = "manifest.json";
pub const FORMAT: &str = "sela-bundle";
/// Newest manifest version this Sela reads and writes.
pub const VERSION: u64 = 1;
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
/// Bounds on the free-text fields.
pub const MAX_TEXT: usize = 1024;

/// What a bundle holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A standalone schedule (File › Export Standalone Schedule…).
    Schedule,
    /// A whole-profile backup (Profiles › Back Up Profile…).
    Profile,
}

impl Kind {
    fn code(self) -> &'static str {
        match self {
            Self::Schedule => "schedule",
            Self::Profile => "profile",
        }
    }
}

/// Why an entry is in the bundle; each kind allows its own roles and paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The `.selaschedule` document of a standalone schedule, or a saved
    /// schedule file in a profile backup.
    Schedule,
    /// A profile image (`Resources/Images`), pinned by name and SHA-256.
    Image,
    /// The verified copy of `library.sqlite`.
    Library,
    Settings,
    Logo,
    /// An imported scripture translation (`Scripture/*.selabible`).
    Scripture,
}

impl Role {
    fn code(self) -> &'static str {
        match self {
            Self::Schedule => "schedule",
            Self::Image => "image",
            Self::Library => "library",
            Self::Settings => "settings",
            Self::Logo => "logo",
            Self::Scripture => "scripture",
        }
    }

    fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "schedule" => Self::Schedule,
            "image" => Self::Image,
            "library" => Self::Library,
            "settings" => Self::Settings,
            "logo" => Self::Logo,
            "scripture" => Self::Scripture,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRecord {
    pub path: String,
    pub size: u64,
    pub sha256: [u8; 32],
    pub role: Role,
}

/// An image the schedule refers to. Embedded images are the entry
/// `images/<name>` with exactly this SHA-256.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRecord {
    pub name: String,
    pub sha256: [u8; 32],
    pub embedded: bool,
}

/// A named font the content uses. Fonts are never embedded: Sela cannot
/// tell whether a font's license allows passing it on (docs/bundle.md).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontRecord {
    pub family: String,
    /// Installed on the exporting machine; `None` when its fonts were not
    /// scanned yet.
    pub installed: Option<bool>,
}

/// A schedule item left out of the bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Omitted {
    pub title: String,
    /// The scripture translation whose license does not allow sharing it.
    pub translation: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub kind: Kind,
    /// "Sela <version>" of the writer; informational.
    pub generator: String,
    /// The schedule title, or the profile folder's name.
    pub title: String,
    pub files: Vec<FileRecord>,
    pub images: Vec<ImageRecord>,
    pub fonts: Vec<FontRecord>,
    pub omitted: Vec<Omitted>,
    /// Profile backups: `PRAGMA user_version` of the library copy.
    pub library_schema: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    /// Not JSON, not a Sela bundle manifest, or a field is missing or of the
    /// wrong type; says which.
    Malformed(&'static str),
    /// Written by a newer Sela: the manifest's version.
    Newer(u64),
    /// A listed path is unsafe or not allowed for its role and kind.
    Path(String),
    /// Two records name the same path or image.
    Duplicate(String),
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

pub(crate) fn unhex32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// Where an embedded image of a standalone schedule lives.
pub fn image_path(name: &str) -> String {
    format!("images/{name}")
}

pub const SCHEDULE_PATH: &str = "schedule.selaschedule";
pub const LIBRARY_PATH: &str = "library.sqlite";
pub const SETTINGS_PATH: &str = "settings.txt";
pub const LOGO_PATH: &str = "logo.txt";

/// Whether `path` is where `role` belongs in a bundle of `kind`.
pub fn allowed(kind: Kind, role: Role, path: &str) -> bool {
    if !safe_name(path) {
        return false;
    }
    let image = |prefix: &str| {
        path.strip_prefix(prefix)
            .is_some_and(|name| !name.contains('/') && images::valid_name(name))
    };
    match (kind, role) {
        (Kind::Schedule, Role::Schedule) => path == SCHEDULE_PATH,
        (Kind::Schedule, Role::Image) => image("images/"),
        (Kind::Profile, Role::Library) => path == LIBRARY_PATH,
        (Kind::Profile, Role::Settings) => path == SETTINGS_PATH,
        (Kind::Profile, Role::Logo) => path == LOGO_PATH,
        (Kind::Profile, Role::Image) => image("Resources/Images/"),
        (Kind::Profile, Role::Schedule) => {
            path.starts_with("Schedules/")
                && path
                    .rsplit_once('.')
                    .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("selaschedule"))
        }
        (Kind::Profile, Role::Scripture) => path.strip_prefix("Scripture/").is_some_and(|name| {
            !name.contains('/')
                && name
                    .strip_suffix(".selabible")
                    .is_some_and(|id| !id.is_empty() && !id.starts_with('.'))
        }),
        _ => false,
    }
}

pub fn encode(manifest: &Manifest) -> Vec<u8> {
    let files: Vec<Value> = manifest
        .files
        .iter()
        .map(|f| {
            json!({
                "path": f.path,
                "size": f.size,
                "sha256": hex(&f.sha256),
                "role": f.role.code(),
            })
        })
        .collect();
    let images: Vec<Value> = manifest
        .images
        .iter()
        .map(|i| json!({"name": i.name, "sha256": hex(&i.sha256), "embedded": i.embedded}))
        .collect();
    let fonts: Vec<Value> = manifest
        .fonts
        .iter()
        .map(|f| json!({"family": f.family, "embedded": false, "installed": f.installed}))
        .collect();
    let omitted: Vec<Value> = manifest
        .omitted
        .iter()
        .map(|o| json!({"kind": "scripture", "title": o.title, "translation": o.translation, "reason": "restricted"}))
        .collect();
    let mut out = Map::new();
    out.insert("format".into(), FORMAT.into());
    out.insert("version".into(), VERSION.into());
    out.insert("kind".into(), manifest.kind.code().into());
    out.insert("generator".into(), manifest.generator.clone().into());
    out.insert("title".into(), manifest.title.clone().into());
    out.insert("files".into(), files.into());
    out.insert("images".into(), images.into());
    out.insert("fonts".into(), fonts.into());
    out.insert("omitted".into(), omitted.into());
    if let Some(schema) = manifest.library_schema {
        out.insert("library_schema".into(), schema.into());
    }
    let mut bytes = serde_json::to_vec_pretty(&Value::Object(out)).expect("plain JSON values");
    bytes.push(b'\n');
    bytes
}

fn string(value: &Value, key: &str, max: usize) -> Result<String, ManifestError> {
    let text = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(ManifestError::Malformed("a text field is missing"))?;
    if text.len() > max || text.chars().any(char::is_control) {
        return Err(ManifestError::Malformed(
            "a text field is too long or unusable",
        ));
    }
    Ok(text.to_owned())
}

fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a [Value], ManifestError> {
    let list = match object.get(key) {
        // Lists added later in version 1 may be absent.
        None => return Ok(&[]),
        Some(value) => value
            .as_array()
            .ok_or(ManifestError::Malformed("a list field is not a list"))?,
    };
    if list.len() > MAX_ENTRIES {
        return Err(ManifestError::Malformed("a list is too long"));
    }
    Ok(list)
}

fn sha(value: &Value) -> Result<[u8; 32], ManifestError> {
    value
        .get("sha256")
        .and_then(Value::as_str)
        .and_then(unhex32)
        .ok_or(ManifestError::Malformed(
            "a SHA-256 is missing or not 64 hex digits",
        ))
}

/// Parses and validates a manifest. The caller has bounded `bytes` by
/// `MAX_BYTES`. Unknown fields are ignored (a later version-1 writer may add
/// some); an unknown `version` is refused.
pub fn decode(bytes: &[u8]) -> Result<Manifest, ManifestError> {
    if bytes.len() > MAX_BYTES {
        return Err(ManifestError::Malformed("the manifest is too large"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| ManifestError::Malformed("the manifest is not JSON"))?;
    let object = value.as_object().ok_or(ManifestError::Malformed(
        "the manifest is not a JSON object",
    ))?;
    if object.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(ManifestError::Malformed("not a Sela bundle manifest"));
    }
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .ok_or(ManifestError::Malformed("the manifest has no version"))?;
    if version > VERSION {
        return Err(ManifestError::Newer(version));
    }
    if version == 0 {
        return Err(ManifestError::Malformed("the manifest has no version"));
    }
    let kind = match object.get("kind").and_then(Value::as_str) {
        Some("schedule") => Kind::Schedule,
        Some("profile") => Kind::Profile,
        _ => return Err(ManifestError::Malformed("the bundle kind is unknown")),
    };
    let generator = string(&value, "generator", MAX_TEXT)?;
    let title = string(&value, "title", MAX_TEXT)?;
    let mut files = Vec::new();
    let mut paths = std::collections::HashSet::new();
    for file in array(object, "files")? {
        let path = string(file, "path", super::archive::MAX_NAME)?;
        let role = file
            .get("role")
            .and_then(Value::as_str)
            .and_then(Role::from_code)
            .ok_or(ManifestError::Malformed("a file has no known role"))?;
        if path == NAME || !allowed(kind, role, &path) {
            return Err(ManifestError::Path(path));
        }
        if !paths.insert(path.to_lowercase()) {
            return Err(ManifestError::Duplicate(path));
        }
        let size = file
            .get("size")
            .and_then(Value::as_u64)
            .ok_or(ManifestError::Malformed("a file has no size"))?;
        files.push(FileRecord {
            path,
            size,
            sha256: sha(file)?,
            role,
        });
    }
    let count = |role: Role| files.iter().filter(|f| f.role == role).count();
    match kind {
        Kind::Schedule if count(Role::Schedule) != 1 => {
            return Err(ManifestError::Malformed(
                "a standalone schedule needs one schedule",
            ));
        }
        Kind::Profile if count(Role::Library) != 1 => {
            return Err(ManifestError::Malformed(
                "a profile backup needs one library",
            ));
        }
        _ => {}
    }
    let mut images = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for image in array(object, "images")? {
        let name = string(image, "name", MAX_TEXT)?;
        if !images::valid_name(&name) {
            return Err(ManifestError::Path(name));
        }
        let sha256 = sha(image)?;
        if !seen.insert((name.clone(), sha256)) {
            return Err(ManifestError::Duplicate(name));
        }
        let embedded = image
            .get("embedded")
            .and_then(Value::as_bool)
            .ok_or(ManifestError::Malformed("an image has no embedded flag"))?;
        // An embedded image is exactly one listed file with its bytes.
        if embedded {
            let path = image_path(&name);
            let file = files.iter().find(|f| f.path == path);
            if kind != Kind::Schedule
                || !file.is_some_and(|f| f.role == Role::Image && f.sha256 == sha256)
            {
                return Err(ManifestError::Path(path));
            }
        }
        images.push(ImageRecord {
            name,
            sha256,
            embedded,
        });
    }
    // Every embedded image file is described by an image record.
    for file in files
        .iter()
        .filter(|f| kind == Kind::Schedule && f.role == Role::Image)
    {
        let listed = images
            .iter()
            .any(|i| i.embedded && image_path(&i.name) == file.path && i.sha256 == file.sha256);
        if !listed {
            return Err(ManifestError::Path(file.path.clone()));
        }
    }
    let mut fonts = Vec::new();
    for font in array(object, "fonts")? {
        let family = string(font, "family", MAX_FONT_NAME)?;
        if family.trim().is_empty() {
            return Err(ManifestError::Malformed("a font has no family"));
        }
        // Version 1 never embeds fonts.
        if font.get("embedded").and_then(Value::as_bool) != Some(false) {
            return Err(ManifestError::Malformed("embedded fonts are not supported"));
        }
        let installed = match font.get("installed") {
            None | Some(Value::Null) => None,
            Some(flag) => Some(flag.as_bool().ok_or(ManifestError::Malformed(
                "a font's installed flag is not true or false",
            ))?),
        };
        fonts.push(FontRecord { family, installed });
    }
    let mut omitted = Vec::new();
    for item in array(object, "omitted")? {
        omitted.push(Omitted {
            title: string(item, "title", MAX_TEXT)?,
            translation: string(item, "translation", MAX_TEXT)?,
        });
    }
    let library_schema = match object.get("library_schema") {
        None => None,
        Some(schema) => Some(schema.as_i64().filter(|s| *s > 0).ok_or(
            ManifestError::Malformed("the library schema is not a number"),
        )?),
    };
    Ok(Manifest {
        kind,
        generator,
        title,
        files,
        images,
        fonts,
        omitted,
        library_schema,
    })
}
