//! A pane's environment: the five variables, and the server's own
//! directory first on `PATH` (doc-2 section 4).
//!
//! This is this task's part of the seam with login-shell capture
//! (TASK-1.87): the five variables and prepending `STEG_BIN_DIR` to
//! `PATH`, here. Where the server's own *base* environment comes from
//! (R99's login shell) is TASK-1.87's part, and it never has to change
//! this function - `server` (how to reach the server) and `bin_dir` are
//! data a caller passes in, never computed inside it, and the one thing
//! this function reads from the environment it is about to modify - the
//! current `PATH`, to prepend to - is passed in too (`current_path`),
//! rather than read live, so this stays a plain function with a plain
//! unit test.
//!
//! **This function's own return value is no longer the pane's whole set
//! of overrides on top of an ambient environment - that ambient seeding
//! is gone (TASK-1.87, `crate::pty::RealLauncher::spawn`'s own
//! `.env_clear()`).** What this function returns is still exactly the
//! five `STEG_*` variables and `PATH`; the caller
//! (`crate::runtime::Runtime::launch_spec`) is what now composes them
//! with the captured login shell's own environment underneath, and
//! `TERM` on top, into the pane's whole environment.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use steg_sdk::{PaneId, SessionId, TabId};

/// Builds the five `STEG_*` variables a pane's program gets (doc-2 section
/// 4), plus a `PATH` with `bin_dir` prepended to `current_path`.
///
/// `server` is `STEG_SERVER`'s value as given - this task takes it as
/// data; `bin_dir` is the directory holding the server's own `steg`
/// binary, used both for `STEG_BIN_DIR` and as `PATH`'s new first entry.
#[must_use]
pub fn pane_environment(
    server: &str,
    session: SessionId,
    tab: TabId,
    pane: PaneId,
    bin_dir: &Path,
    current_path: Option<&OsStr>,
) -> Vec<(OsString, OsString)> {
    let mut path = bin_dir.as_os_str().to_owned();
    if let Some(existing) = current_path {
        path.push(":");
        path.push(existing);
    }

    vec![
        (OsString::from("STEG_SERVER"), OsString::from(server)),
        (
            OsString::from("STEG_SESSION"),
            OsString::from(session.to_string()),
        ),
        (OsString::from("STEG_TAB"), OsString::from(tab.to_string())),
        (
            OsString::from("STEG_PANE"),
            OsString::from(pane.to_string()),
        ),
        (
            OsString::from("STEG_BIN_DIR"),
            bin_dir.as_os_str().to_owned(),
        ),
        (OsString::from("PATH"), path),
    ]
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::Path;

    use steg_sdk::{PaneId, SessionId, TabId};

    use super::pane_environment;

    fn find<'a>(env: &'a [(std::ffi::OsString, std::ffi::OsString)], key: &str) -> &'a OsStr {
        env.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_os_str())
            .unwrap_or_else(|| panic!("{key} is missing from {env:?}"))
    }

    #[test]
    fn carries_the_five_variables() {
        let env = pane_environment(
            "/tmp/steg.sock",
            SessionId::from_raw(1),
            TabId::from_raw(2),
            PaneId::from_raw(3),
            Path::new("/usr/local/steg/bin"),
            None,
        );

        assert_eq!(find(&env, "STEG_SERVER"), OsStr::new("/tmp/steg.sock"));
        assert_eq!(
            find(&env, "STEG_SESSION"),
            OsStr::new(&SessionId::from_raw(1).to_string())
        );
        assert_eq!(
            find(&env, "STEG_TAB"),
            OsStr::new(&TabId::from_raw(2).to_string())
        );
        assert_eq!(
            find(&env, "STEG_PANE"),
            OsStr::new(&PaneId::from_raw(3).to_string())
        );
        assert_eq!(
            find(&env, "STEG_BIN_DIR"),
            OsStr::new("/usr/local/steg/bin")
        );
    }

    #[test]
    fn path_is_just_bin_dir_when_there_is_nothing_to_prepend_to() {
        let env = pane_environment(
            "/tmp/steg.sock",
            SessionId::from_raw(1),
            TabId::from_raw(2),
            PaneId::from_raw(3),
            Path::new("/usr/local/steg/bin"),
            None,
        );

        assert_eq!(find(&env, "PATH"), OsStr::new("/usr/local/steg/bin"));
    }

    #[test]
    fn path_prepends_bin_dir_to_the_existing_path() {
        let env = pane_environment(
            "/tmp/steg.sock",
            SessionId::from_raw(1),
            TabId::from_raw(2),
            PaneId::from_raw(3),
            Path::new("/usr/local/steg/bin"),
            Some(OsStr::new("/usr/bin:/bin")),
        );

        assert_eq!(
            find(&env, "PATH"),
            OsStr::new("/usr/local/steg/bin:/usr/bin:/bin")
        );
    }

    #[test]
    fn exactly_six_variables_are_returned() {
        let env = pane_environment(
            "s",
            SessionId::from_raw(0),
            TabId::from_raw(0),
            PaneId::from_raw(0),
            Path::new("/bin"),
            None,
        );

        assert_eq!(env.len(), 6);
    }
}
