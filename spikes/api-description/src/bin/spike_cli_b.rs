//! Candidate B: the description comes from the server, every run.
//!
//! Identical to candidate A but for the line that gets the description. Everything after
//! that point is the same generator, so what the measurements compare is when a client
//! learns the API.

use std::path::PathBuf;
use std::process::ExitCode;

use steg_spike_api_description::cli_runtime;

/// doc-2 section 4: the server to talk to comes from the terminal's context.
const SOCKET_VARIABLE: &str = "STEG_SERVER";

fn main() -> ExitCode {
    let socket = PathBuf::from(
        std::env::var(SOCKET_VARIABLE).unwrap_or_else(|_| "/tmp/steg-spike.sock".to_owned()),
    );
    let mut argv = vec!["steg".to_owned()];
    argv.extend(std::env::args().skip(1));
    let status = cli_runtime::run(&socket, argv);
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
