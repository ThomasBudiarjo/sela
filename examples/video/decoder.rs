//! Diagnostic child isolation: polling and drop never join workers.
use std::{
    io::{self, Read},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TrySendError},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    Bounds,
    Input,
    Spawn,
    Read,
    Partial,
    Decoder,
    Cancelled,
    Deadline,
}
#[derive(Debug)]
pub struct Frame {
    pub index: usize,
    pub rgba: Vec<u8>,
}
pub struct Decoder {
    cancel: Arc<AtomicBool>,
    saturated: Arc<AtomicBool>,
    frames: Receiver<Frame>,
    done: Receiver<Result<(), Failure>>,
}
pub fn bytes(w: u32, h: u32, frames: usize) -> Result<usize, Failure> {
    if w == 0 || h == 0 || w > 1920 || h > 1080 || !(1..=120).contains(&frames) {
        return Err(Failure::Bounds);
    }
    (w as usize)
        .checked_mul(h as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or(Failure::Bounds)
}
fn read_frame(r: &mut impl Read, size: usize) -> Result<Option<Vec<u8>>, Failure> {
    let mut data = vec![0; size];
    let mut at = 0;
    while at < size {
        match r.read(&mut data[at..]) {
            Ok(0) if at == 0 => return Ok(None),
            Ok(0) => return Err(Failure::Partial),
            Ok(n) => at += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Err(Failure::Read),
        }
    }
    Ok(Some(data))
}
impl Decoder {
    pub fn start(
        path: PathBuf,
        w: u32,
        h: u32,
        count: usize,
        deadline: Duration,
    ) -> Result<Self, Failure> {
        let size = bytes(w, h, count)?;
        if deadline.is_zero() || deadline > Duration::from_secs(30) {
            return Err(Failure::Bounds);
        }
        let mut command = Command::new("ffmpeg");
        command
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-xerror",
                "-protocol_whitelist",
                "file",
                "-f",
                "matroska",
                "-c:v",
                "ffv1",
                "-threads",
                "1",
                "-i",
            ])
            .arg(&path)
            .args([
                "-map",
                "0:v:0",
                "-an",
                "-sn",
                "-dn",
                "-filter_threads",
                "1",
                "-vf",
            ])
            .arg(format!("scale={w}:{h}:flags=neighbor,format=rgba"))
            .args(["-threads", "1", "-frames:v"])
            .arg(count.to_string())
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"]);
        Ok(Self::launch(command, Some(path), size, count, deadline))
    }
    fn launch(
        mut command: Command,
        path: Option<PathBuf>,
        size: usize,
        count: usize,
        deadline: Duration,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let saturated = Arc::new(AtomicBool::new(false));
        let reader_saturated = saturated.clone();
        let (tx, frames) = mpsc::sync_channel(1);
        let (finish, done) = mpsc::channel();
        thread::spawn(move || {
            let result = (|| {
                if let Some(path) = path {
                    let m = std::fs::metadata(path).map_err(|_| Failure::Input)?;
                    if !m.is_file() || m.len() > 32 * 1024 * 1024 {
                        return Err(Failure::Input);
                    }
                }
                let mut child = command
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|_| Failure::Spawn)?;
                let mut stdout = child.stdout.take().expect("piped");
                let reader_stop = stop.clone();
                let reader = thread::spawn(move || {
                    for index in 0..count {
                        if reader_stop.load(Ordering::Acquire) {
                            return Err(Failure::Cancelled);
                        }
                        let rgba = read_frame(&mut stdout, size)?.ok_or(Failure::Partial)?;
                        let mut frame = Frame { index, rgba };
                        loop {
                            if reader_stop.load(Ordering::Acquire) {
                                return Err(Failure::Cancelled);
                            }
                            match tx.try_send(frame) {
                                Ok(()) => break,
                                Err(TrySendError::Full(f)) => {
                                    reader_saturated.store(true, Ordering::Release);
                                    frame = f;
                                    thread::sleep(Duration::from_millis(1));
                                }
                                Err(TrySendError::Disconnected(_)) => {
                                    return Err(Failure::Cancelled);
                                }
                            }
                        }
                    }
                    Ok(())
                });
                let start = Instant::now();
                let mut failure = None;
                while !reader.is_finished() {
                    if stop.load(Ordering::Acquire) {
                        failure = Some(Failure::Cancelled);
                        break;
                    }
                    if start.elapsed() >= deadline {
                        failure = Some(Failure::Deadline);
                        break;
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                if failure.is_some() {
                    stop.store(true, Ordering::Release);
                    let _ = child.kill();
                }
                let read = reader.join().unwrap_or(Err(Failure::Read));
                // Even after all bytes arrive, wait is bounded: child could hang on exit.
                let status = loop {
                    match child.try_wait() {
                        Ok(Some(s)) => break Some(s),
                        Ok(None) => {}
                        Err(_) => {
                            failure = Some(Failure::Decoder);
                            break None;
                        }
                    }
                    if stop.load(Ordering::Acquire) || start.elapsed() >= deadline {
                        failure.get_or_insert(if stop.load(Ordering::Acquire) {
                            Failure::Cancelled
                        } else {
                            Failure::Deadline
                        });
                        break None;
                    }
                    thread::sleep(Duration::from_millis(1));
                };
                if status.is_none() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                if let Some(e) = failure {
                    return Err(e);
                }
                if !status.expect("status").success() {
                    return Err(Failure::Decoder);
                }
                read
            })();
            let _ = finish.send(result);
        });
        Self {
            cancel,
            saturated,
            frames,
            done,
        }
    }
    pub fn poll(&self) -> Option<Frame> {
        if self.cancel.load(Ordering::Acquire) {
            return None;
        }
        self.frames.try_recv().ok()
    }
    pub fn completion(&self) -> Option<Result<(), Failure>> {
        self.done.try_recv().ok()
    }
    pub fn saturated(&self) -> bool {
        self.saturated.load(Ordering::Acquire)
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parser() {
        struct Small(io::Cursor<Vec<u8>>);
        impl Read for Small {
            fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
                self.0.read(&mut b[..1])
            }
        }
        assert_eq!(
            read_frame(&mut Small(io::Cursor::new(vec![1, 2, 3, 4])), 4),
            Ok(Some(vec![1, 2, 3, 4]))
        );
        assert_eq!(read_frame(&mut &b"abc"[..], 4), Err(Failure::Partial));
        assert_eq!(read_frame(&mut &b""[..], 4), Ok(None));
        for (w, h, n) in [(0, 1, 1), (u32::MAX, 2, 1), (1, 1, 0), (1, 1, 121)] {
            assert_eq!(bytes(w, h, n), Err(Failure::Bounds));
        }
    }
    #[cfg(unix)]
    #[test]
    fn watchdog_and_cancel() {
        for cancel in [false, true] {
            let mut c = Command::new("sh");
            c.args(["-c", "exec sleep 10"]);
            let d = Decoder::launch(c, None, 4, 1, Duration::from_millis(30));
            if cancel {
                d.cancel();
            }
            let start = Instant::now();
            loop {
                if let Some(r) = d.completion() {
                    assert_eq!(
                        r,
                        Err(if cancel {
                            Failure::Cancelled
                        } else {
                            Failure::Deadline
                        })
                    );
                    break;
                }
                assert!(start.elapsed() < Duration::from_secs(2));
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn drop_reaps_without_joining_caller() {
        let mut c = Command::new("sh");
        c.args(["-c", "exec sleep 10"]);
        let mut d = Decoder::launch(c, None, 4, 1, Duration::from_secs(2));
        let done = std::mem::replace(&mut d.done, mpsc::channel().1);
        drop(d);
        assert_eq!(
            done.recv_timeout(Duration::from_secs(1)).unwrap(),
            Err(Failure::Cancelled)
        );
    }
    #[cfg(unix)]
    #[test]
    fn saturated_queue_cancel() {
        let mut c = Command::new("sh");
        c.args(["-c", "printf 'abcdefghijkl'"]);
        let d = Decoder::launch(c, None, 4, 3, Duration::from_secs(2));
        // Deterministic gate: producer has actually encountered the full queue.
        let gate = Instant::now();
        while !d.saturated() {
            assert!(gate.elapsed() < Duration::from_secs(1));
            thread::sleep(Duration::from_millis(1));
        }
        d.cancel();
        let start = Instant::now();
        loop {
            if let Some(r) = d.completion() {
                assert_eq!(r, Err(Failure::Cancelled));
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(1));
            thread::sleep(Duration::from_millis(1));
        }
        assert!(d.poll().is_none());
    }
}
