//! What the description yields besides commands.
//!
//! - With no flags, the reference docs as Markdown (R62).
//! - With `--json`, the description itself: the payload a client reads and the only thing
//!   a Python plugin needs (R42, R75).
//! - With `--from-server`, either of those taken from a running server's answer to the
//!   describe call rather than from the definition compiled into this binary. The two are
//!   the same text, which is the point.

use std::path::PathBuf;
use std::process::ExitCode;

use steg_spike_api_description::{api, docs, server};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let from_server = args.iter().any(|arg| arg == "--from-server");
    let as_json = args.iter().any(|arg| arg == "--json");
    let api = if from_server {
        let socket = PathBuf::from(
            std::env::var("STEG_SERVER").unwrap_or_else(|_| "/tmp/steg-spike.sock".to_owned()),
        );
        match server::describe(&socket) {
            Ok(api) => api,
            Err(error) => {
                eprintln!("spike_docs: cannot reach the server: {error}");
                return ExitCode::from(4);
            }
        }
    } else {
        api::description()
    };
    if as_json {
        match serde_json::to_string_pretty(&api) {
            Ok(payload) => println!("{payload}"),
            Err(error) => {
                eprintln!("spike_docs: {error}");
                return ExitCode::from(1);
            }
        }
    } else {
        print!("{}", docs::reference(&api));
    }
    ExitCode::SUCCESS
}
