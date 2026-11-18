//! Login shell identity (TASK-1.131 seam: "login shell identity").

/// The login shell's resolved identity and captured environment - the one
/// value [`Runtime`] is given rather than reading `SHELL`/`HOME` from the
/// process's own (possibly client-inherited) environment in several
/// places (TASK-1.76 §2; doc-8 R99).
///
/// Resolved in two steps, at two different points in the server's own
/// start-up (TASK-1.87 §3): [`LoginShell::resolve_identity`] is cheap (a
/// local passwd lookup, no child process) and runs *before* the socket is
/// bound, at `crates/steg/src/main.rs`'s existing pre-`attempt` call
/// site - its own `base_env` is always empty, since nothing has captured
/// one yet. `base_env` itself is filled in afterward, once, inside
/// [`crate::listen::run`] - after the socket is already bound and before
/// its accept loop starts - by calling
/// [`crate::login_env::capture_login_environment`] with this struct's own
/// `argv0`/`home`/`user` and replacing `base_env` on the result. Neither
/// step is a method on this struct: both are free functions in
/// `crate::login_env`, since only the first one needs no identity to work
/// from yet.
#[derive(Debug, Clone)]
pub struct LoginShell {
    /// What [`steg_sdk::Program::Shell`] resolves to when a pane actually
    /// starts, e.g. `/bin/zsh`.
    pub argv0: String,
    /// The default working directory when nothing else applies: no `cwd`
    /// was given, and (rule B, §2) there is no pane to inherit one from,
    /// or it has not reported one yet.
    pub home: String,
    /// The account's own login name, from the same password-database
    /// lookup as `argv0`/`home` - carried here so
    /// [`crate::login_env::capture_login_environment`] can seed `USER`/
    /// `LOGNAME` in the capture it runs later, without a second lookup.
    /// `None` only when the lookup itself found no identity at all
    /// (TASK-1.87's own honest gap: unreproducible on a real account).
    pub user: Option<String>,
    /// The login shell's own exported environment, captured once at
    /// server start (TASK-1.87) - empty until
    /// [`crate::listen::run`] fills it in; never read before then, since
    /// nothing starts a pane before the socket is even bound.
    pub base_env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
}

impl Default for LoginShell {
    /// A fixed, zero-I/O value - deliberately *not* a real password
    /// -database lookup: `ServerConfig::default()` (and every test that
    /// builds one with `..ServerConfig::default()`) must stay free of
    /// hidden syscalls, so the real resolution is
    /// [`LoginShell::resolve_identity`], called explicitly, never this.
    fn default() -> Self {
        Self {
            argv0: "/bin/zsh".to_string(),
            home: "/".to_string(),
            user: None,
            base_env: Vec::new(),
        }
    }
}

impl LoginShell {
    /// Resolves the login identity from the password database
    /// (`crate::login_env::resolve_login_identity`, TASK-1.87) - never
    /// from `$SHELL`/`$HOME`/`$USER` in this process's own environment,
    /// which is exactly what R99 forbids. `base_env` is always empty on
    /// the result: this step runs before the socket is bound, and
    /// capturing the real environment is deliberately deferred past that
    /// point (§3 of this task's own plan) so a slow shell never delays a
    /// client's `connect()`, only its first request.
    #[must_use]
    pub fn resolve_identity(timeout: std::time::Duration) -> Self {
        let identity = crate::login_env::resolve_login_identity(timeout);
        Self {
            argv0: identity.shell.to_string_lossy().into_owned(),
            home: identity.home.to_string_lossy().into_owned(),
            user: identity.user,
            base_env: Vec::new(),
        }
    }
}

/// Captures `identity`'s own login shell environment and builds the
/// [`LoginShell`] a caller passes to [`Runtime::new`] from it - the
/// "capture, then report a fallback" step `crates/steg/src/main.rs`'s
/// own `run_server_start` needs, written once so it is not duplicated at
/// TASK-1.87's own fixture-only entry point for tests
/// (`crates/steg/tests/fixtures/fixture_server.rs`), which calls this
/// with a *fabricated* identity (a throwaway home, never the password
/// database) instead of [`LoginShell::resolve_identity`]'s real one -
/// everything from this call onward is the real path: the real capture,
/// the real launcher, a real client over a real socket.
///
/// Returns the reason alongside the built `LoginShell` on a fallback,
/// rather than printing anything itself - this crate does no I/O of that
/// kind (no `eprintln!`/`println!` appears anywhere in `crates/server/
/// src`, and this function keeps that true); the caller decides whether
/// and how to report it.
#[must_use]
pub fn build_login_shell(
    identity: crate::login_env::LoginIdentity,
    environment_timeout: std::time::Duration,
) -> (LoginShell, Option<crate::login_env::FallbackReason>) {
    let (base_env, fallback_reason) =
        match crate::login_env::capture_login_environment(&identity, environment_timeout) {
            crate::login_env::Capture::Captured(env) => (env, None),
            crate::login_env::Capture::Fallback(reason, env) => (env, Some(reason)),
        };
    let login = LoginShell {
        argv0: identity.shell.to_string_lossy().into_owned(),
        home: identity.home.to_string_lossy().into_owned(),
        user: identity.user,
        base_env,
    };
    (login, fallback_reason)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::LoginShell;

    #[test]
    fn login_shell_default_is_a_fixed_zero_io_value() {
        // `ServerConfig::default()` (and every test built from
        // `..ServerConfig::default()`) must stay free of hidden syscalls
        // (TASK-1.87) - this is the thing that proves it: not a claim
        // about any real account, just that the literal fallback is
        // well-formed.
        let login = LoginShell::default();
        assert!(!login.argv0.is_empty());
        assert!(!login.home.is_empty());
        assert_eq!(login.user, None);
        assert!(login.base_env.is_empty());
    }

    #[test]
    fn login_shell_resolve_identity_never_panics_and_is_never_empty() {
        // A smoke test, not a claim about this machine's own account -
        // every real account has a passwd entry, so the fallback path
        // itself is proven by fabrication in `login_env`'s own tests.
        // `base_env` stays empty here on purpose: identity resolution
        // never captures an environment (TASK-1.87 §3 - that happens
        // later, after the socket is bound).
        let login = LoginShell::resolve_identity(Duration::from_secs(5));
        assert!(!login.argv0.is_empty());
        assert!(!login.home.is_empty());
        assert!(login.base_env.is_empty());
    }
}
