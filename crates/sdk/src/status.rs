//! What steg knows about a pane's program, richer than [`crate::ProgramStatus`]
//! and with its own ending rules (doc-11 R82/R83; decision-7; doc-15's domain
//! table: "Status: what steg knows about a pane's program at this moment").

use serde::{Deserialize, Serialize};

/// A pane's status, in the first release (doc-11 R82, decision-7 point 1):
/// running while its program runs, and finished or failed from its exit
/// code once it has ended. `Waiting` and the bell are decision-7's second-
/// release additions and are not variants here; adding them later only adds
/// variants, it does not change these three.
///
/// Built on top of [`crate::ProgramStatus`], not in place of it:
/// `ProgramStatus`'s own doc comment names this as "a richer `Status` ...
/// built on top of this in a later task" — this is that task. `ProgramStatus`
/// keeps meaning exactly what it means today (the one clock-free fact: is
/// the program running, or did it end with this code) and still backs
/// `pane.exit_code` and `Pane::restart`'s own checks. `Status` is the
/// separate, longer-lived thing a pane's program being in that state
/// triggers (R83's ending rules), which is why the two can and do disagree:
/// a command can have ended (`ProgramStatus::Ended`) while its `Status` has
/// already ended too, on a key press or after ten minutes, well before the
/// program is run again.
///
/// Carries no timing data on purpose: this crosses the API (doc-11, doc-16
/// R105), and `std::time::Instant` (or anything built from one) has no
/// serde implementation and means nothing off the process that made it, so
/// nothing timing-shaped can ever appear here. Every status's "since", and
/// Finished's 10-minute deadline, live only inside the kernel's own `Pane`
/// (`crates/kernel/src/pane.rs`, TASK-1.41), never on the wire.
///
/// Crosses the API as a pane's own field (doc-2 section 8: "status") and a
/// tab's (`shown_status` names the pane, not this value directly), so it is
/// defined once, here (doc-16 R105), the same reasoning as [`crate::Program`]
/// and [`crate::ProgramStatus`]. Serde's default, externally-tagged
/// representation is used for the same reason those two give: `Failed`
/// alone carries data, so an internally-tagged form would need every
/// variant to serialize as a map, and `Running`/`Finished` aren't reducible
/// to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The program is running (R82).
    Running,
    /// The program ended with exit code 0 (R82, decision-7's state
    /// diagram: "it exits with code 0"). Ends on a key press or click in
    /// the pane, or after ten minutes, never carrying the code itself —
    /// it's always 0 by construction, so there is nothing to carry.
    Finished,
    /// The program ended with a nonzero exit code (R82). Ends only on a
    /// key press or click in the pane, or on being told the person has
    /// dealt with it; never on a timer (R83).
    Failed {
        /// The exit code the program ended with.
        exit_code: i32,
    },
}

#[cfg(test)]
mod tests {
    use super::Status;

    #[test]
    fn running_serde_round_trips_through_its_own_word() {
        let json = serde_json::to_string(&Status::Running).unwrap();
        assert_eq!(json, "\"running\"");
        assert_eq!(
            serde_json::from_str::<Status>(&json).unwrap(),
            Status::Running
        );
    }

    #[test]
    fn finished_serde_round_trips_through_its_own_word() {
        let json = serde_json::to_string(&Status::Finished).unwrap();
        assert_eq!(json, "\"finished\"");
        assert_eq!(
            serde_json::from_str::<Status>(&json).unwrap(),
            Status::Finished
        );
    }

    #[test]
    fn failed_serde_round_trips_with_its_exit_code() {
        let status = Status::Failed { exit_code: 3 };

        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(serde_json::from_str::<Status>(&json).unwrap(), status);
    }

    #[test]
    fn failed_serialises_as_an_object_keyed_failed() {
        let json = serde_json::to_value(Status::Failed { exit_code: 0 }).unwrap();

        assert!(json.get("failed").is_some(), "{json}");
    }

    #[test]
    fn a_negative_exit_code_round_trips() {
        // The same reason `ProgramStatus::Ended`'s own test gives: not
        // every platform's exit code fits in a u8, and i32 is wider on
        // purpose so a negative value is never lost on the wire.
        let status = Status::Failed { exit_code: -1 };

        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(serde_json::from_str::<Status>(&json).unwrap(), status);
    }
}
