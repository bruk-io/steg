//! Minting fresh identifiers.
//!
//! The sdk defines what an identifier is (`steg_sdk::SessionId` and its
//! siblings) and how one is built from a raw value, because it needs that
//! to decode one off the wire. Deciding the *next* one is different: that
//! is server-side behaviour, not a shape, so it lives here rather than in
//! the sdk (doc-16 R105; a client never mints an identifier, and the sdk is
//! the public interface every client uses, R101).

use steg_sdk::{ClientId, PaneId, SessionId, TabId};

/// Mints a fresh, ever-increasing identifier of each kind, from one shared
/// counter, so that no two identifiers in a tree ever collide regardless of
/// their kind.
#[derive(Debug)]
pub struct Ids {
    next: u64,
}

impl Ids {
    /// Starts a sequence with nothing minted yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { next: 0 }
    }

    fn next_raw(&mut self) -> u64 {
        let raw = self.next;
        self.next += 1;
        raw
    }

    /// Mints a fresh session id.
    pub fn next_session(&mut self) -> SessionId {
        SessionId::from_raw(self.next_raw())
    }

    /// Mints a fresh tab id.
    pub fn next_tab(&mut self) -> TabId {
        TabId::from_raw(self.next_raw())
    }

    /// Mints a fresh pane id.
    pub fn next_pane(&mut self) -> PaneId {
        PaneId::from_raw(self.next_raw())
    }

    /// Mints a fresh client id.
    pub fn next_client(&mut self) -> ClientId {
        ClientId::from_raw(self.next_raw())
    }
}

#[cfg(test)]
mod tests {
    use super::Ids;

    #[test]
    fn mints_distinct_increasing_values_across_all_four_kinds() {
        let mut ids = Ids::new();

        let session = ids.next_session();
        let tab = ids.next_tab();
        let pane = ids.next_pane();
        let client = ids.next_client();
        let second_session = ids.next_session();

        assert_eq!(session.into_raw(), 0);
        assert_eq!(tab.into_raw(), 1);
        assert_eq!(pane.into_raw(), 2);
        assert_eq!(client.into_raw(), 3);
        assert_eq!(second_session.into_raw(), 4);
    }
}
