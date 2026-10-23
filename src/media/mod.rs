//! Media backend: FFmpeg location and license check, startup availability,
//! ffprobe metadata and the supervised decoder (M2-01/M2-02). No GPUI types;
//! decisions in docs/media.md.
pub mod backend;
pub mod decode;
pub mod error;
pub mod ffmpeg;
pub mod probe;
pub mod process;

pub use error::MediaError;
