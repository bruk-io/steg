//! A pane: a region of a tab running one program.

use std::time::Duration;

use steg_sdk::{PaneId, PaneKind, Program, ProgramStatus, Status};

use crate::error::Error;
use crate::label;
use crate::moment::Moment;

/// R83: a `Finished` status ends at this mark, measured from the moment
/// the program exited, if nothing else has ended it first.
const FINISHED_LIFETIME: Duration = Duration::from_secs(10 * 60);

/// A pane's status (doc-11, decision-7), layered on top of
/// [`ProgramStatus`]: which of the three M1 shapes it is, and whatever
/// extra state that shape's own ending rule needs. `Running` and `Failed`
/// need nothing beyond their tag; `Finished` needs the moment it began, so
/// `status`/`next_status_deadline` can tell whether R83's ten-minute mark
/// has passed. An illegal pairing (a `Finished` with no moment at all) is
/// not representable, rather than a flag beside a field that's only
/// sometimes valid (`.claude/rules/rust.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusKind {
    /// The program is running.
    Running,
    /// The program exited with code 0, at `since`.
    Finished {
        /// When this status began - R83's ten-minute clock starts here.
        since: Moment,
    },
    /// The program exited with this nonzero code. Never carries a moment:
    /// R83 says this never ends on a timer, so nothing here would ever
    /// read one.
    Failed {
        /// The exit code the program ended with.
        exit_code: i32,
    },
}

/// One pane's current status record: the status itself, and when it began
/// in the tree's own activity order (`Tree::tick`) - used only to break a
/// tie between two panes at the same priority in `Tab::shown_status`
/// (decision-7 point 3: "how pressing, then newest, never first in first
/// out"). Never compared to a duration - that's `StatusKind::Finished`'s
/// own `since`, a real moment, doing a different job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StatusRecord {
    kind: StatusKind,
    tick: u64,
}

/// A pane: a region of a tab running one program (doc-15).
///
/// Carries its identity, its label, its kind, and what it runs. `program`
/// and `program_status` are `None` only in the window between a pane being
/// minted (`Tree::new_session`/`new_tab`/`split_pane`) and
/// `Tree::start_program` being called for it — never observed outside that
/// one pairing, which the server always does back to back, since the
/// pane's own id (needed for `STEG_PANE`) does not exist until minting
/// returns. `start_program` is the only way to leave that window, it can
/// run exactly once per pane ([`Error::ProgramAlreadyStarted`] guards a
/// second call), and nothing else runs between the two calls to observe a
/// half-built pane (doc-15 R97; TASK-1.40).
///
/// `status` is a second, separate idea from `program_status` (TASK-1.41,
/// doc-11): `program_status` is the one clock-free fact about the program
/// itself (is it running, or did it end with this code), and never
/// changes shape; `status` is what doc-11/decision-7 call a pane's
/// "status" - the same exit triggers it, but it then runs through its own
/// longer lifecycle (R83's ending rules) and can end - going back to
/// `None` - well before the program is run again. The two fields are
/// renamed and added for exactly this reason: a single field once called
/// `status`, typed `ProgramStatus`, would be the wrong name for either
/// meaning now that both exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    id: PaneId,
    label: Option<String>,
    kind: PaneKind,
    program: Option<Program>,
    program_status: Option<ProgramStatus>,
    status: Option<StatusRecord>,
}

impl Pane {
    pub(crate) fn new(id: PaneId, kind: PaneKind) -> Self {
        Self {
            id,
            label: None,
            kind,
            program: None,
            program_status: None,
            status: None,
        }
    }

    /// This pane's id.
    #[must_use]
    pub const fn id(&self) -> PaneId {
        self.id
    }

    /// This pane's label, if it has one.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// What this pane runs.
    ///
    /// Nothing that reads this may assume it is `Terminal` (R12): match it
    /// without a wildcard arm, so a new kind fails the build everywhere it
    /// matters.
    #[must_use]
    pub const fn kind(&self) -> PaneKind {
        self.kind
    }

    /// What this pane was started with, and `None` in the window this
    /// struct's own doc comment names.
    #[must_use]
    pub const fn program(&self) -> Option<&Program> {
        self.program.as_ref()
    }

