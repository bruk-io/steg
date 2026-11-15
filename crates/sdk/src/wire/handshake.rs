//! The handshake, and the version rule (R39).
//!
//! The one thing frozen for every future version: the first frame of a
//! connection is a length-prefixed frame (`frame.rs`'s header) whose payload
//! is UTF-8 JSON holding `steg` and `protocol`. Unknown keys are ignored, so
//! a later version may add to it - which is exactly how doc-14's `kind`,
//! `label` and `client` fields, not in decision-2 section 7's original
//! hello, are added here without breaking that rule.
//!
//! Ported from `spikes/task-1.2-wire-protocol/probe-wire/src/hello.rs`.
//! `check_hello` was already sans-I/O there; `shake_hands`'s async round
//! trip is dropped - half A has no transport to round-trip over, driving
//! the handshake over whatever transport a consumer supplies is
//! `wire::connection`'s job.

use serde::{Deserialize, Serialize};

use crate::client_kind::ClientKind;
use crate::ids::ClientId;

/// The version of everything in this crate: the framing, the control
/// envelope and the terminal data representation. They ship in one binary
/// (R61) and a mismatch is not partly recoverable, so they carry one
/// number. The API's own version lives in the self-description (TASK-1.43).
pub const PROTOCOL_VERSION: u32 = 1;

/// Present so that a connection to something that is not steg fails as a
/// refusal with a reason, not as a confusing JSON error.
pub const STEG_MAGIC: &str = "steg";

/// The client's first frame. `steg` and `protocol` are required and are the
/// only two decision-2 section 7 freezes; `kind` and `label` are doc-14's
/// addition, allowed because unknown keys are ignored by any version that
/// does not know them yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHello {
    pub steg: String,
    pub protocol: u32,
    pub client: String,
    pub kind: ClientKind,
    #[serde(default)]
    pub label: Option<String>,
}

impl ClientHello {
    #[must_use]
    pub fn new(client: &str, protocol: u32, kind: ClientKind, label: Option<String>) -> Self {
        Self {
            steg: STEG_MAGIC.to_owned(),
            protocol,
            client: client.to_owned(),
            kind,
            label,
        }
    }
}

/// The server's answer when the two can work together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHello {
    pub steg: String,
    pub protocol: u32,
    pub server: String,
    /// The API description's own version (TASK-1.43), carried here so a
    /// client can be newer in methods without a framing mismatch.
    pub api: String,
    /// The identifier the server has given this connection (doc-14
    /// "Clients"), minted by the caller through the kernel before
    /// `Connection::accept_or_refuse` is called - never minted here.
    pub client: ClientId,
}

/// The server's answer when they cannot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refused {
    pub steg: String,
    pub error: String,
    pub client_protocol: Option<u32>,
    pub server_protocol: u32,
    pub message: String,
}

/// Why a handshake was refused. One variant per thing the client can tell
/// apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Both versions are known and they differ (R39).
    Mismatch { client: u32, server: u32 },
    /// The first frame was not a hello this or any version could read.
    Malformed { reason: String, server: u32 },
}

impl Refusal {
    #[must_use]
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Mismatch { .. } => "protocol-mismatch",
            Self::Malformed { .. } => "malformed-hello",
        }
    }

    /// The message a person reads. R39 requires both versions to be named,
    /// so both are in the text of every variant, including the malformed
    /// one, where the client's is unknown.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Mismatch { client, server } => format!(
                "steg protocol mismatch: the client speaks version {client} and this server speaks version {server}"
            ),
            Self::Malformed { reason, server } => format!(
                "steg could not read the client's hello ({reason}); this server speaks protocol version {server} and the client's version is unknown"
            ),
        }
    }

    #[must_use]
    pub fn to_wire(&self) -> Refused {
        let (client_protocol, server_protocol) = match self {
            Self::Mismatch { client, server } => (Some(*client), *server),
            Self::Malformed { server, .. } => (None, *server),
        };
        Refused {
            steg: STEG_MAGIC.to_owned(),
            error: self.error_code().to_owned(),
            client_protocol,
            server_protocol,
            message: self.message(),
        }
    }
}

/// What the server does with a first frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handshake {
    Accept {
        client: String,
        protocol: u32,
        kind: ClientKind,
        label: Option<String>,
    },
    Refuse(Refusal),
}

/// The whole version decision, as a function of the bytes: no I/O, so it is
/// proved directly by this module's own tests (`.claude/rules/rust.md`,
/// core and shell).
#[must_use]
pub fn check_hello(payload: &[u8], server_protocol: u32) -> Handshake {
    let hello: ClientHello = match serde_json::from_slice(payload) {
        Ok(hello) => hello,
        Err(err) => {
            return Handshake::Refuse(Refusal::Malformed {
                reason: format!("{err}"),
                server: server_protocol,
            });
        }
    };
    if hello.steg != STEG_MAGIC {
        return Handshake::Refuse(Refusal::Malformed {
            reason: format!("the magic is {:?}, not {STEG_MAGIC:?}", hello.steg),
            server: server_protocol,
        });
    }
    if hello.protocol == server_protocol {
        Handshake::Accept {
            client: hello.client,
            protocol: hello.protocol,
            kind: hello.kind,
            label: hello.label,
        }
    } else {
        Handshake::Refuse(Refusal::Mismatch {
            client: hello.protocol,
            server: server_protocol,
        })
    }
}

