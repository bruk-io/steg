//! The tree: every session, tab and pane a server owns (doc-15, R85).

use steg_sdk::{
    ClientId, ClientKind, Direction, PaneId, PaneKind, Program, Ratio, SessionId, Size, TabId,
};

use crate::client::Client;
use crate::colour::next_colour;
use crate::error::Error;
use crate::ids::Ids;
use crate::label;
use crate::moment::Moment;
use crate::session::{Session, TabClosed};
use crate::tab::{DirectionalFocus, Tab};

/// What happened when a pane was closed, and what else closed because of
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosedPane {
    /// The pane that was closed.
    pub pane: PaneId,
    /// The tab that was also closed, if closing the pane left it empty
    /// (R94).
    pub tab: Option<TabId>,
    /// The session that was also closed, if closing the tab left it empty
    /// (R94).
    pub session: Option<SessionId>,
}

/// The whole path a focus operation left current (R85): a session's
/// current tab and that tab's current pane. Returned by `Tree::focus_pane`,
/// `Tree::focus_tab`, and (inside `FocusMove`) `Tree::focus_direction`, so a
/// caller — the server, raising one event per TASK-1.46 — never has to look
/// the rest of the path up afterward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Focused {
    /// The session whose current tab this is.
    pub session: SessionId,
    /// The tab that is now current in that session.
    pub tab: TabId,
    /// The pane that is now current in that tab.
    pub pane: PaneId,
}

/// What `Tree::focus_direction` found and did (R9, R113, doc-2 section 4):
/// the same three outcomes `Tab::neighbour`'s [`DirectionalFocus`] finds,
/// carrying the resulting whole path on the two outcomes that moved
/// something, rather than a bare pane id, so a caller needs no further
/// lookup. Kept as its own type, not a generic form of
/// [`DirectionalFocus`]: the two live at different levels (a tab's own
/// geometry versus the whole tree), for different callers, and nothing
/// here needs them unified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMove {
    /// Moved to the pane in that direction; this is the path now current.
    Neighbour(Focused),
    /// Wrap was asked for, and this is the path landed on at the tab's far
    /// edge.
    Wrapped(Focused),
    /// Nothing that way (and, if wrap was asked for, nothing to wrap to
    /// either): nothing changed. Not an error — doc-2 section 4 says the
    /// command exits 0 and stays put.
    Edge,
}

/// What happened when a pane's program ended (doc-15 R97; TASK-1.40):
/// `Tree::record_exit`'s result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitOutcome {
    /// The pane stays (it was started with a given command): its program
    /// can be run again with `Tree::restart_program`.
    Stayed,
    /// The pane was a plain shell, so it closed - the exact cascade a
    /// manual `Tree::close_pane` runs, not a second copy of it.
    Closed(ClosedPane),
}

/// What happened when a pane moved to another tab (R9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovedPane {
    /// The pane that moved.
    pub pane: PaneId,
    /// The tab it moved to.
    pub to: TabId,
    /// The source tab that was also closed, if the move left it empty
    /// (R94) — the same cascade `close_pane` runs.
    pub source_tab_closed: Option<TabId>,
    /// The source session that was also closed, if closing the source
    /// tab left it empty (R94).
    pub source_session_closed: Option<SessionId>,
}

/// What happened when a tab's size changed (TASK-1.42, doc-15 R91): a
/// client declared a new size and is latest, a client's input or
/// departure made a different client's already-declared size apply, or
/// the previously-latest client left and the fallback did either of
/// those. Carries everything a caller (the server, TASK-1.46/1.47) needs
/// to resize each terminal and tell every other viewer, with no further
/// lookup — the same reason `Focused`/`ClosedPane`/`MovedPane` exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeChanged {
    /// The tab whose size changed.
    pub tab: TabId,
    /// Its new size, in whole cells.
    pub size: Size,
    /// Every pane's absolute size at the new total
    /// (`Layout::leaf_sizes(size)`) — what the server resizes each pane's
    /// `Terminal` to (TASK-1.40's `PaneHandle::resize`).
    pub panes: Vec<(PaneId, Size)>,
    /// Every client currently viewing this tab (R91: "every other client
    /// draws the terminal at that size") — who to tell.
    pub viewers: Vec<ClientId>,
}