    /// The one clock-free fact about this pane's program right now (doc-15
    /// R97): running, or ended with an exit code. `None` in the same
    /// window [`Pane::program`] is. See this struct's own doc comment for
    /// how this differs from [`Pane::status`].
    #[must_use]
    pub const fn program_status(&self) -> Option<ProgramStatus> {
        self.program_status
    }

    /// This pane's status as doc-11/decision-7 define it, as of `now`:
    /// `None` if there has never been one, or if the one there was has
    /// ended - by input ([`crate::Tree::note_input`]), by being
    /// acknowledged ([`crate::Tree::acknowledge_pane`]), or, for
    /// `Finished` only, because R83's ten-minute mark has passed as of
    /// `now`. A pure read: it never mutates the stored record, so an
    /// already-timed-out `Finished` simply stops being reported the
    /// moment `now` says it should, with nothing to sweep.
    ///
    /// If `now` is earlier than the moment this status began (the
    /// caller's own clock looked like it went backward), a `Finished`
    /// status is reported exactly as if little or no time had passed:
    /// still showing. Nothing here subtracts one `Moment` from another,
    /// so there is no operation that could panic on going backward, and
    /// no special case is needed to get that answer.
    #[must_use]
    pub fn status(&self, now: Moment) -> Option<Status> {
        self.status_for_ranking(now).map(|(status, _tick)| status)
    }

    /// The same filtering [`Pane::status`] does, plus the record's own
    /// `tick` (decision-7's tie-break), so [`crate::Tab::shown_status`]
    /// doesn't duplicate the expiry check to rank this pane against
    /// others in its tab.
    pub(crate) fn status_for_ranking(&self, now: Moment) -> Option<(Status, u64)> {
        let record = self.status?;
        let status = match record.kind {
            StatusKind::Running => Status::Running,
            StatusKind::Finished { since } => {
                if since.plus(FINISHED_LIFETIME) > now {
                    Status::Finished
                } else {
                    return None; // R83: the ten-minute mark has passed.
                }
            }
            StatusKind::Failed { exit_code } => Status::Failed { exit_code },
        };
        Some((status, record.tick))
    }

    /// When this pane's `Finished` status will end on its own, with no
    /// input or acknowledgement - the moment [`Pane::status`] would start
    /// returning `None` for it unprompted. `None` for `Running` or
    /// `Failed` (R83: "never on a timer"), for no status at all, or for a
    /// `Finished` deadline that has already passed as of `now` - at that
    /// point `status` already reports it as gone, so there is nothing
    /// left to schedule for it. Lets a caller with a real clock (the
    /// server) ask "when is the next thing due" across many panes without
    /// polling any of them.
    pub(crate) fn next_status_deadline(&self, now: Moment) -> Option<Moment> {
        let StatusKind::Finished { since } = self.status?.kind else {
            return None;
        };
        let deadline = since.plus(FINISHED_LIFETIME);
        (deadline > now).then_some(deadline)
    }

    /// Ends this pane's `Finished` or `Failed` status, if it has one -
    /// the one mutating operation, shared by "ends on input" and "ends on
    /// being told the person has dealt with it" (R83; decision-7 point
    /// 5). A no-op, not an error, when there is nothing to end: either no
    /// status at all, or `Running` - R83 gives `Running` no ending rule of
    /// its own (it ends only by the program itself exiting, through
    /// `Pane::record_exit`), so this leaves it exactly as it was rather
    /// than erasing it.
    pub(crate) fn clear_status(&mut self) {
        if matches!(
            self.status,
            Some(StatusRecord {
                kind: StatusKind::Finished { .. } | StatusKind::Failed { .. },
                ..
            })
        ) {
            self.status = None;
        }
    }

    /// Whether this pane's program ending should close the pane outright
    /// (R97: a plain shell closes; a given command stays). Only meaningful
    /// once `Pane::start_program` has run — `false` for a pane with no
    /// program yet, since there is nothing to close for.
    #[must_use]
    pub const fn closes_on_exit(&self) -> bool {
        matches!(self.program, Some(Program::Shell { .. }))
    }

