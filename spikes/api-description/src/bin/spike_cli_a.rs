//! Candidate A: the description is compiled into the binary.
//!
//! The whole of candidate A is the one call to `api::description()` below. Nothing is
//! read, nothing is connected to, before what was typed is parsed, so `--help` and
//! `--version` are answered with no server running (doc-2, section 5).

use std::path::PathBuf;
use std::process::ExitCode;

use steg_spike_api_description::{api, cli_build};

/// doc-2 section 4: the server to talk to comes from the terminal's context.
const SOCKET_VARIABLE: &str = "STEG_SERVER";

fn main() -> ExitCode {
    let socket = PathBuf::from(
        std::env::var(SOCKET_VARIABLE).unwrap_or_else(|_| "/tmp/steg-spike.sock".to_owned()),
    );
    let mut argv = vec!["steg".to_owned()];
    argv.extend(std::env::args().skip(1));
    let status = cli_build::run(&api::description(), &socket, argv);
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
