//! A client: one connection to a server (doc-15 R86).
//!
//! Carries its identity, its kind and its label, plus the data `Tree` needs
//! to derive what it views and who is latest in a session (doc-15 R86, R91;
//! decision-5; decision-6): which panes it is attached to, the size it last
//! declared for each tab it views, and the activity tick it last reached in
//! each session it views. None of that is exposed as "views" or "is latest"
//! directly - `Tree` derives both (`client.rs` doesn't decide anything on
//! its own; see `tree.rs`'s `sessions_viewed`/`latest_client`), so there is
//! exactly one place either can be computed wrong.
//!
//! Plain `Vec`s, linear scan, mutated only through `Tree`: the same shape
//! every other small collection in this crate already uses (`Tab::panes`,
//! `Session::tabs`), and these lists are at most a handful of entries.

use steg_sdk::{ClientId, ClientKind, PaneId, SessionId, Size, TabId};

/// A client: one connection to a server, and what it is attached to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    id: ClientId,
    kind: ClientKind,
    label: Option<String>,
    viewing: Vec<PaneId>,
    declared: Vec<(TabId, Size)>,
    activity: Vec<(SessionId, u64)>,
}

impl Client {
    /// Builds a newly connected client, viewing nothing yet.
    pub(crate) const fn new(id: ClientId, kind: ClientKind, label: Option<String>) -> Self {
        Self {
            id,
            kind,
            label,
            viewing: Vec::new(),
            declared: Vec::new(),
            activity: Vec::new(),
        }
    }

    /// This client's id.
    #[must_use]
    pub const fn id(&self) -> ClientId {
        self.id
    }

    /// What kind of connection this client is.
    #[must_use]
    pub const fn kind(&self) -> ClientKind {
        self.kind
    }

    /// This client's label, if it has one.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Every pane this client is attached to (doc-14: viewing follows from
    /// attachment). Not "which sessions it views" directly - `Tree` derives
    /// that from this.
    #[must_use]
    pub fn viewing(&self) -> &[PaneId] {
        &self.viewing
    }

    /// Whether this client is attached to `pane`.
    #[must_use]
    pub(crate) fn is_viewing(&self, pane: PaneId) -> bool {
        self.viewing.contains(&pane)
    }

    /// Attaches this client to `pane`, idempotently: viewing a pane it
    /// already views again changes nothing.
    pub(crate) fn start_viewing(&mut self, pane: PaneId) {
        if !self.is_viewing(pane) {
            self.viewing.push(pane);
        }
    }

    /// Detaches this client from `pane`. Returns whether it was viewing it.
    pub(crate) fn stop_viewing(&mut self, pane: PaneId) -> bool {
        let before = self.viewing.len();
        self.viewing.retain(|&viewed| viewed != pane);
        self.viewing.len() != before
    }

    /// The size this client last declared for `tab`, if any.
    #[must_use]
    pub(crate) fn declared_size(&self, tab: TabId) -> Option<Size> {
        self.declared
            .iter()
            .find(|(declared_tab, _)| *declared_tab == tab)
            .map(|(_, size)| *size)
    }

    /// Records the size this client declares for `tab`, replacing any
    /// previous declaration for that tab.
    pub(crate) fn declare_size(&mut self, tab: TabId, size: Size) {
        if let Some(entry) = self
            .declared
            .iter_mut()
            .find(|(declared_tab, _)| *declared_tab == tab)
        {
            entry.1 = size;
        } else {
            self.declared.push((tab, size));
        }
    }

    /// This client's last activity tick in `session`, if it has one.
    #[must_use]
    pub(crate) fn activity_in(&self, session: SessionId) -> Option<u64> {
        self.activity
            .iter()
            .find(|(activity_session, _)| *activity_session == session)
            .map(|(_, tick)| *tick)
    }

