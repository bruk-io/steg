//! Binding the socket with its mode set, not assumed (AC #1), without
//! touching the process-wide `umask` - `bind_with_mode` is called from
//! this crate's own in-process functional tests as well as from a real
//! server, and a `umask` set-then-restore dance would transiently
//! corrupt every other test sharing that process under `cargo test`'s
//! default parallelism. Bind under a temporary name in the same
//! directory, set `0600` on that temp path (`std`'s own
//! `PermissionsExt::set_mode`, no `nix` needed for this), then `rename`
//! onto the final path: the socket is never *observable* at its final
//! path with any mode other than `0600`, whatever the ambient umask was
//! (`crates/server/tests/socket_mode.rs` proves exactly that, in a
//! process of its own, since that test is the one place in this task
//! that does set the umask, for real).

use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const SOCKET_MODE: u32 = 0o600;

/// A short temporary name in `path`'s own directory, unique per call
/// within this process - built from the directory alone, not from
/// `path`'s own basename, so a long final socket name (already close to
/// `sockaddr_un`'s 104-byte `sun_path` budget on its own, same as
/// `steg_sdk::paths`'s own socket path) never has a suffix appended on
/// top of it that could push the *temporary* bind over that limit even
/// when the final path would not be (found while testing this: a first
/// version of this function appended the suffix to the whole of `path`,
/// and a long-but-valid final path failed only at the temporary-name
/// step). `bind` itself refuses a path that already exists, so this only
/// has to avoid colliding with another call in the same process, not
/// with anything left over from a previous run (nothing is ever renamed
/// away from this name without being bound first, so nothing is ever
/// left at it either).
fn temp_name(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    dir.join(format!(".steg-tmp-{}-{suffix}", std::process::id()))
}

/// Binds a Unix listener at `path` with mode `0600`, set rather than
/// assumed (AC #1). Callers that hold the arbiter's lock (`arbiter.rs`)
/// are the only ones expected to call this - nothing here checks for a
/// stale file at `path`, since the arbiter has already decided it is
/// safe to remove one.
///
/// # Errors
/// Any [`std::io::Error`] from binding under the temporary name, setting
/// its mode, or renaming it onto `path`.
pub fn bind_with_mode(path: &Path) -> std::io::Result<UnixListener> {
    let temp = temp_name(path);
    let listener = UnixListener::bind(&temp)?;
    if let Err(err) = std::fs::set_permissions(&temp, Permissions::from_mode(SOCKET_MODE)) {
        let _ = std::fs::remove_file(&temp);
        return Err(err);
    }
    if let Err(err) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(err);
    }
    Ok(listener)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::bind_with_mode;

    /// `label` names the test in a comment or a variable, never in the
    /// path itself: `sockaddr_un`'s 104-byte `sun_path` budget, minus
    /// `$TMPDIR`'s own length, leaves too little room for a descriptive
    /// name once `bind_with_mode`'s own temporary-name suffix is added
    /// too (`bind.rs`'s own doc comment: found exactly this way, not
    /// assumed safe).
    fn unique_path(_label: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
        // "b" distinguishes this file's own counter from `paths.rs`'s
        // and `arbiter.rs`'s - see `paths.rs`'s own comment on this.
        std::env::temp_dir().join(format!("{}-b{suffix}", std::process::id()))
    }

    #[test]
    fn a_real_bind_sets_0600_under_the_ambient_umask() {
        let path = unique_path("mode");
        let listener = bind_with_mode(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
        drop(listener);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn nothing_is_left_at_the_temporary_name_after_a_successful_bind() {
        // A private directory for this one test, not the shared system
        // temp directory other tests also bind into concurrently
        // (`cargo test`'s default thread parallelism shares one pid
        // across every test in this binary) - scanning for leftovers
        // needs a directory nothing else is writing to at the same
        // time, or it could catch another, concurrently-running test's
        // own in-flight temporary file and call it a leak.
        let dir = unique_path("no-leftover-dir");
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("steg.sock");
        let listener = bind_with_mode(&path).unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path() != path)
            .collect();
        assert!(
            leftovers.is_empty(),
            "expected nothing beside {path:?} but the socket itself, found {leftovers:?}"
        );
        drop(listener);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    /// `bind_with_mode` itself does not refuse a second bind at an
    /// occupied path - `rename` onto an existing path replaces it
    /// atomically, confirmed by running exactly this (not assumed: a
    /// first attempt at this module assumed the opposite). That is
    /// deliberate and safe *only* because this function is never called
    /// except under the arbiter's lock (`arbiter.rs`), which is what
    /// actually decides whether replacing a given path is safe - this
    /// test pins the low-level behaviour `arbiter.rs` relies on, not a
    /// guarantee `bind_with_mode` makes on its own.
    #[test]
    fn a_second_bind_replaces_the_first_at_the_same_path() {
        let path = unique_path("replace");
        let first = bind_with_mode(&path).unwrap();
        let second = bind_with_mode(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
        drop(first);
        drop(second);
        let _ = std::fs::remove_file(&path);
    }
}
