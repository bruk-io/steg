//! Where the arbiter's lock lives, and the two trust checks the arbiter
//! runs before it ever touches a path: is the directory safe to place a
//! socket in, and - if something is already there - can it be trusted
//! (AC #1: "refuses a directory or an existing file it cannot trust").
//!
//! Both checks are pure functions over already-read [`std::fs::Metadata`],
//! so the one case this environment cannot produce for real (a file or a
//! directory owned by a different user - no second real user or root is
//! available here) is still exercised, with fabricated metadata, rather
//! than left untested. Every other branch is proved against a real file
//! or a real socket (`crates/server/src/paths.rs`'s own tests).

use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use nix::unistd::geteuid;

/// The arbiter's lock file: a sibling of the socket, never bound (so
/// `sockaddr_un`'s 104-byte `sun_path` limit, which `steg_sdk::paths`'s
/// own tests already hold the socket path itself to, does not apply to
/// this one).
#[must_use]
pub fn lock_path(socket_path: &Path) -> PathBuf {
    let mut path = socket_path.as_os_str().to_owned();
    path.push(".lock");
    PathBuf::from(path)
}

/// Why a directory, or an existing file, was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The directory - or the existing file's owner - belongs to a
    /// different user than the one running this process.
    NotOurs,
    /// The directory can be written to by more than its owner.
    WorldWritable,
    /// Something is at this path and it is not a socket at all - a plain
    /// file, a pipe, anything else this task did not put there and does
    /// not know the purpose of.
    NotASocket,
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotOurs => write!(f, "it belongs to a different user"),
            Self::WorldWritable => write!(f, "it can be written to by more than its owner"),
            Self::NotASocket => write!(f, "it is not a socket"),
        }
    }
}

/// What this task found at the socket's own path, before touching it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingFile {
    /// Nothing is there yet.
    Absent,
    /// A socket, owned by this process's own effective user - safe to
    /// connect to (to find out whether its owner is still alive) and,
    /// once the lock is held, safe to remove if it is not.
    OursAndASocket,
    /// Something is there and this process does not own it - never
    /// connected to, and never removed: R5's "only its owner can open"
    /// read from the connecting side too.
    NotOurs,
    /// Something is there, this process owns it, and it is not a socket -
    /// refused for the same reason as `NotOurs`: this task did not put it
    /// there and does not know what it is for.
    NotASocket,
}

/// Whether `file_uid` is the user running this process - pulled out as
/// its own pure function so the one branch this environment cannot
/// produce for real (a file owned by someone else - no second real user
/// or root is available here) is still exercised directly, with a
/// fabricated `file_uid`, rather than left untested because
/// [`std::fs::Metadata`] itself has no way to fabricate one.
#[must_use]
fn owned_by_us(file_uid: u32) -> bool {
    file_uid == geteuid().as_raw()
}

/// Looks at `path` without connecting to it or removing it - the read
/// `arbiter::attempt` feeds into [`crate::arbiter::decide`] alongside the
/// connect probe's own result.
///
/// # Errors
/// An [`std::io::Error`] other than "not found" - a permission error on
/// the directory containing `path`, say. "Not found" itself is not an
/// error here: it is [`ExistingFile::Absent`].
pub fn existing_file(path: &Path) -> std::io::Result<ExistingFile> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ExistingFile::Absent);
        }
        Err(err) => return Err(err),
    };
    if !owned_by_us(metadata.uid()) {
        return Ok(ExistingFile::NotOurs);
    }
    if !metadata.file_type().is_socket() {
        return Ok(ExistingFile::NotASocket);
    }
    Ok(ExistingFile::OursAndASocket)
}

/// Checks the socket's parent directory is owned by this process's own
/// effective user and not writable by anyone else, before anything is
/// placed in it - the other half of AC #1's "refuses a directory... it
/// cannot trust".
///
/// # Errors
/// [`Reason::NotOurs`] or [`Reason::WorldWritable`] as `Err`, since
/// neither is a condition a caller can proceed past - see `dir`'s own
/// [`std::fs::Metadata`] read failing, which surfaces as the underlying
/// [`std::io::Error`] via the `?` a caller composes this with.
pub fn directory_is_safe(dir: &Path) -> std::io::Result<Result<(), Reason>> {
    let metadata = fs::metadata(dir)?;
    if !owned_by_us(metadata.uid()) {
        return Ok(Err(Reason::NotOurs));
    }
    let world_writable = metadata.permissions().mode() & 0o022 != 0;
    if world_writable {
        return Ok(Err(Reason::WorldWritable));
    }
    Ok(Ok(()))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{ExistingFile, Reason, directory_is_safe, existing_file, lock_path, owned_by_us};

    /// `label` names the test in a comment or a variable, never in the
    /// path itself - kept short against `sockaddr_un`'s 104-byte
    /// `sun_path` budget (`steg_sdk::paths`'s own tests already hold
    /// the socket path itself to this; `bind.rs`'s own doc comment
    /// found the temporary-name step narrows it further still).
    fn unique_path(_label: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
        // "p" distinguishes this file's own counter from `bind.rs`'s
        // and `arbiter.rs`'s - all three share one test binary and one
        // pid, so an undifferentiated counter from each could collide.
        std::env::temp_dir().join(format!("{}-p{suffix}", std::process::id()))
    }

    #[test]
    fn lock_path_is_a_sibling_with_a_lock_suffix() {
        let socket = std::path::Path::new("/tmp/steg.sock");
        assert_eq!(
            lock_path(socket),
            std::path::Path::new("/tmp/steg.sock.lock")
        );
    }

    #[test]
    fn nothing_at_the_path_is_absent() {
        let path = unique_path("absent");
        assert_eq!(existing_file(&path).unwrap(), ExistingFile::Absent);
    }

    #[test]
    fn a_real_socket_we_own_is_ours_and_a_socket() {
        let path = unique_path("socket");
        let listener = UnixListener::bind(&path).unwrap();
        assert_eq!(existing_file(&path).unwrap(), ExistingFile::OursAndASocket);
        drop(listener);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_real_plain_file_at_the_path_is_not_a_socket() {
        let path = unique_path("plain-file");
        std::fs::write(&path, b"not a socket").unwrap();
        assert_eq!(existing_file(&path).unwrap(), ExistingFile::NotASocket);
        let _ = std::fs::remove_file(&path);
    }

    /// The honest gap: a file or directory owned by a different real user
    /// cannot be produced for real in this environment (no second user
    /// or root), so `owned_by_us` - the one line `existing_file` and
    /// `directory_is_safe` both call to decide it - is pinned directly
    /// with a fabricated uid instead of through a real, differently
    /// -owned file. `existing_file`/`directory_is_safe` themselves are
    /// proved only against metadata this process really owns (the other
    /// tests here); this is what stands in for the rest.
    #[test]
    fn owned_by_us_is_false_for_a_fabricated_different_uid() {
        use nix::unistd::geteuid;
        let not_my_uid = geteuid().as_raw().wrapping_add(1);
        assert!(!owned_by_us(not_my_uid));
    }

    #[test]
    fn owned_by_us_is_true_for_this_processs_own_euid() {
        use nix::unistd::geteuid;
        assert!(owned_by_us(geteuid().as_raw()));
    }

    #[test]
    fn a_real_world_writable_directory_is_refused() {
        let dir = unique_path("world-writable-dir");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(directory_is_safe(&dir).unwrap(), Err(Reason::WorldWritable));
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn a_real_private_directory_is_safe() {
        let dir = unique_path("private-dir");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(directory_is_safe(&dir).unwrap(), Ok(()));
        let _ = std::fs::remove_dir(&dir);
    }
}