/// The tree: every session, tab, pane and client a server owns.
///
/// This is the whole structure doc-15 calls "a server's tree" (R85) and
/// later tasks call simply "the tree" (TASK-1.46's title).
#[derive(Debug)]
pub struct Tree {
    ids: Ids,
    sessions: Vec<Session>,
    clients: Vec<Client>,
    /// Advances every time a client sends input or declares a size
    /// (TASK-1.42, decision-6): not a clock, nothing reads real time here —
    /// the kernel advancing its own counter, the same shape `Ids`' own
    /// `next_raw` already is, just for "more recent than", never for an
    /// identifier.
    activity: u64,
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

impl Tree {
    /// An empty tree, with no sessions and no clients.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ids: Ids::new(),
            sessions: Vec::new(),
            clients: Vec::new(),
            activity: 0,
        }
    }

    /// Advances the tree's own activity counter and returns the new tick -
    /// the kernel's own state, not a clock (`.claude/rules/kernel.md`).
    fn tick(&mut self) -> u64 {
        self.activity += 1;
        self.activity
    }

    /// Every session in the tree, in order.
    #[must_use]
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    /// The session with this id, if the tree holds it.
    #[must_use]
    pub fn session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.iter().find(|session| session.id() == id)
    }

    /// The session with this id, if the tree holds it.
    pub fn session_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|session| session.id() == id)
    }

    /// Creates a new session, with one tab and one pane already in it
    /// (R93, R94), and the next colour not in use (R26, R109).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidLabel`] if `label` is invalid (doc-2 section
    /// 4). The label is validated before anything is minted, so a failed
    /// call never consumes an id (see `new_tab`'s comment below).
    ///
    /// # Panics
    ///
    /// Never in practice: `label` was already validated above, so setting
    /// it on the session just built cannot fail.
    pub fn new_session(&mut self, label: Option<String>) -> Result<SessionId, Error> {
        let label = label::validate(label)?;

        let colour = next_colour(self.sessions.iter().map(Session::colour));

        let id = self.ids.next_session();
        let first_tab = self.ids.next_tab();
        let first_pane = self.ids.next_pane();

        let mut session = Session::new(id, colour, first_tab, first_pane, PaneKind::Terminal);
        session.set_label(label).expect("already validated above");
        self.sessions.push(session);

        Ok(id)
    }

    /// Ends a session outright. There is no cascade above a session.
    ///
    /// Also drops every one of this session's tabs and panes from every
    /// client's attachment data (TASK-1.42: whether this is a direct call
    /// or a cascade from [`Tree::close_tab`], nothing should stay
    /// attached to a session that no longer exists). Captured before the
    /// session is removed, since nothing can be read off it afterwards;
    /// when this is the cascade from `close_tab`, that tab (and its own
    /// panes) are already gone from this session by the time this runs -
    /// `close_tab` already dropped them itself, so there is nothing left
    /// here to double up on.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SessionNotFound`] if the tree holds no session
    /// with that id.
    pub fn close_session(&mut self, id: SessionId) -> Result<(), Error> {
        let position = self
            .sessions
            .iter()
            .position(|session| session.id() == id)
            .ok_or(Error::SessionNotFound(id))?;

        let tabs: Vec<TabId> = self.sessions[position].tabs().iter().map(Tab::id).collect();
        let panes: Vec<PaneId> = self.sessions[position]
            .tabs()
            .iter()
            .flat_map(Tab::panes)
            .map(|pane| pane.id())
            .collect();

        self.sessions.remove(position);

        for tab in tabs {
            self.forget_tab_everywhere(tab);
        }
        for pane in panes {
            self.forget_pane_everywhere(pane);
        }
        self.forget_session_everywhere(id);

        Ok(())
    }

    fn session_id_owning_tab(&self, id: TabId) -> Option<SessionId> {
        self.sessions
            .iter()
            .find(|session| session.tab(id).is_some())
            .map(Session::id)
    }

    fn session_owning_tab_mut(&mut self, id: TabId) -> Option<&mut Session> {
        self.sessions
            .iter_mut()
            .find(|session| session.tab(id).is_some())
    }

    /// Closes the tab with this id, ending its session too if that tab
    /// was its last (R94).
    ///
    /// Also drops this tab and every one of its panes from every client's
    /// attachment data (TASK-1.42), and, if the session survives, recomputes
    /// its latest client and reapplies its declared size to whatever it
    /// still views (`Tree::reapply_latest`) - closing a tab can change who
    /// is latest for the tabs that remain. Captured before anything is
    /// removed; when this is the cascade from [`Tree::close_pane`], that
    /// pane is already gone from this tab by the time this runs - `close_pane`
    /// already dropped it itself, so there is nothing left here to double
    /// up on.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if no session in the tree holds a
    /// tab with that id.
    ///
    /// # Panics
    ///
    /// Never in practice: the session this just found to hold `id` cannot
    /// have been removed from the tree in between, since nothing else runs
    /// while this method holds `&mut self`.
    pub fn close_tab(&mut self, id: TabId) -> Result<TabClosed, Error> {
        let session = self
            .session_owning_tab_mut(id)
            .ok_or(Error::TabNotFound(id))?;
        let session_id = session.id();
        let panes: Vec<PaneId> = session
            .tab(id)
            .map(|tab| tab.panes().iter().map(|pane| pane.id()).collect())
            .unwrap_or_default();
        let closed = session.close_tab(id)?;

        self.forget_tab_everywhere(id);
        for pane in panes {
            self.forget_pane_everywhere(pane);
        }

        if closed.session_now_empty {
            self.close_session(session_id)
                .expect("the session that just reported itself empty is still in the tree");
        } else {
            self.reapply_latest(session_id);
        }

        Ok(closed)
    }

    fn tab_owning_pane_mut(&mut self, id: PaneId) -> Option<&mut Tab> {
        self.sessions
            .iter_mut()
            .flat_map(Session::tabs_mut)
            .find(|tab| tab.pane(id).is_some())
    }

    fn tab_by_id_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.sessions
            .iter_mut()
            .flat_map(Session::tabs_mut)
            .find(|tab| tab.id() == id)
    }

    /// The immutable counterpart of [`Tree::tab_by_id_mut`] - TASK-1.42's
    /// client/size API only ever reads a tab, never mutates it directly
    /// (every write goes through `set_tab_size`), so it needs this rather
    /// than the `_mut` version every earlier task added.
    fn tab_by_id(&self, id: TabId) -> Option<&Tab> {
        self.sessions
            .iter()
            .flat_map(Session::tabs)
            .find(|tab| tab.id() == id)
    }

    /// The tab that holds this pane, if the tree holds it (TASK-1.42: the
    /// immutable counterpart of [`Tree::tab_owning_pane_mut`], for
    /// `declare_size`/`note_input`, which never mutate the pane's own
    /// tab directly).
    fn tab_id_owning_pane(&self, id: PaneId) -> Option<TabId> {
        self.sessions
            .iter()
            .flat_map(Session::tabs)
            .find(|tab| tab.pane(id).is_some())
            .map(Tab::id)
    }

    /// The session that holds this pane, if the tree holds it (TASK-1.42:
    /// every client-view operation is keyed off a pane, but "latest"
    /// (decision-6) is tracked per session, so this is the one lookup
    /// `view_pane`/`stop_viewing`/`note_input`/`sessions_viewed`/
    /// `clients_viewing` all share).
    fn session_id_owning_pane(&self, id: PaneId) -> Option<SessionId> {
        self.tab_id_owning_pane(id)
            .and_then(|tab| self.session_id_owning_tab(tab))
    }

    fn client_mut(&mut self, id: ClientId) -> Option<&mut Client> {
        self.clients.iter_mut().find(|client| client.id() == id)
    }

    /// Every client currently viewing any pane in `tab` (R91's "every
    /// other client" — who gets told when this tab's size changes).
    fn viewers_of_tab(&self, tab: &Tab) -> Vec<ClientId> {
        self.clients
            .iter()
            .filter(|client| {
                client
                    .viewing()
                    .iter()
                    .any(|pane| tab.pane(*pane).is_some())
            })
            .map(Client::id)
            .collect()
    }

    /// Builds the "what happened" struct for a tab whose size just changed
    /// to `size` — assumes the caller already applied it via
    /// `set_tab_size` and that `tab` is known to exist.
    fn size_changed(&self, tab: TabId, size: Size) -> SizeChanged {
        let tab_ref = self
            .tab_by_id(tab)
            .expect("caller just set this tab's size, so it exists");
        SizeChanged {
            tab,
            size,
            panes: tab_ref.layout().leaf_sizes(size),
            viewers: self.viewers_of_tab(tab_ref),
        }
    }

    /// Recomputes the latest client in `session` (decision-6: derived, not
    /// stored - see `Tree::latest_client`) and, for every tab it views
    /// there, reapplies its own last-declared size if that differs from
    /// the tab's current one. This is the fallback this plan uses for
    /// "what happens when the latest leaves" (decision-6 does not say;
    /// `disconnect_client` and `stop_viewing` both call this after removing
    /// a viewer, and `close_pane`/`close_tab` call it after a pane or tab
    /// closes out from under a viewer).
    fn reapply_latest(&mut self, session: SessionId) -> Vec<SizeChanged> {
        let Ok(Some(latest)) = self.latest_client(session) else {
            return Vec::new();
        };

        let viewed_tabs: Vec<TabId> = {
            let Some(latest_client) = self.client(latest) else {
                return Vec::new();
            };
            let mut tabs = Vec::new();
            for &pane in latest_client.viewing() {
                if self.session_id_owning_pane(pane) != Some(session) {
                    continue;
                }
                if let Some(tab) = self.tab_id_owning_pane(pane)
                    && !tabs.contains(&tab)
                {
                    tabs.push(tab);
                }
            }
            tabs
        };

        let mut changes = Vec::new();
        for tab in viewed_tabs {
            let Some(size) = self
                .client(latest)
                .and_then(|client| client.declared_size(tab))
            else {
                continue;
            };
            let current = self.tab_by_id(tab).and_then(Tab::size);
            if current == Some(size) {
                continue;
            }
            self.set_tab_size(tab, size)
                .expect("tab just found via this client's own viewing list exists");
            changes.push(self.size_changed(tab, size));
        }
        changes
    }

    /// Drops `pane` from every client's viewing list (TASK-1.42: a closed
    /// pane cannot stay attached to anything - `close_pane` calls this
    /// before returning, regardless of whether it cascades).
    fn forget_pane_everywhere(&mut self, pane: PaneId) {
        for client in &mut self.clients {
            client.stop_viewing(pane);
        }
    }

    /// Drops `tab` from every client's declared-size records (TASK-1.42:
    /// a closed tab's declared size is meaningless - `close_tab` calls
    /// this before returning, regardless of whether it cascades).
    fn forget_tab_everywhere(&mut self, tab: TabId) {
        for client in &mut self.clients {
            client.forget_tab(tab);
        }
    }

    /// Drops `session` from every client's activity records (TASK-1.42:
    /// a closed session's activity is meaningless - `close_session` calls
    /// this before returning).
    fn forget_session_everywhere(&mut self, session: SessionId) {
        for client in &mut self.clients {
            client.forget_session(session);
        }
    }

    /// Closes the pane with this id, cascading up through its tab and its
    /// session if either was left with nothing in it (R94). This is a
    /// decision function: it returns what closed, and does none of the
    /// I/O that turns that into an event (R38) — that is the server's job.
    ///
    /// Also drops this pane from every client's attachment data
    /// (TASK-1.42), and, if the tab survives, recomputes its session's
    /// latest client and reapplies its declared size to whatever it still
    /// views (`Tree::reapply_latest`) - closing a pane can change who is
    /// latest for the tab it was in. When the tab does not survive, that
    /// cascade is `Tree::close_tab`'s job, called below, which does its
    /// own reapply for whatever remains of the session.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id.
    ///
    /// # Panics
    ///
    /// Never in practice: the tab this just found to hold `id` cannot have
    /// gone anywhere in between, since nothing else runs while this method
    /// holds `&mut self`.
    pub fn close_pane(&mut self, id: PaneId) -> Result<ClosedPane, Error> {
        let tab_id = self
            .tab_owning_pane_mut(id)
            .ok_or(Error::PaneNotFound(id))?
            .id();

        // Captured before any cascade can remove the session that holds it.
        let session_id = self.session_id_owning_tab(tab_id);

        let tab = self
            .tab_owning_pane_mut(id)
            .expect("just found this pane's tab above");
        let pane_closed = tab.close_pane(id)?;

        self.forget_pane_everywhere(id);

        if !pane_closed.tab_now_empty {
            if let Some(session_id) = session_id {
                self.reapply_latest(session_id);
            }
            return Ok(ClosedPane {
                pane: pane_closed.pane,
                tab: None,
                session: None,
            });
        }

        let tab_closed = self
            .close_tab(tab_id)
            .expect("the tab that just reported itself empty is still in the tree");

        Ok(ClosedPane {
            pane: pane_closed.pane,
            tab: Some(tab_closed.tab),
            session: if tab_closed.session_now_empty {
                session_id
            } else {
                None
            },
        })
    }

    /// Records what a pane runs, for the first time (doc-9 R13; TASK-1.40),
    /// and its status as `Running` (doc-11; TASK-1.41), stamped with this
    /// tree's own current tick (decision-6/R91's non-clock activity
    /// counter) - reused here purely so two panes' statuses can be told
    /// apart by "which started more recently" (decision-7 point 3), with
    /// no new parameter on this method: nothing about *when* in real time
    /// a program started needs to reach this call at all. Called once,
    /// right after the pane that minted `id` was created
    /// ([`Tree::new_session`]/[`Tree::new_tab`]/[`Tree::split_pane`]), so
    /// `STEG_PANE` (which needs the id) is already known.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id, or [`Error::ProgramAlreadyStarted`] if it already has
    /// a program.
    pub fn start_program(&mut self, id: PaneId, program: Program) -> Result<(), Error> {
        let tick = self.tick();
        self.tab_owning_pane_mut(id)
            .and_then(|tab| tab.pane_mut(id))
            .ok_or(Error::PaneNotFound(id))?
            .start_program(program, tick)
    }

    /// Records that a pane's program ended with `exit_code` at `now`, and
    /// runs R97's cascade: a pane started as a plain shell closes, exactly
    /// as a manual [`Tree::close_pane`] would; a pane started with a given
    /// command stays, and can be run again with [`Tree::restart_program`].
    /// Also updates the pane's status (doc-11 R82) to `Finished` or
    /// `Failed` - `now` is the one place real elapsed time enters this
    /// crate at all (TASK-1.41), the caller's own clock reading converted
    /// to a [`Moment`], used only for R83's ten-minute `Finished` rule;
    /// this method never reads a clock itself (doc-16 R110).
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id, or [`Error::ProgramNotStarted`] if it has no program
    /// yet.
    ///
    /// # Panics
    ///
    /// Never in practice: a pane this just found and recorded an exit on
    /// cannot have gone anywhere in between, since nothing else runs while
    /// this method holds `&mut self`.
    pub fn record_exit(
        &mut self,
        id: PaneId,
        exit_code: i32,
        now: Moment,
    ) -> Result<ExitOutcome, Error> {
        let tick = self.tick();
        let pane = self
            .tab_owning_pane_mut(id)
            .and_then(|tab| tab.pane_mut(id))
            .ok_or(Error::PaneNotFound(id))?;
        pane.record_exit(exit_code, now, tick)?;

        if pane.closes_on_exit() {
            let closed = self
                .close_pane(id)
                .expect("the pane that just recorded its own exit is still in the tree");
            Ok(ExitOutcome::Closed(closed))
        } else {
            Ok(ExitOutcome::Stayed)
        }
    }

    /// Runs a pane's program again, in the same directory, with the same
    /// arguments (R97). Only the kernel's own state moves back to
    /// `Running`; actually spawning the new process is the server's job
    /// (TASK-1.40's brief). Its status (doc-11) moves back to `Running`
    /// too, stamped with this tree's current tick, the same reason
    /// [`Tree::start_program`] is.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id, or whatever `Pane::restart` returns -
    /// [`Error::ProgramStillRunning`] or [`Error::ShellCannotRestart`].
    pub fn restart_program(&mut self, id: PaneId) -> Result<(), Error> {
        let tick = self.tick();
        self.tab_owning_pane_mut(id)
            .and_then(|tab| tab.pane_mut(id))
            .ok_or(Error::PaneNotFound(id))?
            .restart(tick)
    }

    /// The pane whose status `tab`'s ring stands for, as of `now` (doc-11
    /// R50, decision-7; TASK-1.41) - [`crate::Tab::shown_status`]'s own
    /// doc comment has the ranking and tie-break. `None` if no pane in
    /// the tab currently has a status.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if the tree holds no tab with that
    /// id.
    pub fn tab_shown_status(&self, tab: TabId, now: Moment) -> Result<Option<PaneId>, Error> {
        Ok(self
            .tab_by_id(tab)
            .ok_or(Error::TabNotFound(tab))?
            .shown_status(now))
    }

    /// The soonest moment some pane's status will end on its own, with no
    /// further input - the minimum of every pane's own
    /// [`crate::Pane::next_status_deadline`] across the whole tree, or
    /// `None` if nothing has a live timer (doc-11 R83: only `Finished`
    /// ever has one). Lets a caller with a real clock (the server) sleep
    /// until this moment, or until something else happens first, instead
    /// of polling every pane on an interval; this method itself never
    /// reads a clock (doc-16 R110) - it only compares the `Moment`s
    /// already stored against the `now` it's given.
    #[must_use]
    pub fn next_status_deadline(&self, now: Moment) -> Option<Moment> {
        self.sessions
            .iter()
            .flat_map(Session::tabs)
            .flat_map(Tab::panes)
            .filter_map(|pane| pane.next_status_deadline(now))
            .min()
    }

    /// Tells the kernel the person has dealt with `pane`'s status - R83's
    /// "a key press or click in that pane" ending, for the half of it
    /// that is not a key press: `Tree::note_input` already ends a status
    /// on input, so this is the kernel's own answer to doc-11's open
    /// question #3 ("a click is not [input the server already sees]") for
    /// *what the kernel operation is*; how a click reaches the server at
    /// all is still open there, and in doc-14.
    ///
    /// Ends whichever status `pane` currently has, `Finished` or `Failed`
    /// alike (decision-7 point 5 names "a key press or click" for both,
    /// not for `Failed` alone - see this task's notes for the full
    /// reconciliation against the acceptance criterion's narrower
    /// wording). A no-op, not an error, if `pane` has no status to end.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id.
    pub fn acknowledge_pane(&mut self, id: PaneId) -> Result<(), Error> {
        self.tab_owning_pane_mut(id)
            .and_then(|tab| tab.pane_mut(id))
            .ok_or(Error::PaneNotFound(id))?
            .clear_status();
        Ok(())
    }

    /// Creates a new tab in the session with this id, with one pane
    /// already in it (R94).
    ///
    /// # Errors
    ///
    /// Returns [`Error::SessionNotFound`] if the tree holds no session
    /// with that id, or [`Error::InvalidLabel`] if `label` is invalid
    /// (doc-2 section 4). The label is validated before anything is
    /// minted, same as the target-exists check below, so neither kind of
    /// failure ever consumes an id.
    ///
    /// # Panics
    ///
    /// Never in practice: `label` was already validated above, so setting
    /// it on the tab just added cannot fail.
    pub fn new_tab(
        &mut self,
        session_id: SessionId,
        label: Option<String>,
    ) -> Result<TabId, Error> {
        let label = label::validate(label)?;

        // Check the target exists before minting anything: an id only
        // ever names something actually being created, never a failed
        // call (step 10 of the plan). Destructuring `self` into its
        // fields, rather than calling `self.session_mut(...)`, is what
        // lets `ids` be minted from while `session` (borrowed from
        // `sessions`) is still held below.
        let Self { ids, sessions, .. } = self;
        let session = sessions
            .iter_mut()
            .find(|session| session.id() == session_id)
            .ok_or(Error::SessionNotFound(session_id))?;

        let tab_id = ids.next_tab();
        let pane_id = ids.next_pane();

        let tab = session.add_tab(tab_id, pane_id, PaneKind::Terminal);
        tab.set_label(label).expect("already validated above");

        Ok(tab_id)
    }

    /// Moves the tab with this id to a new position among its session's
    /// other tabs (R8: "reordered").
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if no session in the tree holds a
    /// tab with that id, or [`Error::PositionOutOfRange`] if
    /// `to_position` is not a valid index among its siblings.
    pub fn move_tab(&mut self, id: TabId, to_position: usize) -> Result<(), Error> {
        let session = self
            .session_owning_tab_mut(id)
            .ok_or(Error::TabNotFound(id))?;
        session.move_tab(id, to_position)
    }

    /// Splits `target`'s leaf, in `direction`, with a newly-minted pane
    /// of this `kind` on `direction`'s side, and makes the new pane the
    /// tab's current pane (AC #1).
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id, or [`Error::SplitTooSmall`] if either resulting
    /// pane would be under R100's floor (only possible once the tab's
    /// size is known — [`Tree::set_tab_size`]).
    ///
    /// # Panics
    ///
    /// Never in practice: the tab this just found to hold `target`
    /// cannot have gone anywhere in between, since nothing else runs
    /// while this method holds `&mut self`.
    pub fn split_pane(
        &mut self,
        target: PaneId,
        direction: Direction,
        kind: PaneKind,
    ) -> Result<PaneId, Error> {
        // Check the target exists, and that the split clears the floor,
        // before minting anything (see `new_tab`'s comment): a refused
        // split must consume no id.
        if self.tab_owning_pane_mut(target).is_none() {
            return Err(Error::PaneNotFound(target));
        }

        let Self { ids, sessions, .. } = self;
        let tab = sessions
            .iter_mut()
            .flat_map(Session::tabs_mut)
            .find(|tab| tab.pane(target).is_some())
            .expect("just found this pane's tab above");

        let new_id = ids.next_pane();
        tab.split_pane(target, direction, new_id, kind)?;

        Ok(new_id)
    }

    /// Changes the ratio of the split bordering `pane` on `direction`'s
    /// side (AC #3), clamping at R100's floor rather than refusing.
    /// Returns the ratio actually applied.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id, or [`Error::NoSplitInDirection`] if it has no split
    /// bordering it on that side.
    pub fn resize_pane(
        &mut self,
        pane: PaneId,
        direction: Direction,
        ratio: Ratio,
    ) -> Result<Ratio, Error> {
        let tab = self
            .tab_owning_pane_mut(pane)
            .ok_or(Error::PaneNotFound(pane))?;
        tab.resize_pane(pane, direction, ratio)
    }

    /// Exchanges the positions of two panes in the same tab's layout
    /// (AC #3).
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if either pane is in no tab, or
    /// [`Error::PanesNotInSameTab`] if they are in different tabs.
    ///
    /// # Panics
    ///
    /// Never in practice: the tab this just found to hold `a` cannot
    /// have gone anywhere in between, since nothing else runs while this
    /// method holds `&mut self`.
    pub fn swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), Error> {
        let tab_a = self.tab_owning_pane_mut(a).ok_or(Error::PaneNotFound(a))?;
        if tab_a.pane(b).is_none() {
            // `b` might still be a real pane, just in a different tab;
            // that's `PanesNotInSameTab`, not `PaneNotFound`.
            if self.tab_owning_pane_mut(b).is_none() {
                return Err(Error::PaneNotFound(b));
            }
            return Err(Error::PanesNotInSameTab { a, b });
        }

        let tab = self
            .tab_owning_pane_mut(a)
            .expect("just confirmed both panes are in this tab");
        tab.swap_panes(a, b)
    }

    /// Sets which pane fills its tab (R96), replacing any previous fill.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id.
    pub fn fill_pane(&mut self, pane: PaneId) -> Result<(), Error> {
        let tab = self
            .tab_owning_pane_mut(pane)
            .ok_or(Error::PaneNotFound(pane))?;
        tab.fill_pane(pane)
    }

    /// Makes `id` this pane's tab's current pane, and that tab this
    /// session's current tab, both in the same call (R85: "Focusing a pane
    /// makes its tab the current tab of its session as well").
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with that id.
    ///
    /// # Panics
    ///
    /// Never in practice: the tab and session this just found to hold
    /// `id` cannot have gone anywhere in between, since nothing else runs
    /// while this method holds `&mut self`.
    pub fn focus_pane(&mut self, id: PaneId) -> Result<Focused, Error> {
        let tab_id = self
            .tab_owning_pane_mut(id)
            .ok_or(Error::PaneNotFound(id))?
            .id();
        let tab = self
            .tab_owning_pane_mut(id)
            .expect("just found this pane's tab above");
        tab.focus(id).expect("just confirmed `id` is in this tab");

        let session = self
            .session_owning_tab_mut(tab_id)
            .expect("every tab in the tree belongs to some session");
        session
            .set_current_tab(tab_id)
            .expect("just confirmed this session holds `tab_id`");

        Ok(Focused {
            session: session.id(),
            tab: tab_id,
            pane: id,
        })
    }

    /// Makes `id` this tab's session's current tab (doc-2 section 4:
    /// "Makes it the current tab of its session"). Does not touch any
    /// tab's current pane — decision-5 point 1 cascades pane focus to tab,
    /// never tab focus to pane.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if no session in the tree holds a
    /// tab with that id.
    ///
    /// # Panics
    ///
    /// Never in practice: the session this just found to hold `id` cannot
    /// have gone anywhere in between, since nothing else runs while this
    /// method holds `&mut self`.
    pub fn focus_tab(&mut self, id: TabId) -> Result<Focused, Error> {
        let session = self
            .session_owning_tab_mut(id)
            .ok_or(Error::TabNotFound(id))?;
        session
            .set_current_tab(id)
            .expect("just confirmed this session holds `id`");
        let pane = session
            .tab(id)
            .expect("just confirmed this session holds `id`")
            .current();

        Ok(Focused {
            session: session.id(),
            tab: id,
            pane,
        })
    }

    /// Focuses `from`'s neighbour in `direction` (R9, R113), optionally
    /// wrapping around the tab's far edge if there is nothing that way.
    /// Acts on all three outcomes `Tab::neighbour` can find: moving focus
    /// (through [`Tree::focus_pane`]) on [`DirectionalFocus::Neighbour`]
    /// and [`DirectionalFocus::Wrapped`], and changing nothing on
    /// [`DirectionalFocus::Edge`] — doc-2 section 4 says the command exits
    /// 0 and stays put at the edge, so that is not an error.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if no tab in the tree holds a pane
    /// with id `from`.
    ///
    /// # Panics
    ///
    /// Never in practice: a neighbour or wrap target `Tab::neighbour` finds
    /// is always a pane in the same tab as `from`, which this method just
    /// confirmed exists, and nothing else runs while it holds `&mut self`.
    pub fn focus_direction(
        &mut self,
        from: PaneId,
        direction: Direction,
        wrap: bool,
    ) -> Result<FocusMove, Error> {
        let tab = self
            .tab_owning_pane_mut(from)
            .ok_or(Error::PaneNotFound(from))?;
        let outcome = tab.neighbour(from, direction, wrap);

        match outcome {
            DirectionalFocus::Edge => Ok(FocusMove::Edge),
            DirectionalFocus::Neighbour(id) => {
                let focused = self.focus_pane(id).expect(
                    "a neighbour found by `Tab::neighbour` is always a pane in the same tab as `from`",
                );
                Ok(FocusMove::Neighbour(focused))
            }
            DirectionalFocus::Wrapped(id) => {
                let focused = self.focus_pane(id).expect(
                    "a wrap target found by `Tab::neighbour` is always a pane in the same tab as `from`",
                );
                Ok(FocusMove::Wrapped(focused))
            }
        }
    }

    /// Clears a tab's fill, if it has one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if the tree holds no tab with that
    /// id.
    pub fn unfill_pane(&mut self, tab: TabId) -> Result<(), Error> {
        let tab = self.tab_by_id_mut(tab).ok_or(Error::TabNotFound(tab))?;
        tab.unfill();
        Ok(())
    }

    /// The tab that holds this pane, if any (TASK-1.75's dispatch: finding
    /// a pane's tab from a target that only names the pane, such as
    /// `pane.unset --zoomed`'s call into [`Tree::unfill_pane`], which takes
    /// a [`TabId`]).
    #[must_use]
    pub fn tab_of_pane(&self, pane: PaneId) -> Option<TabId> {
        self.sessions()
            .iter()
            .flat_map(crate::session::Session::tabs)
            .find(|tab| tab.pane(pane).is_some())
            .map(crate::tab::Tab::id)
    }

    /// Makes `pane` this tab's own current pane, with no cascade to the
    /// session's current tab — unlike [`Tree::focus_pane`] (R85 cascades a
    /// pane focus to its tab; this does not, by design). Used by
    /// `pane.split --no-focus` (TASK-1.75's dispatch) to put a tab's
    /// previous current pane back after a split unconditionally moved it
    /// (R115: "creating without taking focus is done by putting the
    /// previous focus back").
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if the tree holds no tab with that
    /// id, or [`Error::PaneNotFound`] if `pane` is not in that tab.
    pub fn set_tab_current_pane(&mut self, tab: TabId, pane: PaneId) -> Result<(), Error> {
        let tab = self.tab_by_id_mut(tab).ok_or(Error::TabNotFound(tab))?;
        tab.focus(pane)
    }

    /// Stores a tab's current size in whole cells (AC #6: unconditional
    /// — no floor check, ever; a shrink "refuses and closes nothing").
    ///
    /// `pub(crate)`, not `pub`: its only caller is now the client-driven
    /// path (`declare_size`/`note_input`/`reapply_latest`, TASK-1.42) —
    /// nothing outside the kernel sets a tab's size except through a
    /// client's declared activity.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TabNotFound`] if the tree holds no tab with that
    /// id.
    pub(crate) fn set_tab_size(&mut self, tab: TabId, size: Size) -> Result<(), Error> {
        let tab = self.tab_by_id_mut(tab).ok_or(Error::TabNotFound(tab))?;
        tab.set_size(size);
        Ok(())
    }

    /// Moves `pane` out of its own tab and into `target_tab`, splitting
    /// `target_tab`'s current pane in `direction` to make room for it
    /// (R9: "moved to another tab"), becoming `target_tab`'s new current
    /// pane. The source tab (and, cascading, its session) closes if
    /// `pane` was its last, exactly as [`Tree::close_pane`]'s cascade
    /// does. Atomic: the floor is checked in `target_tab` *before*
    /// anything is removed from the source tab, so a refusal here leaves
    /// both tabs untouched.
    ///
    /// This is not a focus: it is `--direction`'s insertion point (doc-2
    /// section 3), not `pane focus`. It never changes any session's
    /// current tab, including `target_tab`'s own, even though it does
    /// change `target_tab`'s current *pane*.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneNotFound`] if `pane` is in no tab,
    /// [`Error::TabNotFound`] if `target_tab` doesn't exist,
    /// [`Error::AlreadyInTab`] if `target_tab` is already `pane`'s own
    /// tab, or [`Error::SplitTooSmall`] if the floor would be violated.
    ///
    /// # Panics
    ///
    /// Never in practice: the tabs this just found to hold `pane` and
    /// `target_tab` cannot have gone anywhere in between, since nothing
    /// else runs while this method holds `&mut self`.
    pub fn move_pane_to_tab(
        &mut self,
        pane: PaneId,
        target_tab: TabId,
        direction: Direction,
    ) -> Result<MovedPane, Error> {
        let source_tab_id = self
            .tab_owning_pane_mut(pane)
            .ok_or(Error::PaneNotFound(pane))?
            .id();

        if source_tab_id == target_tab {
            return Err(Error::AlreadyInTab {
                pane,
                tab: target_tab,
            });
        }

        let target = self
            .tab_by_id_mut(target_tab)
            .ok_or(Error::TabNotFound(target_tab))?;
        let target_current = target.current();
        if target.would_violate_floor_inserting(target_current, direction, pane) {
            return Err(Error::SplitTooSmall {
                pane: target_current,
            });
        }

        // Captured before any cascade can remove the session that holds
        // the source tab (mirrors `close_pane`'s own comment).
        let source_session_id = self.session_id_owning_tab(source_tab_id);

        let source_tab = self
            .tab_owning_pane_mut(pane)
            .expect("just found this pane's tab above, and nothing else ran meanwhile");
        let (removed_pane, pane_closed) = source_tab.remove_pane_for_move(pane);

        let (source_tab_closed, source_session_closed) = if pane_closed.tab_now_empty {
            let tab_closed = self
                .close_tab(source_tab_id)
                .expect("the tab that just reported itself empty is still in the tree");
            (
                Some(tab_closed.tab),
                if tab_closed.session_now_empty {
                    source_session_id
                } else {
                    None
                },
            )
        } else {
            (None, None)
        };

        let target = self
            .tab_by_id_mut(target_tab)
            .expect("confirmed present above, and nothing else ran meanwhile");
        target.insert_pane_for_move(direction, removed_pane);

        Ok(MovedPane {
            pane,
            to: target_tab,
            source_tab_closed,
            source_session_closed,
        })
    }

    /// Moves the session with this id to a new position among its
    /// siblings (R6: "reordered").
    ///
    /// # Errors
    ///
    /// Returns [`Error::SessionNotFound`] if the tree holds no session
    /// with that id, or [`Error::PositionOutOfRange`] if `to_position` is
    /// not a valid index among the sessions.
    pub fn move_session(&mut self, id: SessionId, to_position: usize) -> Result<(), Error> {
        let from = self
            .sessions
            .iter()
            .position(|session| session.id() == id)
            .ok_or(Error::SessionNotFound(id))?;

        if to_position >= self.sessions.len() {
            return Err(Error::PositionOutOfRange {
                len: self.sessions.len(),
                position: to_position,
            });
        }

        let session = self.sessions.remove(from);
        self.sessions.insert(to_position, session);

        Ok(())
    }

    /// Connects a new client (doc-15 R86): one connection to this server,
    /// viewing nothing yet. Never touches focus or any session, tab or
    /// pane (decision-5 point 4, R88) - connecting is wholly separate
    /// state from the tree above it.
    pub fn connect_client(&mut self, kind: ClientKind, label: Option<String>) -> ClientId {
        let id = self.ids.next_client();
        self.clients.push(Client::new(id, kind, label));
        id
    }

    /// Every client connected to this server, in connection order.
    #[must_use]
    pub fn clients(&self) -> &[Client] {
        &self.clients
    }

    /// The client with this id, if the tree holds it.
    #[must_use]
    pub fn client(&self, id: ClientId) -> Option<&Client> {
        self.clients.iter().find(|client| client.id() == id)
    }

    /// Disconnects a client (doc-15 R86). For every session it was
    /// viewing, the latest client there is recomputed and, if it changed,
    /// its own declared size is reapplied to whatever it views (decision-6
    /// does not say what happens when the latest leaves; this plan decides
    /// it, reversibly: fall back to the next most recently active
    /// remaining viewer, or to no latest at all).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ClientNotFound`] if the tree holds no client with
    /// that id.
    ///
    /// # Panics
    ///
    /// Never in practice: `id` was just confirmed to exist, so the
    /// `expect` below cannot fail.
    pub fn disconnect_client(&mut self, id: ClientId) -> Result<Vec<SizeChanged>, Error> {
        let position = self
            .clients
            .iter()
            .position(|client| client.id() == id)
            .ok_or(Error::ClientNotFound(id))?;

        let sessions = self
            .sessions_viewed(id)
            .expect("just found this client above");
        self.clients.remove(position);

        let mut changes = Vec::new();
        for session in sessions {
            changes.extend(self.reapply_latest(session));
        }
        Ok(changes)
    }

    /// Attaches `client` to `pane` (doc-14: the smallest thing an
    /// attachment is, at the kernel's level - TASK-1.47 builds the real
    /// attach, with streaming and restoring after a client goes away, on
    /// top of this). Idempotent: viewing a pane a client already views
    /// again changes nothing. Not itself activity (R91's "input or
    /// resize" - viewing alone is neither), so this never changes who is
    /// latest.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ClientNotFound`] if `client` is unknown, or
    /// [`Error::PaneNotFound`] if `pane` is unknown.
    pub fn view_pane(&mut self, client: ClientId, pane: PaneId) -> Result<(), Error> {
        if self.session_id_owning_pane(pane).is_none() {
            return Err(Error::PaneNotFound(pane));
        }
        let client_mut = self
            .client_mut(client)
            .ok_or(Error::ClientNotFound(client))?;
        client_mut.start_viewing(pane);
        Ok(())
    }

    /// Detaches `client` from `pane`. Unconditionally recomputes and
    /// reapplies `pane`'s session's latest client afterwards, the same
    /// [`Tree::reapply_latest`] call [`Tree::disconnect_client`] makes for
    /// every session a departing client viewed - not only when this was
    /// `client`'s last pane in that session. Running it even when `client`
    /// still views other panes there is harmless and simpler than
    /// tracking whether it was needed: `reapply_latest`'s own equal-size
    /// guard makes the extra calls a no-op, and a client losing just one
    /// of several attachments can still change which tabs the session's
    /// latest client views, so the recompute is correct to run either way.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ClientNotFound`] if `client` is unknown, or
    /// [`Error::ClientNotViewingPane`] if `client` is not attached to
    /// `pane` (which includes `pane` not existing - nothing can be
    /// viewing a pane that was never there).
    pub fn stop_viewing(
        &mut self,
        client: ClientId,
        pane: PaneId,
    ) -> Result<Vec<SizeChanged>, Error> {
        let session = self
            .session_id_owning_pane(pane)
            .ok_or(Error::ClientNotViewingPane { client, pane })?;
        let client_mut = self
            .client_mut(client)
            .ok_or(Error::ClientNotFound(client))?;
        if !client_mut.stop_viewing(pane) {
            return Err(Error::ClientNotViewingPane { client, pane });
        }
        Ok(self.reapply_latest(session))
    }

    /// A client declares a whole tab's size, in cells - the tab it is
    /// viewing (this task's plan settles doc-14's per-attachment wording
    /// as the report direction, not the accept direction: what a client
    /// declares in is the tab total, what it is told is each pane's size
    /// through [`SizeChanged::panes`]). Requires `client` to already view
    /// some pane in `tab`'s session, since a client attaches before it
    /// declares (doc-14: "at attach and whenever..."). Declaring is
    /// itself activity (R91, decision-6), so `client` becomes the new
    /// latest in that session unconditionally, because its tick is now
    /// the freshest of anyone's, and its size is therefore unconditionally
    /// applied, with no floor check, ever (R100's last clause).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ClientNotFound`] if `client` is unknown,
    /// [`Error::TabNotFound`] if `tab` is unknown, or
    /// [`Error::ClientNotViewingTab`] if `client` views nothing in `tab`'s
    /// session.
    ///
    /// # Panics
    ///
    /// Never in practice: `session` was just confirmed to own `tab`, and
    /// `client` was just confirmed to exist, so neither `expect` below can
    /// fail.
    pub fn declare_size(
        &mut self,
        client: ClientId,
        tab: TabId,
        size: Size,
    ) -> Result<SizeChanged, Error> {
        if self.client(client).is_none() {
            return Err(Error::ClientNotFound(client));
        }
        let session = self
            .session_id_owning_tab(tab)
            .ok_or(Error::TabNotFound(tab))?;
        let viewed = self.sessions_viewed(client).expect("checked above");
        if !viewed.contains(&session) {
            return Err(Error::ClientNotViewingTab { client, tab });
        }

        let tick = self.tick();
        let client_mut = self.client_mut(client).expect("checked above");
        client_mut.declare_size(tab, size);
        client_mut.record_activity(session, tick);

        self.set_tab_size(tab, size)
            .expect("tab existence already checked above");
        Ok(self.size_changed(tab, size))
    }

    /// A client sends input to `pane` (R91's other half of "latest": "the
    /// last one to send input to it or be resized"). Makes `client` the
    /// new latest in `pane`'s session unconditionally, the same as
    /// [`Tree::declare_size`]; unlike it, there is no fresh size to apply,
    /// so instead, whatever size `client` last declared for `pane`'s tab
    /// (if any) is reapplied if it differs from the tab's current size.
    /// This is the path that makes an *old* declaration win again once
    /// its client becomes latest by a later input, not a fresh declare.
    ///
    /// Also ends `pane`'s status, if it has one (doc-11 R83; decision-7
    /// point 5's "a key press" - TASK-1.41): this is exactly what "input
    /// to the pane" already means in this codebase, so a key press ends a
    /// `Finished` or `Failed` status whether or not that key press also
    /// changes anything about the tab's size. A resize alone
    /// ([`Tree::declare_size`]) and a bare focus
    /// ([`Tree::focus_pane`]/[`Tab::focus`]) are not input and touch
    /// nothing here (doc-15's own "opening a tab clears nothing").
    ///
    /// # Errors
    ///
    /// Returns [`Error::ClientNotFound`] if `client` is unknown, or
    /// [`Error::ClientNotViewingPane`] if `client` is not attached to
    /// `pane`.
    ///
    /// # Panics
    ///
    /// Never in practice: `pane` was just confirmed to belong to a tab, and
    /// every tab belongs to a session (R93), so the `expect`s below cannot
    /// fail.
    pub fn note_input(
        &mut self,
        client: ClientId,
        pane: PaneId,
    ) -> Result<Option<SizeChanged>, Error> {
        let tab = self
            .tab_id_owning_pane(pane)
            .ok_or(Error::ClientNotViewingPane { client, pane })?;
        let session = self
            .session_id_owning_tab(tab)
            .expect("a tab found above always has an owning session");

        let client_ref = self.client(client).ok_or(Error::ClientNotFound(client))?;
        if !client_ref.is_viewing(pane) {
            return Err(Error::ClientNotViewingPane { client, pane });
        }

        self.tab_by_id_mut(tab)
            .and_then(|t| t.pane_mut(pane))
            .expect("pane and its owning tab were both just confirmed to exist")
            .clear_status();

        let tick = self.tick();
        let declared = {
            let client_mut = self.client_mut(client).expect("checked above");
            client_mut.record_activity(session, tick);
            client_mut.declared_size(tab)
        };
        let Some(size) = declared else {
            return Ok(None);
        };
        let current = self.tab_by_id(tab).and_then(Tab::size);
        if current == Some(size) {
            return Ok(None);
        }

        self.set_tab_size(tab, size)
            .expect("tab existence already checked above");
        Ok(Some(self.size_changed(tab, size)))
    }

    /// Every session `client` views (doc-15 R86, doc-14: derived from its
    /// attachments, never sent by the client itself - see [`Client::viewing`]'s
    /// own doc comment). Deduplicated; order is not meaningful.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ClientNotFound`] if `client` is unknown.
    pub fn sessions_viewed(&self, client: ClientId) -> Result<Vec<SessionId>, Error> {
        let client = self.client(client).ok_or(Error::ClientNotFound(client))?;
        let mut sessions = Vec::new();
        for &pane in client.viewing() {
            if let Some(session) = self.session_id_owning_pane(pane)
                && !sessions.contains(&session)
            {
                sessions.push(session);
            }
        }
        Ok(sessions)
    }

    /// Every client currently viewing `session` (doc-15 R86).
    ///
    /// # Errors
    ///
    /// Returns [`Error::SessionNotFound`] if the tree holds no session
    /// with that id.
    pub fn clients_viewing(&self, session: SessionId) -> Result<Vec<ClientId>, Error> {
        if self.session(session).is_none() {
            return Err(Error::SessionNotFound(session));
        }
        Ok(self
            .clients
            .iter()
            .filter(|client| {
                client
                    .viewing()
                    .iter()
                    .any(|&pane| self.session_id_owning_pane(pane) == Some(session))
            })
            .map(Client::id)
            .collect())
    }

    /// The latest client in `session` (decision-6): among those currently
    /// viewing it, the one whose activity there is most recent. `None` if
    /// nobody views it - derived fresh from [`Tree::clients_viewing`] and
    /// each client's own activity record, never cached, so a departure
    /// needs no special case (`Tree::reapply_latest`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::SessionNotFound`] if the tree holds no session
    /// with that id.
    ///
    /// # Panics
    ///
    /// Never in practice: every id this walks was just listed by
    /// [`Tree::clients_viewing`] as a real client, so the `expect` below
    /// cannot fail.
    pub fn latest_client(&self, session: SessionId) -> Result<Option<ClientId>, Error> {
        let viewers = self.clients_viewing(session)?;
        Ok(viewers
            .into_iter()
            .filter_map(|id| {
                let client = self.client(id).expect("just listed as a viewer");
                client.activity_in(session).map(|tick| (tick, id))
            })
            .max_by_key(|&(tick, _)| tick)
            .map(|(_, id)| id))
    }
}

