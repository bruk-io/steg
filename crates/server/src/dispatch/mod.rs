//! Dispatching a request to the kernel, over the API's description
//! (TASK-1.75; doc-8 R36, R37; doc-16 R98, R101; decision-1).
//!
//! [`dispatch`] is a plain function from a [`steg_sdk::Request`] and a
//! [`Context`] to a [`steg_sdk::Response`], over a [`steg_kernel::Tree`]
//! and the server's remembered [`steg_kernel::Settings`] (R80, decision-9;
//! TASK-1.48) — no socket, no I/O, in its signature: both are already in
//! memory, and the request is already parsed. The server's own remaining
//! job (TASK-1.44/1.45) is building a `Context` from a connection's
//! `STEG_SESSION`/`STEG_TAB`/`STEG_PANE` and calling this function
//! unchanged.

mod api;
mod event;
mod kernel_error;
mod lookup;
pub(crate) mod outcome;
mod pane;
mod server;
mod session;
mod setting;
mod tab;

pub(crate) use api::resolve as resolve_api_call;
pub(crate) use event::{Filter, resolve_filter, tree_snapshot};
pub(crate) use kernel_error::map_kernel_error;
pub(crate) use lookup::{
    any_pane_running, find_tab, pane_mut, panes_in_close_scope, param_bool, param_i64,
    param_present, param_str, resolve_pane_str, resolve_tab_str, tab_mut, tab_position,
    target_pane, target_session, target_tab, wire_position_to_index,
};
pub use outcome::Outcome;
pub(crate) use server::SERVER_VERSION;

use steg_kernel::{Settings, Tree};
use steg_sdk::{
    ApiError, Description, ErrorCode, PaneId, Request, Response, SessionId, TabId, validate,
};

/// The defaults a request's target falls back to when its own `target`
/// parameter is left out (doc-2 section 4's context fallback), passed in
/// explicitly rather than read off a connection. `None` means this
/// connection has no default of that kind at all (TASK-1.45 §1): nothing
/// on the wire supplies a session, tab or pane today - the hello (decision-2
/// §7, doc-14 "Clients") carries only `kind`/`label`, and `STEG_SESSION`/
/// `TAB`/`PANE` (doc-2 §4) are environment variables the server puts into a
/// *pane's own program's* environment, never sent by a client. A `Context`
/// naming a real-looking id nobody gave would be a false statement, so
/// absence is represented, not filled with a placeholder id - see
/// [`Context::none`] and `dispatch/lookup.rs`'s `target_session`/
/// `target_tab`/`target_pane`, which refuse rather than guess when a
/// default is needed and missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    /// The session a request with no session part of its target acts on,
    /// or `None` when this connection has no session context at all.
    pub default_session: Option<SessionId>,
    /// The tab a request with no tab part of its target acts on, or `None`.
    pub default_tab: Option<TabId>,
    /// The pane a request with no pane part of its target acts on, or `None`.
    pub default_pane: Option<PaneId>,
}

impl Context {
    /// A connection with nothing to fall back on at all (TASK-1.45 §1):
    /// every target-less request, and every partial target that would need
    /// a default this connection does not have, is refused with
    /// `ErrorCode::BadRequest` (doc-2 section 4/6's own "no context" case
    /// at the CLI, enforced here too) rather than resolved against an
    /// invented value.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            default_session: None,
            default_tab: None,
            default_pane: None,
        }
    }
}

/// Every `(noun, verb)` pair this task dispatches, built from every
/// per-noun `HANDLED` — the method set this task actually matches on,
/// checked against `description().methods` both ways by this module's own
/// tests (AC #2). Used only by that test: the match in [`dispatch`] below
/// handles `server`/`event`/`client` inline, so nothing in the production
/// path reads this list.
#[cfg(test)]
fn handled() -> Vec<(&'static str, &'static str)> {
    let mut all = Vec::new();
    all.extend(session::HANDLED.iter().copied());
    all.extend(tab::HANDLED.iter().copied());
    all.extend(pane::HANDLED.iter().copied());
    all.extend(api::HANDLED.iter().copied());
    all.extend(setting::HANDLED.iter().copied());
    all.extend(server::HANDLED.iter().copied());
    all.extend(NOT_YET_NOUNS.iter().copied());
    all.extend(INTERCEPTED_BEFORE_DISPATCH.iter().copied());
    all
}

