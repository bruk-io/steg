//! The server, the CLI, and the entry point that starts a server, shipped as
//! one binary (R61, doc-16). This crate holds the CLI; a thin client whose
//! commands come from the API description (R32), with no protocol code of
//! its own (R101) - and no server logic of its own (R98).
//!
//! `server start [--on-demand]` (TASK-1.77) is the first real behaviour:
//! this crate composes `steg_server::attempt`/`run`/`detach`, all already
//! public, and prints doc-2's own outcome lines - nothing here decides
//! how arbitration, detaching or shutdown work, only when to call each.
//! Every other command is not decided yet - that is TASK-1.49 (the API
//! description and the CLI's commands generated from it).

use steg_server::Arbitration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["server", "start"] => run_server_start(false),
        ["server", "start", "--on-demand"] => run_server_start(true),
        _ => {
            println!("steg: not yet implemented");
            0
        }
    };
    std::process::exit(code);
}

/// Starts a server, explicitly (`on_demand: false`) or on demand. Prints doc-2's
/// own outcome lines on standard output/error and returns the exit status doc-2
/// section 6 gives for each outcome (the plan's own table).
fn run_server_start(on_demand: bool) -> i32 {
    let config = steg_server::ServerConfig::default();
    let socket_path = steg_server::resolve_socket_path(&config);
    match steg_server::attempt(&config, &socket_path) {
        Ok(Arbitration::AlreadyRunning) => {
            println!("a server is already running");
            0
        }
        Ok(Arbitration::Refuse(reason)) => {
            eprintln!("steg: could not start a server: {reason}");
            4
        }
        Err(err) => {
            eprintln!("steg: could not start a server: {err}");
            4
        }
        Ok(Arbitration::Became(handle)) => {
            // Printed strictly after `attempt` has already bound the listener
            // (`Arbitration::Became` is only ever returned once `bind_with_mode`
            // has already succeeded) - a client connecting the instant this
            // line is printed is guaranteed to succeed, not racing the bind.
            println!("started a new server");
            if on_demand && let Err(err) = steg_server::detach() {
                eprintln!(
                    "steg: could not detach from this terminal ({err}); continuing \
                     attached - closing this terminal will end this server"
                );
            }
            // `listen::run` requires a `&Stopper`; one must exist regardless of
            // whether anything outside this call can reach it yet (nothing can,
            // today - no dispatch exists that reaches `Stopper::stop`, TASK-1.45's).
            let stopper = steg_server::Stopper::new();
            match steg_server::run(handle, &socket_path, &config, &stopper, None) {
                Ok(()) => 0,
                Err(_) => 1,
            }
        }
    }
}
