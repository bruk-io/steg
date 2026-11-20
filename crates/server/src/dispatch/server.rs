//! The `server` noun's two methods that need nothing beyond `&Tree` and
//! the clock already arriving as data (TASK-1.88; doc-2 section 8):
//! `server.show` and `server.start`. `server.stop` is not here - it needs
//! a connection's own `Arc<Outbound>` and the server's `Stopper`, neither
//! reachable from this module's signature, so it is intercepted in
//! `connection.rs` instead (the same structural reason `pane.attach`/
//! `pane.detach`/`tab.declare-size` are).

use steg_kernel::{Moment, Tree};
use steg_sdk::{ApiError, ServerRecord};

use super::outcome::Outcome;

/// Every `server.*` method this module matches on, checked against
/// `description().methods` by `mod.rs`'s own completeness test.
pub(super) const HANDLED: &[(&str, &str)] = &[("server", "show"), ("server", "start")];

/// The server's own version string (`"steg/<cargo version>"`) - lives
/// once, here, read by `connection.rs`'s own hello reply too, so there is
/// exactly one definition rather than two inline copies of the same
/// `concat!`.
pub(crate) const SERVER_VERSION: &str = concat!("steg/", env!("CARGO_PKG_VERSION"));

pub(super) fn dispatch(tree: &Tree, now: Moment, verb: &str) -> Result<Outcome, ApiError> {
    match verb {
        "show" => Ok(show(tree, now)),
        // Always `Nothing`, regardless of `on_demand` (AC #4): over the
        // API you are, by definition, already talking to a running
        // server, and doc-2 names no `AlreadyRunning` error for this
        // method (its own `errors` list is `BadRequest` only) - the
        // plain, consistent reading, not invented. `run_server_start`'s
        // own CLI-level equivalent already treats "already running" the
        // same way (exit 0, "a server is already running").
        "start" => Ok(Outcome::Nothing),
        _ => unreachable!("HANDLED names every verb this match covers"),
    }
}

/// Builds `server.show`'s own record (AC #1): `now`'s own duration since
/// `Runtime::new`'s construction is the uptime - time is read by the
/// server (`Runtime::handle`'s own `Instant::now()`, at the one call site
/// that builds `now`) and reaches this, a pure function, as data.
fn show(tree: &Tree, now: Moment) -> Outcome {
    Outcome::ServerRecord(ServerRecord {
        version: SERVER_VERSION.to_string(),
        uptime: now.since_origin().as_secs(),
        sessions: tree.sessions().len(),
        clients: tree.clients().len(),
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use steg_kernel::{Moment, Tree};
    use steg_sdk::ClientKind;

    use super::{SERVER_VERSION, dispatch};

    #[test]
    fn start_always_answers_nothing() {
        let outcome = dispatch(&Tree::new(), Moment::new(Duration::ZERO), "start").unwrap();
        assert_eq!(outcome, super::Outcome::Nothing);
    }

    #[test]
    fn show_reports_the_real_version_uptime_sessions_and_clients() {
        let mut tree = Tree::new();
        tree.new_session(None).unwrap();
        tree.new_session(None).unwrap();
        tree.connect_client(ClientKind::App, None);

        let outcome = dispatch(&tree, Moment::new(Duration::from_secs(123)), "show").unwrap();

        let super::Outcome::ServerRecord(record) = outcome else {
            panic!("server.show must answer with a ServerRecord");
        };
        assert_eq!(record.version, SERVER_VERSION);
        assert_eq!(
            record.uptime, 123,
            "uptime is now's own duration, passed through exactly"
        );
        assert_eq!(record.sessions, 2);
        assert_eq!(record.clients, 1);
    }

    #[test]
    fn show_with_an_empty_tree_reports_zero_sessions_and_clients() {
        let outcome = dispatch(&Tree::new(), Moment::new(Duration::ZERO), "show").unwrap();

        let super::Outcome::ServerRecord(record) = outcome else {
            panic!("server.show must answer with a ServerRecord");
        };
        assert_eq!(record.sessions, 0);
        assert_eq!(record.clients, 0);
    }
}