/// Described, fully built methods that never reach [`dispatch`] at all:
/// `connection.rs`'s own read loop intercepts them before falling through to
/// [`Runtime::handle`][crate::runtime::Runtime::handle], because answering
/// them needs something `dispatch`'s own signature has no way to carry.
/// `pane.attach`/`pane.detach` (TASK-1.47) need a connection's own
/// `Arc<Outbound>` and its own per-connection attachment bookkeeping, both
/// created only inside `connection.rs`, after the handshake. `tab.declare
/// -size` (TASK-1.114) needs only a `ClientId` - `Context` carries session/
/// tab/pane defaults and nothing else, so `Tree::declare_size`/`Tree::
/// note_input` are just as unreachable from here, one step further back.
/// The same structural reason `event.watch` is expected to leave
/// [`NOT_YET_NOUNS`] for this list too, once its own server-side task
/// lands. `server.stop` (TASK-1.88) joins this list for the identical
/// reason: it needs the connection's own `Arc<Outbound>` (to flush its
/// reply before anything closes) and the server's `Stopper` (to actually
/// end the accept loop), neither reachable from `dispatch`'s plain
/// `(&mut Tree, ...)` signature. Test-only, for the same reason as
/// [`handled`].
#[cfg(test)]
const INTERCEPTED_BEFORE_DISPATCH: &[(&str, &str)] = &[
    ("pane", "attach"),
    ("pane", "detach"),
    // TASK-1.114: `Tree::declare_size` needs a `ClientId`, which `dispatch`'s
    // own signature has no way to carry at all (`Context` holds only
    // session/tab/pane defaults) - the same structural reason `pane.attach`/
    // `pane.detach` are on this list, one step further back (they also
    // needed a connection's own `Arc<Outbound>`; this needs neither, only
    // the `ClientId` itself).
    ("tab", "declare-size"),
    ("server", "stop"),
];

/// The nouns/methods with no served arm at all in this task (`event`,
/// `client`), plus `server.stop` is NOT here (see
/// [`INTERCEPTED_BEFORE_DISPATCH`] instead) - `server.show`/`server.start`
/// are not here either (TASK-1.88: both are now plain `HANDLED` arms in
/// [`server`]). Every pair here answers `ErrorCode::NotImplementedYet`,
/// naming what it waits on. Test-only, for the same reason as [`handled`].
#[cfg(test)]
const NOT_YET_NOUNS: &[(&str, &str)] = &[
    ("event", "watch"),
    ("client", "list"),
    ("client", "show"),
];

/// Builds the `ApiError` a not-yet-built method or branch answers with
/// (AC #2's second half): a distinct code naming what it waits on, never
/// an invented value.
pub(crate) fn not_implemented_yet(waiting_on: &str) -> ApiError {
    ApiError::new(
        ErrorCode::NotImplementedYet,
        format!("not implemented yet: waiting on {waiting_on}"),
    )
}

/// A `BadRequest` naming `message` — for a malformed value `validate`
/// itself cannot catch (such as `api.call`'s own `method`, checked before
/// the generic validation step runs at all; see `api::call`).
pub(crate) fn bad_request(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::BadRequest, message.into())
}

/// `ErrorCode::ProgramRunning`, with the message every `pane.close`/
/// `tab.close`/`session.close` refusal in this task uses (TASK-1.76 §5):
/// a program is still running somewhere in scope and `--force` was not
/// given, so nothing was touched. The code's own doc comment, verbatim:
/// "A program is still running and `--force` was not given."
pub(crate) fn program_running() -> ApiError {
    ApiError::new(
        ErrorCode::ProgramRunning,
        "a program is still running; use --force",
    )
}

/// `ErrorCode::ProgramNotRunning` (TASK-1.90): the mirror of
/// [`program_running`], used by `pane.send`'s own refusal - nothing is
/// currently running in the target pane, so there is nothing to deliver
/// bytes to.
pub(crate) fn program_not_running() -> ApiError {
    ApiError::new(
        ErrorCode::ProgramNotRunning,
        "nothing is currently running in this pane",
    )
}