#[cfg(test)]
mod tests {
    use super::{FocusMove, Focused, Tree};
    use crate::moment::Moment;
    use std::time::Duration;
    use steg_sdk::Colour;

    /// A `Moment` this many seconds after an arbitrary origin - plain
    /// data, no clock, used throughout this module's tests.
    fn moment(secs: u64) -> Moment {
        Moment::new(Duration::from_secs(secs))
    }

    #[test]
    fn new_session_gives_the_first_five_sessions_distinct_colours_in_order() {
        let mut tree = Tree::new();

        let colours: Vec<Colour> = (0..5)
            .map(|_| {
                let id = tree.new_session(None).unwrap();
                tree.session(id).unwrap().colour()
            })
            .collect();

        assert_eq!(
            colours,
            vec![
                Colour::Violet,
                Colour::Pink,
                Colour::Orange,
                Colour::Blue,
                Colour::Green,
            ]
        );
    }

    #[test]
    fn the_sixth_concurrent_session_repeats_the_first_colour() {
        let mut tree = Tree::new();
        for _ in 0..5 {
            tree.new_session(None).unwrap();
        }

        let sixth = tree.new_session(None).unwrap();

        assert_eq!(tree.session(sixth).unwrap().colour(), Colour::Violet);
    }

    #[test]
    fn close_session_removes_it_directly() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();

