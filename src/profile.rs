//! Local profile location and the single-instance lock (M1-12).
//!
//! A profile is the folder that holds `library.sqlite`, `settings.txt`, the
//! `Resources/` and `Schedules/` folders and the autosave `Recovery/` folder.
//! The default is `%LOCALAPPDATA%\sela` on Windows and
//! `$XDG_DATA_HOME/sela` (else `~/.local/share/sela`) elsewhere; the
//! `--operator-library PATH` argument makes PATH's folder the profile.
//!
//! One operator process owns a profile at a time. [`ProfileLock::acquire`]
//! takes an OS file lock on `<profile>/sela.lock` (`File::try_lock`: `flock`
//! on Unix, `LockFileEx` on Windows). The OS drops it when the process ends,
//! however it ends (crash, SIGKILL, power loss), so a stale lock file never
//! blocks the next launch. A second operator on the same profile gets
//! [`LockError::Held`] before it opens the library or touches any profile
//! file, and shows a message instead of an operator window.
//!
//! Blocking: one `create_dir_all`, one open and one non-blocking `try_lock`.
//! `main` calls it once before the first window exists.
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

pub const LOCK_FILE: &str = "sela.lock";

#[derive(Debug)]
pub enum LockError {
    /// Another process holds the profile.
    Held,
    /// The lock file could not be created or locked (read-only folder, a
    /// filesystem without locks, ...). The reason is the OS error text.
    Unavailable(String),
}

/// Holds the profile until dropped (or until the process ends).
#[derive(Debug)]
pub struct ProfileLock {
    _file: File,
    path: PathBuf,
}

impl ProfileLock {
    pub fn acquire(profile: &Path) -> Result<Self, LockError> {
        let unavailable = |e: io::Error| LockError::Unavailable(e.to_string());
        fs::create_dir_all(profile).map_err(unavailable)?;
        let path = profile.join(LOCK_FILE);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(unavailable)?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file, path }),
            Err(fs::TryLockError::WouldBlock) => Err(LockError::Held),
            Err(fs::TryLockError::Error(error)) => Err(unavailable(error)),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_lock_on_the_same_profile_is_refused_until_the_first_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let first = ProfileLock::acquire(dir.path()).unwrap();
        assert!(matches!(
            ProfileLock::acquire(dir.path()),
            Err(LockError::Held)
        ));
        drop(first);
        let again = ProfileLock::acquire(dir.path()).unwrap();
        assert_eq!(again.path(), dir.path().join(LOCK_FILE));
    }

    #[test]
    fn a_missing_profile_folder_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("nested").join("sela");
        ProfileLock::acquire(&profile).unwrap();
        assert!(profile.join(LOCK_FILE).is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_profile_is_unavailable_not_held() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("ro");
        fs::create_dir(&profile).unwrap();
        fs::set_permissions(&profile, fs::Permissions::from_mode(0o500)).unwrap();
        let result = ProfileLock::acquire(&profile);
        fs::set_permissions(&profile, fs::Permissions::from_mode(0o700)).unwrap();
        // Root ignores directory permissions; then the lock simply works.
        if !is_root() {
            assert!(
                matches!(result, Err(LockError::Unavailable(_))),
                "{result:?}"
            );
        }
    }

    #[cfg(unix)]
    pub(crate) fn is_root() -> bool {
        std::fs::metadata("/proc/self")
            .map(|m| std::os::unix::fs::MetadataExt::uid(&m) == 0)
            .unwrap_or(false)
    }

    /// The lock dies with its process, however it ends: a child holds it,
    /// is killed (SIGKILL on Unix), and the profile opens again at once.
    #[test]
    fn a_killed_holder_releases_the_profile() {
        const CHILD: &str = "SELA_PROFILE_LOCK_CHILD";
        if let Some(profile) = std::env::var_os(CHILD) {
            let profile = Path::new(&profile);
            let _lock = ProfileLock::acquire(profile).unwrap();
            fs::write(profile.join("held"), b"1").unwrap();
            loop {
                std::thread::park();
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "profile::tests::a_killed_holder_releases_the_profile",
            ])
            .env(CHILD, dir.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !dir.path().join("held").exists() {
            assert!(
                std::time::Instant::now() < end,
                "bounded wait for the child"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(matches!(
            ProfileLock::acquire(dir.path()),
            Err(LockError::Held)
        ));
        child.kill().unwrap();
        child.wait().unwrap();
        ProfileLock::acquire(dir.path()).unwrap();
    }
}
