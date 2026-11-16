//! The whole of a "real client" for TASK-1.87's own end-to-end proof (AC
//! #1): calls `steg_sdk::local_socket::connect` with `autostart: true`,
//! exactly as a real client would, so the server it causes to exist
//! inherits *this process's own* ambient environment - the stand-in here
//! for whatever a real client's own environment would otherwise leak
//! through. Does the real handshake, starts a real session whose shell
//! writes `TERM`/`STEG_SERVER`/this process's own unique marker to a file
//! the test polls (there is no `pane.capture`-style API yet to read a
//! pane's screen back over the wire, so the real program's own output is
//! what the test reads instead - the same technique `server_start.rs`'s own
//! `"echo $$ > pid_file"` fixtures already use), then exits.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde_json::{Map, json};
use steg_sdk::{ClientKind, Connection, Incoming, Response, WireRole as Role, description};

fn main() {
    let steg_binary =
        std::env::var("STEG_TEST_BINARY").expect("the test should set STEG_TEST_BINARY");
    let output_path =
        std::env::var("STEG_TEST_OUTPUT_PATH").expect("the test should set STEG_TEST_OUTPUT_PATH");

    let options = steg_sdk::ConnectOptions {
        binary: std::path::PathBuf::from(steg_binary),
        socket_path: None,
        autostart: true,
        connect_timeout: Duration::from_secs(10),
    };
    let mut stream = steg_sdk::connect(&options).expect("autostart should succeed");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("setting a read timeout should succeed");

    let hello_bytes = Connection::send_hello("env-leak-test-client", ClientKind::Cli, None);
    stream
        .write_all(&hello_bytes)
        .expect("writing the hello should succeed");

    let mut connection = Connection::new(Role::Client, description());
    loop {
        if connection.poll().is_some() {
            break;
        }
        read_some(&mut stream, &mut connection);
    }

    let program = json!([
        "sh",
        "-c",
        format!(
            "{{ echo TERM=$TERM; echo STEG_SERVER=$STEG_SERVER; echo PATH=$PATH; }} > {}",
            output_path
        )
    ]);
    let mut params = Map::new();
    params.insert("program".to_string(), program);
    let (id, bytes) = connection
        .call("session.new", params)
        .expect("a valid call");
    stream
        .write_all(&bytes)
        .expect("writing the session.new request should succeed");

    loop {
        if let Some(Incoming::Reply {
            id: reply_id,
            response,
        }) = connection.poll()
        {
            assert_eq!(reply_id, id);
            match response {
                Response::Ok(_) => break,
                Response::Error(error) => panic!("session.new failed: {error:?}"),
            }
        }
        read_some(&mut stream, &mut connection);
    }
}

fn read_some(stream: &mut UnixStream, connection: &mut Connection) {
    let mut buf = [0_u8; 4096];
    let n = stream.read(&mut buf).expect("reading from the server");
    assert!(n > 0, "server closed before replying");
    connection.receive(&buf[..n]);
}
