//! Where the local socket lives (doc-13 R60), as a pure computation - no
//! environment access here, no I/O. Shared by the `local-socket` feature's
//! provider and, later, the server (TASK-1.44): both call this with the
//! environment they each read themselves, so a client and a server
//! converge on one path with no message ever naming it.

use std::path::{Path, PathBuf};

/// Where the local socket is: `explicit` wins outright when given, and is
/// not empty (an override a caller reads from `STEG_SERVER`, same category
/// as the other `STEG_*` variables doc-2 section 4 already lists - used by
/// tests, and later by a person's own debugging). Otherwise, preference
/// order: `$XDG_RUNTIME_DIR/steg.sock` (doc-13 R60's own name), else
/// `$TMPDIR/steg.sock` (macOS's de facto per-user, mode-700, runtime-only
/// equivalent - `XDG_RUNTIME_DIR` is rarely set on macOS), else
/// `/tmp/steg-<user>.sock` as a last resort shared-tmp fallback.
///
/// **An explicit value that is present but empty is treated the same as
/// absent, not as "use this path" (TASK-1.147's own finding):**
/// `std::env::var_os` returns `Some("")`, never `None`, for a variable
/// that is set to the empty string, and every caller of this function
/// reads `explicit` from exactly that - `STEG_SERVER` is also one of
/// `pane_environment`'s five variables, set on every real pane (doc-2
/// section 4) and still an empty placeholder today (TASK-1.146 gives it
/// a real value). Without this check, a `steg` command run inside a pane
/// would read that placeholder as an override, fail to connect to an
/// empty path, and either give up or autostart a second server that
/// itself fails to bind the same empty path - worse off than today,
/// when the pane carries no such variable at all and falls through to
/// `XDG_RUNTIME_DIR`/`TMPDIR` correctly.
///
/// Flat, no subdirectory: `sockaddr_un.sun_path` is 104 bytes on macOS, and
/// a realistic `$TMPDIR` such as
/// `/var/folders/zz/zyxvpxvq6csfxvn_n0000000000000/T/` plus `steg.sock` is
/// already close to half that budget, so no extra directory component is
/// spent on it (tested below against exactly that example).
#[must_use]
pub fn socket_path(
    explicit: Option<&Path>,
    xdg_runtime_dir: Option<&Path>,
    tmp_dir: Option<&Path>,
    user: Option<&str>,
) -> PathBuf {
    if let Some(path) = explicit.filter(|path| !path.as_os_str().is_empty()) {
        return path.to_path_buf();
    }
    if let Some(dir) = xdg_runtime_dir {
        return dir.join("steg.sock");
    }
    if let Some(dir) = tmp_dir {
        return dir.join("steg.sock");
    }
    PathBuf::from(format!("/tmp/steg-{}.sock", user.unwrap_or("unknown")))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::socket_path;

    #[test]
    fn an_explicit_override_wins_over_everything_else() {
        let path = socket_path(
            Some(Path::new("/tmp/explicit.sock")),
            Some(Path::new("/run/user/501")),
            Some(Path::new("/tmp/t")),
            Some("bruk"),
        );
        assert_eq!(path, Path::new("/tmp/explicit.sock"));
    }

    /// `STEG_SERVER` is also one of `pane_environment`'s five variables,
    /// set to an empty placeholder on every pane today
    /// (`crates/server/src/runtime/start.rs`, pending TASK-1.146) - this
    /// must fall through exactly like `None`, never like a real override
    /// of the empty path itself.
    #[test]
    fn an_empty_explicit_override_is_treated_as_absent() {
        let path = socket_path(
            Some(Path::new("")),
            Some(Path::new("/run/user/501")),
            Some(Path::new("/tmp/t")),
            Some("bruk"),
        );
        assert_eq!(path, Path::new("/run/user/501/steg.sock"));
    }

    #[test]
    fn xdg_runtime_dir_wins_over_tmp_dir_and_the_user_fallback() {
        let path = socket_path(
            None,
            Some(Path::new("/run/user/501")),
            Some(Path::new("/tmp/t")),
            Some("bruk"),
        );
        assert_eq!(path, Path::new("/run/user/501/steg.sock"));
    }

    #[test]
    fn tmp_dir_is_used_when_xdg_runtime_dir_is_not_set() {
        let path = socket_path(None, None, Some(Path::new("/tmp/t")), Some("bruk"));
        assert_eq!(path, Path::new("/tmp/t/steg.sock"));
    }

    #[test]
    fn the_user_fallback_is_used_when_nothing_else_is_set() {
        assert_eq!(
            socket_path(None, None, None, Some("bruk")),
            Path::new("/tmp/steg-bruk.sock")
        );
        assert_eq!(
            socket_path(None, None, None, None),
            Path::new("/tmp/steg-unknown.sock")
        );
    }

    /// `sockaddr_un.sun_path` is 104 bytes on macOS. A real `$TMPDIR` is
    /// usually the longest of the three inputs, so it is the one tested
    /// against the actual limit, with a margin - not a formality.
    #[test]
    fn a_realistic_macos_tmp_dir_stays_well_under_the_104_byte_sockaddr_un_limit() {
        let tmp_dir = Path::new("/var/folders/zz/zyxvpxvq6csfxvn_n0000000000000/T");
        let path = socket_path(None, None, Some(tmp_dir), Some("bruk"));
        let encoded_len = path.to_str().unwrap().len() + 1; // + the NUL terminator
        assert!(
            encoded_len < 104,
            "{} bytes is too close to the 104 byte sockaddr_un.sun_path limit",
            encoded_len
        );
    }
}
