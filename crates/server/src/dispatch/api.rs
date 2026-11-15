//! The `api` noun's dispatch arms: the API's self-description, and
//! calling any method by name (doc-2 section 2; decision-1).
//!
//! `api.call`'s own selector is `method`, not `target` (TASK-1.81): it
//! forwards every other parameter given flat into the called method's own
//! `params` ([`call`]), and that method may itself declare a parameter
//! literally named `target` (`pane.split`, `setting.show`, ...). Naming
//! the selector `method` instead keeps the two apart with no nesting — the
//! description's own `api.call` entry (`steg-sdk`) says the same thing.

use steg_kernel::{Settings, Tree};
use steg_sdk::{ApiError, Description, ErrorCode, Request, Response};

use super::outcome::Outcome;
use super::{Context, bad_request, param_str};

/// Every `api.*` method this module matches on, checked against
/// `description().methods` by `mod.rs`'s own completeness test.
///
/// `("api", "call")` is here for that completeness check only: `api.call`
/// never reaches this module's own `dispatch` function — `mod.rs`'s
/// top-level `dispatch` intercepts it before `validate` ever runs, since a
/// real call's own extra parameters (the called method's own) are not
/// declared on `api.call` itself, which `validate` would refuse (see
/// [`call`]). It is still "handled", by [`call`], just not through this
/// match.
pub(super) const HANDLED: &[(&str, &str)] = &[("api", "list"), ("api", "show"), ("api", "call")];

pub(super) fn dispatch(
    description: &Description,
    request: &Request,
    verb: &str,
) -> Result<Outcome, ApiError> {
    match verb {
        "list" => Ok(Outcome::Methods(description.methods.clone())),
        "show" => show(description, request),
        "call" => unreachable!(
            "api.call is intercepted by mod.rs's top-level dispatch before this match is ever reached"
        ),
        _ => unreachable!("HANDLED names every verb this match covers"),
    }
}

fn show(description: &Description, request: &Request) -> Result<Outcome, ApiError> {
    match param_str(request, "target") {
        None => Ok(Outcome::Description(Box::new(description.clone()))),
        Some(name) => description
            .method(name)
            .map(|method| Outcome::Method(Box::new(method.clone())))
            .ok_or_else(|| {
                ApiError::new(ErrorCode::NoSuchTarget, format!("no such method: {name}"))
            }),
    }
}

/// `api.call`'s own answer: rewrites the request into the called method's
/// own `Request` and dispatches that, returning its `Response` unchanged
/// — no `Outcome` wraps or re-encodes it (TASK-1.75's plan, decision 3).
///
/// Runs *before* `mod.rs`'s generic `validate` step: `api.call` declares
/// only `method`, but a real call carries the called method's own
/// parameters alongside it, which `validate` would refuse against
/// `api.call`'s own schema.
///
/// The selector is read and stripped under the key `method`, not
/// `target` (TASK-1.81): the called method may itself declare a parameter
/// literally named `target` (`pane.split`, `setting.show`, ...), and
/// stripping that key instead would both lose it from the inner request
/// and leave `api.call`'s own selector sitting in `params` unexamined.
pub(super) fn call(
    tree: &mut Tree,
    settings: &mut Settings,
    description: &Description,
    context: &Context,
    request: &Request,
) -> Response {
    let Some(method_name) = param_str(request, "method") else {
        return Response::Error(bad_request("method: must be text"));
    };
    if method_name == "api.call" {
        return Response::Error(bad_request("api.call cannot call api.call"));
    }

    let mut params = request.params.clone();
    params.remove("method");
    let inner = Request {
        method: method_name.to_string(),
        params,
    };

    super::dispatch(tree, settings, description, context, &inner)
}
