//! The strict ZIP subset every Sela bundle uses (docs/bundle.md
//! "Container"). Any unzip tool can list and extract a bundle; Sela itself
//! reads only what it writes:
//!
//! - one disk, no ZIP64, no encryption, no data descriptors, no comment
//!   requirements, methods 0 (stored) and 8 (deflate) only;
//! - UTF-8 names (flag bit 11, or plain ASCII) that pass [`safe_name`]: no
//!   absolute or drive paths, no `..`, `.` or empty components, no
//!   backslashes, colons, control or Windows-reserved characters and names,
//!   at most `MAX_DEPTH` components; no directory or symbolic-link entries;
//! - no two names equal ignoring case, and no file name that is also another
//!   entry's folder (both collide on Windows and macOS volumes);
//! - every local header agrees with its central record, entries do not
//!   overlap each other or the central directory (the overlapping-entry zip
//!   bomb), the central directory ends exactly at the end record;
//! - declared sizes are bounded by [`Limits`] before anything is read, and
//!   inflating stops with an error the moment output passes the declared size;
//!   the CRC-32 and the declared size are checked for every entry.
//!
//! Blocking file I/O: bundle worker threads only.
use crc32fast::Hasher as Crc;
use miniz_oxide::{
    DataFormat, MZError, MZFlush, MZStatus,
    deflate::core::{CompressorOxide, create_comp_flags_from_zip_params},
    inflate::stream::InflateState,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
};

const LOCAL_SIG: u32 = 0x0403_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const END_SIG: u32 = 0x0605_4b50;
const LOCAL_LEN: usize = 30;
const CENTRAL_LEN: usize = 46;
const END_LEN: usize = 22;
const UTF8_FLAG: u16 = 1 << 11;
/// Version 2.0: deflate, no ZIP64.
const VERSION_NEEDED: u16 = 20;
/// Made by "Unix" 2.0, so the external attributes carry a regular-file mode.
const VERSION_MADE_BY: u16 = (3 << 8) | 20;
/// `-rw-r--r--` regular file in the high half of the external attributes.
const REGULAR_FILE: u32 = 0o100_644 << 16;
/// 1980-01-01 00:00: bundles carry no file times (reproducible output).
const DOS_DATE: u16 = (1 << 5) | 1;
const STORED: u16 = 0;
const DEFLATED: u16 = 8;
const CHUNK: usize = 256 * 1024;
/// Every offset and size must fit the 32-bit fields: no ZIP64.
pub const MAX_ARCHIVE_BYTES: u64 = 0xFFFF_FFFE;
/// The format allows 65535 entries without ZIP64.
pub const MAX_ENTRIES: usize = 65_535;
/// Bytes of one entry name.
pub const MAX_NAME: usize = 240;
/// Path components of one entry name.
pub const MAX_DEPTH: usize = 8;
/// Bound on the central directory read into memory.
const MAX_CENTRAL_BYTES: u64 = 16 * 1024 * 1024;

/// What one archive may declare; checked before any entry is extracted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub entries: usize,
    /// Uncompressed bytes of the largest entry.
    pub entry_bytes: u64,
    /// Uncompressed bytes of all entries together.
    pub total_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArchiveError {
    Io(io::ErrorKind),
    /// No ZIP end record: not a bundle at all.
    NotArchive,
    /// Structurally invalid; says what.
    Malformed(&'static str),
    /// A ZIP feature Sela does not accept (encryption, ZIP64, multi-part,
    /// another compression method, a link or folder entry).
    Unsupported(&'static str),
    /// A name that could escape the destination or is unusable on some
    /// system (absolute, `..`, drive letter, reserved name, ...).
    UnsafeName(String),
    /// Two entries whose names collide (equal ignoring case, or a file that
    /// is also a folder of another entry).
    DuplicateName(String),
    TooManyEntries,
    /// An entry (by name) or the whole archive (empty name) is larger than
    /// allowed, or inflates past its declared size.
    TooLarge(String),
    /// The entry's bytes do not match its CRC-32.
    Checksum(String),
    Cancelled,
}

impl From<io::Error> for ArchiveError {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            Self::Malformed("the archive is truncated")
        } else {
            Self::Io(error.kind())
        }
    }
}

