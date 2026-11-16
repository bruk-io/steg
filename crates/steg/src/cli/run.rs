//! Part 1's own seam for "call the method for real": there is no
//! connecting logic in this task (TASK-1.49b's), so a command that
//! parsed and validated locally - clap's own value parsers, built from
//! the description's `ValueKind`s, already ran - needs somewhere to go
//! that is not silently nothing. [`run`] is that seam, meant to be
//! replaced outright by TASK-1.49b, not built around.

use clap::ArgMatches;
use steg_sdk::Method;

use super::params::extract_params;

/// Prints nothing on stdout, names the method on stderr, and returns 1.
///
/// Reasoning for 1, not 0 or 2 (doc-2 section 6): nothing succeeded, so
/// not 0; the command and its arguments were entirely valid - local
/// parsing and validation against the description's own value kinds
/// (closed sets, required-ness, switch pairs, the trailing program)
/// already ran and passed - so not 2 ("bad usage") either. 1, "the
/// command ran and failed", is section 6's literal fit for "parsed and
/// validated, then could not proceed."
#[must_use]
pub fn run(method: &Method, matches: &ArgMatches) -> i32 {
    // Built and discarded here, not left unused: proves this seam really
    // is reached only once a command's params have already been read out
    // in the same shape TASK-1.49b's real call will use, not before.
    let _params = extract_params(method, matches);
    eprintln!(
        "steg: {} {} is valid, but connecting to a server is not built yet (TASK-1.49b)",
        method.name.noun, method.name.verb
    );
    1
}
