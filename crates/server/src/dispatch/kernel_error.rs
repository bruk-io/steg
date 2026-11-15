//! Mapping a kernel [`steg_kernel::Error`] to the [`ApiError`] a client
//! sees (AC #4).
//!
//! One arm per `Error` variant, with no wildcard arm: a thirteenth kernel
//! variant fails this match, which is the point — a new kernel error is
//! supposed to be refused here until someone decides what code it becomes,
//! not silently swallowed into `BadRequest`.
//!
//! This direction only. The reverse — every [`ErrorCode`] the description
//! lists has a kernel source that can produce it today — is not asserted:
//! `ErrorCode::ProgramRunning` now has two kernel sources below
//! (TASK-1.40's `Error::ProgramAlreadyStarted`/`ProgramStillRunning`),
//! but neither is reachable through a served method yet — dispatch calls
//! none of `Tree::start_program`/`record_exit`/`restart_program` (that is
//! a later task's job, not this one's) — and `ErrorCode::NotImplementedYet`
//! never comes from this function at all; it is dispatch's own synthetic
//! error for an arm or branch that is not built (`not_implemented_yet`, in
//! `mod.rs`).

use steg_kernel::Error;
use steg_sdk::{ApiError, ErrorCode};

/// Maps a kernel error to the `ApiError` a client sees, with the code the
/// description gives it (AC #4). `message` is `error`'s own `Display`
/// text; `candidates`, where the kernel error carries any, are each
/// candidate's full identifier (`Candidate::id`, already a `Display`
/// string) — not the label, which is how a client *shows* a candidate,
/// left to the CLI (TASK-1.49), not built here.
#[must_use]
pub fn map_kernel_error(error: Error) -> ApiError {
    match &error {
        Error::SessionNotFound(_) | Error::TabNotFound(_) | Error::PaneNotFound(_) => {
            ApiError::new(ErrorCode::NoSuchTarget, error.to_string())
        }
        Error::TargetNotFound { candidates, .. } => with_candidates(
            ApiError::new(ErrorCode::NoSuchTarget, error.to_string()),
            candidates,
        ),
        Error::TargetAmbiguous { candidates, .. } => with_candidates(
            ApiError::new(ErrorCode::AmbiguousTarget, error.to_string()),
            candidates,
        ),
        Error::PositionOutOfRange { .. } => {
            ApiError::new(ErrorCode::PositionOutOfRange, error.to_string())
        }
        Error::InvalidLabel(_) | Error::InvalidTargetPath(_) => {
            ApiError::new(ErrorCode::BadRequest, error.to_string())
        }
        Error::SplitTooSmall { .. } => ApiError::new(ErrorCode::SplitTooSmall, error.to_string()),
        Error::NoSplitInDirection { .. } => {
            ApiError::new(ErrorCode::NoSplitInDirection, error.to_string())
        }
        Error::PanesNotInSameTab { .. } => {
            ApiError::new(ErrorCode::PanesNotInSameTab, error.to_string())
        }
        Error::AlreadyInTab { .. } => ApiError::new(ErrorCode::AlreadyInTab, error.to_string()),
        Error::Engine(_) => ApiError::new(ErrorCode::EngineFailed, error.to_string()),
        // `ErrorCode::ProgramRunning` is documented as "a program is still
        // running and `--force` was not given" (shapes.rs): true of both
        // of these - starting a second program on a pane that already has
        // one, and restarting one whose program has not ended yet (or has
        // no program at all - `Pane::restart`'s own guard folds "no
        // program" and "still running" into this one variant, since a
        // caller can't restart either). `ProgramNotStarted` and
        // `ShellCannotRestart`, below, are the opposite state or an
        // unrelated one, so they don't belong here.
        Error::ProgramAlreadyStarted(_) | Error::ProgramStillRunning(_) => {
            ApiError::new(ErrorCode::ProgramRunning, error.to_string())
        }
        // No dedicated code exists for either: `ProgramNotStarted` is the
        // literal opposite of `ProgramRunning` (nothing is running at
        // all), and `ShellCannotRestart` has no `--force`-shaped flag
        // anywhere in doc-2 to attach it to. `BadRequest` is this file's
        // own existing bucket for "doesn't fit the described shape given
        // what's known" (the same reasoning `InvalidLabel`/
        // `InvalidTargetPath` above already rest on, and TASK-1.42's
        // `ClientNotViewingPane`/`ClientNotViewingTab` - not yet merged as
        // this is written, but reviewed with exactly this same reasoning
        // in its own comment there) - not a guess at a code that doesn't
        // exist. Neither is reachable through a served method today
        // (dispatch calls none of `Tree::start_program`/`restart_program`/
        // `record_exit`); the task that adds program methods to dispatch
        // is the one that decides whether either deserves a code of its
        // own.
        Error::ProgramNotStarted(_) | Error::ShellCannotRestart(_) => {
            ApiError::new(ErrorCode::BadRequest, error.to_string())
        }
    }
}

fn with_candidates(mut error: ApiError, candidates: &[steg_kernel::Candidate]) -> ApiError {
    error.candidates = candidates.iter().map(|c| c.id.clone()).collect();
    error
}

#[cfg(test)]
mod tests {
    use super::map_kernel_error;
    use steg_kernel::{Candidate, Error};
    use steg_sdk::{Direction, ErrorCode, PaneId, SessionId, TabId};

