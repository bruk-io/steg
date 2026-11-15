//! What kind of thing a client is.

use serde::{Deserialize, Serialize};

/// What connected: the app, the CLI, or a plugin (doc-14 "Clients").
///
/// Carried in the client's hello (`kind`) and in `client.list`/`client.show`'s
/// `kind` field (`methods.rs`), both of which this type backs rather than a
/// bare string either reinvents.
///
/// Crosses the API as its own word (`"app"`, `"cli"`, `"plugin"`), same as
/// [`crate::PaneKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    /// Steg.app, the window.
    App,
    /// A short-lived CLI invocation.
    Cli,
    /// A Python plugin (M2).
    Plugin,
}

#[cfg(test)]
mod tests {
    use super::ClientKind;

    #[test]
    fn serde_round_trips_through_its_own_word() {
        for (kind, word) in [
            (ClientKind::App, "\"app\""),
            (ClientKind::Cli, "\"cli\""),
            (ClientKind::Plugin, "\"plugin\""),
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, word);
            assert_eq!(serde_json::from_str::<ClientKind>(&json).unwrap(), kind);
        }
    }
}