    /// Records what this pane runs, for the first time (doc-9 R13), and
    /// sets its status to `Running` (doc-11), stamped with `tick` -
    /// `crate::Tree::start_program`'s own current tick (decision-6/R91's
    /// non-clock activity counter), reused here purely for decision-7's
    /// "newest" tie-break between panes, never compared to a duration.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProgramAlreadyStarted`] if this pane already has a
    /// program — a pane's program is started exactly once; running the same
    /// command again goes through [`Pane::restart`].
    pub(crate) fn start_program(&mut self, program: Program, tick: u64) -> Result<(), Error> {
        if self.program.is_some() {
            return Err(Error::ProgramAlreadyStarted(self.id));
        }
        self.program = Some(program);
        self.program_status = Some(ProgramStatus::Running);
        self.status = Some(StatusRecord {
            kind: StatusKind::Running,
            tick,
        });
        Ok(())
    }

    /// Records that this pane's program ended with `exit_code` at `now`,
    /// and updates its status (doc-11 R82) to `Finished` (code 0) or
    /// `Failed` (otherwise), stamped with `tick` the same way
    /// [`Pane::start_program`] is. Does not decide whether the pane closes
    /// — a `Pane` cannot run the tab/session cascade R94 closing a pane
    /// may need; [`crate::Tree::record_exit`] does, using
    /// [`Pane::closes_on_exit`] right after calling this.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProgramNotStarted`] if this pane has no program yet.
    pub(crate) fn record_exit(
        &mut self,
        exit_code: i32,
        now: Moment,
        tick: u64,
    ) -> Result<(), Error> {
        if self.program.is_none() {
            return Err(Error::ProgramNotStarted(self.id));
        }
        self.program_status = Some(ProgramStatus::Ended { exit_code });
        let kind = if exit_code == 0 {
            StatusKind::Finished { since: now }
        } else {
            StatusKind::Failed { exit_code }
        };
        self.status = Some(StatusRecord { kind, tick });
        Ok(())
    }

    /// Runs this pane's program again, in the same directory, with the same
    /// arguments (R97) — the stored [`Program`] is untouched, only
    /// [`ProgramStatus`] moves back to `Running`, and the status (doc-11)
    /// moves back to `Running` too, stamped with `tick`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProgramStillRunning`] unless this pane's program
    /// status is `Ended` (covers "no program yet" too: that status is
    /// `None`, not `Ended`), or [`Error::ShellCannotRestart`] if this
    /// pane's program is [`Program::Shell`] — a shell pane closes when it
    /// exits (R97) rather than offering anything to restart.
    pub(crate) fn restart(&mut self, tick: u64) -> Result<(), Error> {
        if !matches!(self.program_status, Some(ProgramStatus::Ended { .. })) {
            return Err(Error::ProgramStillRunning(self.id));
        }
        if self.closes_on_exit() {
            return Err(Error::ShellCannotRestart(self.id));
        }
        self.program_status = Some(ProgramStatus::Running);
        self.status = Some(StatusRecord {
            kind: StatusKind::Running,
            tick,
        });
        Ok(())
    }

    /// Sets or clears this pane's label.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLabel`] if `label` contains `/`, or is made
    /// only of digits (doc-2 section 4).
    pub fn set_label(&mut self, label: Option<String>) -> Result<(), Error> {
        self.label = label::validate(label)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FINISHED_LIFETIME, Pane};
    use crate::error::Error;
    use crate::moment::Moment;
    use std::time::Duration;
    use steg_sdk::{PaneId, PaneKind, Program, ProgramStatus, Status};

    fn shell() -> Program {
        Program::Shell {
            cwd: "/tmp".to_string(),
        }
    }

    fn command() -> Program {
        Program::Command {
            argv: vec!["htop".to_string()],
            cwd: "/tmp".to_string(),
        }
    }

    /// A `Moment` this many seconds after an arbitrary origin - plain
    /// data, no clock, used throughout this module's tests.
    fn moment(secs: u64) -> Moment {
        Moment::new(Duration::from_secs(secs))
    }

