//! The spike's server: one or more panes, each a program in a pty with its own terminal thread,
//! served over a Unix socket in one of two framings with one of two data representations.
//!
//! Arguments, all optional but `--socket`:
//!
//! ```text
//! probe-server --socket PATH [--framing ndjson|length] [--overflow resync|block]
//!              [--tick-ms N] [--queue N] [--stdio]
//! ```
//!
//! `--stdio` serves one connection on standard input and output instead of a socket, which is
//! how the pipe-proxy run shows the framing surviving a byte stream it does not control.

mod attach;
mod pty;
mod socket;
mod term;

use std::path::PathBuf;
use std::time::Duration;

use crate::term::Overflow;

struct Args {
    socket: Option<PathBuf>,
    framing: String,
    overflow: Overflow,
    tick: Duration,
    queue: usize,
    stdio: bool,
}

fn parse() -> Result<Args, String> {
    let mut args = Args {
        socket: None,
        framing: "length".to_owned(),
        overflow: Overflow::Resync,
        tick: Duration::from_millis(8),
        queue: 64,
        stdio: false,
    };
    let mut rest = std::env::args().skip(1);
    while let Some(flag) = rest.next() {
        let mut value = || rest.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--socket" => args.socket = Some(PathBuf::from(value()?)),
            "--framing" => args.framing = value()?,
            "--overflow" => args.overflow = Overflow::named(&value()?)?,
            "--tick-ms" => {
                let millis: u64 = value()?.parse().map_err(|_| "--tick-ms wants a number")?;
                args.tick = Duration::from_millis(millis);
            }
            "--queue" => {
                args.queue = value()?.parse().map_err(|_| "--queue wants a number")?;
            }
            "--stdio" => args.stdio = true,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    if args.queue == 0 {
        return Err("--queue must be at least 1".to_owned());
    }
    Ok(args)
}

fn main() -> std::process::ExitCode {
    let args = match parse() {
        Ok(args) => args,
        Err(why) => {
            eprintln!("probe-server: {why}");
            return std::process::ExitCode::from(2);
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("probe-server: {err}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let server = attach::Server::new(args.framing.clone(), args.overflow, args.tick, args.queue);
    runtime.block_on(async {
        if args.stdio {
            socket::one(server, tokio::io::stdin(), tokio::io::stdout()).await;
            return std::process::ExitCode::SUCCESS;
        }
        let Some(path) = args.socket else {
            eprintln!("probe-server: --socket PATH or --stdio");
            return std::process::ExitCode::from(2);
        };
        match socket::listen(server, &path).await {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("probe-server: {err}");
                std::process::ExitCode::FAILURE
            }
        }
    })
}
