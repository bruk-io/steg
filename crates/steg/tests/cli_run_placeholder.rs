//! What part 1 proves about a command it cannot yet run (TASK-1.49's own
//! plan): every described method, called through the real binary with a
//! minimally-valid invocation, parses and validates locally (clap's own
//! value parsers) and reaches the named placeholder - exit 1, nothing on
//! stdout, the method named on stderr - not silently nothing and not
//! exit 0. A second invocation per method, with one bad flag added,
//! proves local validation genuinely runs first: exit 2, not 1.
//!
//! `server.start` is excluded from the first half: it is TASK-1.49's one
//! hand-written exception (TASK-1.77's own dependency note), intercepted
//! in `main.rs` before the generated tree is ever built, and already
//! proved by `tests/server_start.rs` - a minimally-valid invocation of it
//! is exactly `steg server start`, which this test must not actually run.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

use steg_sdk::{Method, Role, ValueKind, description};

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

/// A throwaway socket path nothing listens on - set on every invocation
/// here as a defensive habit (this task's own placeholder never reads
/// it), never this machine's real server.
fn throwaway_socket_path() -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-placeholder-{suffix}.sock", std::process::id()))
}

fn minimal_arg_value(kind: &ValueKind) -> String {
    match kind {
        ValueKind::Text | ValueKind::Target | ValueKind::Path | ValueKind::Argv => "x".to_owned(),
        ValueKind::Integer => "1".to_owned(),
        ValueKind::Boolean => "true".to_owned(),
        ValueKind::Choice(words) => words[0].clone(),
    }
}

/// `steg <noun> <verb>` plus every required param, with a minimally-valid
/// value - never a switch or a program (`.claude/rules/rust.md`: neither
/// is ever `required` in this description, confirmed by the sdk's own
/// `switch`/`program` builders, so there is no case for them here).
fn valid_args(method: &Method) -> Vec<String> {
    let mut args = vec![method.name.noun.clone(), method.name.verb.clone()];
    for param in method.params.iter().filter(|p| p.required) {
        match param.role {
            Role::Target => args.push(minimal_arg_value(&param.value)),
            Role::Flag => {
                args.push(format!("--{}", param.flag()));
                args.push(minimal_arg_value(&param.value));
            }
            Role::Switch | Role::Program => {
                panic!(
                    "{}'s {} is required but is a switch/program",
                    method.name.dotted(),
                    param.name
                )
            }
        }
    }
    args
}

fn run(args: &[String]) -> std::process::Output {
    Command::new(steg_binary())
        .args(args)
        .env("STEG_SOCKET_PATH", throwaway_socket_path())
        .output()
        .expect("the real steg binary should run to completion")
}

#[test]
fn every_described_method_parses_and_reaches_the_not_yet_connected_placeholder() {
    let api = description();
    for method in &api.methods {
        let dotted = method.name.dotted();
        if dotted == "server.start" {
            continue; // the one hand-written exception; see this file's own doc.
        }

        let output = run(&valid_args(method));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{dotted}: expected the placeholder's exit 1, stderr was {stderr:?}"
        );
        assert!(
            output.stdout.is_empty(),
            "{dotted}: a command that cannot run yet must print nothing on stdout, got {:?}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            stderr.contains(&method.name.noun) && stderr.contains(&method.name.verb),
            "{dotted}: the placeholder's message should name the method, got {stderr:?}"
        );
    }
}

#[test]
fn every_described_method_with_one_bad_flag_added_fails_local_validation_first() {
    let api = description();
    for method in &api.methods {
        let mut args = valid_args(method);
        args.push("--totally-bogus-flag-xyz".to_owned());

        let output = run(&args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{}: an unrecognised flag should fail clap's own usage check (status 2), \
             before the placeholder is ever reached, stderr was {:?}",
            method.name.dotted(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stdout.is_empty(),
            "{}: bad usage must print nothing on stdout",
            method.name.dotted()
        );
    }
}

/// Every method's dotted name appears at least once across this file's
/// two tests, so a method added later and missed by both would still be
/// walked (this loops over `description()` itself, not a fixed list) -
/// recorded here as the sanity check that this file's own loops are not
/// accidentally empty.
#[test]
fn this_files_own_loop_covers_every_described_method() {
    let api = description();
    let covered: HashSet<String> = api.methods.iter().map(|m| m.name.dotted()).collect();
    assert_eq!(covered.len(), api.methods.len(), "no duplicate method");
    assert!(!covered.is_empty());
}
