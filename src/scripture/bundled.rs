//! Translations shipped inside Sela: the public-domain KJV and WEB (M4-03).
//!
//! Each is a deflated UTF-8 bundle in `assets/scripture/`, built once from
//! the pinned public-domain source files by `examples/scripture_bundle.rs`
//! (provenance, licences and hashes are in docs/scripture.md). The bundle
//! is checked against its SHA-256 before use, inflated with a size cap and
//! turned into a `<profile>/Scripture/<ID>.selabible` file by the scripture
//! worker the first time the Scriptures tab opens.
//!
//! Bundle text, before deflate:
//!
//! ```text
//! sela-bible 1
//! id<TAB>KJV
//! name<TAB>King James Version
//! ...more key<TAB>value lines...
//! <empty line>
//! 43003016<TAB>For God so loved the world, ...
//! ```
use super::model::{MAX_META, TranslationInfo, VerseId, valid_id};
use super::store::{MAX_VERSE_BYTES, hex};
use sha2::{Digest, Sha256};
use std::borrow::Cow;

const MAGIC: &str = "sela-bible 1";
/// Largest inflated bundle accepted.
pub const MAX_INFLATED: usize = 16 * 1024 * 1024;

/// One shipped (or, in tests, synthetic) translation bundle.
#[derive(Clone, Debug)]
pub struct Bundle {
    pub id: &'static str,
    pub bytes: Cow<'static, [u8]>,
    /// Expected SHA-256 of `bytes`, lowercase hex.
    pub sha256: String,
}

const KJV: &[u8] = include_bytes!("../../assets/scripture/kjv.selabible.z");
const KJV_SHA256: &str = "b9ce7933e081f6d869d2c8f1c6f5395c2951f9efd0acbd2a2f6b96ecb5922e8d";
const WEB: &[u8] = include_bytes!("../../assets/scripture/web.selabible.z");
const WEB_SHA256: &str = "aaccbee472ce797f985c9587fcc284bcfe9f8b8f32e44a948a17b9cf36ba4686";

