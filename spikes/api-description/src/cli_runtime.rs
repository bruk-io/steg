//! Candidate B: the CLI learns the API at run time, from the server.
//!
//! This is how Rex does it. Its CLI and scripting API "derive dynamically from the
//! server API it is connected to", through a `help.all` method, so a server that gains a
//! method gives every client that method with no new client build.
//!
//! The whole of candidate B is the function below: ask the server for the description,
//! then hand it to the same generator candidate A uses. That is deliberate, so the
//! comparison measures when a client learns the API and nothing else.
//!
//! What it costs is in the order of the two steps. The description has to arrive before
//! anything can be parsed, so `--help` cannot be answered without a running server, and
//! every run pays for a connection and a round trip before it starts work. doc-2 section
//! 5 says `--help` contacts no server, and the team's CLI design policy says help never
//! starts or contacts a service, so this is measured against a requirement rather than a
//! preference.

use std::path::Path;

use crate::server;

/// Run the CLI, having asked the server what the API is.
///
/// Returns 4, "the server could not be reached or started", when the description cannot
/// be fetched, whatever was asked for: `steg pane split --help` fails the same way
/// `steg pane split` does.
pub fn run(socket: &Path, argv: Vec<String>) -> i32 {
    let api = match server::describe(socket) {
        Ok(api) => api,
        Err(error) => {
            eprintln!(
                "steg: cannot reach the server at {}, so there is no API to run: {error}",
                socket.display()
            );
            return 4;
        }
    };
    crate::cli_build::run(&api, socket, argv)
}

/// What candidate B would need to answer `--help` with nothing running: a copy of the
/// description it keeps and refreshes. Named here, not built, because a cached copy has
/// to be invalidated when the server's version changes (R39), which is the compiled-in
/// description with extra steps and a staleness bug to own.
pub const WORKAROUND: &str = "cache the description on disk and refresh it on a version mismatch";
