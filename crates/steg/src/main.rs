//! The server, the CLI, and the entry point that starts a server, shipped as
//! one binary (R61, doc-16). This crate holds the CLI; a thin client whose
//! commands come from the API description (R32), with no protocol code of
//! its own (R101) - and no server logic of its own (R98).
//!
//! `server start [--on-demand]` (TASK-1.77) stays the one hand-written case
//! (TASK-1.49's own dependency note): starting a server calls
//! `steg_server::attempt`/`run`/`detach` directly and prints doc-2's own
//! outcome lines, which a command that is *calling* a method (generated,
//! `steg::cli::build`) never does - a server being started never had a
//! server to connect to in the first place. Every other command is built
//! from the API description at run time (TASK-1.49); `steg::cli::run` is
//! its own placeholder for calling one for real (TASK-1.119's).

use steg_server::Arbitration;

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let rest = &argv[1..];

    let code = match rest
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["server", "start"] => run_server_start(false),
        ["server", "start", "--on-demand"] => run_server_start(true),
        _ => run_generated(argv),
    };
    std::process::exit(code);
}

/// Everything else: the whole tree `steg::cli::build` generates from the
/// API description, `version` and `completion` (doc-2 section 2's two
/// non-method commands), and `steg`/`steg <noun>` alone listing rather
/// than failing (this task's own engineer decision 1, taken). Any other
/// variant of `server start` (an unrecognised flag, `--help`) reaches the
/// generated `server start` subcommand here too, same as every other
/// method - clap's own usage error (status 2) or `--help` (status 0, no
/// server contacted) answers it, which is what replaces the old
/// catch-all "unrecognised arguments to server start" arm.
fn run_generated(argv: Vec<String>) -> i32 {
    let api = steg_sdk::description();
    let tree = steg::cli::build(&api);
    let matches = tree.get_matches_from(argv);

    match matches.subcommand() {
        None => {
            for noun in api.nouns() {
                println!("{noun}");
            }
            0
        }
        Some(("version", _)) => {
            println!(env!("CARGO_PKG_VERSION"));
            0
        }
        Some(("completion", sub)) => {
            let shell = *sub
                .get_one::<clap_complete::Shell>("shell")
                .expect("required");
            clap_complete::generate(
                shell,
                &mut steg::cli::build(&api),
                "steg",
                &mut std::io::stdout(),
            );
            0
        }
        Some((noun, noun_matches)) => match noun_matches.subcommand() {
            None => {
                for method in api.verbs_of(noun) {
                    println!("{}", method.name.verb);
                }
                0
            }
            Some((verb, verb_matches)) => {
                let dotted = format!("{noun}.{verb}");
                let method = api
                    .method(&dotted)
                    .expect("every generated subcommand names a described method");
                steg::cli::run(method, verb_matches)
            }
        },
    }
}

/// Starts a server, explicitly (`on_demand: false`) or on demand. Prints doc-2's
/// own outcome lines on standard output/error and returns the exit status doc-2
/// section 6 gives for each outcome (the plan's own table).
fn run_server_start(on_demand: bool) -> i32 {
    let mut config = steg_server::ServerConfig::default();
    // The login identity (doc-8 R99, TASK-1.87) - cheap (a local
    // passwd lookup, no child process), so resolved here, before the
    // socket is even bound: a directory-service-bound account's own
    // lookup *can* stall (TASK-1.77's own accepted risk for this exact
    // call), bounded by `login_identity_timeout`. Capturing the actual
    // environment is deliberately *not* done here - see below, after
    // the socket is bound.
    config.login = steg_server::LoginShell::resolve_identity(config.login_identity_timeout);
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

            // The login shell's own environment (doc-8 R99, TASK-1.87) -
            // captured here, deliberately: the socket is already bound
            // (so a client's `connect()` already succeeds, racing
            // nothing), and `run` below has not been called yet (so this
            // bounded wait delays a client's *first request*, never its
            // `connect()`). On any failure, the fallback is a fixed,
            // minimal environment, never this process's own inherited
            // one - reported here, since `steg-server` itself prints
            // nothing. `build_login_shell` is the one place this
            // "capture, then report a fallback" step is written - shared
            // with TASK-1.87's own fixture-only entry point for tests
            // (`crates/steg/tests/fixtures/fixture_server.rs`), which
            // calls it with a fabricated identity instead of the real
            // one just resolved above.
            let identity = steg_server::LoginIdentity {
                shell: std::path::PathBuf::from(&config.login.argv0),
                home: std::path::PathBuf::from(&config.login.home),
                user: config.login.user.clone(),
            };
            let (login, fallback_reason) =
                steg_server::build_login_shell(identity, config.login_environment_timeout);
            config.login = login;
            if let Some(reason) = fallback_reason {
                eprintln!(
                    "steg: could not capture the login shell's environment ({reason:?}); \
                     panes will start with a minimal default environment instead"
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