/// The bundles Sela ships, KJV first.
pub fn shipped() -> Vec<Bundle> {
    vec![
        Bundle {
            id: "KJV",
            bytes: Cow::Borrowed(KJV),
            sha256: KJV_SHA256.into(),
        },
        Bundle {
            id: "WEB",
            bytes: Cow::Borrowed(WEB),
            sha256: WEB_SHA256.into(),
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The bytes do not have the expected SHA-256.
    Checksum,
    Inflate,
    /// Inflated, but not a valid bundle; the string names the problem.
    Format(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Checksum => write!(f, "checksum mismatch"),
            Error::Inflate => write!(f, "cannot be decompressed"),
            Error::Format(what) => write!(f, "invalid bundle ({what})"),
        }
    }
}

pub fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// The deflated bundle for a translation.
pub fn encode(info: &TranslationInfo, verses: &[(VerseId, String)]) -> Vec<u8> {
    let mut text = String::from(MAGIC);
    text.push('\n');
    let flag = |b: bool| if b { "1" } else { "0" };
    for (key, value) in [
        ("id", info.id.as_str()),
        ("name", &info.name),
        ("language", &info.language),
        ("rights", &info.rights),
        ("source", &info.source),
        ("redistributable", flag(info.redistributable)),
    ] {
        text.push_str(key);
        text.push('\t');
        text.push_str(&value.replace(['\t', '\n'], " "));
        text.push('\n');
    }
    text.push('\n');
    for (id, verse) in verses {
        text.push_str(&format!(
            "{}\t{}\n",
            id.key(),
            verse.replace(['\t', '\n'], " ")
        ));
    }
    miniz_oxide::deflate::compress_to_vec(text.as_bytes(), 9)
}

/// Checks, inflates and parses a bundle. The result is marked `bundled`.
pub fn decode(bundle: &Bundle) -> Result<(TranslationInfo, Vec<(VerseId, String)>), Error> {
    if sha256(&bundle.bytes) != bundle.sha256 {
        return Err(Error::Checksum);
    }
    let inflated = miniz_oxide::inflate::decompress_to_vec_with_limit(&bundle.bytes, MAX_INFLATED)
        .map_err(|_| Error::Inflate)?;
    let text = std::str::from_utf8(&inflated).map_err(|_| Error::Format("UTF-8".into()))?;
    let mut lines = text.lines();
    if lines.next() != Some(MAGIC) {
        return Err(Error::Format("header".into()));
    }
    let mut info = TranslationInfo {
        id: String::new(),
        name: String::new(),
        language: String::new(),
        rights: String::new(),
        source: String::new(),
        redistributable: false,
        bundled: true,
    };
    for line in lines.by_ref() {
        if line.is_empty() {
            break;
        }
        let (key, value) = line
            .split_once('\t')
            .ok_or_else(|| Error::Format("metadata line".into()))?;
        if value.len() > MAX_META {
            return Err(Error::Format("metadata length".into()));
        }
        let value = value.to_owned();
        match key {
            "id" => info.id = value,
            "name" => info.name = value,
            "language" => info.language = value,
            "rights" => info.rights = value,
            "source" => info.source = value,
            "redistributable" => info.redistributable = value == "1",
            _ => {}
        }
    }
    if !valid_id(&info.id) || info.id != bundle.id {
        return Err(Error::Format("id".into()));
    }
    let mut verses: Vec<(VerseId, String)> = Vec::new();
    for line in lines {
        let (key, verse) = line
            .split_once('\t')
            .ok_or_else(|| Error::Format("verse line".into()))?;
        let id = key
            .parse::<i64>()
            .ok()
            .and_then(VerseId::from_key)
            .ok_or_else(|| Error::Format(format!("verse key {key}")))?;
        if verse.is_empty() || verse.len() > MAX_VERSE_BYTES {
            return Err(Error::Format(format!("verse text {key}")));
        }
        if verses.last().is_some_and(|(last, _)| *last >= id) {
            return Err(Error::Format(format!("verse order {key}")));
        }
        verses.push((id, verse.to_owned()));
    }
    if verses.is_empty() {
        return Err(Error::Format("no verses".into()));
    }
    Ok((info, verses))
}

/// A bundle built in memory, for tests and the bundling tool.
pub fn synthetic(id: &'static str, info: &TranslationInfo, verses: &[(VerseId, String)]) -> Bundle {
    let bytes = encode(info, verses);
    Bundle {
        id,
        sha256: sha256(&bytes),
        bytes: Cow::Owned(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripture::store::tests::{info, v};

    #[test]
    fn a_bundle_round_trips_and_any_flipped_byte_is_refused() {
        let verses = vec![
            (v(1, 1, 1), "Synthetic one".into()),
            (v(1, 1, 2), "Two".into()),
        ];
        let bundle = synthetic("SYN", &info("SYN"), &verses);
        let (decoded, read) = decode(&bundle).unwrap();
        assert_eq!(read, verses);
        assert_eq!(decoded.id, "SYN");
        assert!(decoded.bundled && decoded.redistributable);
        let mut damaged = bundle.clone();
        damaged.bytes.to_mut()[3] ^= 1;
        assert_eq!(decode(&damaged), Err(Error::Checksum));
        let mut wrong = bundle.clone();
        wrong.id = "OTHER";
        assert_eq!(decode(&wrong), Err(Error::Format("id".into())));
        let mut garbage = Bundle {
            id: "SYN",
            bytes: Cow::Owned(vec![1, 2, 3]),
            sha256: String::new(),
        };
        garbage.sha256 = sha256(&garbage.bytes);
        assert_eq!(decode(&garbage), Err(Error::Inflate));
    }

    #[test]
    fn the_shipped_bundles_are_intact_public_domain_texts() {
        for (bundle, count) in shipped().iter().zip([31_102, 31_098]) {
            let (info, verses) = decode(bundle).unwrap();
            assert_eq!(info.id, bundle.id);
            assert!(info.redistributable && info.bundled);
            assert!(info.rights.to_lowercase().contains("public domain"));
            assert_eq!(verses.len(), count, "{} verse count", info.id);
            let books: std::collections::BTreeSet<_> =
                verses.iter().map(|(id, _)| id.book).collect();
            assert_eq!(books.len(), 66, "{}", info.id);
        }
        let (_, kjv) = decode(&shipped()[0]).unwrap();
        let john = kjv.iter().find(|(id, _)| id.key() == 43_003_016).unwrap();
        assert!(john.1.starts_with("For God so loved the world"));
    }
}
