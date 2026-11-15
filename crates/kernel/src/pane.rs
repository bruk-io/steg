//! A pane: a region of a tab running one program.

use steg_sdk::{PaneId, PaneKind, Program, ProgramStatus};

use crate::error::Error;
use crate::label;

/// A pane: a region of a tab running one program (doc-15).
///
/// Carries its identity, its label, its kind, and what it runs. `program`
/// and `status` are `None` only in the window between a pane being minted
/// (`Tree::new_session`/`new_tab`/`split_pane`) and `Tree::start_program`
/// being called for it — never observed outside that one pairing, which the
/// server always does back to back, since the pane's own id (needed for
/// `STEG_PANE`) does not exist until minting returns. `start_program` is the
/// only way to leave that window, it can run exactly once per pane
/// ([`Error::ProgramAlreadyStarted`] guards a second call), and nothing else
/// runs between the two calls to observe a half-built pane (doc-15 R97;
/// TASK-1.40).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    id: PaneId,
    label: Option<String>,
    kind: PaneKind,
    program: Option<Program>,
    status: Option<ProgramStatus>,
}

impl Pane {
    pub(crate) fn new(id: PaneId, kind: PaneKind) -> Self {
        Self {
            id,
            label: None,
            kind,
            program: None,
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

    /// What steg knows about this pane's program right now (doc-15 R97),
    /// and `None` in the same window [`Pane::program`] is.
    #[must_use]
    pub const fn status(&self) -> Option<ProgramStatus> {
        self.status
    }

    /// Whether this pane's program ending should close the pane outright
    /// (R97: a plain shell closes; a given command stays). Only meaningful
    /// once `Pane::start_program` has run — `false` for a pane with no
    /// program yet, since there is nothing to close for.
    #[must_use]
    pub const fn closes_on_exit(&self) -> bool {
        matches!(self.program, Some(Program::Shell { .. }))
    }

    /// Records what this pane runs, for the first time (doc-9 R13).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProgramAlreadyStarted`] if this pane already has a
    /// program — a pane's program is started exactly once; running the same
    /// command again goes through [`Pane::restart`].
    pub(crate) fn start_program(&mut self, program: Program) -> Result<(), Error> {
        if self.program.is_some() {
            return Err(Error::ProgramAlreadyStarted(self.id));
        }
        self.program = Some(program);
        self.status = Some(ProgramStatus::Running);
        Ok(())
    }

    /// Records that this pane's program ended with `exit_code`. Does not
    /// decide whether the pane closes — a `Pane` cannot run the tab/session
    /// cascade R94 closing a pane may need; [`crate::Tree::record_exit`]
    /// does, using [`Pane::closes_on_exit`] right after calling this.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProgramNotStarted`] if this pane has no program yet.
    pub(crate) fn record_exit(&mut self, exit_code: i32) -> Result<(), Error> {
        if self.program.is_none() {
            return Err(Error::ProgramNotStarted(self.id));
        }
        self.status = Some(ProgramStatus::Ended { exit_code });
        Ok(())
    }

    /// Runs this pane's program again, in the same directory, with the same
    /// arguments (R97) — the stored [`Program`] is untouched, only
    /// [`ProgramStatus`] moves back to `Running`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProgramStillRunning`] unless this pane's status is
    /// `Ended` (covers "no program yet" too: that status is `None`, not
    /// `Ended`), or [`Error::ShellCannotRestart`] if this pane's program is
    /// [`Program::Shell`] — a shell pane closes when it exits (R97) rather
    /// than offering anything to restart.
    pub(crate) fn restart(&mut self) -> Result<(), Error> {
        if !matches!(self.status, Some(ProgramStatus::Ended { .. })) {
            return Err(Error::ProgramStillRunning(self.id));
        }
        if self.closes_on_exit() {
            return Err(Error::ShellCannotRestart(self.id));
        }
        self.status = Some(ProgramStatus::Running);
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
    use super::Pane;
    use crate::error::Error;
    use steg_sdk::{PaneId, PaneKind, Program, ProgramStatus};

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

    #[test]
    fn a_new_pane_has_its_kind_no_label_and_no_program_yet() {
        let pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        assert_eq!(pane.kind(), PaneKind::Terminal);
        assert_eq!(pane.label(), None);
        assert_eq!(pane.program(), None);
        assert_eq!(pane.status(), None);
        assert!(!pane.closes_on_exit());
    }

    #[test]
    fn starting_a_program_records_it_as_running() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        pane.start_program(command()).unwrap();

        assert_eq!(pane.program(), Some(&command()));
        assert_eq!(pane.status(), Some(ProgramStatus::Running));
    }

    #[test]
    fn starting_a_program_twice_errors_and_keeps_the_first() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command()).unwrap();

        assert_eq!(
            pane.start_program(shell()),
            Err(Error::ProgramAlreadyStarted(PaneId::from_raw(1)))
        );
        assert_eq!(pane.program(), Some(&command()));
    }

    #[test]
    fn recording_an_exit_before_starting_errors() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        assert_eq!(
            pane.record_exit(0),
            Err(Error::ProgramNotStarted(PaneId::from_raw(1)))
        );
    }

    #[test]
    fn a_command_pane_stays_ended_and_does_not_close_on_exit() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command()).unwrap();

        pane.record_exit(3).unwrap();

        assert_eq!(pane.status(), Some(ProgramStatus::Ended { exit_code: 3 }));
        assert!(!pane.closes_on_exit());
    }

    #[test]
    fn a_shell_pane_closes_on_exit() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(shell()).unwrap();

        pane.record_exit(0).unwrap();

        assert!(pane.closes_on_exit());
    }

    #[test]
    fn restarting_a_command_pane_resets_its_status_and_keeps_its_spec() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command()).unwrap();
        pane.record_exit(3).unwrap();

        pane.restart().unwrap();

        assert_eq!(pane.status(), Some(ProgramStatus::Running));
        assert_eq!(pane.program(), Some(&command()));
    }

    #[test]
    fn restarting_a_still_running_program_errors() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(command()).unwrap();

        assert_eq!(
            pane.restart(),
            Err(Error::ProgramStillRunning(PaneId::from_raw(1)))
        );
    }

    #[test]
    fn restarting_a_pane_with_no_program_yet_errors_as_still_running() {
        // `status` is `None`, not `Ended`, so this falls under the same
        // "not ended" guard as a program that is actually running - there
        // is nothing to restart either way.
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);

        assert_eq!(
            pane.restart(),
            Err(Error::ProgramStillRunning(PaneId::from_raw(1)))
        );
    }

    #[test]
    fn restarting_a_shell_pane_errors() {
        let mut pane = Pane::new(PaneId::from_raw(1), PaneKind::Terminal);
        pane.start_program(shell()).unwrap();
        pane.record_exit(0).unwrap();

        assert_eq!(
            pane.restart(),
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
}