    /// Records `tick` as this client's latest activity in `session`.
    /// `Tree` only ever calls this with an increasing tick, so there is no
    /// need to compare against a stale one here.
    pub(crate) fn record_activity(&mut self, session: SessionId, tick: u64) {
        if let Some(entry) = self
            .activity
            .iter_mut()
            .find(|(activity_session, _)| *activity_session == session)
        {
            entry.1 = tick;
        } else {
            self.activity.push((session, tick));
        }
    }

    /// Drops this client's declared size for `tab`, if it has one
    /// (`Tree::close_tab`/`close_session`: a closed tab's declared size is
    /// meaningless).
    pub(crate) fn forget_tab(&mut self, tab: TabId) {
        self.declared
            .retain(|(declared_tab, _)| *declared_tab != tab);
    }

    /// Drops this client's activity record for `session`, if it has one
    /// (`Tree::close_session`: a closed session's activity is
    /// meaningless).
    pub(crate) fn forget_session(&mut self, session: SessionId) {
        self.activity
            .retain(|(activity_session, _)| *activity_session != session);
    }
}

#[cfg(test)]
mod tests {
    use steg_sdk::{ClientId, ClientKind, PaneId, SessionId, Size, TabId};

    use super::Client;

    fn client() -> Client {
        Client::new(ClientId::from_raw(1), ClientKind::App, None)
    }

    #[test]
    fn a_new_client_views_nothing() {
        let client = client();

        assert_eq!(client.viewing(), &[]);
        assert!(!client.is_viewing(PaneId::from_raw(1)));
    }

    #[test]
    fn start_viewing_is_idempotent() {
        let mut client = client();
        let pane = PaneId::from_raw(1);

        client.start_viewing(pane);
        client.start_viewing(pane);

        assert_eq!(client.viewing(), &[pane]);
    }

    #[test]
    fn stop_viewing_removes_the_pane_and_reports_whether_it_was_viewed() {
        let mut client = client();
        let pane = PaneId::from_raw(1);
        client.start_viewing(pane);

        assert!(client.stop_viewing(pane));
        assert_eq!(client.viewing(), &[]);
        assert!(!client.stop_viewing(pane));
    }

    #[test]
    fn declared_size_is_none_until_declared_and_then_tracks_the_latest_call() {
        let mut client = client();
        let tab = TabId::from_raw(1);
        assert_eq!(client.declared_size(tab), None);

        client.declare_size(tab, Size::new(80, 24));
        assert_eq!(client.declared_size(tab), Some(Size::new(80, 24)));

        client.declare_size(tab, Size::new(100, 30));
        assert_eq!(client.declared_size(tab), Some(Size::new(100, 30)));
    }

    #[test]
    fn activity_in_a_session_is_none_until_recorded_and_then_tracks_the_latest_tick() {
        let mut client = client();
        let session = SessionId::from_raw(1);
        assert_eq!(client.activity_in(session), None);

        client.record_activity(session, 5);
        assert_eq!(client.activity_in(session), Some(5));

        client.record_activity(session, 9);
        assert_eq!(client.activity_in(session), Some(9));
    }

    #[test]
    fn forget_tab_drops_the_declared_size_for_that_tab_only() {
        let mut client = client();
        let closed = TabId::from_raw(1);
        let other = TabId::from_raw(2);
        client.declare_size(closed, Size::new(80, 24));
        client.declare_size(other, Size::new(100, 30));

        client.forget_tab(closed);

        assert_eq!(client.declared_size(closed), None);
        assert_eq!(client.declared_size(other), Some(Size::new(100, 30)));
    }

    #[test]
    fn forget_session_drops_the_activity_for_that_session_only() {
        let mut client = client();
        let closed = SessionId::from_raw(1);
        let other = SessionId::from_raw(2);
        client.record_activity(closed, 5);
        client.record_activity(other, 7);

        client.forget_session(closed);

        assert_eq!(client.activity_in(closed), None);
        assert_eq!(client.activity_in(other), Some(7));
    }
}