        tree.close_session(session_id).unwrap();

        assert!(tree.session(session_id).is_none());
        assert!(tree.sessions().is_empty());
    }

    #[test]
    fn closing_a_sessions_only_tabs_only_pane_ends_the_session() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane_id = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();

        let closed = tree.close_pane(pane_id).unwrap();

        assert_eq!(closed.pane, pane_id);
        assert!(closed.tab.is_some());
        assert_eq!(closed.session, Some(session_id));
        assert!(tree.session(session_id).is_none());
        assert!(tree.sessions().is_empty());
    }

    #[test]
    fn closing_a_tabs_last_pane_closes_the_tab_without_ending_a_session_that_has_others() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();

        let closed = tree.close_pane(first_pane).unwrap();

        assert_eq!(closed.tab, Some(first_tab));
        assert_eq!(closed.session, None);
        let session = tree.session(session_id).unwrap();
        assert_eq!(session.tabs().len(), 1);
        assert_eq!(session.tabs()[0].id(), second_tab);
    }

    #[test]
    fn closing_one_of_several_panes_cascades_no_further_than_it_should() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();

        let closed = tree.close_pane(second_pane).unwrap();

        assert_eq!(closed.pane, second_pane);
        assert!(closed.tab.is_none());
        assert!(closed.session.is_none());
        assert!(tree.session(session_id).is_some());
    }

    #[test]
    fn closing_one_of_several_tabs_cascades_no_further_than_it_should() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let second_tab = tree.new_tab(session_id, None).unwrap();

        let closed = tree.close_tab(second_tab).unwrap();

        assert!(!closed.session_now_empty);
        assert!(tree.session(session_id).is_some());
    }

    fn command(cwd: &str) -> steg_sdk::Program {
        steg_sdk::Program::Command {
            argv: vec!["htop".to_string()],
            cwd: cwd.to_string(),
        }
    }

    fn shell(cwd: &str) -> steg_sdk::Program {
        steg_sdk::Program::Shell {
            cwd: cwd.to_string(),
        }
    }

    #[test]
    fn start_program_records_it_on_the_newly_minted_pane() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane_id = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();

        tree.start_program(pane_id, command("/tmp")).unwrap();

        let pane = &tree.session(session_id).unwrap().tabs()[0].panes()[0];
        assert_eq!(pane.program(), Some(&command("/tmp")));
        assert_eq!(
            pane.program_status(),
            Some(steg_sdk::ProgramStatus::Running)
        );
    }

    #[test]
    fn start_program_on_an_unknown_pane_errors() {
        let mut tree = Tree::new();

        assert_eq!(
            tree.start_program(steg_sdk::PaneId::from_raw(999), command("/tmp")),
            Err(crate::error::Error::PaneNotFound(
                steg_sdk::PaneId::from_raw(999)
            ))
        );
    }

    #[test]
    fn a_command_panes_exit_stays_and_can_be_restarted() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane_id = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(pane_id, command("/tmp")).unwrap();

        let outcome = tree.record_exit(pane_id, 3, moment(0)).unwrap();

        assert_eq!(outcome, super::ExitOutcome::Stayed);
        assert!(tree.session(session_id).is_some());
        let pane = &tree.session(session_id).unwrap().tabs()[0].panes()[0];
        assert_eq!(
            pane.program_status(),
            Some(steg_sdk::ProgramStatus::Ended { exit_code: 3 })
        );

        tree.restart_program(pane_id).unwrap();

        let pane = &tree.session(session_id).unwrap().tabs()[0].panes()[0];
        assert_eq!(
            pane.program_status(),
            Some(steg_sdk::ProgramStatus::Running)
        );
        assert_eq!(pane.program(), Some(&command("/tmp")));
    }

    #[test]
    fn a_shell_panes_exit_closes_it_through_the_same_cascade_as_a_manual_close() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane_id = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(pane_id, shell("/tmp")).unwrap();

        let outcome = tree.record_exit(pane_id, 0, moment(0)).unwrap();

        let super::ExitOutcome::Closed(closed) = outcome else {
            panic!("expected the shell pane's exit to close it, got {outcome:?}");
        };
        assert_eq!(closed.pane, pane_id);
        // It was the session's only pane, so the cascade (R94) takes the
        // tab and the session with it - the exact same shape
        // `closing_a_sessions_only_tabs_only_pane_ends_the_session` already
        // asserts for a manual `close_pane`.
        assert!(closed.tab.is_some());
        assert_eq!(closed.session, Some(session_id));
        assert!(tree.session(session_id).is_none());
    }

    #[test]
    fn a_shell_panes_exit_cascades_no_further_than_it_should_among_several_panes() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        tree.start_program(second_pane, shell("/tmp")).unwrap();

        let outcome = tree.record_exit(second_pane, 0, moment(0)).unwrap();

        let super::ExitOutcome::Closed(closed) = outcome else {
            panic!("expected the shell pane's exit to close it, got {outcome:?}");
        };
        assert_eq!(closed.pane, second_pane);
        assert!(closed.tab.is_none());
        assert!(closed.session.is_none());
        assert!(tree.session(session_id).is_some());
    }

    #[test]
    fn record_exit_on_an_unknown_pane_errors() {
        let mut tree = Tree::new();

        assert_eq!(
            tree.record_exit(steg_sdk::PaneId::from_raw(999), 0, moment(0)),
            Err(crate::error::Error::PaneNotFound(
                steg_sdk::PaneId::from_raw(999)
            ))
        );
    }

    #[test]
    fn restart_program_on_an_unknown_pane_errors() {
        let mut tree = Tree::new();

        assert_eq!(
            tree.restart_program(steg_sdk::PaneId::from_raw(999)),
            Err(crate::error::Error::PaneNotFound(
                steg_sdk::PaneId::from_raw(999)
            ))
        );
    }

    #[test]
    fn a_shell_pane_is_gone_after_its_exit_so_restarting_it_finds_no_pane_at_all() {
        // `Tree::record_exit` already cascaded this pane away (R97: a
        // plain shell closes), so there is no lingering "ended shell" for
        // `restart_program` to refuse by name - it is simply not in the
        // tree any more. `Pane::restart`'s own `ShellCannotRestart` guard
        // is a `Pane`-level invariant (see pane.rs's
        // `restarting_a_shell_pane_errors`), not one this path can reach,
        // since the tree never keeps a shell pane around long enough to
        // ask it to restart.
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        tree.start_program(second_pane, shell("/tmp")).unwrap();
        tree.record_exit(second_pane, 0, moment(0)).unwrap();

        assert_eq!(
            tree.restart_program(second_pane),
            Err(crate::error::Error::PaneNotFound(second_pane))
        );
    }

    #[test]
    fn new_tab_adds_a_labelled_tab_with_one_pane_already_in_it() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();

        let tab_id = tree.new_tab(session_id, Some("logs".to_string())).unwrap();

        let session = tree.session(session_id).unwrap();
        assert_eq!(session.tabs().len(), 2);
        let tab = session.tab(tab_id).unwrap();
        assert_eq!(tab.label(), Some("logs"));
        assert_eq!(tab.panes().len(), 1);
    }

    #[test]
    fn a_failed_new_tab_does_not_consume_an_id() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap(); // mints raw ids 0 (session), 1 (tab), 2 (pane)
        let unknown_session = steg_sdk::SessionId::from_raw(999);

        assert!(tree.new_tab(unknown_session, None).is_err());

        // Had the failed call minted a tab id and a pane id before
        // checking, this would mint raw 5, not 3.
        let tab_id = tree.new_tab(session_id, None).unwrap();
        assert_eq!(tab_id.into_raw(), 3);
    }

    #[test]
    fn a_failed_split_pane_does_not_consume_an_id() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap(); // mints raw ids 0, 1, 2
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let unknown_pane = steg_sdk::PaneId::from_raw(999);

        assert!(
            tree.split_pane(
                unknown_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal
            )
            .is_err()
        );

        // Had the failed call minted a pane id before checking, this
        // would mint raw 4, not 3.
        let pane_id = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        assert_eq!(pane_id.into_raw(), 3);
    }

    #[test]
    fn split_pane_adds_a_pane_to_an_existing_tab() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();

        let pane_id = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();

        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let tab = tree.session(session_id).unwrap().tab(tab_id).unwrap();
        assert_eq!(tab.panes().len(), 2);
        assert!(tab.pane(pane_id).is_some());
    }

    #[test]
    fn move_tab_reorders_a_sessions_tabs() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();

        tree.move_tab(second_tab, 0).unwrap();

        let session = tree.session(session_id).unwrap();
        assert_eq!(session.tabs()[0].id(), second_tab);
        assert_eq!(session.tabs()[1].id(), first_tab);
    }

    #[test]
    fn session_mut_lets_a_caller_change_a_sessions_label_and_colour() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();

        let session = tree.session_mut(session_id).unwrap();
        session.set_label(Some("work".to_string())).unwrap();
        session.set_colour(Colour::Green);

        let session = tree.session(session_id).unwrap();
        assert_eq!(session.label(), Some("work"));
        assert_eq!(session.colour(), Colour::Green);
    }

    #[test]
    fn new_session_sets_the_label_given_to_it() {
        let mut tree = Tree::new();

        let session_id = tree.new_session(Some("work".to_string())).unwrap();

        assert_eq!(tree.session(session_id).unwrap().label(), Some("work"));
    }

    #[test]
    fn new_session_rejects_an_invalid_label_and_mints_no_id() {
        let mut tree = Tree::new();

        assert_eq!(
            tree.new_session(Some("123".to_string())),
            Err(crate::error::Error::InvalidLabel("123".to_string()))
        );
        assert!(tree.sessions().is_empty());

        // Had the failed call minted ids before validating the label,
        // this session would start at raw id 3, not 0.
        let session_id = tree.new_session(None).unwrap();
        assert_eq!(session_id.into_raw(), 0);
    }

    #[test]
    fn new_tab_rejects_an_invalid_label_and_mints_no_id() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap(); // mints raw ids 0, 1, 2

        assert_eq!(
            tree.new_tab(session_id, Some("a/b".to_string())),
            Err(crate::error::Error::InvalidLabel("a/b".to_string()))
        );
        assert_eq!(tree.session(session_id).unwrap().tabs().len(), 1);

        // Had the failed call minted a tab id and a pane id before
        // validating the label, this would mint raw 5, not 3.
        let tab_id = tree.new_tab(session_id, None).unwrap();
        assert_eq!(tab_id.into_raw(), 3);
    }

    #[test]
    fn move_session_reorders_and_rejects_an_out_of_range_position() {
        let mut tree = Tree::new();
        let first = tree.new_session(None).unwrap();
        let second = tree.new_session(None).unwrap();

        tree.move_session(second, 0).unwrap();

        assert_eq!(tree.sessions()[0].id(), second);
        assert_eq!(tree.sessions()[1].id(), first);

        assert_eq!(
            tree.move_session(first, 5),
            Err(crate::error::Error::PositionOutOfRange {
                len: 2,
                position: 5
            })
        );
    }

    #[test]
    fn a_not_found_id_at_any_level_is_the_matching_error() {
        let mut tree = Tree::new();

        let unknown_session = steg_sdk::SessionId::from_raw(999);
        let unknown_tab = steg_sdk::TabId::from_raw(999);
        let unknown_pane = steg_sdk::PaneId::from_raw(999);

        assert_eq!(
            tree.close_session(unknown_session),
            Err(crate::error::Error::SessionNotFound(unknown_session))
        );
        assert_eq!(
            tree.close_tab(unknown_tab),
            Err(crate::error::Error::TabNotFound(unknown_tab))
        );
        assert_eq!(
            tree.close_pane(unknown_pane),
            Err(crate::error::Error::PaneNotFound(unknown_pane))
        );
        assert_eq!(
            tree.new_tab(unknown_session, None),
            Err(crate::error::Error::SessionNotFound(unknown_session))
        );
        assert_eq!(
            tree.split_pane(
                unknown_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal
            ),
            Err(crate::error::Error::PaneNotFound(unknown_pane))
        );
        assert_eq!(
            tree.move_tab(unknown_tab, 0),
            Err(crate::error::Error::TabNotFound(unknown_tab))
        );
    }

    #[test]
    fn focus_methods_reject_unknown_ids_without_changing_a_populated_tree() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(Some("work".to_string())).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        tree.split_pane(
            first_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
        tree.new_tab(session_id, None).unwrap();

        let unknown_tab = steg_sdk::TabId::from_raw(999);
        let unknown_pane = steg_sdk::PaneId::from_raw(999);
        let before = tree.sessions().to_vec();

        assert_eq!(
            tree.focus_pane(unknown_pane),
            Err(crate::error::Error::PaneNotFound(unknown_pane))
        );
        assert_eq!(tree.sessions().to_vec(), before);

        assert_eq!(
            tree.focus_tab(unknown_tab),
            Err(crate::error::Error::TabNotFound(unknown_tab))
        );
        assert_eq!(tree.sessions().to_vec(), before);

        assert_eq!(
            tree.focus_direction(unknown_pane, steg_sdk::Direction::Right, false),
            Err(crate::error::Error::PaneNotFound(unknown_pane))
        );
        assert_eq!(tree.sessions().to_vec(), before);
    }

    #[test]
    fn focus_pane_moves_the_pane_and_its_tabs_level_together() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        // `new_tab` makes the new tab current (R115), so focus starts
        // pointed away from `first_tab`.
        let second_tab = tree.new_tab(session_id, None).unwrap();
        assert_eq!(tree.session(session_id).unwrap().current_tab(), second_tab);

        let focused = tree.focus_pane(first_pane).unwrap();

        assert_eq!(
            focused,
            Focused {
                session: session_id,
                tab: first_tab,
                pane: first_pane,
            }
        );
        // AC #2: one call moved both levels together.
        assert_eq!(tree.session(session_id).unwrap().current_tab(), first_tab);
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(first_tab)
                .unwrap()
                .current(),
            first_pane
        );
    }

    #[test]
    fn focus_tab_does_not_touch_any_tabs_current_pane() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        // `split_pane` already made `second_pane` current in `first_tab`.
        let _second_tab = tree.new_tab(session_id, None).unwrap();

        let focused = tree.focus_tab(first_tab).unwrap();

        assert_eq!(
            focused,
            Focused {
                session: session_id,
                tab: first_tab,
                pane: second_pane,
            }
        );
        assert_eq!(tree.session(session_id).unwrap().current_tab(), first_tab);
        // decision-5 point 1: `tab focus` cascades pane to tab, never tab
        // to pane — the tab's own current pane is untouched.
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(first_tab)
                .unwrap()
                .current(),
            second_pane
        );
    }

    #[test]
    fn focus_direction_moves_to_the_neighbour_and_reports_it() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        tree.focus_pane(first_pane).unwrap();

        let outcome = tree
            .focus_direction(first_pane, steg_sdk::Direction::Right, false)
            .unwrap();

        assert_eq!(
            outcome,
            FocusMove::Neighbour(Focused {
                session: session_id,
                tab: first_tab,
                pane: second_pane,
            })
        );
        assert_eq!(tree.session(session_id).unwrap().current_tab(), first_tab);
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(first_tab)
                .unwrap()
                .current(),
            second_pane
        );
    }

    #[test]
    fn focus_direction_at_the_edge_changes_nothing_and_reports_edge() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        // A second pane in `first_tab` — `first_pane` is the *left* pane
        // of a Right split, so there genuinely is a pane elsewhere in the
        // tab (to its right), but none to its left: a real edge, not the
        // vacuous case of a single-pane tab where every direction
        // trivially has nothing.
        tree.split_pane(
            first_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
        // A second tab too, so this is a tree with more in it than just
        // the one pane being focused from.
        tree.new_tab(session_id, None).unwrap();
        // Focus back onto `first_tab`/`first_pane`, so the edge check
        // below is against a chosen position, not whatever creation left
        // current.
        tree.focus_pane(first_pane).unwrap();

        let before = tree.sessions().to_vec();

        let outcome = tree
            .focus_direction(first_pane, steg_sdk::Direction::Left, false)
            .unwrap();

        assert_eq!(outcome, FocusMove::Edge);
        // The whole tree — both tabs, both panes, every field — is
        // unchanged, not just the two fields a hand-picked assertion
        // might check.
        assert_eq!(tree.sessions().to_vec(), before);
        assert_eq!(tree.session(session_id).unwrap().current_tab(), first_tab);
    }

    #[test]
    fn focus_direction_wraps_when_asked() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        // `split_pane` made `second_pane` current; moving Right from it
        // has no neighbour, so wrapping lands back on `first_pane`.

        let outcome = tree
            .focus_direction(second_pane, steg_sdk::Direction::Right, true)
            .unwrap();

        assert_eq!(
            outcome,
            FocusMove::Wrapped(Focused {
                session: session_id,
                tab: first_tab,
                pane: first_pane,
            })
        );
        // As the sibling `Neighbour` test does, confirm the tree itself
        // moved, not just the returned value.
        assert_eq!(tree.session(session_id).unwrap().current_tab(), first_tab);
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(first_tab)
                .unwrap()
                .current(),
            first_pane
        );
    }

    #[test]
    fn focus_pane_changes_only_the_current_path_fields() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(Some("work".to_string())).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let _second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        // A second tab, current by R115 — so, before the `focus_pane`
        // below, the session's current tab genuinely is somewhere else
        // (`second_tab`, not `first_tab`), and `first_tab`'s own current
        // pane is `_second_pane` (R115's own split-pane precedent), not
        // `first_pane`. Both fields this test cares about start away from
        // where the focus below sends them.
        let second_tab = tree.new_tab(session_id, None).unwrap();

        let before = tree.sessions().to_vec();
        assert_eq!(
            before
                .iter()
                .find(|session| session.id() == session_id)
                .unwrap()
                .current_tab(),
            second_tab
        );

        tree.focus_pane(first_pane).unwrap();

        let after = tree.sessions().to_vec();

        // Both current-path fields really did move.
        assert_eq!(
            after
                .iter()
                .find(|session| session.id() == session_id)
                .unwrap()
                .current_tab(),
            first_tab
        );
        assert_eq!(
            after
                .iter()
                .find(|session| session.id() == session_id)
                .unwrap()
                .tab(first_tab)
                .unwrap()
                .current(),
            first_pane
        );

        // Patch `before`'s current-path fields, by hand, through the same
        // pub(crate) setters `focus_pane` itself uses, to what `after`
        // moved them to — a *real* change on each field, not a no-op (the
        // session had two tabs, and `first_tab` had two panes, with
        // `current_tab`/`current` both starting away from `first_tab`/
        // `first_pane`). If the patched clone then equals `after` exactly,
        // nothing else anywhere in the tree — label, colour, tab order,
        // pane order, layout, fill — moved along with focus. This is the
        // strongest statement the kernel alone can make toward R88 (AC
        // #4): there is no client type yet (TASK-1.42).
        let mut expected = before;
        let session = expected
            .iter_mut()
            .find(|session| session.id() == session_id)
            .expect("the session is still in `before`'s clone");
        session
            .set_current_tab(first_tab)
            .expect("`first_tab` is still in this session");
        session
            .tab_mut(first_tab)
            .expect("just confirmed present")
            .focus(first_pane)
            .expect("`first_pane` is still in `first_tab`");

        assert_eq!(expected, after);
    }

    #[test]
    fn move_pane_to_tabs_target_tabs_current_pane_moves_but_no_sessions_current_tab_does() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        // A second pane in `first_tab`, so moving one of them out does not
        // also empty (and so close) `first_tab` — this test is about
        // `current_tab`, not R94's cascade.
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        // `new_tab` just made `second_tab` current (R115); move focus
        // back to `first_tab` so the move below is *not* also a focus of
        // its target.
        tree.focus_tab(first_tab).unwrap();
        assert_eq!(tree.session(session_id).unwrap().current_tab(), first_tab);

        tree.move_pane_to_tab(second_pane, second_tab, steg_sdk::Direction::Right)
            .unwrap();

        // The target tab's own current pane did move (TASK-1.36).
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(second_tab)
                .unwrap()
                .current(),
            second_pane
        );
        // But the session's current *tab* is untouched: moving a pane is
        // not a focus (decision 5 / this task's plan).
        assert_eq!(tree.session(session_id).unwrap().current_tab(), first_tab);
    }

    #[test]
    fn closing_the_current_tabs_last_pane_cascades_and_repairs_the_sessions_current_tab() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        // Focus `first_tab`, then close its only pane, which cascades to
        // close the tab itself (R94), triggering the session-level repair
        // (R114).
        tree.focus_tab(first_tab).unwrap();

        let closed = tree.close_pane(first_pane).unwrap();

        assert_eq!(closed.tab, Some(first_tab));
        assert_eq!(tree.session(session_id).unwrap().current_tab(), second_tab);
    }

    #[test]
    fn move_pane_to_tab_leaves_both_tabs_with_a_valid_tree() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        let second_tab = tree.new_tab(session_id, None).unwrap();

        let moved = tree
            .move_pane_to_tab(second_pane, second_tab, steg_sdk::Direction::Right)
            .unwrap();

        assert_eq!(moved.pane, second_pane);
        assert_eq!(moved.to, second_tab);
        assert_eq!(moved.source_tab_closed, None);
        assert_eq!(moved.source_session_closed, None);

        let source = tree.session(session_id).unwrap().tab(first_tab).unwrap();
        assert_eq!(source.panes().len(), 1);
        assert!(source.pane(first_pane).is_some());

        let target = tree.session(session_id).unwrap().tab(second_tab).unwrap();
        assert_eq!(target.panes().len(), 2);
        assert!(target.pane(second_pane).is_some());
        assert_eq!(target.current(), second_pane);
    }

    #[test]
    fn move_pane_to_tab_cascades_closing_the_source_tab_when_it_was_the_last_pane() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();

        let moved = tree
            .move_pane_to_tab(first_pane, second_tab, steg_sdk::Direction::Right)
            .unwrap();

        assert_eq!(moved.source_tab_closed, Some(first_tab));
        assert_eq!(moved.source_session_closed, None);
        let session = tree.session(session_id).unwrap();
        assert_eq!(session.tabs().len(), 1);
        assert_eq!(session.tabs()[0].id(), second_tab);
    }

    #[test]
    fn move_pane_to_tab_cascading_close_of_the_current_source_tab_repairs_the_sessions_current_tab()
    {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        // `new_tab` just made `second_tab` current (R115); focus back
        // onto the *source* tab, so its cascading close below exercises
        // R114's session-level repair, not just R94's pane/tab cascade.
        tree.focus_tab(first_tab).unwrap();
        assert_eq!(tree.session(session_id).unwrap().current_tab(), first_tab);

        let moved = tree
            .move_pane_to_tab(first_pane, second_tab, steg_sdk::Direction::Right)
            .unwrap();

        assert_eq!(moved.source_tab_closed, Some(first_tab));
        // R114: `first_tab` (the current tab) closed, and `second_tab` —
        // the only tab left, so also the new last one — takes its place.
        assert_eq!(tree.session(session_id).unwrap().current_tab(), second_tab);
        assert!(tree.session(session_id).unwrap().tab(second_tab).is_some());
    }

    #[test]
    fn move_pane_to_tab_refuses_a_panes_own_tab() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();

        assert_eq!(
            tree.move_pane_to_tab(first_pane, first_tab, steg_sdk::Direction::Right),
            Err(crate::error::Error::AlreadyInTab {
                pane: first_pane,
                tab: first_tab,
            })
        );
    }

    #[test]
    fn move_pane_to_tab_is_refused_when_the_target_tabs_floor_would_be_violated_and_nothing_changes()
     {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        let second_tab_pane = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .panes()[0]
            .id();
        // 39 columns halves to 19 and 19 — under the 20-column floor.
        tree.set_tab_size(second_tab, steg_sdk::Size::new(39, 10))
            .unwrap();

        // The whole of both tabs, before the refused move — `Tab`
        // derives `PartialEq`, so this one comparison covers layout,
        // pane order, current pane and fill together, not just whatever
        // a hand-picked set of assertions happened to check.
        let source_before = tree
            .session(session_id)
            .unwrap()
            .tab(first_tab)
            .unwrap()
            .clone();
        let target_before = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .clone();

        let result = tree.move_pane_to_tab(first_pane, second_tab, steg_sdk::Direction::Right);

        assert_eq!(
            result,
            Err(crate::error::Error::SplitTooSmall {
                pane: second_tab_pane
            })
        );
        assert_eq!(
            tree.session(session_id).unwrap().tab(first_tab).unwrap(),
            &source_before
        );
        assert_eq!(
            tree.session(session_id).unwrap().tab(second_tab).unwrap(),
            &target_before
        );
    }

    #[test]
    fn panes_are_in_layout_order_after_a_move() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        let second_tab_first_pane = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .panes()[0]
            .id();

        tree.move_pane_to_tab(first_pane, second_tab, steg_sdk::Direction::Left)
            .unwrap();

        let target = tree.session(session_id).unwrap().tab(second_tab).unwrap();
        // Direction::Left puts the moved pane first.
        assert_eq!(
            target
                .panes()
                .iter()
                .map(crate::pane::Pane::id)
                .collect::<Vec<_>>(),
            vec![first_pane, second_tab_first_pane]
        );
    }

    #[test]
    fn swap_panes_is_refused_when_the_two_panes_are_in_different_tabs() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        let second_tab_pane = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .panes()[0]
            .id();

        assert_eq!(
            tree.swap_panes(first_pane, second_tab_pane),
            Err(crate::error::Error::PanesNotInSameTab {
                a: first_pane,
                b: second_tab_pane,
            })
        );
    }

    #[test]
    fn set_tab_size_smaller_always_succeeds_and_closes_nothing() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();

        tree.set_tab_size(tab_id, steg_sdk::Size::new(5, 1))
            .unwrap();

        assert!(tree.session(session_id).is_some());
        let tab = tree.session(session_id).unwrap().tab(tab_id).unwrap();
        assert_eq!(tab.panes().len(), 1);
    }

    // TASK-1.42: clients, what each views, and the size a terminal takes.

    #[test]
    fn connect_client_then_disconnect_client_round_trips() {
        let mut tree = Tree::new();

        let client = tree.connect_client(steg_sdk::ClientKind::Cli, Some("work".to_string()));

        assert_eq!(tree.clients().len(), 1);
        assert_eq!(
            tree.client(client).unwrap().kind(),
            steg_sdk::ClientKind::Cli
        );
        assert_eq!(tree.client(client).unwrap().label(), Some("work"));

        tree.disconnect_client(client).unwrap();

        assert!(tree.client(client).is_none());
        assert!(tree.clients().is_empty());
    }

    #[test]
    fn disconnect_client_rejects_an_unknown_id() {
        let mut tree = Tree::new();
        let unknown = steg_sdk::ClientId::from_raw(999);

        assert_eq!(
            tree.disconnect_client(unknown),
            Err(crate::error::Error::ClientNotFound(unknown))
        );
    }

    #[test]
    fn viewing_follows_from_attachment_not_a_separate_setter() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);

        assert_eq!(tree.sessions_viewed(client).unwrap(), Vec::new());

        tree.view_pane(client, pane).unwrap();
        assert_eq!(tree.sessions_viewed(client).unwrap(), vec![session_id]);

        tree.stop_viewing(client, pane).unwrap();
        assert_eq!(tree.sessions_viewed(client).unwrap(), Vec::new());
    }

    #[test]
    fn view_pane_is_idempotent_and_rejects_an_unknown_pane() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);

        tree.view_pane(client, pane).unwrap();
        tree.view_pane(client, pane).unwrap();
        assert_eq!(tree.client(client).unwrap().viewing(), &[pane]);

        let unknown_pane = steg_sdk::PaneId::from_raw(999);
        assert_eq!(
            tree.view_pane(client, unknown_pane),
            Err(crate::error::Error::PaneNotFound(unknown_pane))
        );
    }

    #[test]
    fn stop_viewing_rejects_a_pane_the_client_does_not_view() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);

        assert_eq!(
            tree.stop_viewing(client, pane),
            Err(crate::error::Error::ClientNotViewingPane { client, pane })
        );
    }

    /// Captures every one of `tree`'s sessions' current tab *and* every
    /// tab's current pane - the whole focus state R88/decision-5 point 4
    /// says a client operation must never touch - as one comparable
    /// value. R85 is where focus bottoms out: a session's current tab,
    /// and a tab's current pane. There is no "current session" above
    /// that for this to also capture - nothing in the kernel designates
    /// one session as current (that is the app's, per decision-5, not
    /// the tree's).
    ///
    /// Each row is `(session, session's current tab, this tab, this
    /// tab's current pane)`, one row per tab - not `(session, this tab,
    /// this tab's current pane)` as an earlier version of this helper
    /// had it, which never read `session.current_tab()` at all and so
    /// could not have caught a fault that moved only *that* (the
    /// session's choice of current tab), only one that moved a tab's own
    /// current pane.
    fn whole_focus(
        tree: &Tree,
    ) -> Vec<(
        steg_sdk::SessionId,
        steg_sdk::TabId,
        steg_sdk::TabId,
        steg_sdk::PaneId,
    )> {
        tree.sessions()
            .iter()
            .flat_map(|session| {
                let session_id = session.id();
                let current_tab = session.current_tab();
                session
                    .tabs()
                    .iter()
                    .map(move |tab| (session_id, current_tab, tab.id(), tab.current()))
            })
            .collect()
    }

    #[test]
    fn connecting_viewing_declaring_note_input_stop_viewing_and_disconnecting_never_change_focus() {
        let mut tree = Tree::new();

        // Session A: two tabs, each with two panes. Splitting makes the
        // new pane current, and a new tab becomes its session's current
        // tab (R115) - so the deliberately-unfocused target, tab_a1's
        // *first* pane, falls out of this setup with no extra focus call
        // of our own that could itself get the baseline wrong.
        let session_a = tree.new_session(None).unwrap();
        let tab_a1 = tree.session(session_a).unwrap().tabs()[0].id();
        let a1_first = tree.session(session_a).unwrap().tabs()[0].panes()[0].id();
        tree.split_pane(
            a1_first,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
        let tab_a2 = tree.new_tab(session_a, None).unwrap();
        let a2_first = tree
            .session(session_a)
            .unwrap()
            .tab(tab_a2)
            .unwrap()
            .panes()[0]
            .id();
        tree.split_pane(
            a2_first,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();

        // Session B: a second session, a bystander the client never
        // touches - its focus is a control that must also hold still.
        let session_b = tree.new_session(None).unwrap();
        let b1_first = tree.session(session_b).unwrap().tabs()[0].panes()[0].id();
        tree.split_pane(
            b1_first,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
        let tab_b2 = tree.new_tab(session_b, None).unwrap();
        let b2_first = tree
            .session(session_b)
            .unwrap()
            .tab(tab_b2)
            .unwrap()
            .panes()[0]
            .id();
        tree.split_pane(
            b2_first,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();

        // Confirm the deliberate mismatch before touching any client:
        // session A's current tab is tab_a2, not tab_a1 - the tab the
        // client is about to view and act on - and tab_a1's current pane
        // is its *second* pane, not `a1_first`.
        assert_eq!(tree.session(session_a).unwrap().current_tab(), tab_a2);
        assert_ne!(
            tree.session(session_a)
                .unwrap()
                .tab(tab_a1)
                .unwrap()
                .current(),
            a1_first
        );

        let before = whole_focus(&tree);

        let client = tree.connect_client(steg_sdk::ClientKind::App, None);
        assert_eq!(whole_focus(&tree), before, "after connect_client");

        tree.view_pane(client, a1_first).unwrap();
        assert_eq!(whole_focus(&tree), before, "after view_pane");

        tree.declare_size(client, tab_a1, steg_sdk::Size::new(80, 24))
            .unwrap();
        assert_eq!(whole_focus(&tree), before, "after declare_size");

        tree.note_input(client, a1_first).unwrap();
        assert_eq!(whole_focus(&tree), before, "after note_input");

        tree.stop_viewing(client, a1_first).unwrap();
        assert_eq!(whole_focus(&tree), before, "after stop_viewing");

        tree.disconnect_client(client).unwrap();
        assert_eq!(whole_focus(&tree), before, "after disconnect_client");
    }

    #[test]
    fn viewing_alone_never_makes_a_client_latest() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let viewer = tree.connect_client(steg_sdk::ClientKind::App, None);
        tree.view_pane(viewer, pane).unwrap();
        let declarer = tree.connect_client(steg_sdk::ClientKind::Cli, None);
        tree.view_pane(declarer, pane).unwrap();
        tree.declare_size(declarer, tab_id, steg_sdk::Size::new(80, 24))
            .unwrap();

        assert_eq!(tree.latest_client(session_id).unwrap(), Some(declarer));
    }

    #[test]
    fn latest_client_is_none_when_nobody_views_the_session() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();

        assert_eq!(tree.latest_client(session_id).unwrap(), None);
    }

    #[test]
    fn latest_client_tracks_whoever_last_declared_or_sent_input() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let a = tree.connect_client(steg_sdk::ClientKind::App, None);
        let b = tree.connect_client(steg_sdk::ClientKind::Cli, None);
        tree.view_pane(a, pane).unwrap();
        tree.view_pane(b, pane).unwrap();

        tree.declare_size(a, tab_id, steg_sdk::Size::new(80, 24))
            .unwrap();
        assert_eq!(tree.latest_client(session_id).unwrap(), Some(a));

        tree.declare_size(b, tab_id, steg_sdk::Size::new(100, 30))
            .unwrap();
        assert_eq!(tree.latest_client(session_id).unwrap(), Some(b));

        tree.note_input(a, pane).unwrap();
        assert_eq!(tree.latest_client(session_id).unwrap(), Some(a));
    }

    #[test]
    fn latest_falls_back_to_the_next_most_recently_active_remaining_viewer_on_departure() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let a = tree.connect_client(steg_sdk::ClientKind::App, None);
        let b = tree.connect_client(steg_sdk::ClientKind::Cli, None);
        tree.view_pane(a, pane).unwrap();
        tree.view_pane(b, pane).unwrap();
        tree.declare_size(a, tab_id, steg_sdk::Size::new(80, 24))
            .unwrap();
        tree.declare_size(b, tab_id, steg_sdk::Size::new(100, 30))
            .unwrap();
        assert_eq!(tree.latest_client(session_id).unwrap(), Some(b));

        tree.disconnect_client(b).unwrap();

        assert_eq!(tree.latest_client(session_id).unwrap(), Some(a));
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(tab_id)
                .unwrap()
                .size(),
            Some(steg_sdk::Size::new(80, 24)),
            "falling back to a reapplies its own last-declared size"
        );
    }

    #[test]
    fn reapply_latest_leaves_the_tabs_size_as_it_was_when_the_new_latest_has_no_declared_size() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let a = tree.connect_client(steg_sdk::ClientKind::App, None);
        let b = tree.connect_client(steg_sdk::ClientKind::Cli, None);
        tree.view_pane(a, pane).unwrap();
        tree.view_pane(b, pane).unwrap();
        tree.declare_size(a, tab_id, steg_sdk::Size::new(80, 24))
            .unwrap();

        // `b` becomes latest by sending input, never by declaring a size.
        tree.note_input(b, pane).unwrap();
        assert_eq!(tree.latest_client(session_id).unwrap(), Some(b));

        // `a` leaves; `reapply_latest` recomputes latest as `b` again (no
        // change there) and tries to reapply `b`'s declared size - `b`
        // has none, so the tab's size holds as it was, and no change is
        // reported.
        let changes = tree.disconnect_client(a).unwrap();

        assert_eq!(
            changes,
            Vec::new(),
            "b, the remaining latest, has no declared size to reapply"
        );
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(tab_id)
                .unwrap()
                .size(),
            Some(steg_sdk::Size::new(80, 24)),
            "the tab's size holds as it was"
        );
        assert_eq!(tree.latest_client(session_id).unwrap(), Some(b));
    }

    #[test]
    fn a_declaration_always_wins_regardless_of_magnitude() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let big = tree.connect_client(steg_sdk::ClientKind::App, None);
        let small = tree.connect_client(steg_sdk::ClientKind::Cli, None);
        tree.view_pane(big, pane).unwrap();
        tree.view_pane(small, pane).unwrap();

        tree.declare_size(big, tab_id, steg_sdk::Size::new(200, 60))
            .unwrap();
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(tab_id)
                .unwrap()
                .size(),
            Some(steg_sdk::Size::new(200, 60))
        );

        // A later, smaller declaration still wins - magnitude plays no
        // part, only recency (decision-6).
        tree.declare_size(small, tab_id, steg_sdk::Size::new(40, 10))
            .unwrap();
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(tab_id)
                .unwrap()
                .size(),
            Some(steg_sdk::Size::new(40, 10))
        );
    }

    #[test]
    fn declare_size_reports_every_panes_absolute_size_and_every_viewer() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();

        let declarer = tree.connect_client(steg_sdk::ClientKind::App, None);
        let other_viewer = tree.connect_client(steg_sdk::ClientKind::Cli, None);
        tree.view_pane(declarer, first_pane).unwrap();
        tree.view_pane(other_viewer, second_pane).unwrap();

        let changed = tree
            .declare_size(declarer, tab_id, steg_sdk::Size::new(40, 10))
            .unwrap();

        assert_eq!(changed.tab, tab_id);
        assert_eq!(changed.size, steg_sdk::Size::new(40, 10));
        let expected_panes = tree
            .session(session_id)
            .unwrap()
            .tab(tab_id)
            .unwrap()
            .layout()
            .leaf_sizes(steg_sdk::Size::new(40, 10));
        assert_eq!(changed.panes, expected_panes);
        assert_eq!(changed.panes.len(), 2);
        let mut viewers = changed.viewers;
        viewers.sort_by_key(|client| client.into_raw());
        let mut expected_viewers = vec![declarer, other_viewer];
        expected_viewers.sort_by_key(|client| client.into_raw());
        assert_eq!(viewers, expected_viewers);
    }

    #[test]
    fn declare_size_never_fails_the_floor() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);
        tree.view_pane(client, pane).unwrap();

        // Well under R100's 20x4 floor - still applied, nothing refused.
        let changed = tree
            .declare_size(client, tab_id, steg_sdk::Size::new(5, 1))
            .unwrap();

        assert_eq!(changed.size, steg_sdk::Size::new(5, 1));
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(tab_id)
                .unwrap()
                .size(),
            Some(steg_sdk::Size::new(5, 1))
        );
    }

    #[test]
    fn declare_size_requires_viewing_a_pane_in_the_tabs_session() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);

        assert_eq!(
            tree.declare_size(client, tab_id, steg_sdk::Size::new(80, 24)),
            Err(crate::error::Error::ClientNotViewingTab {
                client,
                tab: tab_id
            })
        );
    }

    #[test]
    fn note_input_reapplies_the_now_latest_clients_last_declared_size() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let a = tree.connect_client(steg_sdk::ClientKind::App, None);
        let b = tree.connect_client(steg_sdk::ClientKind::Cli, None);
        tree.view_pane(a, pane).unwrap();
        tree.view_pane(b, pane).unwrap();
        tree.declare_size(a, tab_id, steg_sdk::Size::new(80, 24))
            .unwrap();
        tree.declare_size(b, tab_id, steg_sdk::Size::new(100, 30))
            .unwrap();

        let changed = tree.note_input(a, pane).unwrap().unwrap();

        assert_eq!(changed.tab, tab_id);
        assert_eq!(changed.size, steg_sdk::Size::new(80, 24));
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(tab_id)
                .unwrap()
                .size(),
            Some(steg_sdk::Size::new(80, 24))
        );
    }

    #[test]
    fn note_input_is_a_no_op_when_the_client_has_no_declared_size() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);
        tree.view_pane(client, pane).unwrap();

        assert_eq!(tree.note_input(client, pane).unwrap(), None);
    }

    #[test]
    fn note_input_is_a_no_op_when_the_declared_size_already_matches_the_tabs_size() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);
        tree.view_pane(client, pane).unwrap();
        tree.declare_size(client, tab_id, steg_sdk::Size::new(80, 24))
            .unwrap();

        // The tab's size is already exactly what this client declared -
        // nothing to reapply, so no change is reported.
        assert_eq!(tree.note_input(client, pane).unwrap(), None);
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(tab_id)
                .unwrap()
                .size(),
            Some(steg_sdk::Size::new(80, 24))
        );
    }

    #[test]
    fn note_input_rejects_a_pane_the_client_does_not_view() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);

        assert_eq!(
            tree.note_input(client, pane),
            Err(crate::error::Error::ClientNotViewingPane { client, pane })
        );
    }

    #[test]
    fn closing_a_pane_a_client_views_drops_the_attachment_and_reapplies_the_session() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_pane = tree
            .split_pane(
                first_pane,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();

        let leaving = tree.connect_client(steg_sdk::ClientKind::App, None);
        let staying = tree.connect_client(steg_sdk::ClientKind::Cli, None);
        tree.view_pane(leaving, first_pane).unwrap();
        tree.view_pane(staying, second_pane).unwrap();
        tree.declare_size(leaving, tab_id, steg_sdk::Size::new(80, 24))
            .unwrap();
        tree.declare_size(staying, tab_id, steg_sdk::Size::new(100, 30))
            .unwrap();
        assert_eq!(tree.latest_client(session_id).unwrap(), Some(staying));

        // Give `leaving` the freshest activity, then close the pane it is
        // attached to: it should drop out of the session's viewers
        // entirely, falling back to `staying`.
        tree.note_input(leaving, first_pane).unwrap();
        assert_eq!(tree.latest_client(session_id).unwrap(), Some(leaving));

        tree.close_pane(first_pane).unwrap();

        assert_eq!(tree.client(leaving).unwrap().viewing(), &[]);
        assert_eq!(tree.sessions_viewed(leaving).unwrap(), Vec::new());
        assert_eq!(tree.latest_client(session_id).unwrap(), Some(staying));
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(tab_id)
                .unwrap()
                .size(),
            Some(steg_sdk::Size::new(100, 30)),
            "the surviving tab falls back to the remaining viewer's declared size"
        );
    }

    #[test]
    fn closing_a_tab_drops_its_declared_size_record_from_every_client() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        let second_pane = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .panes()[0]
            .id();

        let client = tree.connect_client(steg_sdk::ClientKind::App, None);
        tree.view_pane(client, first_pane).unwrap();
        tree.view_pane(client, second_pane).unwrap();
        tree.declare_size(client, first_tab, steg_sdk::Size::new(80, 24))
            .unwrap();

        tree.close_pane(first_pane).unwrap();

        assert_eq!(tree.client(client).unwrap().viewing(), &[second_pane]);
        // `first_tab`'s id is retired for good (ids are never reused), so
        // nothing in this tree's remaining lifetime could ever ask
        // `declared_size(first_tab)` again - behaviour alone cannot tell
        // a purged record apart from a merely-unreachable one. Check the
        // record directly instead (`Client::declared_size` is
        // `pub(crate)`, same crate as this test module).
        assert_eq!(tree.client(client).unwrap().declared_size(first_tab), None);
    }

    #[test]
    fn closing_a_session_drops_every_clients_attachment_and_activity_for_it() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);
        tree.view_pane(client, pane).unwrap();
        tree.declare_size(client, tab_id, steg_sdk::Size::new(80, 24))
            .unwrap();

        assert_eq!(
            tree.client(client).unwrap().activity_in(session_id),
            Some(1),
            "declare_size above ticked activity for this session once"
        );

        tree.close_session(session_id).unwrap();

        assert_eq!(tree.client(client).unwrap().viewing(), &[]);
        assert_eq!(tree.sessions_viewed(client).unwrap(), Vec::new());
        // The name's other half: `session_id`'s own id is retired for
        // good, so nothing could ever behaviourally observe a stray
        // activity record for it either - check it directly, the same
        // reasoning as `closing_a_tab_drops_its_declared_size_record_
        // from_every_client`.
        assert_eq!(tree.client(client).unwrap().activity_in(session_id), None);
    }

    #[test]
    fn closing_a_session_drops_both_clients_when_each_views_a_different_tab_of_it() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        let second_pane = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .panes()[0]
            .id();

        let first_client = tree.connect_client(steg_sdk::ClientKind::App, None);
        let second_client = tree.connect_client(steg_sdk::ClientKind::Cli, None);
        tree.view_pane(first_client, first_pane).unwrap();
        tree.view_pane(second_client, second_pane).unwrap();

        tree.close_session(session_id).unwrap();

        // Nothing is left naming either gone pane or the gone tab, for
        // either client.
        assert_eq!(tree.client(first_client).unwrap().viewing(), &[]);
        assert_eq!(tree.client(second_client).unwrap().viewing(), &[]);
        assert_eq!(tree.sessions_viewed(first_client).unwrap(), Vec::new());
        assert_eq!(tree.sessions_viewed(second_client).unwrap(), Vec::new());
    }

    #[test]
    fn closing_one_of_two_tabs_only_drops_the_client_viewing_that_tab() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        let second_pane = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .panes()[0]
            .id();

        let leaving = tree.connect_client(steg_sdk::ClientKind::App, None);
        let staying = tree.connect_client(steg_sdk::ClientKind::Cli, None);
        tree.view_pane(leaving, first_pane).unwrap();
        tree.view_pane(staying, second_pane).unwrap();
        tree.declare_size(staying, second_tab, steg_sdk::Size::new(80, 24))
            .unwrap();

        tree.close_tab(first_tab).unwrap();

        // The session survives (it had a second tab), so this is the
        // non-cascading path - only `leaving`'s attachment to the now-gone
        // tab is dropped.
        assert_eq!(tree.client(leaving).unwrap().viewing(), &[]);
        assert_eq!(tree.sessions_viewed(leaving).unwrap(), Vec::new());
        assert_eq!(tree.client(staying).unwrap().viewing(), &[second_pane]);
        assert_eq!(
            tree.sessions_viewed(staying).unwrap(),
            vec![session_id],
            "staying still views the surviving tab, so it still views the session"
        );
    }

    #[test]
    fn tab_of_pane_finds_the_tab_holding_a_pane() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        let second_tab_pane = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .panes()[0]
            .id();

        assert_eq!(tree.tab_of_pane(first_pane), Some(first_tab));
        assert_eq!(tree.tab_of_pane(second_tab_pane), Some(second_tab));
    }

    #[test]
    fn tab_of_pane_is_none_for_an_unknown_pane() {
        let tree = Tree::new();
        let unknown = steg_sdk::PaneId::from_raw(999);

        assert_eq!(tree.tab_of_pane(unknown), None);
    }

    #[test]
    fn set_tab_current_pane_moves_only_the_tabs_own_current_pane() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        tree.split_pane(
            first_pane,
            steg_sdk::Direction::Right,
            steg_sdk::PaneKind::Terminal,
        )
        .unwrap();
        // `split_pane` made the new pane current (R115); a second tab,
        // current by the same rule, so the session's own `current_tab` is
        // genuinely somewhere else before the call below.
        let second_tab = tree.new_tab(session_id, None).unwrap();
        assert_eq!(tree.session(session_id).unwrap().current_tab(), second_tab);

        tree.set_tab_current_pane(first_tab, first_pane).unwrap();

        // The tab's own current pane moved...
        assert_eq!(
            tree.session(session_id)
                .unwrap()
                .tab(first_tab)
                .unwrap()
                .current(),
            first_pane
        );
        // ...but, unlike `focus_pane`, the session's current tab did not
        // cascade to `first_tab`.
        assert_eq!(tree.session(session_id).unwrap().current_tab(), second_tab);
    }

    #[test]
    fn set_tab_current_pane_rejects_an_unknown_tab_or_pane() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let unknown_tab = steg_sdk::TabId::from_raw(999);
        let unknown_pane = steg_sdk::PaneId::from_raw(999);
        let first_tab = tree.session(session_id).unwrap().tabs()[0].id();

        assert_eq!(
            tree.set_tab_current_pane(unknown_tab, first_pane),
            Err(crate::error::Error::TabNotFound(unknown_tab))
        );
        assert_eq!(
            tree.set_tab_current_pane(first_tab, unknown_pane),
            Err(crate::error::Error::PaneNotFound(unknown_pane))
        );
    }

    // --- status (doc-11, decision-7; TASK-1.41) ---------------------------

    fn finish(tree: &mut Tree, pane: steg_sdk::PaneId) {
        tree.start_program(pane, command("/tmp")).unwrap();
        tree.record_exit(pane, 0, moment(0)).unwrap();
    }

    fn fail(tree: &mut Tree, pane: steg_sdk::PaneId) {
        tree.start_program(pane, command("/tmp")).unwrap();
        tree.record_exit(pane, 3, moment(0)).unwrap();
    }

    #[test]
    fn note_input_ends_a_finished_status_on_the_pane_it_was_sent_to() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        finish(&mut tree, pane);
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);
        tree.view_pane(client, pane).unwrap();

        tree.note_input(client, pane).unwrap();

        let pane_ref = &tree.session(session_id).unwrap().tabs()[0].panes()[0];
        assert_eq!(pane_ref.status(moment(0)), None);
    }

    #[test]
    fn note_input_ends_a_failed_status_on_the_pane_it_was_sent_to() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        fail(&mut tree, pane);
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);
        tree.view_pane(client, pane).unwrap();

        tree.note_input(client, pane).unwrap();

        let pane_ref = &tree.session(session_id).unwrap().tabs()[0].panes()[0];
        assert_eq!(pane_ref.status(moment(0)), None);
    }

    #[test]
    fn note_input_has_nothing_to_end_on_a_running_pane() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        tree.start_program(pane, command("/tmp")).unwrap();
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);
        tree.view_pane(client, pane).unwrap();

        tree.note_input(client, pane).unwrap();

        let pane_ref = &tree.session(session_id).unwrap().tabs()[0].panes()[0];
        assert_eq!(pane_ref.status(moment(0)), Some(steg_sdk::Status::Running));
    }

    #[test]
    fn note_input_to_one_pane_does_not_end_another_panes_status_in_the_same_tab() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second = tree
            .split_pane(
                first,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        finish(&mut tree, first);
        fail(&mut tree, second);
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);
        tree.view_pane(client, first).unwrap();

        tree.note_input(client, first).unwrap();

        let tab = &tree.session(session_id).unwrap().tabs()[0];
        assert_eq!(tab.pane(first).unwrap().status(moment(0)), None);
        assert_eq!(
            tab.pane(second).unwrap().status(moment(0)),
            Some(steg_sdk::Status::Failed { exit_code: 3 })
        );
    }

    #[test]
    fn acknowledge_pane_ends_a_finished_status() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        finish(&mut tree, pane);

        tree.acknowledge_pane(pane).unwrap();

        let pane_ref = &tree.session(session_id).unwrap().tabs()[0].panes()[0];
        assert_eq!(pane_ref.status(moment(0)), None);
    }

    #[test]
    fn acknowledge_pane_ends_a_failed_status() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        fail(&mut tree, pane);

        tree.acknowledge_pane(pane).unwrap();

        let pane_ref = &tree.session(session_id).unwrap().tabs()[0].panes()[0];
        assert_eq!(pane_ref.status(moment(0)), None);
    }

    #[test]
    fn acknowledge_pane_is_a_no_op_when_the_pane_has_no_status() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();

        tree.acknowledge_pane(pane).unwrap();

        let pane_ref = &tree.session(session_id).unwrap().tabs()[0].panes()[0];
        assert_eq!(pane_ref.status(moment(0)), None);
    }

    #[test]
    fn acknowledge_pane_on_an_unknown_pane_errors() {
        let mut tree = Tree::new();
        let unknown = steg_sdk::PaneId::from_raw(999);

        assert_eq!(
            tree.acknowledge_pane(unknown),
            Err(crate::error::Error::PaneNotFound(unknown))
        );
    }

    #[test]
    fn a_command_panes_full_cycle_and_each_steps_tie_goes_to_the_newer_pane() {
        // `a` runs through start, exit non-zero, input, restart, exit
        // zero, restart, exit zero again - doc-15 R97's full cycle for a
        // command pane. At every step, `b` is driven through the exact
        // same step first, so the two panes always tie on rank; `a`'s
        // tick is always the newer of the two, and decision-7 point 3
        // ("how pressing, then newest") says it should win the tie every
        // time - proving the tie-break holds across the whole lifecycle,
        // not just once.
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let a = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let b = tree
            .split_pane(a, steg_sdk::Direction::Right, steg_sdk::PaneKind::Terminal)
            .unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        let client = tree.connect_client(steg_sdk::ClientKind::App, None);
        tree.view_pane(client, a).unwrap();

        macro_rules! assert_status_and_tie {
            ($status:expr) => {
                let tab = &tree.session(session_id).unwrap().tabs()[0];
                assert_eq!(tab.pane(a).unwrap().status(moment(0)), $status);
                assert_eq!(tree.tab_shown_status(tab_id, moment(0)), Ok(Some(a)));
            };
        }

        // Step 1: start. Both running (tied rank) - `b` first, `a`
        // second, so `a` is the newer and wins.
        tree.start_program(b, command("/tmp")).unwrap();
        tree.start_program(a, command("/tmp")).unwrap();
        assert_status_and_tie!(Some(steg_sdk::Status::Running));

        // Step 2: exit non-zero - both Failed (tied rank).
        tree.record_exit(b, 3, moment(0)).unwrap();
        tree.record_exit(a, 3, moment(0)).unwrap();
        assert_status_and_tie!(Some(steg_sdk::Status::Failed { exit_code: 3 }));

        // Step 3: input ends `a`'s status (never `b`'s, which was never
        // sent any) - no tie to break any more: `a` has no status at
        // all, so `b`'s Failed is what the tab shows instead.
        tree.note_input(client, a).unwrap();
        let tab = &tree.session(session_id).unwrap().tabs()[0];
        assert_eq!(tab.pane(a).unwrap().status(moment(0)), None);
        assert_eq!(tree.tab_shown_status(tab_id, moment(0)), Ok(Some(b)));

        // Step 4: restart - both running again (tied rank).
        tree.restart_program(b).unwrap();
        tree.restart_program(a).unwrap();
        assert_status_and_tie!(Some(steg_sdk::Status::Running));

        // Step 5: exit zero - both Finished (tied rank).
        tree.record_exit(b, 0, moment(0)).unwrap();
        tree.record_exit(a, 0, moment(0)).unwrap();
        assert_status_and_tie!(Some(steg_sdk::Status::Finished));

        // Step 6: restart again.
        tree.restart_program(b).unwrap();
        tree.restart_program(a).unwrap();
        assert_status_and_tie!(Some(steg_sdk::Status::Running));

        // Step 7: exit zero again - Finished once more.
        tree.record_exit(b, 0, moment(0)).unwrap();
        tree.record_exit(a, 0, moment(0)).unwrap();
        assert_status_and_tie!(Some(steg_sdk::Status::Finished));
    }

    #[test]
    fn tab_shown_status_delegates_to_the_tabs_own_ranking() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second = tree
            .split_pane(
                first,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        finish(&mut tree, first);
        fail(&mut tree, second);

        assert_eq!(tree.tab_shown_status(tab_id, moment(0)), Ok(Some(second)));
    }

    #[test]
    fn tab_shown_status_on_an_unknown_tab_errors() {
        let tree = Tree::new();
        let unknown = steg_sdk::TabId::from_raw(999);

        assert_eq!(
            tree.tab_shown_status(unknown, moment(0)),
            Err(crate::error::Error::TabNotFound(unknown))
        );
    }

    #[test]
    fn opening_a_tab_or_focusing_a_pane_ends_nothing() {
        // doc-15/decision-7: "opening a tab clears nothing" - neither
        // `focus_pane`, `focus_tab` nor `focus_direction` touches a
        // pane's status, however pressing it is.
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second = tree
            .split_pane(
                first,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        let tab_id = tree.session(session_id).unwrap().tabs()[0].id();
        finish(&mut tree, first);
        fail(&mut tree, second);
        let deadline_before = tree.next_status_deadline(moment(0));

        tree.focus_pane(first).unwrap();
        tree.focus_pane(second).unwrap();
        tree.focus_tab(tab_id).unwrap();
        tree.focus_direction(first, steg_sdk::Direction::Right, false)
            .unwrap();

        let tab = &tree.session(session_id).unwrap().tabs()[0];
        assert_eq!(
            tab.pane(first).unwrap().status(moment(0)),
            Some(steg_sdk::Status::Finished)
        );
        assert_eq!(
            tab.pane(second).unwrap().status(moment(0)),
            Some(steg_sdk::Status::Failed { exit_code: 3 })
        );
        // `first`'s `Finished` deadline (R83's ten-minute timer) is
        // untouched too - not just its status - so there is no later
        // "focusing secretly reset the clock" bug this would miss.
        assert_eq!(deadline_before, Some(moment(600)));
        assert_eq!(tree.next_status_deadline(moment(0)), deadline_before);
    }

    #[test]
    fn next_status_deadline_on_an_empty_tree_is_none() {
        let tree = Tree::new();

        assert_eq!(tree.next_status_deadline(moment(0)), None);
    }

    #[test]
    fn next_status_deadline_is_none_when_nothing_is_finished() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second = tree
            .split_pane(
                first,
                steg_sdk::Direction::Right,
                steg_sdk::PaneKind::Terminal,
            )
            .unwrap();
        tree.start_program(first, command("/tmp")).unwrap();
        fail(&mut tree, second);

        assert_eq!(tree.next_status_deadline(moment(0)), None);
    }

    #[test]
    fn next_status_deadline_is_the_earliest_across_several_panes_and_tabs() {
        let mut tree = Tree::new();
        let session_id = tree.new_session(None).unwrap();
        let first = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
        let second_tab = tree.new_tab(session_id, None).unwrap();
        let second = tree
            .session(session_id)
            .unwrap()
            .tab(second_tab)
            .unwrap()
            .panes()[0]
            .id();

        // `first` finishes at moment(500) (deadline 500 + 600 = 1100);
        // `second` finishes at moment(0) (deadline 0 + 600 = 600) - the
        // earlier of the two, in a different tab entirely.
        tree.start_program(first, command("/tmp")).unwrap();
        tree.record_exit(first, 0, moment(500)).unwrap();
        tree.start_program(second, command("/tmp")).unwrap();
        tree.record_exit(second, 0, moment(0)).unwrap();

        assert_eq!(tree.next_status_deadline(moment(0)), Some(moment(600)));
    }

    #[test]
    fn next_status_deadline_skips_a_pane_already_past_its_deadline_and_is_none_once_every_pane_is()
    {
        let mut tree = Tree::new();
        let session_one = tree.new_session(None).unwrap();
        let tab_one = tree.session(session_one).unwrap().tabs()[0].panes()[0].id();
        let tab_two_id = tree.new_tab(session_one, None).unwrap();
        let tab_two = tree
            .session(session_one)
            .unwrap()
            .tab(tab_two_id)
            .unwrap()
            .panes()[0]
            .id();
        let session_two = tree.new_session(None).unwrap();
        let other_session = tree.session(session_two).unwrap().tabs()[0].panes()[0].id();

        // Three panes, each in a different tab, two of them in different
        // sessions entirely, each finishing at a different moment:
        // - `tab_one` at 0 (deadline 600) - already past by the time
        //   anything here is asked at `moment(700)`.
        // - `tab_two` at 500 (deadline 1100) - still ahead, and the
        //   earliest of the two still ahead.
        // - `other_session` at 1000 (deadline 1600) - also still ahead,
        //   but later than `tab_two`'s.
        tree.start_program(tab_one, command("/tmp")).unwrap();
        tree.record_exit(tab_one, 0, moment(0)).unwrap();
        tree.start_program(tab_two, command("/tmp")).unwrap();
        tree.record_exit(tab_two, 0, moment(500)).unwrap();
        tree.start_program(other_session, command("/tmp")).unwrap();
        tree.record_exit(other_session, 0, moment(1_000)).unwrap();

        // `tab_one`'s deadline (600) has already passed by 700, so the
        // answer is the earliest of the two still ahead (1100), not 600.
        assert_eq!(tree.next_status_deadline(moment(700)), Some(moment(1_100)));

        // Asked again once every pane's deadline has passed, there is
        // nothing left to schedule for.
        assert_eq!(tree.next_status_deadline(moment(1_600)), None);
    }
}
