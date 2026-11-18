//! `api call <method>` (TASK-1.119 AC #5): the one command this task
//! special-cases outside the generic path in `connect.rs`, for two
//! reasons doc-2 states about it specifically (section 2, section 3):
//! its parameters are read from standard input as one JSON object, not
//! from flags, and its own output is JSON by its nature - never gated on
//! `--json` the way every other generated command's success output is
//! (TASK-1.119 AC #3's own exception).

use std::io::Read;
use std::io::Write as _;

use clap::ArgMatches;
use serde_json::Value;
use steg_sdk::{ConnectError, Description, Response};

use super::connect::{block_until_reply, connect_and_handshake};
use super::exit::{api_error_status, connect_error_status, malformed_api_call_input_status};
use super::tree::autostart_flag;

/// `api` is the compiled-in [`Description`], used only for its `errors`
/// table (`api_error_status`) - `api.call`'s own request is never
/// validated against it client-side (that is the whole point of
/// `Connection::call_unvalidated`, which this calls). `matches` is `api
/// call`'s own leaf `ArgMatches`: its one positional, `method` (the
/// method to call, by its dotted name), and the global
/// `--autostart`/`--no-autostart` flags this command still honours like
/// every other.
///
/// # Panics
/// Never in practice: `method` is `api.call`'s own required positional
/// (`tree.rs`'s own `Role::Target` mapping, `required(param.required)`
/// with `required: true` for `api.call`'s one param), so clap itself
/// refuses to reach this function at all without it.
#[must_use]
pub fn run(api: &Description, matches: &ArgMatches) -> i32 {
    let method_name = matches
        .get_one::<String>("method")
        .expect("api.call's own target is required")
        .clone();

    let mut input = String::new();
    if let Err(err) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("steg: reading standard input: {err}");
        return malformed_api_call_input_status();
    }
    let parsed: Value = match serde_json::from_str(&input) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("steg: standard input is not valid JSON: {err}");
            return malformed_api_call_input_status();
        }
    };
    let Value::Object(mut params) = parsed else {
        eprintln!("steg: standard input must be a JSON object");
        return malformed_api_call_input_status();
    };
    params.insert("method".to_owned(), Value::String(method_name));

    let autostart = autostart_flag(matches);
    let (mut stream, mut connection) = match connect_and_handshake(autostart) {
        Ok(pair) => pair,
        Err(status) => return status,
    };

    let (id, bytes) = connection.call_unvalidated("api.call", params);
    if let Err(err) = stream.write_all(&bytes) {
        eprintln!("steg: writing to the server: {err}");
        return connect_error_status(&ConnectError::Io(err));
    }

    match block_until_reply(&mut stream, &mut connection, id) {
        // `api call`'s own output is always JSON (doc-2 section 3), never
        // gated on `--json` - the stated exception to the generic path.
        Ok(Response::Ok(value)) => {
            println!("{value}");
            0
        }
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
