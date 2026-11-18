//! Mapping an outcome to the exit status doc-2 section 6 gives it
//! (TASK-1.119 AC #2): the server-answered half is read straight from the
//! API description's own `errors` table, never written a second time; the
//! CLI-local half (0, clap's own 2, 4, 130) is mapped directly here, in
//! one place, since none of it comes from the description at all.

use steg_sdk::{ConnectError, Description, ErrorCode};

/// Looks `code` up in `api.errors` and returns that record's own
/// `exit_status` - the whole of "never written a second time" (AC #2).
///
/// # Panics
/// Never in practice: `description()`'s own completeness test
/// (`errors_has_thirteen_codes_with_their_exit_statuses`, `steg-sdk`)
/// guarantees every [`ErrorCode`] the server can answer with has a
/// matching record in `api.errors`.
#[must_use]
pub fn api_error_status(api: &Description, code: ErrorCode) -> i32 {
    i32::from(
        api.errors
            .iter()
            .find(|record| record.code == code)
            .unwrap_or_else(|| panic!("{code:?} has no ErrorRecord in the description"))
            .exit_status,
    )
}

/// Every [`ConnectError`] variant maps to 4 ("the server could not be
/// reached or started", doc-2 section 6) - including
/// [`ConnectError::ClosedBeforeHello`] (a server was there and then
/// wasn't, twice, which is the same outcome from this side) and
/// [`ConnectError::Io`] (point 7 of this task's plan: grouping every
/// "could not finish talking to a server" case under one status is the
/// simpler rule for a script to check against).
#[must_use]
pub const fn connect_error_status(err: &ConnectError) -> i32 {
    match err {
        ConnectError::NoServerRunning
        | ConnectError::Spawn(_)
        | ConnectError::Timeout
        | ConnectError::ClosedBeforeHello
        | ConnectError::Io(_) => 4,
    }
}

/// A hello reply that was read but refused at the protocol level (a
/// version mismatch, or a malformed hello) - point 6 of this task's plan:
/// not in the description's own `errors` table (that table is
/// server-answered API errors only), so mapped directly here. An unusable
/// server is, for the CLI's purposes, the same outcome as an unreachable
/// one.
#[must_use]
pub const fn protocol_refused_status() -> i32 {
    4
}

/// `api call`'s own stdin is not valid JSON, or not a JSON object - point
/// 5 of this task's plan: bad usage (the shape of what was given), never
/// a server failure.
#[must_use]
pub const fn malformed_api_call_input_status() -> i32 {
    2
}

#[cfg(test)]
mod tests {
    use steg_sdk::{ConnectError, description};

    use super::{api_error_status, connect_error_status};

    /// AC #2's "never written a second time," proved directly: every
    /// `ErrorRecord` the real description carries maps back to exactly
    /// its own `exit_status`, read through `api_error_status` rather than
    /// a parallel table.
    #[test]
    fn every_described_error_maps_to_its_own_exit_status() {
        let api = description();
        assert!(!api.errors.is_empty(), "sanity: the description has errors");
        for record in &api.errors {
            assert_eq!(
                api_error_status(&api, record.code),
                i32::from(record.exit_status),
                "{:?} should map to its own exit_status",
                record.code
            );
        }
    }

    /// Negative control: a description whose table says something
    /// different is followed, not overridden by a hard-coded match - the
    /// proof that `api_error_status` actually reads the table rather than
    /// happening to agree with it.
    #[test]
    fn api_error_status_follows_the_table_not_a_parallel_match() {
        let mut api = description();
        let record = api
            .errors
            .iter_mut()
            .find(|record| record.code == steg_sdk::ErrorCode::BadRequest)
            .expect("BadRequest is described");
        let original = record.exit_status;
        record.exit_status = original.wrapping_add(50);
        assert_eq!(
            api_error_status(&api, steg_sdk::ErrorCode::BadRequest),
            i32::from(original.wrapping_add(50)),
            "the function should follow the table once it's changed, not a value baked in elsewhere"
        );
    }

    #[test]
    fn every_connect_error_maps_to_4() {
        assert_eq!(connect_error_status(&ConnectError::NoServerRunning), 4);
        assert_eq!(
            connect_error_status(&ConnectError::Spawn(std::io::Error::other("boom"))),
            4
        );
        assert_eq!(connect_error_status(&ConnectError::Timeout), 4);
        assert_eq!(connect_error_status(&ConnectError::ClosedBeforeHello), 4);
        assert_eq!(
            connect_error_status(&ConnectError::Io(std::io::Error::other("severed"))),
            4
        );
    }
}
