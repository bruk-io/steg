//! What kind of connection a client is.

use serde::{Deserialize, Serialize};

/// What a client is: one connection to a server (doc-15 R86), identified by
/// which side of the API opened it (doc-14: the hello's `kind`).
///
/// Crosses the API as its own word (`"app"`, `"cli"`, `"plugin"`), same as
/// [`crate::Colour`] and [`crate::PaneKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    /// A window of the app.
    App,
    /// A CLI invocation.
    Cli,
    /// A plugin (M2).
    Plugin,
}

#[cfg(test)]
mod tests {
    use super::ClientKind;

    #[test]
    fn serde_round_trips_through_its_own_word() {
        for kind in [ClientKind::App, ClientKind::Cli, ClientKind::Plugin] {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(serde_json::from_str::<ClientKind>(&json).unwrap(), kind);
        }
    }

    #[test]
    fn serializes_as_the_word_not_a_discriminant() {
        assert_eq!(serde_json::to_string(&ClientKind::App).unwrap(), "\"app\"");
        assert_eq!(serde_json::to_string(&ClientKind::Cli).unwrap(), "\"cli\"");
        assert_eq!(
            serde_json::to_string(&ClientKind::Plugin).unwrap(),
            "\"plugin\""
        );
    }
}