/// Windows device names, refused with or without an extension.
const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// A relative `/`-separated path that extracts to the same place, and stays
/// inside the destination, on Windows, macOS and Linux.
pub fn safe_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_NAME || name.starts_with('/') {
        return false;
    }
    let components: Vec<&str> = name.split('/').collect();
    components.len() <= MAX_DEPTH && components.iter().all(|c| safe_component(c))
}

fn safe_component(component: &str) -> bool {
    if component.is_empty() || component == "." || component == ".." {
        return false;
    }
    // Windows drops trailing dots and spaces, so "a." and "a" would collide.
    if component.ends_with(['.', ' ']) || component.starts_with(' ') {
        return false;
    }
    if component
        .chars()
        .any(|c| c.is_control() || matches!(c, '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return false;
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    !RESERVED.contains(&stem.trim_end())
}

/// Case-insensitive collision check over every name and its folders.
#[derive(Default)]
struct Names {
    files: HashSet<String>,
    folders: HashSet<String>,
}

impl Names {
    fn insert(&mut self, name: &str) -> Result<(), ArchiveError> {
        let key = name.to_lowercase();
        let duplicate = || ArchiveError::DuplicateName(name.to_owned());
        if self.files.contains(&key) || self.folders.contains(&key) {
            return Err(duplicate());
        }
        let mut folder = String::new();
        let parts: Vec<&str> = key.split('/').collect();
        for part in &parts[..parts.len() - 1] {
            if !folder.is_empty() {
                folder.push('/');
            }
            folder.push_str(part);
            if self.files.contains(&folder) {
                return Err(duplicate());
            }
        }
        let mut folder = String::new();
        for part in &parts[..parts.len() - 1] {
            if !folder.is_empty() {
                folder.push('/');
            }
            folder.push_str(part);
            self.folders.insert(folder.clone());
        }
        self.files.insert(key);
        Ok(())
    }
}

/// Size and SHA-256 of an entry written or extracted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Digested {
    pub size: u64,
    pub sha256: [u8; 32],
}

/// Writes a new archive front to back. The caller owns publication: write to
/// a temporary file, `finish`, then rename or link it into place.
pub struct Writer {
    file: File,
    offset: u64,
    central: Vec<u8>,
    count: usize,
    names: Names,
}

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn fit32(value: u64) -> Result<u32, ArchiveError> {
    u32::try_from(value)
        .ok()
        .filter(|v| u64::from(*v) <= MAX_ARCHIVE_BYTES)
        .ok_or_else(|| ArchiveError::TooLarge(String::new()))
}

impl Writer {
    pub fn new(file: File) -> Self {
        Self {
            file,
            offset: 0,
            central: Vec::new(),
            count: 0,
            names: Names::default(),
        }
    }

    /// Streams `source` (at most `max` bytes) into a new entry. `deflate`
    /// compresses it; already compressed media should be stored.
    pub fn add(
        &mut self,
        name: &str,
        deflate: bool,
        source: &mut dyn Read,
        max: u64,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Digested, ArchiveError> {
        if !safe_name(name) {
            return Err(ArchiveError::UnsafeName(name.to_owned()));
        }
        if self.count >= MAX_ENTRIES {
            return Err(ArchiveError::TooManyEntries);
        }
        self.names.insert(name)?;
        let start = fit32(self.offset)?;
        let method = if deflate { DEFLATED } else { STORED };
        let mut header = Vec::with_capacity(LOCAL_LEN + name.len());
        put32(&mut header, LOCAL_SIG);
        put16(&mut header, VERSION_NEEDED);
        put16(&mut header, UTF8_FLAG);
        put16(&mut header, method);
        put16(&mut header, 0);
        put16(&mut header, DOS_DATE);
        // CRC and sizes are patched in once the data is written.
        put32(&mut header, 0);
        put32(&mut header, 0);
        put32(&mut header, 0);
        put16(&mut header, name.len() as u16);
        put16(&mut header, 0);
        header.extend_from_slice(name.as_bytes());
        self.file.write_all(&header)?;
        let mut crc = Crc::new();
        let mut sha = Sha256::new();
        let mut size = 0u64;
        let mut compressed = 0u64;
        let mut input = vec![0u8; CHUNK];
        let mut output = vec![0u8; CHUNK];
        let mut compressor = deflate.then(|| {
            Box::new(CompressorOxide::new(create_comp_flags_from_zip_params(
                6, -15, 0,
            )))
        });
        loop {
            if cancel() {
                return Err(ArchiveError::Cancelled);
            }
            let n = match source.read(&mut input) {
                Ok(n) => n,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(ArchiveError::Io(error.kind())),
            };
            if n == 0 {
                break;
            }
            size += n as u64;
            if size > max || size > MAX_ARCHIVE_BYTES {
                return Err(ArchiveError::TooLarge(name.to_owned()));
            }
            crc.update(&input[..n]);
            sha.update(&input[..n]);
            match compressor.as_deref_mut() {
                Some(c) => {
                    compressed +=
                        deflate_into(c, &input[..n], MZFlush::None, &mut output, &mut self.file)?;
                }
                None => {
                    self.file.write_all(&input[..n])?;
                    compressed += n as u64;
                }
            }
        }
        if let Some(c) = compressor.as_deref_mut() {
            compressed += deflate_into(c, &[], MZFlush::Finish, &mut output, &mut self.file)?;
        }
        let crc = crc.finalize();
        let (csize, usize) = (fit32(compressed)?, fit32(size)?);
        self.offset += (LOCAL_LEN + name.len()) as u64 + compressed;
        fit32(self.offset)?;
        self.file.seek(SeekFrom::Start(u64::from(start) + 14))?;
        let mut patch = Vec::with_capacity(12);
        put32(&mut patch, crc);
        put32(&mut patch, csize);
        put32(&mut patch, usize);
        self.file.write_all(&patch)?;
        self.file.seek(SeekFrom::End(0))?;
        let central = &mut self.central;
        put32(central, CENTRAL_SIG);
        put16(central, VERSION_MADE_BY);
        put16(central, VERSION_NEEDED);
        put16(central, UTF8_FLAG);
        put16(central, method);
        put16(central, 0);
        put16(central, DOS_DATE);
        put32(central, crc);
        put32(central, csize);
        put32(central, usize);
        put16(central, name.len() as u16);
        put16(central, 0);
        put16(central, 0);
        put16(central, 0);
        put16(central, 0);
        put32(central, REGULAR_FILE);
        put32(central, start);
        central.extend_from_slice(name.as_bytes());
        self.count += 1;
        Ok(Digested {
            size,
            sha256: sha.finalize().into(),
        })
    }

    pub fn add_bytes(
        &mut self,
        name: &str,
        deflate: bool,
        bytes: &[u8],
    ) -> Result<Digested, ArchiveError> {
        self.add(name, deflate, &mut &bytes[..], bytes.len() as u64, &|| {
            false
        })
    }

    /// Writes the central directory and end record and flushes the file to
    /// disk. Returns the file for the caller to publish.
    pub fn finish(mut self) -> Result<File, ArchiveError> {
        let cd_offset = fit32(self.offset)?;
        let cd_size = fit32(self.central.len() as u64)?;
        fit32(self.offset + self.central.len() as u64 + END_LEN as u64)?;
        let mut end = std::mem::take(&mut self.central);
        put32(&mut end, END_SIG);
        put16(&mut end, 0);
        put16(&mut end, 0);
        put16(&mut end, self.count as u16);
        put16(&mut end, self.count as u16);
        put32(&mut end, cd_size);
        put32(&mut end, cd_offset);
        put16(&mut end, 0);
        self.file.write_all(&end)?;
        self.file.sync_all()?;
        Ok(self.file)
    }
}

/// Compresses `input` (or finishes the stream) and writes what comes out.
fn deflate_into(
    compressor: &mut CompressorOxide,
    mut input: &[u8],
    flush: MZFlush,
    output: &mut [u8],
    file: &mut File,
) -> Result<u64, ArchiveError> {
    let mut written = 0u64;
    loop {
        let result = miniz_oxide::deflate::stream::deflate(compressor, input, output, flush);
        let status = result
            .status
            .map_err(|_| ArchiveError::Malformed("compression failed"))?;
        file.write_all(&output[..result.bytes_written])?;
        written += result.bytes_written as u64;
        input = &input[result.bytes_consumed..];
        let done = match flush {
            MZFlush::Finish => status == MZStatus::StreamEnd,
            _ => input.is_empty(),
        };
        if done {
            return Ok(written);
        }
    }
}

/// One entry as the central directory declares it, after every check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// Declared uncompressed size.
    pub size: u64,
    compressed: u64,
    crc: u32,
    method: u16,
    /// Offset of the entry's data, after its local header.
    data: u64,
}

/// An opened and structurally validated archive. Entry data is checked when
/// it is extracted.
pub struct Archive {
    file: File,
    entries: Vec<Entry>,
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn read_at(file: &mut File, offset: u64, len: usize) -> Result<Vec<u8>, ArchiveError> {
    let mut buffer = vec![0u8; len];
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(&mut buffer)?;
    Ok(buffer)
}

/// Shared checks of the general-purpose flags and method.
fn check_flags(flags: u16, method: u16) -> Result<(), ArchiveError> {
    if flags & 0x0041 != 0 || flags & (1 << 13) != 0 {
        return Err(ArchiveError::Unsupported("encrypted entries"));
    }
    if flags & (1 << 3) != 0 {
        return Err(ArchiveError::Unsupported("data descriptors"));
    }
    if flags & !(UTF8_FLAG | 0x0006) != 0 {
        return Err(ArchiveError::Unsupported("unknown entry flags"));
    }
    if method != STORED && method != DEFLATED {
        return Err(ArchiveError::Unsupported(
            "compression methods other than deflate",
        ));
    }
    Ok(())
}

fn decode_name(bytes: &[u8], flags: u16) -> Result<String, ArchiveError> {
    let lossy = || String::from_utf8_lossy(bytes).into_owned();
    // Without the UTF-8 flag a name is CP437; only its ASCII subset is
    // unambiguous.
    if flags & UTF8_FLAG == 0 && !bytes.is_ascii() {
        return Err(ArchiveError::UnsafeName(lossy()));
    }
    let name = std::str::from_utf8(bytes).map_err(|_| ArchiveError::UnsafeName(lossy()))?;
    if !safe_name(name) {
        return Err(ArchiveError::UnsafeName(name.to_owned()));
    }
    Ok(name.to_owned())
}

/// Extra fields must be well formed; a ZIP64 record is refused.
fn check_extra(extra: &[u8]) -> Result<(), ArchiveError> {
    let mut at = 0;
    while at < extra.len() {
        if extra.len() - at < 4 {
            return Err(ArchiveError::Malformed("an extra field is cut short"));
        }
        let id = u16_at(extra, at);
        let len = usize::from(u16_at(extra, at + 2));
        if id == 0x0001 {
            return Err(ArchiveError::Unsupported("ZIP64"));
        }
        at += 4 + len;
        if at > extra.len() {
            return Err(ArchiveError::Malformed("an extra field is cut short"));
        }
    }
    Ok(())
}

impl Archive {
    pub fn open(mut file: File, limits: Limits) -> Result<Self, ArchiveError> {
        let len = file.metadata()?.len();
        if len < END_LEN as u64 {
            return Err(ArchiveError::NotArchive);
        }
        if len > MAX_ARCHIVE_BYTES {
            return Err(ArchiveError::TooLarge(String::new()));
        }
        let tail_len = len.min((END_LEN + 0xFFFF) as u64) as usize;
        let tail = read_at(&mut file, len - tail_len as u64, tail_len)?;
        let at = (0..=tail_len - END_LEN)
            .rev()
            .find(|&at| {
                u32_at(&tail, at) == END_SIG
                    && at + END_LEN + usize::from(u16_at(&tail, at + 20)) == tail_len
            })
            .ok_or(ArchiveError::NotArchive)?;
        let end_offset = len - (tail_len - at) as u64;
        let end = &tail[at..at + END_LEN];
        let (disk, cd_disk) = (u16_at(end, 4), u16_at(end, 6));
        let (on_disk, total) = (u16_at(end, 8), u16_at(end, 10));
        let (cd_size, cd_offset) = (u32_at(end, 12), u32_at(end, 16));
        if total == 0xFFFF || cd_size == 0xFFFF_FFFF || cd_offset == 0xFFFF_FFFF {
            return Err(ArchiveError::Unsupported("ZIP64"));
        }
        if disk != 0 || cd_disk != 0 || on_disk != total {
            return Err(ArchiveError::Unsupported("multi-part archives"));
        }
        let count = usize::from(total);
        if count > limits.entries.min(MAX_ENTRIES) {
            return Err(ArchiveError::TooManyEntries);
        }
        if u64::from(cd_offset) + u64::from(cd_size) != end_offset {
            return Err(ArchiveError::Malformed(
                "the central directory is not where the end record says",
            ));
        }
        if u64::from(cd_size) > MAX_CENTRAL_BYTES {
            return Err(ArchiveError::TooManyEntries);
        }
        let central = read_at(&mut file, u64::from(cd_offset), cd_size as usize)?;
        let mut entries = Vec::with_capacity(count);
        let mut names = Names::default();
        let mut ranges = Vec::with_capacity(count);
        let mut total_bytes = 0u64;
        let mut at = 0usize;
        for _ in 0..count {
            if central.len() - at < CENTRAL_LEN || u32_at(&central, at) != CENTRAL_SIG {
                return Err(ArchiveError::Malformed(
                    "a central directory record is damaged",
                ));
            }
            let record = &central[at..];
            let made_by = u16_at(record, 4);
            let needed = u16_at(record, 6);
            let flags = u16_at(record, 8);
            let method = u16_at(record, 10);
            let crc = u32_at(record, 16);
            let compressed = u64::from(u32_at(record, 20));
            let size = u64::from(u32_at(record, 24));
            let name_len = usize::from(u16_at(record, 28));
            let extra_len = usize::from(u16_at(record, 30));
            let comment_len = usize::from(u16_at(record, 32));
            let start_disk = u16_at(record, 34);
            let external = u32_at(record, 38);
            let local = u64::from(u32_at(record, 42));
            let record_len = CENTRAL_LEN + name_len + extra_len + comment_len;
            if record.len() < record_len {
                return Err(ArchiveError::Malformed(
                    "a central directory record is damaged",
                ));
            }
            if needed > 45 {
                return Err(ArchiveError::Unsupported("ZIP features newer than 4.5"));
            }
            if compressed == 0xFFFF_FFFF || size == 0xFFFF_FFFF || local == 0xFFFF_FFFF {
                return Err(ArchiveError::Unsupported("ZIP64"));
            }
            if start_disk != 0 {
                return Err(ArchiveError::Unsupported("multi-part archives"));
            }
            check_flags(flags, method)?;
            let name = decode_name(&record[CENTRAL_LEN..CENTRAL_LEN + name_len], flags)?;
            check_extra(&record[CENTRAL_LEN + name_len..CENTRAL_LEN + name_len + extra_len])?;
            // A Unix mode, when given, must be a regular file: no links,
            // folders or devices. The MS-DOS folder bit is refused too.
            // Permission bits alone (as Python's zipfile writes) are fine.
            let file_type = (external >> 16) & 0o170_000;
            if made_by >> 8 == 3 && file_type != 0 && file_type != 0o100_000 {
                return Err(ArchiveError::Unsupported("links, folders or special files"));
            }
            if external & 0x10 != 0 {
                return Err(ArchiveError::Unsupported("links, folders or special files"));
            }
            if method == STORED && compressed != size {
                return Err(ArchiveError::Malformed("a stored entry has two sizes"));
            }
            if size > limits.entry_bytes {
                return Err(ArchiveError::TooLarge(name));
            }
            total_bytes += size;
            if total_bytes > limits.total_bytes {
                return Err(ArchiveError::TooLarge(String::new()));
            }
            names.insert(&name)?;
            // The local header must repeat the central record.
            if local + LOCAL_LEN as u64 > u64::from(cd_offset) {
                return Err(ArchiveError::Malformed(
                    "an entry points past the archive data",
                ));
            }
            let header = read_at(&mut file, local, LOCAL_LEN)?;
            let local_name_len = usize::from(u16_at(&header, 26));
            let local_extra_len = usize::from(u16_at(&header, 28));
            if u32_at(&header, 0) != LOCAL_SIG
                || u16_at(&header, 6) != flags
                || u16_at(&header, 8) != method
                || u32_at(&header, 14) != crc
                || u64::from(u32_at(&header, 18)) != compressed
                || u64::from(u32_at(&header, 22)) != size
                || local_name_len != name_len
            {
                return Err(ArchiveError::Malformed(
                    "a local header disagrees with the central directory",
                ));
            }
            let data = local + (LOCAL_LEN + local_name_len + local_extra_len) as u64;
            if data + compressed > u64::from(cd_offset) {
                return Err(ArchiveError::Malformed(
                    "an entry points past the archive data",
                ));
            }
            let local_names = read_at(
                &mut file,
                local + LOCAL_LEN as u64,
                local_name_len + local_extra_len,
            )?;
            if &local_names[..name_len] != name.as_bytes() {
                return Err(ArchiveError::Malformed(
                    "a local header disagrees with the central directory",
                ));
            }
            check_extra(&local_names[name_len..])?;
            ranges.push((local, data + compressed));
            entries.push(Entry {
                name,
                size,
                compressed,
                crc,
                method,
                data,
            });
            at += record_len;
        }
        if at != central.len() {
            return Err(ArchiveError::Malformed(
                "the central directory has more records than the end record says",
            ));
        }
        // Overlapping entries would let a small archive inflate many times.
        ranges.sort_unstable();
        if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err(ArchiveError::Malformed("two entries share the same data"));
        }
        Ok(Self { file, entries })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.name == name)
    }

    /// Writes the entry's bytes to `out`, verifying its declared size and
    /// CRC-32; returns its SHA-256. Data past the declared size is an error
    /// as soon as it appears, so nothing larger than declared is ever written.
    pub fn extract(
        &mut self,
        index: usize,
        out: &mut dyn Write,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Digested, ArchiveError> {
        let entry = self.entries[index].clone();
        self.file.seek(SeekFrom::Start(entry.data))?;
        let mut crc = Crc::new();
        let mut sha = Sha256::new();
        let mut produced = 0u64;
        let mut emit = |bytes: &[u8], produced: &mut u64| -> Result<(), ArchiveError> {
            *produced += bytes.len() as u64;
            if *produced > entry.size {
                return Err(ArchiveError::TooLarge(entry.name.clone()));
            }
            crc.update(bytes);
            sha.update(bytes);
            out.write_all(bytes).map_err(|e| ArchiveError::Io(e.kind()))
        };
        let mut input = vec![0u8; CHUNK];
        let mut remaining = entry.compressed;
        if entry.method == STORED {
            while remaining > 0 {
                if cancel() {
                    return Err(ArchiveError::Cancelled);
                }
                let n = remaining.min(CHUNK as u64) as usize;
                self.file.read_exact(&mut input[..n])?;
                remaining -= n as u64;
                emit(&input[..n], &mut produced)?;
            }
        } else {
            let mut state = InflateState::new_boxed(DataFormat::Raw);
            let mut output = vec![0u8; CHUNK];
            let (mut start, mut end) = (0usize, 0usize);
            let mut stalls = 0;
            loop {
                if cancel() {
                    return Err(ArchiveError::Cancelled);
                }
                if start == end && remaining > 0 {
                    let n = remaining.min(CHUNK as u64) as usize;
                    self.file.read_exact(&mut input[..n])?;
                    remaining -= n as u64;
                    (start, end) = (0, n);
                }
                let result = miniz_oxide::inflate::stream::inflate(
                    &mut state,
                    &input[start..end],
                    &mut output,
                    MZFlush::None,
                );
                start += result.bytes_consumed;
                emit(&output[..result.bytes_written], &mut produced)?;
                let progress = result.bytes_consumed > 0 || result.bytes_written > 0;
                match result.status {
                    Ok(MZStatus::StreamEnd) => break,
                    Ok(_) => {}
                    Err(MZError::Buf) if start == end && remaining == 0 => {
                        return Err(ArchiveError::Malformed("compressed data is cut short"));
                    }
                    Err(MZError::Buf) => {}
                    Err(_) => return Err(ArchiveError::Malformed("compressed data is damaged")),
                }
                stalls = if progress { 0 } else { stalls + 1 };
                if stalls > 2 {
                    return Err(ArchiveError::Malformed("compressed data is damaged"));
                }
            }
            if start != end || remaining != 0 {
                return Err(ArchiveError::Malformed(
                    "an entry has data after its compressed stream",
                ));
            }
        }
        if produced != entry.size {
            return Err(ArchiveError::Malformed("an entry is shorter than declared"));
        }
        if crc.finalize() != entry.crc {
            return Err(ArchiveError::Checksum(entry.name));
        }
        Ok(Digested {
            size: produced,
            sha256: sha.finalize().into(),
        })
    }

    /// `extract` into memory; for entries already bounded by `Limits`.
    pub fn read(
        &mut self,
        index: usize,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(Vec<u8>, Digested), ArchiveError> {
        let mut bytes = Vec::with_capacity(self.entries[index].size as usize);
        let digest = self.extract(index, &mut bytes, cancel)?;
        Ok((bytes, digest))
    }
}
