//! Replaces this crate's own former placeholder (TASK-1.49's `run.rs`)
//! outright with the real path (TASK-1.119 AC #1): find or start a
//! server through the sdk, hand the parsed command's own method and
//! params to it over the wire, and exit with the status doc-2 section 6
//! gives. `api.call` is `main.rs`'s own special case, answered by
//! [`super::api_call::run`] instead, which shares this module's own
//! [`connect_and_handshake`]/[`block_until_reply`] - everything else,
//! including `server.start`'s own hand-written case, reaches
//! [`run`] below exactly once each (`server.start` through `main.rs`'s
//! own `run_server_start`, never through here).

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use clap::ArgMatches;
use steg_sdk::{
    ClientKind, ConnectError, ConnectOptions, Connection, Description, Incoming, Method, Response,
    WireRole, connect_handshaken, description, parse_server_hello,
};

use super::exit::{api_error_status, connect_error_status, protocol_refused_status};
use super::output::render;
use super::params::extract_params;
use super::tree::{autostart_flag, json_flag, quiet_flag};

/// How long [`steg_sdk::connect_handshaken`] may spend finding or
/// starting a server, and waiting for its hello reply, per attempt - the
/// same 10 s the app's own `crates/app/src/startup.rs` uses for its
/// equivalent wait. Worst case (one EOF, one retry) is bounded by twice
/// this.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The real path every generated command but `server.start`/`api.call`
/// takes. `api` is the same compiled-in [`Description`] `main.rs` already
/// built the command tree from; `method` is the one this invocation
/// resolved to; `matches` is that method's own leaf `ArgMatches` (also
/// where the global `--autostart`/`--no-autostart`/`--json`/`-q` flags
/// are read from, since they are `global(true)` on the root command). A
/// successful reply's own bytes are [`super::output::render`]'s job
/// (TASK-1.120), not this function's - it only writes whatever that
/// returns (or nothing, for `Returns::Nothing`'s own empty `Vec`).
#[must_use]
pub fn run(api: &Description, method: &Method, matches: &ArgMatches) -> i32 {
    let params = extract_params(method, matches);
    let autostart = autostart_flag(matches);
    let json = json_flag(matches);
    let quiet = quiet_flag(matches);

    let (mut stream, mut connection) = match connect_and_handshake(autostart) {
        Ok(pair) => pair,
        Err(status) => return status,
    };

    let dotted = method.name.dotted();
    let (id, bytes) = match connection.call(&dotted, params) {
        Ok(pair) => pair,
        Err(err) => {
            eprintln!("steg: {}", err.message);
            return api_error_status(api, err.code);
        }
    };
    if let Err(err) = stream.write_all(&bytes) {
        eprintln!("steg: writing to the server: {err}");
        return connect_error_status(&ConnectError::Io(err));
    }

    match block_until_reply(&mut stream, &mut connection, id) {
        Ok(Response::Ok(value)) => match render(&method.returns, &value, json, quiet) {
            Ok(bytes) => {
                if let Err(err) = std::io::stdout().write_all(&bytes) {
                    eprintln!("steg: writing to stdout: {err}");
                    return connect_error_status(&ConnectError::Io(err));
                }
                0
            }
            Err(err) => {
                eprintln!("steg: {err}");
                // `err` (a `RenderError`) is the `io::Error`'s own
                // source, not flattened into a `String` first (.claude/
                // rules/rust.md: "add context... never by turning it
                // into a string") - `io::Error::other` takes anything
                // `Into<Box<dyn Error + Send + Sync>>`, which `RenderError`
                // already satisfies through its own `std::error::Error`
                // impl.
                connect_error_status(&ConnectError::Io(std::io::Error::other(err)))
            }
        },
        Ok(Response::Error(err)) => {
            eprintln!("steg: {}", err.message);
            api_error_status(api, err.code)
        }
        Err(err) => {
            eprintln!("steg: {err}");
            connect_error_status(&err)
        }
    }
}

/// Finds or starts a server (through the sdk, with the retry-once EOF
/// protection TASK-1.119 AC #4 adds), shakes hands, and hands back a
/// connected stream and a fresh [`Connection`] ready to [`Connection::call`]
/// on - the shared first half of both [`run`] and `super::api_call::run`.
/// On any failure, the message is already printed to stderr and `Err`
/// carries the exit status doc-2 section 6 gives it, so every caller only
/// has to `return` it.
pub(super) fn connect_and_handshake(autostart: bool) -> Result<(UnixStream, Connection), i32> {
    let binary = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("steg: could not find my own binary to autostart with: {err}");
            return Err(4);
        }
    };
    let options = ConnectOptions {
        binary,
        socket_path: None,
        autostart,
        connect_timeout: CONNECT_TIMEOUT,
    };

    let (stream, hello_payload) = match connect_handshaken(
        &options,
        &format!("steg-cli/{}", env!("CARGO_PKG_VERSION")),
        ClientKind::Cli,
        None,
    ) {
        Ok(pair) => pair,
        Err(err) => {
            eprintln!("steg: {err}");
            return Err(connect_error_status(&err));
        }
    };

    if let Err(message) = parse_server_hello(&hello_payload) {
        eprintln!("steg: {message}");
        return Err(protocol_refused_status());
    }

    Ok((stream, Connection::new(WireRole::Client, description())))
}

/// Reads from `stream` and polls `connection` until the reply matching
/// `id` arrives - the same shape `crates/server/tests/attach.rs`'s own
/// `call_and_read_reply` and `crates/app/src/startup.rs`'s own
/// `block_until` already use: glue code reading bytes and driving
/// `Connection::poll`, not protocol logic of its own (doc-16 R101 is
/// about not holding a second copy of the handshake/encoding/decoding
/// logic itself, which this does not).
pub(super) fn block_until_reply(
    stream: &mut UnixStream,
    connection: &mut Connection,
    id: u64,
) -> Result<Response, ConnectError> {
    loop {
        if let Some(incoming) = connection.poll() {
            match incoming {
                Incoming::Reply {
                    id: reply_id,
                    response,
                } if reply_id == id => return Ok(response),
                Incoming::Malformed(message) => {
                    return Err(ConnectError::Io(std::io::Error::other(message)));
                }
                Incoming::Reply { .. }
                | Incoming::Hello(_)
                | Incoming::Request { .. }
                | Incoming::Event { .. }
                | Incoming::UnknownEvent { .. }
                | Incoming::StreamControl { .. }
                | Incoming::Data { .. }
                | Incoming::UnknownStream { .. } => continue,
            }
        }
        let mut buf = [0_u8; 65536];
        let n = stream.read(&mut buf).map_err(ConnectError::Io)?;
        if n == 0 {
            return Err(ConnectError::Io(std::io::Error::other(
                "the server closed the connection before answering",
            )));
        }
        connection.receive(&buf[..n]);
    }
}