    #[test]
    fn not_found_ids_map_to_no_such_target_with_no_candidates() {
        let id = SessionId::from_raw(1);
        let mapped = map_kernel_error(Error::SessionNotFound(id));
        assert_eq!(mapped.code, ErrorCode::NoSuchTarget);
        assert!(mapped.candidates.is_empty());

        assert_eq!(
            map_kernel_error(Error::TabNotFound(TabId::from_raw(1))).code,
            ErrorCode::NoSuchTarget
        );
        assert_eq!(
            map_kernel_error(Error::PaneNotFound(PaneId::from_raw(1))).code,
            ErrorCode::NoSuchTarget
        );
    }

    #[test]
    fn target_not_found_maps_to_no_such_target_with_the_candidates_ids() {
        let candidates = vec![
            Candidate {
                id: "00000001".to_string(),
                label: Some("logs".to_string()),
            },
            Candidate {
                id: "00000002".to_string(),
                label: None,
            },
        ];
        let mapped = map_kernel_error(Error::TargetNotFound {
            part: "nonsense".to_string(),
            candidates,
        });

        assert_eq!(mapped.code, ErrorCode::NoSuchTarget);
        assert_eq!(mapped.candidates, vec!["00000001", "00000002"]);
    }

    #[test]
    fn target_ambiguous_maps_to_ambiguous_target_with_the_candidates_ids() {
        let candidates = vec![Candidate {
            id: "00000001".to_string(),
            label: Some("logs".to_string()),
        }];
        let mapped = map_kernel_error(Error::TargetAmbiguous {
            part: "logs".to_string(),
            candidates,
        });

        assert_eq!(mapped.code, ErrorCode::AmbiguousTarget);
        assert_eq!(mapped.candidates, vec!["00000001"]);
    }

    #[test]
    fn position_out_of_range_maps_to_position_out_of_range() {
        let mapped = map_kernel_error(Error::PositionOutOfRange {
            len: 2,
            position: 5,
        });
        assert_eq!(mapped.code, ErrorCode::PositionOutOfRange);
    }

    #[test]
    fn invalid_label_and_invalid_target_path_map_to_bad_request() {
        assert_eq!(
            map_kernel_error(Error::InvalidLabel("a/b".to_string())).code,
            ErrorCode::BadRequest
        );
        assert_eq!(
            map_kernel_error(Error::InvalidTargetPath(String::new())).code,
            ErrorCode::BadRequest
        );
    }

    #[test]
    fn split_too_small_maps_to_split_too_small() {
        let mapped = map_kernel_error(Error::SplitTooSmall {
            pane: PaneId::from_raw(1),
        });
        assert_eq!(mapped.code, ErrorCode::SplitTooSmall);
    }

    #[test]
    fn no_split_in_direction_maps_to_no_split_in_direction() {
        let mapped = map_kernel_error(Error::NoSplitInDirection {
            pane: PaneId::from_raw(1),
            direction: Direction::Left,
        });
        assert_eq!(mapped.code, ErrorCode::NoSplitInDirection);
    }

    #[test]
    fn panes_not_in_same_tab_maps_to_panes_not_in_same_tab() {
        let mapped = map_kernel_error(Error::PanesNotInSameTab {
            a: PaneId::from_raw(1),
            b: PaneId::from_raw(2),
        });
        assert_eq!(mapped.code, ErrorCode::PanesNotInSameTab);
    }

    #[test]
    fn already_in_tab_maps_to_already_in_tab() {
        let mapped = map_kernel_error(Error::AlreadyInTab {
            pane: PaneId::from_raw(1),
            tab: TabId::from_raw(1),
        });
        assert_eq!(mapped.code, ErrorCode::AlreadyInTab);
    }

    #[test]
    fn starting_a_second_program_or_restarting_one_still_running_maps_to_program_running() {
        let id = PaneId::from_raw(1);

        assert_eq!(
            map_kernel_error(Error::ProgramAlreadyStarted(id)).code,
            ErrorCode::ProgramRunning
        );
        assert_eq!(
            map_kernel_error(Error::ProgramStillRunning(id)).code,
            ErrorCode::ProgramRunning
        );
    }

    #[test]
    fn recording_an_exit_with_none_started_or_restarting_a_shell_maps_to_bad_request() {
        let id = PaneId::from_raw(1);

        assert_eq!(
            map_kernel_error(Error::ProgramNotStarted(id)).code,
            ErrorCode::BadRequest
        );
        assert_eq!(
            map_kernel_error(Error::ShellCannotRestart(id)).code,
            ErrorCode::BadRequest
        );
    }

    // No unit test constructs `Error::Engine(..)` here: its inner
    // `ghostty_vt::Error` would need `ghostty-vt` as a direct dependency
    // of `steg-server`, which doc-16's package table does not allow (R92:
    // the terminal engine is the kernel's dependency, not the server's).
    // The exhaustive match in `map_kernel_error` is itself the proof this
    // arm exists and compiles - a thirteenth kernel variant with no arm
    // fails the build, which is the property this function exists for.

    #[test]
    fn the_message_is_the_kernel_errors_own_display_text() {
        let id = SessionId::from_raw(1);
        let mapped = map_kernel_error(Error::SessionNotFound(id));
        assert_eq!(mapped.message, Error::SessionNotFound(id).to_string());
    }
}
