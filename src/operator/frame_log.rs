//! Opt-in frame pacing log for native performance checks. With
//! `SELA_FRAME_LOG=<path>` set, every operator render appends
//! `<ms since start> <ms since the previous render>` to that file. Lines are
//! buffered and written in batches, so the file is only a diagnostic and
//! never read by the app.

use std::{
    fmt::Write as _,
    io::Write as _,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const BATCH: usize = 128;

pub(crate) struct FrameLog {
    path: PathBuf,
    rows: Duration,
    last: Option<Instant>,
    lines: String,
    pending: usize,
}

impl FrameLog {
    pub(crate) fn from_env() -> Option<Self> {
        let path = PathBuf::from(std::env::var_os("SELA_FRAME_LOG")?);
        Some(Self {
            path,
            rows: Duration::ZERO,
            last: None,
            lines: String::new(),
            pending: 0,
        })
    }

    pub(crate) fn tick(&mut self) {
        let now = Instant::now();
        let gap = self
            .last
            .replace(now)
            .map_or(Duration::ZERO, |last| now - last);
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0., |d| d.as_secs_f64() * 1000.);
        let _ = writeln!(
            self.lines,
            "{unix:.3} {:.3} {:.3}",
            gap.as_secs_f64() * 1000.,
            std::mem::take(&mut self.rows).as_secs_f64() * 1000.
        );
        self.pending += 1;
        if self.pending >= BATCH {
            self.flush();
        }
    }

    pub(crate) fn add_rows(&mut self, spent: Duration) {
        self.rows += spent;
    }

    fn flush(&mut self) {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = file.write_all(self.lines.as_bytes());
        }
        self.lines.clear();
        self.pending = 0;
    }
}

impl Drop for FrameLog {
    fn drop(&mut self) {
        self.flush();
    }
}
