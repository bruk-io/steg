//! Shared fixture helpers for this binary's own noun modules - plain data
//! and a real `Tree`, no doubles (`.claude/rules/testing-rust.md`). Not
//! `crates/server/tests/support/mod.rs` (the bounded-wait helpers shared
//! across several of this crate's other test binaries): this file is
//! local to the `dispatch` binary alone, never included anywhere else.

use std::time::Duration;

use serde_json::Value;
use steg_kernel::{Moment, Tree};
use steg_sdk::{Description, Request, Response, description};
use steg_server::Context;

/// A `Moment` this many seconds after an arbitrary origin - the same
/// pattern `steg_kernel`'s own test modules use (`crates/kernel/src/
/// pane.rs`'s `moment` helper): plain data, no clock, so every call to
/// `dispatch` below can pick whichever moment its own assertion needs
/// without a real wait. Most calls across this binary pass `moment(0)`:
/// they are not about *when*, only about *what* - the few that are (the
/// ten-minute expiry test) say so in their own name.
pub(super) fn moment(secs: u64) -> Moment {
    Moment::new(Duration::from_secs(secs))
}

pub(super) fn request(method: &str, pairs: Vec<(&str, Value)>) -> Request {
    Request {
        method: method.into(),
        params: pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
    }
}

/// A tree with one session, one tab and one pane - `Tree::new`'s own
/// deterministic ids - plus a `Context` defaulted to that path and the
/// real API description.
pub(super) fn fresh() -> (Tree, Context, Description) {
    let mut tree = Tree::new();
    let session = tree.new_session(None).unwrap();
    let tab = tree.session(session).unwrap().tabs()[0].id();
    let pane = tree.session(session).unwrap().tabs()[0].panes()[0].id();
    (
        tree,
        Context {
            default_session: Some(session),
            default_tab: Some(tab),
            default_pane: Some(pane),
        },
        description(),
    )
}

pub(super) fn ok(response: Response) -> Value {
    match response {
        Response::Ok(value) => value,
        Response::Error(error) => panic!("expected Response::Ok, got {error:?}"),
    }
}

pub(super) fn err(response: Response) -> steg_sdk::ApiError {
    match response {
        Response::Error(error) => error,
        Response::Ok(value) => panic!("expected Response::Error, got {value}"),
    }
}
