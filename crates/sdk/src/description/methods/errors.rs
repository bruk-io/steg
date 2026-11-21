//! Every error the API can answer with, and the CLI exit status it
//! becomes (doc-2 section 6).

use crate::description::shapes::{ErrorCode, ErrorRecord};

pub(super) fn errors() -> Vec<ErrorRecord> {
    vec![
        ErrorRecord {
            code: ErrorCode::BadRequest,
            summary: "The request did not match the API: an unknown method, an unknown or \
                      missing parameter, or a value outside a closed set"
                .into(),
            exit_status: 2,
        },
        ErrorRecord {
            code: ErrorCode::NoSuchTarget,
            summary: "The target matched nothing".into(),
            exit_status: 3,
        },
        ErrorRecord {
            code: ErrorCode::AmbiguousTarget,
            summary: "The target matched more than one thing, so nothing was done".into(),
            exit_status: 3,
        },
        ErrorRecord {
            code: ErrorCode::ProgramRunning,
            summary: "A program is still running and --force was not given".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::ProgramNotRunning,
            summary: "Nothing is currently running in the pane".into(),
            // The mirror of ProgramRunning, which is also 1, not 2
            // ("bad usage") - the request is well-formed; the pane's own
            // current state is what refuses it (doc-2, "pane send refuses
            // a pane with no running program, with status 1").
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::PositionOutOfRange,
            summary: "The position given is beyond the end of its siblings".into(),
            exit_status: 2,
        },
        ErrorRecord {
            code: ErrorCode::SplitTooSmall,
            summary: "The split would leave a pane under the smallest size allowed".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::NoSplitInDirection,
            summary: "There is no split bordering the pane on that side".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::PanesNotInSameTab,
            summary: "The two panes named are not in the same tab".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::AlreadyInTab,
            summary: "The pane is already in the tab it was asked to move to".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::NotImplementedYet,
            summary: "The method is described, but nothing yet implements it".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::EngineFailed,
            summary: "The terminal engine failed".into(),
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::ProgramFailed,
            summary: "The program could not be started".into(),
            // The request was well-formed; the OS refused it - doc-2
            // section 6's "the command ran and failed", the same status
            // ProgramRunning/SplitTooSmall/EngineFailed already use, not
            // status 2 ("bad usage"), which BadRequest already owns.
            exit_status: 1,
        },
        ErrorRecord {
            code: ErrorCode::TimedOut,
            summary: "A wait reached its own timeout before the condition was met".into(),
            // doc-2 section 6's own exact wording for pane.wait: "a wait
            // that times out is status 1" - the same bucket as every
            // other state-based refusal in this table, never status 2
            // (BadRequest's own "bad usage").
            exit_status: 1,
        },
    ]
}