    #[test]
    fn a_new_pane_has_its_kind_no_label_no_program_and_no_status_yet() {
        let pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        assert_eq!(pane.kind(), PaneKind::Terminal);
        assert_eq!(pane.label(), None);
        assert_eq!(pane.program(), None);
        assert_eq!(pane.program_status(), None);
        assert_eq!(pane.status(moment(0)), None);
        assert!(!pane.closes_on_exit());
    }

    #[test]
    fn starting_a_program_records_it_as_running() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        pane.start_program(command(), 0).unwrap();

        assert_eq!(pane.program(), Some(&command()));
        assert_eq!(pane.program_status(), Some(ProgramStatus::Running));
        assert_eq!(pane.status(moment(0)), Some(Status::Running));
    }

    #[test]
    fn starting_a_program_twice_errors_and_keeps_the_first() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();

        assert_eq!(
            pane.start_program(shell(), 1),
            Err(Error::ProgramAlreadyStarted(PaneId::from_raw(1)))
        );
        assert_eq!(pane.program(), Some(&command()));
    }

    #[test]
    fn recording_an_exit_before_starting_errors() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        assert_eq!(
            pane.record_exit(0, moment(0), 0),
            Err(Error::ProgramNotStarted(PaneId::from_raw(1)))
        );
    }

    #[test]
    fn a_command_pane_stays_ended_and_does_not_close_on_exit() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();

        pane.record_exit(3, moment(0), 1).unwrap();

        assert_eq!(
            pane.program_status(),
            Some(ProgramStatus::Ended { exit_code: 3 })
        );
        assert!(!pane.closes_on_exit());
    }

    #[test]
    fn a_shell_pane_closes_on_exit() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(shell(), 0).unwrap();

        pane.record_exit(0, moment(0), 1).unwrap();

        assert!(pane.closes_on_exit());
    }

    #[test]
    fn restarting_a_command_pane_resets_its_status_and_keeps_its_spec() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(3, moment(0), 1).unwrap();

        pane.restart(2).unwrap();

        assert_eq!(pane.program_status(), Some(ProgramStatus::Running));
        assert_eq!(pane.status(moment(0)), Some(Status::Running));
        assert_eq!(pane.program(), Some(&command()));
    }

    #[test]
    fn restarting_a_still_running_program_errors() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();

        assert_eq!(
            pane.restart(1),
            Err(Error::ProgramStillRunning(PaneId::from_raw(1)))
        );
    }

    #[test]
    fn restarting_a_pane_with_no_program_yet_errors_as_still_running() {
        // `program_status` is `None`, not `Ended`, so this falls under the
        // same "not ended" guard as a program that is actually running -
        // there is nothing to restart either way.
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        assert_eq!(
            pane.restart(0),
            Err(Error::ProgramStillRunning(PaneId::from_raw(1)))
        );
    }

    #[test]
    fn restarting_a_shell_pane_errors() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(shell(), 0).unwrap();
        pane.record_exit(0, moment(0), 1).unwrap();

        assert_eq!(
            pane.restart(2),
            Err(Error::ShellCannotRestart(PaneId::from_raw(1)))
        );
    }

    #[test]
    fn set_label_changes_it() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        pane.set_label(Some("logs".to_string())).unwrap();

        assert_eq!(pane.label(), Some("logs"));
    }

    #[test]
    fn set_label_rejects_an_invalid_label() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        assert_eq!(
            pane.set_label(Some("123".to_string())),
            Err(crate::error::Error::InvalidLabel("123".to_string()))
        );
        assert_eq!(pane.label(), None);
    }

    // --- status (doc-11, decision-7; TASK-1.41) ---------------------------

    #[test]
    fn exiting_with_code_zero_is_finished() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();

        pane.record_exit(0, moment(100), 1).unwrap();

        assert_eq!(pane.status(moment(100)), Some(Status::Finished));
    }

    #[test]
    fn exiting_with_a_nonzero_code_is_failed() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();

        pane.record_exit(3, moment(100), 1).unwrap();

        assert_eq!(
            pane.status(moment(100)),
            Some(Status::Failed { exit_code: 3 })
        );
    }

    #[test]
    fn a_finished_status_is_still_shown_599_seconds_after_it_began() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(0, moment(0), 1).unwrap();

        assert_eq!(pane.status(moment(599)), Some(Status::Finished));
    }

    #[test]
    fn a_finished_status_is_gone_exactly_600_seconds_after_it_began() {
        // R83: "finished ... after 10 minutes" ends it at the mark, not
        // only strictly after it.
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(0, moment(0), 1).unwrap();

        assert_eq!(pane.status(moment(600)), None);
    }

    #[test]
    fn a_finished_status_is_gone_601_seconds_after_it_began() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(0, moment(0), 1).unwrap();

        assert_eq!(pane.status(moment(601)), None);
    }

    #[test]
    fn a_failed_status_never_expires_however_far_past_ten_minutes() {
        // R83: "never on a timer" - unlike `Finished`, there is no mark at
        // all, however far `now` is asked from `since`.
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(3, moment(0), 1).unwrap();

        assert_eq!(
            pane.status(moment(60 * 60 * 24 * 365)),
            Some(Status::Failed { exit_code: 3 })
        );
    }

    #[test]
    fn a_moment_earlier_than_when_the_status_began_still_reads_as_not_yet_expired() {
        // Stated rule: nothing here subtracts one `Moment` from another,
        // so a query "before" the status began (a caller's own clock
        // looking like it went backward) just reads the same comparison -
        // still well inside the ten-minute window - rather than panicking
        // or needing a special case.
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(0, moment(1_000), 1).unwrap();

        assert_eq!(pane.status(moment(1)), Some(Status::Finished));
    }

    #[test]
    fn restarting_moves_status_back_to_running_from_finished() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(0, moment(0), 1).unwrap();

        pane.restart(2).unwrap();

        assert_eq!(pane.status(moment(10_000)), Some(Status::Running));
    }

    #[test]
    fn clear_status_ends_a_finished_status() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(0, moment(0), 1).unwrap();

        pane.clear_status();

        assert_eq!(pane.status(moment(0)), None);
    }

    #[test]
    fn clear_status_ends_a_failed_status() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(3, moment(0), 1).unwrap();

        pane.clear_status();

        assert_eq!(pane.status(moment(0)), None);
    }

    #[test]
    fn clear_status_is_a_no_op_with_no_status() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        pane.clear_status();

        assert_eq!(pane.status(moment(0)), None);
    }

    #[test]
    fn clear_status_does_not_touch_a_running_status() {
        // R83 gives `Running` no ending rule: input/acknowledgement end a
        // `Finished` or `Failed` status, never a `Running` one.
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();

        pane.clear_status();

        assert_eq!(pane.status(moment(0)), Some(Status::Running));
    }

    #[test]
    fn next_status_deadline_is_none_with_no_status() {
        let pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        assert_eq!(pane.next_status_deadline(moment(0)), None);
    }

    #[test]
    fn next_status_deadline_is_none_while_running() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();

        assert_eq!(pane.next_status_deadline(moment(0)), None);
    }

    #[test]
    fn next_status_deadline_is_none_for_failed() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(3, moment(0), 1).unwrap();

        assert_eq!(pane.next_status_deadline(moment(0)), None);
    }

    #[test]
    fn next_status_deadline_is_ten_minutes_after_a_finished_exit() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(0, moment(100), 1).unwrap();

        assert_eq!(
            pane.next_status_deadline(moment(100)),
            Some(moment(100).plus(FINISHED_LIFETIME))
        );
    }

    #[test]
    fn next_status_deadline_is_none_once_it_has_already_passed() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(0, moment(0), 1).unwrap();

        assert_eq!(pane.next_status_deadline(moment(600)), None);
    }

    #[test]
    fn status_for_ranking_carries_the_tick_for_decision_7s_tie_break() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 7).unwrap();

        assert_eq!(
            pane.status_for_ranking(moment(0)),
            Some((Status::Running, 7))
        );
    }

    #[test]
    fn status_for_ranking_is_none_once_a_finished_status_has_expired() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command(), 0).unwrap();
        pane.record_exit(0, moment(0), 1).unwrap();

        assert_eq!(pane.status_for_ranking(moment(600)), None);
    }
}