/// Reads the server's answer to a hello: the accepted `ServerHello`, or the
/// message of a `Refused` naming both versions (R39). The client-side
/// counterpart to [`check_hello`] - both are pure functions over bytes, no
/// transport.
///
/// # Errors
/// The refusal's own message (which names both versions), or a
/// description of why the payload could not be read as either shape.
pub fn parse_server_hello(payload: &[u8]) -> Result<ServerHello, String> {
    if let Ok(server_hello) = serde_json::from_slice::<ServerHello>(payload) {
        return Ok(server_hello);
    }
    match serde_json::from_slice::<Refused>(payload) {
        Ok(refused) => Err(refused.message),
        Err(err) => Err(format!("the server's hello could not be read: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ClientHello, ClientKind, Handshake, Refusal, check_hello};

    #[test]
    fn a_matching_version_is_accepted() {
        let hello = ClientHello::new("steg-cli/0.1.0", 1, ClientKind::Cli, None);
        let payload = serde_json::to_vec(&hello).unwrap();
        assert_eq!(
            check_hello(&payload, 1),
            Handshake::Accept {
                client: "steg-cli/0.1.0".into(),
                protocol: 1,
                kind: ClientKind::Cli,
                label: None,
            }
        );
    }

    #[test]
    fn a_client_newer_than_the_server_is_refused_naming_both_versions() {
        let hello = ClientHello::new("c", 2, ClientKind::App, None);
        let payload = serde_json::to_vec(&hello).unwrap();
        let Handshake::Refuse(refusal) = check_hello(&payload, 1) else {
            panic!("expected a refusal");
        };
        assert_eq!(
            refusal,
            Refusal::Mismatch {
                client: 2,
                server: 1
            }
        );
        assert!(refusal.message().contains('2'));
        assert!(refusal.message().contains('1'));
    }

    #[test]
    fn a_server_newer_than_the_client_is_refused_naming_both_versions() {
        let hello = ClientHello::new("c", 1, ClientKind::App, None);
        let payload = serde_json::to_vec(&hello).unwrap();
        let Handshake::Refuse(refusal) = check_hello(&payload, 5) else {
            panic!("expected a refusal");
        };
        assert_eq!(
            refusal,
            Refusal::Mismatch {
                client: 1,
                server: 5
            }
        );
    }

    #[test]
    fn a_wrong_magic_is_malformed_naming_the_server_version() {
        let payload = serde_json::to_vec(
            &json!({"steg": "nope", "protocol": 1, "client": "c", "kind": "cli"}),
        )
        .unwrap();
        let Handshake::Refuse(refusal) = check_hello(&payload, 1) else {
            panic!("expected a refusal");
        };
        assert_eq!(refusal.error_code(), "malformed-hello");
        assert!(refusal.message().contains('1'));
    }

    #[test]
    fn a_missing_required_field_is_malformed() {
        let payload = serde_json::to_vec(&json!({"steg": "steg"})).unwrap();
        let Handshake::Refuse(refusal) = check_hello(&payload, 1) else {
            panic!("expected a refusal");
        };
        assert_eq!(refusal.error_code(), "malformed-hello");
    }

    #[test]
    fn a_first_frame_that_is_not_json_is_malformed() {
        let Handshake::Refuse(refusal) = check_hello(b"not json", 1) else {
            panic!("expected a refusal");
        };
        assert_eq!(refusal.error_code(), "malformed-hello");
    }

    /// Forward compatibility in action: a field this version has never
    /// heard of is ignored, not refused.
    #[test]
    fn an_unknown_field_is_ignored() {
        let payload = serde_json::to_vec(&json!({
            "steg": "steg",
            "protocol": 1,
            "client": "c",
            "kind": "cli",
            "something_future_versions_added": 42,
        }))
        .unwrap();
        assert!(matches!(check_hello(&payload, 1), Handshake::Accept { .. }));
    }

    #[test]
    fn parse_server_hello_reads_an_accepted_hello() {
        let hello = super::ServerHello {
            steg: super::STEG_MAGIC.to_owned(),
            protocol: 1,
            server: "steg/0.1.0".into(),
            api: "1.0".into(),
            client: crate::ids::ClientId::from_raw(1),
        };
        let payload = serde_json::to_vec(&hello).unwrap();
        let parsed = super::parse_server_hello(&payload).unwrap();
        assert_eq!(parsed.client, crate::ids::ClientId::from_raw(1));
    }

    #[test]
    fn parse_server_hello_surfaces_a_refusals_message() {
        let hello = super::ClientHello::new("c", 99, ClientKind::Cli, None);
        let payload = serde_json::to_vec(&hello).unwrap();
        let Handshake::Refuse(refusal) = check_hello(&payload, 1) else {
            panic!("expected a refusal");
        };
        let refused_payload = serde_json::to_vec(&refusal.to_wire()).unwrap();
        let err = super::parse_server_hello(&refused_payload).unwrap_err();
        assert!(err.contains('1'));
        assert!(err.contains("99"));
    }

    #[test]
    fn kind_and_label_round_trip_through_the_hello() {
        let hello = ClientHello::new("c", 1, ClientKind::Plugin, Some("agent".into()));
        let payload = serde_json::to_vec(&hello).unwrap();
        assert_eq!(
            check_hello(&payload, 1),
            Handshake::Accept {
                client: "c".into(),
                protocol: 1,
                kind: ClientKind::Plugin,
                label: Some("agent".into()),
            }
        );
    }
}
