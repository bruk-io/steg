//! Identifiers for a session, a tab, a pane and a client.
//!
//! A session id is not a tab id, and neither is a pane id or a client id:
//! each is its own type, so the compiler refuses the mix-up a bare `u64`
//! would allow (rust.md). Nothing outside this crate mints one from nothing
//! — a value comes from `from_raw`, which this crate uses to decode an
//! identifier off the wire, and which the kernel uses to mint a fresh one
//! (steg-kernel's `Ids`, since deciding the next id is the kernel's own
//! behaviour, not a shape; doc-16 R105).
//!
//! Each id's `Display` is a hex string at least 8 digits wide, padded with
//! leading zeros when the value is smaller and wider when it is not — long
//! enough, in the common case, that an identifier's last four characters
//! are still a meaningful match. `Serialize` writes exactly that string, and
//! `FromStr`/`Deserialize` parse it back (doc-16 R105: the wire form, defined
//! once, here) — an identifier crosses the API as the same string a person
//! or a script already sees from `Display`, never as its raw integer.

use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Parses an id's `Display` form (lowercase hex, padded to a minimum of 8
/// digits by `format_raw`, but any number of digits is accepted on the way
/// in) back into its raw value. Shared by all three types' `FromStr`, so the
/// parsing rule exists once.
fn parse_raw(s: &str) -> Result<u64, ParseIntError> {
    u64::from_str_radix(s, 16)
}

/// Identifies one session, unique across a server's whole tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(u64);

/// Identifies one tab, unique across a server's whole tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(u64);

/// Identifies one pane, unique across a server's whole tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PaneId(u64);

/// Identifies one client: one connection to a server (doc-15 R86), unique
/// across a server's whole tree.
///
/// Defined here rather than in `crates/sdk/src/wire/`, because it is an
/// identifier that crosses the API (doc-16 R105) and TASK-1.42's kernel
/// client model needs it independently of the wire protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClientId(u64);

/// Formats an identifier's raw value as hex, at least 8 digits wide: padded
/// with leading zeros when `raw` is smaller, and wider (never truncated)
/// when it is not.
fn format_raw(f: &mut fmt::Formatter<'_>, raw: u64) -> fmt::Result {
    write!(f, "{raw:08x}")
}

impl SessionId {
    /// Builds a session id from a raw value, such as one decoded off the wire.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the id's raw value, such as to encode it onto the wire.
    #[must_use]
    pub const fn into_raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_raw(f, self.0)
    }
}

impl FromStr for SessionId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_raw(s).map(Self::from_raw)
    }
}

impl Serialize for SessionId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SessionId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl TabId {
    /// Builds a tab id from a raw value, such as one decoded off the wire.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the id's raw value, such as to encode it onto the wire.
    #[must_use]
    pub const fn into_raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_raw(f, self.0)
    }
}

impl FromStr for TabId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_raw(s).map(Self::from_raw)
    }
}

impl Serialize for TabId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for TabId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl PaneId {
    /// Builds a pane id from a raw value, such as one decoded off the wire.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the id's raw value, such as to encode it onto the wire.
    #[must_use]
    pub const fn into_raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_raw(f, self.0)
    }
}

impl FromStr for PaneId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_raw(s).map(Self::from_raw)
    }
}

impl Serialize for PaneId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PaneId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl ClientId {
    /// Builds a client id from a raw value, such as one decoded off the wire.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the id's raw value, such as to encode it onto the wire.
    #[must_use]
    pub const fn into_raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_raw(f, self.0)
    }
}

impl FromStr for ClientId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_raw(s).map(Self::from_raw)
    }
}

impl Serialize for ClientId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ClientId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientId, PaneId, SessionId, TabId};

    #[test]
    fn from_raw_round_trips_through_into_raw() {
        assert_eq!(SessionId::from_raw(42).into_raw(), 42);
        assert_eq!(TabId::from_raw(42).into_raw(), 42);
        assert_eq!(PaneId::from_raw(42).into_raw(), 42);
        assert_eq!(ClientId::from_raw(42).into_raw(), 42);
    }

    #[test]
    fn from_str_parses_what_display_prints() {
        for raw in [0_u64, 1, 42, 0xabcd, u64::from(u32::MAX) + 1] {
            assert_eq!(
                SessionId::from_raw(raw).to_string().parse(),
                Ok(SessionId::from_raw(raw))
            );
            assert_eq!(
                TabId::from_raw(raw).to_string().parse(),
                Ok(TabId::from_raw(raw))
            );
            assert_eq!(
                PaneId::from_raw(raw).to_string().parse(),
                Ok(PaneId::from_raw(raw))
            );
            assert_eq!(
                ClientId::from_raw(raw).to_string().parse(),
                Ok(ClientId::from_raw(raw))
            );
        }
    }

    #[test]
    fn from_str_rejects_a_non_hex_string() {
        assert!("not-hex".parse::<SessionId>().is_err());
        assert!("not-hex".parse::<ClientId>().is_err());
    }

    #[test]
    fn serde_round_trips_through_the_same_string_display_prints() {
        for raw in [0_u64, 1, 42, 0xabcd, u64::from(u32::MAX) + 1] {
            let id = SessionId::from_raw(raw);
            let json = serde_json::to_string(&id).unwrap();
            assert_eq!(
                json,
                format!("\"{id}\""),
                "serialises as its Display string"
            );
            assert_eq!(serde_json::from_str::<SessionId>(&json).unwrap(), id);

            let client_id = ClientId::from_raw(raw);
            let client_json = serde_json::to_string(&client_id).unwrap();
            assert_eq!(client_json, format!("\"{client_id}\""));
            assert_eq!(
                serde_json::from_str::<ClientId>(&client_json).unwrap(),
                client_id
            );
        }
    }

    #[test]
    fn deserialize_rejects_a_non_hex_string() {
        assert!(serde_json::from_str::<SessionId>("\"not-hex\"").is_err());
        assert!(serde_json::from_str::<ClientId>("\"not-hex\"").is_err());
    }

    #[test]
    fn display_is_at_least_eight_hex_digits_wide() {
        assert_eq!(SessionId::from_raw(1).to_string(), "00000001");
        assert_eq!(TabId::from_raw(0xabcd).to_string(), "0000abcd");
        assert_eq!(
            PaneId::from_raw(u64::from(u32::MAX)).to_string(),
            "ffffffff"
        );
        assert_eq!(ClientId::from_raw(1).to_string(), "00000001");
    }

    #[test]
    fn display_is_wider_than_eight_digits_for_a_larger_raw_value() {
        // `format_raw` pads to a minimum of 8 digits; it does not truncate,
        // so a value that needs more digits than that gets them.
        assert_eq!(
            SessionId::from_raw(u64::from(u32::MAX) + 1).to_string(),
            "100000000"
        );
    }

    #[test]
    fn distinct_raw_values_compare_unequal() {
        assert_ne!(SessionId::from_raw(1), SessionId::from_raw(2));
        assert_ne!(TabId::from_raw(1), TabId::from_raw(2));
        assert_ne!(PaneId::from_raw(1), PaneId::from_raw(2));
        assert_ne!(ClientId::from_raw(1), ClientId::from_raw(2));
    }
}
