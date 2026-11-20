//! The `client` noun's dispatch arms (doc-2 section 8; decision-5).
//!
//! Neither verb takes a `Context`: `client.list` has no target at all, and
//! `client.show`'s target is `required: true` with no context fallback
//! possible - decision-5 ("There is no `STEG_CLIENT` in a terminal's
//! environment") - confirmed in `crates/sdk/src/description/methods/
//! tests.rs`'s own `doc_2_section_8_targets`. Both verbs also take
//! `&Tree`, not `&mut Tree`: neither one ever changes the tree.

use steg_kernel::{Client, Tree};
use steg_sdk::{ApiError, ClientKind, ClientRecord, Request};

use super::outcome::Outcome;
use super::{map_kernel_error, param_bool, param_str};

/// Every `client.*` method this module matches on, checked against
/// `description().methods` by `mod.rs`'s own completeness test.
pub(super) const HANDLED: &[(&str, &str)] = &[("client", "list"), ("client", "show")];

pub(super) fn dispatch(tree: &Tree, request: &Request, verb: &str) -> Result<Outcome, ApiError> {
    match verb {
        "list" => Ok(list(tree, request)),
        "show" => show(tree, request),
        _ => unreachable!("HANDLED names every verb this match covers"),
    }
}

/// Builds the `ClientRecord` for `client` (AC #1): `sessions` is read
/// fresh from `Tree::sessions_viewed`, and `last_active` is always `None`
/// for M1 (settled in this task's plan, question 1) - the same "not
/// fabricated here" shape `PaneRecord::title`/`TabRecord::title` already
/// use.
fn record_for(tree: &Tree, client: &Client) -> ClientRecord {
    let id = client.id();
    ClientRecord {
        id,
        kind: client.kind(),
        label: client.label().map(str::to_string),
        sessions: tree
            .sessions_viewed(id)
            .expect("this client is in the tree right now"),
        last_active: None,
    }
}

/// `client.list` (AC #1): every connected client, in connection order,
/// unless `--all` is given - then every command-line client as well
/// (doc-2 section 5's `--all` row; section 8's: "Command line clients
/// only with `--all`"). A CLI client issuing a plain `client list` does
/// not see itself in the default output under this rule, same as it
/// would not see any other bare CLI client - settled in this task's plan,
/// question 2, as an intended consequence, not a bug.
fn list(tree: &Tree, request: &Request) -> Outcome {
    let all = param_bool(request, "all").unwrap_or(false);
    let records = tree
        .clients()
        .iter()
        .filter(|client| all || client.kind() != ClientKind::Cli)
        .map(|client| record_for(tree, client))
        .collect();
    Outcome::ClientRecords(records)
}

/// `client.show` (AC #1, AC #2): resolves `target` the way `session.show`
/// resolves a session (`Tree::resolve_client` mirrors `Tree::
/// resolve_session`), so an unknown or ambiguous target answers with the
/// same codes (`map_kernel_error`'s existing `TargetNotFound`/
/// `TargetAmbiguous` arms, unchanged).
fn show(tree: &Tree, request: &Request) -> Result<Outcome, ApiError> {
    let part = param_str(request, "target")
        .expect("validate already confirmed client.show's target is present - it is required");
    let id = tree.resolve_client(part).map_err(map_kernel_error)?;
    let client = tree
        .client(id)
        .expect("resolve_client just confirmed this id exists");
    Ok(Outcome::ClientRecord(record_for(tree, client)))
}
