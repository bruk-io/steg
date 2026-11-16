//! A server whose login identity is given directly, never resolved from
//! the real password database - TASK-1.87's own review finding: the only
//! end-to-end proof of AC #1 must not run a real person's real login
//! shell against their real `~/.zprofile`/`/etc/zprofile`, since the
//! result would then depend on the speed and content of whoever happens
//! to run the test.
//!
//! Gated behind `fixtures`, exactly like `already_a_group_leader`/
//! `autostart_client`/`env_leak_client` already are - this entry point
//! does not exist in the shipped binary at all. Everything *after*
//! taking the identity from this fixture's own environment is the real
//! path: `attempt`, `detach`, `build_login_shell` (the real capture),
//! `run` - all exactly what `crates/steg/src/main.rs`'s own
//! `run_server_start(true)` calls, which this mirrors rather than
//! imports from (a `[[bin]]` fixture cannot depend on `crates/steg`'s
//! own binary crate, only on `steg-server`, same as every other fixture
//! here).

use std::path::PathBuf;

use steg_server::{Arbitration, LoginIdentity};

fn main() {
    let shell =
        std::env::var("STEG_TEST_LOGIN_SHELL").expect("the test should set STEG_TEST_LOGIN_SHELL");
    let home =
        std::env::var("STEG_TEST_LOGIN_HOME").expect("the test should set STEG_TEST_LOGIN_HOME");
    let user = std::env::var("STEG_TEST_LOGIN_USER").ok();
    let identity = LoginIdentity {
        shell: PathBuf::from(shell),
        home: PathBuf::from(home),
        user,
    };

    let config = steg_server::ServerConfig::default();
    let socket_path = steg_server::resolve_socket_path(&config);
    let code = match steg_server::attempt(&config, &socket_path) {
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
            println!("started a new server");
            if let Err(err) = steg_server::detach() {
                eprintln!("steg: could not detach from this terminal ({err}); continuing attached");
            }

            let mut config = config;
            let (login, fallback_reason) =
                steg_server::build_login_shell(identity, config.login_environment_timeout);
            config.login = login;
            if let Some(reason) = fallback_reason {
                eprintln!(
                    "steg: could not capture the login shell's environment ({reason:?}); \
                     panes will start with a minimal default environment instead"
                );
            }

            let stopper = steg_server::Stopper::new();
            match steg_server::run(handle, &socket_path, &config, &stopper, None) {
                Ok(()) => 0,
                Err(_) => 1,
            }
        }
    };
    std::process::exit(code);
}