/// The core of [`dispatch`] (TASK-1.46): validates, then routes to the
/// matching noun's own arm, returning the *typed* [`Outcome`] rather than
/// the `Value` it becomes. `dispatch` itself is the one public entry point
/// every existing caller keeps using unchanged; this is `pub(crate)`
/// specifically so `Runtime::close_and_dispatch` can read a close's own
/// kernel-reported cascade (`Outcome::PaneClosed`/`TabClosed`) directly,
/// rather than re-deriving it with a second read of the tree after the
/// fact - the bug a review of this task found and this split fixes.
///
/// Never called with `request.method == "api.call"`: `api.call` recurses
/// into another method entirely and returns that inner call's own
/// `Response`, which has no single `Outcome` to give back - callers that
/// might still be holding an unresolved `api.call` request (the generic
/// path in `Runtime::handle`) go through [`dispatch`] instead, which
/// resolves it first. `close_and_dispatch` and `start_and_dispatch` never
/// have this problem: both are only ever reached with an already-resolved
/// method name (`Runtime::handle` resolves `api.call` once, up front,
/// before either match arm is chosen).
///
/// # Panics
///
/// Never in practice: splitting `request.method` on `.` always succeeds
/// once `validate` has already confirmed the method is in `description`
/// (every described method's name is `{noun}.{verb}`, R101), and every
/// described method is matched by one of the per-noun `HANDLED` lists, the
/// remaining not-yet nouns, or `pane.attach`/`pane.detach`/
/// `tab.declare-size`/`server.stop`'s own arms (TASK-1.47/TASK-1.114/
/// TASK-1.88, each built but never reached this way in the real system),
/// which `mod.rs`'s own completeness test checks against `description()`
/// directly. Panics instead, rather than silently misrouting, if ever
/// called with `"api.call"` itself - the one case the doc above says
/// never to pass here.
pub(crate) fn dispatch_raw(
    tree: &mut Tree,
    settings: &mut Settings,
    description: &Description,
    context: &Context,
    request: &Request,
    now: steg_kernel::Moment,
) -> Result<Outcome, ApiError> {
    debug_assert_ne!(
        request.method, "api.call",
        "dispatch_raw is never called with an unresolved api.call - dispatch resolves it first"
    );

    validate(description, request)?;

    let noun_verb = request
        .method
        .split_once('.')
        .expect("validate already confirmed this method is in the description");

    match noun_verb {
        (noun, verb) if session::HANDLED.contains(&(noun, verb)) => {
            session::dispatch(tree, context, request, verb)
        }
        (noun, verb) if tab::HANDLED.contains(&(noun, verb)) => {
            tab::dispatch(tree, context, request, verb, now)
        }
        (noun, verb) if pane::HANDLED.contains(&(noun, verb)) => {
            pane::dispatch(tree, context, request, verb, now)
        }
        (noun, verb) if api::HANDLED.contains(&(noun, verb)) => {
            api::dispatch(description, request, verb)
        }
        (noun, verb) if setting::HANDLED.contains(&(noun, verb)) => {
            setting::dispatch(settings, request, verb)
        }
        (noun, verb) if server::HANDLED.contains(&(noun, verb)) => {
            server::dispatch(tree, now, verb)
        }
        ("server", "stop") => Err(not_implemented_yet(
            "connection.rs's own interception (TASK-1.88) - a described method, fully built, \
             but never reached through dispatch in the real system: answering it needs the \
             connection's own Arc<Outbound> (to flush its reply) and the server's Stopper (to \
             actually stop), neither reachable from this function's signature",
        )),
        // `event.watch` (TASK-1.46) is never actually routed here in
        // production: it needs a connection's own registered subscriber
        // (its `Outbound`) to reply atomically with the filter it sets,
        // which this function has no access to (`dispatch` is a plain
        // function over `&mut Tree`, no socket in its signature) -
        // `Runtime::subscribe` answers it directly, before `dispatch`
        // would ever be called with it. This arm stays only so the
        // method is still "handled" by `mod.rs`'s own completeness test
        // and so a test that calls `dispatch` directly with it (bypassing
        // `Runtime`) gets a clear refusal rather than a panic.
        ("event", _) => Err(not_implemented_yet(
            "event.watch is answered by Runtime::subscribe, not dispatch::dispatch - this \
             path is unreachable through a real connection",
        )),
        ("client", _) => Err(not_implemented_yet("a client model (TASK-1.42)")),
        ("pane", "attach" | "detach") => Err(not_implemented_yet(
            "connection.rs's own interception (TASK-1.47) - a described method, fully built, \
             but never reached through dispatch in the real system: answering it needs a \
             connection's own Arc<Outbound> and attachment bookkeeping, which only exist in \
             connection.rs, created after the handshake",
        )),
        ("tab", "declare-size") => Err(not_implemented_yet(
            "connection.rs's own interception (TASK-1.114) - a described method, fully built, \
             but never reached through dispatch in the real system: answering it needs a \
             ClientId, which this function's own signature has no way to carry at all",
        )),
        _ => unreachable!(
            "validate already refused any method not in the description, and every \
             described method is in one of the lists above"
        ),
    }
}

/// Dispatches one request against `description`, over `tree`, with
/// `context`'s defaults for any target part left out. `settings` is the
/// server's remembered overrides (R80, decision-9; TASK-1.48) — orthogonal
/// to `tree`: a setting's target is its own name, never a session/tab/pane
/// path, so it needs no `Context` either.
#[must_use]
pub fn dispatch(
    tree: &mut Tree,
    settings: &mut Settings,
    description: &Description,
    context: &Context,
    request: &Request,
    now: steg_kernel::Moment,
) -> Response {
    if request.method == "api.call" {
        return api::call(tree, settings, description, context, request, now);
    }

    match dispatch_raw(tree, settings, description, context, request, now) {
        Ok(outcome) => Response::Ok(outcome.into_value()),
        Err(error) => Response::Error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{handled, not_implemented_yet};
    use steg_sdk::{ErrorCode, description};

    #[test]
    fn every_described_method_has_exactly_one_handler() {
        let described: Vec<String> = description()
            .methods
            .iter()
            .map(|m| m.name.dotted())
            .collect();
        let handled: Vec<String> = handled()
            .iter()
            .map(|(noun, verb)| format!("{noun}.{verb}"))
            .collect();

        let mut described_sorted = described.clone();
        described_sorted.sort();
        let mut handled_sorted = handled.clone();
        handled_sorted.sort();

        assert_eq!(
            described_sorted, handled_sorted,
            "every described method must have exactly one handler, and every handler must \
             name a described method"
        );
    }

    #[test]
    fn not_implemented_yet_names_what_it_waits_on() {
        let error = not_implemented_yet("the terminal engine (TASK-1.40)");
        assert_eq!(error.code, ErrorCode::NotImplementedYet);
        assert!(error.message.contains("the terminal engine (TASK-1.40)"));
    }
}
