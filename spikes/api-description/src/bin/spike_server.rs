//! The toy server. One argument: the socket to listen on.
//!
//! It prints `listening on <socket>` once the socket is bound and not before, so a script
//! or a test can wait on that line instead of sleeping.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use steg_spike_api_description::server;

fn main() -> ExitCode {
    let Some(socket) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: spike_server <socket>");
        return ExitCode::from(2);
    };
    let listener = match server::bind(&socket) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("spike_server: cannot take {}: {error}", socket.display());
            return ExitCode::from(4);
        }
    };
    println!("listening on {}", socket.display());
    if std::io::stdout().flush().is_err() {
        return ExitCode::from(4);
    }
    match server::serve_on(&listener) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("spike_server: {error}");
            ExitCode::from(4)
        }
    }
}
