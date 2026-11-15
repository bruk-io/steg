//! The `api` noun's dispatch arms: the API's self-description, and
//! calling any method by name (doc-2 section 2; decision-1).

use steg_kernel::{Settings, Tree};
use steg_sdk::{ApiError, Description, ErrorCode, Request, Response};

use super::outcome::Outcome;
use super::{Context, bad_request, param_str};

/// Every `api.*` method this module matches on, checked against
/// `description().methods` by `mod.rs`'s own completeness test.
///
/// `("api", "call")` is here for that completeness check only: `api.call`
/// never reaches this module's own `dispatch` function — `mod.rs`'s
/// top-level `dispatch` intercepts it before `validate` ever runs, since
/// `api.call`'s own declared parameters collide with `validate` (see
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

/// Resolves an `api.call` request to the inner `Request` it names - the
/// one implementation both [`call`] (below) and `Runtime::handle`
/// (TASK-1.76) use, so every path that can mint and start a pane reads
/// the selector's own key the same way, through this function, rather
/// than as a literal string of its own (TASK-1.81 renames that key from
/// `target` to `method`; nothing outside this function needs to know
/// either name).
///
/// # Errors
///
/// Returns a `BadRequest` if the selector is missing or not text, or
/// names `api.call` itself.
pub(crate) fn resolve(request: &Request) -> Result<Request, ApiError> {
    let Some(target) = param_str(request, "target") else {
        return Err(bad_request("target: must be text"));
    };
    if target == "api.call" {
        return Err(bad_request("api.call cannot call api.call"));
    }

    let mut params = request.params.clone();
    params.remove("target");
    Ok(Request {
        method: target.to_string(),
        params,
    })
}

/// `api.call`'s own answer: resolves the request into the target
/// method's own `Request` ([`resolve`]) and dispatches that, returning
/// its `Response` unchanged — no `Outcome` wraps or re-encodes it
/// (TASK-1.75's plan, decision 3).
///
/// Runs *before* `mod.rs`'s generic `validate` step: `api.call` declares
/// only `target`, but a real call carries the target method's own
/// parameters alongside it, which `validate` would refuse against
/// `api.call`'s own schema.
pub(super) fn call(
    tree: &mut Tree,
    settings: &mut Settings,
    description: &Description,
    context: &Context,
    request: &Request,
) -> Response {
    match resolve(request) {
        Ok(inner) => super::dispatch(tree, settings, description, context, &inner),
        Err(error) => Response::Error(error),
    }
}
