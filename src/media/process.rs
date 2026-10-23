//! Supervised FFmpeg child processes (M2-02, VAL-MEDIA-030/031).
//!
//! Every long-running FFmpeg tool runs in its own kill domain so that
//! cancelling, a stall or dropping its owner takes the whole tree down. Only
//! on Windows does Sela's own abnormal exit do the same:
//!
//! - Unix: the child leads a new process group (`process_group(0)`) and
//!   [`Supervised::kill`] sends `SIGKILL` to the group before the leader is
//!   reaped, so the group id cannot have been reused. There is no
//!   parent-death signal: if Sela crashes or is `SIGKILL`ed, nothing kills
//!   the group. A decoder then ends when its next write to the closed pipe
//!   fails, but a child blocked on a slow read (a hung network mount) or a
//!   sleeping ffprobe lingers, and its timeout died with Sela. Open gap,
//!   tracked in docs/work-log.md (media-core).
//! - Windows: the child is assigned to a Job Object with
//!   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; closing the job handle (on drop,
//!   or when Sela's process ends for any reason) terminates every process in
//!   it. `CREATE_NO_WINDOW` keeps a console from flashing.
//!
//! Dropping a [`Supervised`] that has not exited kills it and reaps it, so
//! no zombie or orphaned decoder outlives its owner.

use std::io;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus};
use std::time::{Duration, Instant};

pub struct Supervised {
    child: Child,
    /// Set once `wait`/`try_wait` returned a status: the pid (and on Unix
    /// the process group id) may be reused from then on, so no more kills.
    reaped: Option<ExitStatus>,
    #[cfg(windows)]
    job: Option<job::Job>,
}

impl Supervised {
    /// Spawns `command` in its own process group (Unix) or Job Object
    /// (Windows) with no console window. The caller sets stdio.
    pub fn spawn(mut command: Command) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let child = command.spawn()?;
        #[cfg(windows)]
        let job = job::Job::assign(&child);
        Ok(Self {
            child,
            reaped: None,
            #[cfg(windows)]
            job,
        })
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    /// Kills the whole process group or job. Safe to call repeatedly and
    /// after the child exited; a no-op once it was reaped.
    pub fn kill(&mut self) {
        if self.reaped.is_some() {
            return;
        }
        #[cfg(unix)]
        {
            // SAFETY: `kill` takes plain integers. The negative id names the
            // process group this child leads; the child is not reaped yet,
            // so its id (and the group id) still belong to it.
            unsafe {
                unix::kill(-(self.child.id() as i32), unix::SIGKILL);
            }
        }
        #[cfg(windows)]
        if let Some(job) = &self.job {
            job.terminate();
        }
        let _ = self.child.kill();
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if let Some(status) = self.reaped {
            return Ok(Some(status));
        }
        let status = self.child.try_wait()?;
        self.reaped = status;
        Ok(status)
    }

    /// Waits up to `timeout` for the child to exit on its own.
    pub fn wait_timeout(&mut self, timeout: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(None) | Err(_) => return None,
            }
        }
    }

    /// Kills the tree and reaps the child.
    pub fn kill_and_wait(&mut self) -> Option<ExitStatus> {
        self.kill();
        if let Some(status) = self.reaped {
            return Some(status);
        }
        let status = self.child.wait().ok();
        self.reaped = status;
        status
    }
}

impl Drop for Supervised {
    fn drop(&mut self) {
        if self.reaped.is_none() {
            self.kill_and_wait();
        }
    }
}

#[cfg(unix)]
mod unix {
    pub const SIGKILL: i32 = 9;
    unsafe extern "C" {
        pub fn kill(pid: i32, signal: i32) -> i32;
    }
}

#[cfg(windows)]
mod job {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;

    type Handle = *mut c_void;

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

    #[repr(C)]
    #[derive(Default)]
    struct BasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    /// `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` (144 bytes on x64).
    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimitInformation {
        basic: BasicLimitInformation,
        io: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            class: i32,
            information: *const c_void,
            length: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn TerminateJobObject(job: Handle, exit_code: u32) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    /// A Job Object that kills its processes when its last handle closes.
    pub struct Job(Handle);

    // SAFETY: a job handle is a kernel handle usable from any thread.
    unsafe impl Send for Job {}

    impl Job {
        /// `None` when the job cannot be created or the child cannot join it
        /// (then `Supervised::kill` still kills the child itself).
        pub fn assign(child: &std::process::Child) -> Option<Self> {
            // SAFETY: null attributes and name create an anonymous job; the
            // information block is a live, correctly sized repr(C) struct;
            // the process handle stays valid while `child` is borrowed.
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return None;
                }
                let job = Job(handle);
                let mut info = ExtendedLimitInformation::default();
                info.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let set = SetInformationJobObject(
                    handle,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                    (&info as *const ExtendedLimitInformation).cast(),
                    std::mem::size_of::<ExtendedLimitInformation>() as u32,
                );
                if set == 0 || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0 {
                    return None;
                }
                Some(job)
            }
        }

        pub fn terminate(&self) {
            // SAFETY: the handle is owned and open until drop.
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: closing the owned handle once; KILL_ON_JOB_CLOSE ends
            // any process still in the job.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::process::Stdio;

    fn alive(pid: u32) -> bool {
        // Signal 0 checks for existence; a zombie still counts, so the
        // caller reaps first.
        // SAFETY: plain integers.
        unsafe { unix::kill(pid as i32, 0) == 0 }
    }

    #[test]
    fn kill_takes_the_whole_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("grandchild.pid");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(format!(
                "sleep 30 & echo $! > '{}'; wait",
                pid_file.display()
            ))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = Supervised::spawn(command).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let grandchild = loop {
            if let Some(pid) = std::fs::read_to_string(&pid_file)
                .ok()
                .and_then(|text| text.trim().parse::<u32>().ok())
            {
                break pid;
            }
            assert!(Instant::now() < end, "the shell started its child");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(alive(grandchild));
        let status = child.kill_and_wait().unwrap();
        assert!(!status.success());
        let end = Instant::now() + Duration::from_secs(5);
        // The grandchild is re-parented and reaped by init (or the test
        // harness); it must disappear, not keep running.
        while alive(grandchild) && !zombie(grandchild) {
            assert!(Instant::now() < end, "grandchild {grandchild} survived");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn zombie(pid: u32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .map(|stat| {
                stat.rsplit_once(')')
                    .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'))
            })
            .unwrap_or(false)
    }

    #[test]
    fn dropping_a_running_child_kills_and_reaps_it() {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sleep 30"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = Supervised::spawn(command).unwrap();
        let pid = child.id();
        assert!(alive(pid));
        drop(child);
        assert!(!alive(pid), "dropped child {pid} still exists (not reaped)");
    }

    #[test]
    fn kill_after_exit_is_a_no_op() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exit 3"]).stdin(Stdio::null());
        let mut child = Supervised::spawn(command).unwrap();
        let status = child.wait_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(status.code(), Some(3));
        child.kill();
        assert_eq!(child.kill_and_wait().unwrap().code(), Some(3));
    }
}
