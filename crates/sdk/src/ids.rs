//! Identifiers for a session, a tab and a pane.
//!
//! A session id is not a tab id, and neither is a pane id: each is its own
//! type, so the compiler refuses the mix-up a bare `u64` would allow
//! (rust.md). Nothing outside this crate mints one from nothing — a value
//! comes from `from_raw`, which this crate uses to decode an identifier off
//! the wire, and which the kernel uses to mint a fresh one (steg-kernel's
//! `Ids`, since deciding the next id is the kernel's own behaviour, not a
//! shape; doc-16 R105).
//!
//! Each id's `Display` is a hex string at least 8 digits wide, padded with
//! leading zeros when the value is smaller and wider when it is not — long
//! enough, in the common case, that an identifier's last four characters
//! are still a meaningful match.

use std::fmt;

/// Identifies one session, unique across a server's whole tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(u64);

/// Identifies one tab, unique across a server's whole tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(u64);

/// Identifies one pane, unique across a server's whole tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PaneId(u64);

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

#[cfg(test)]
mod tests {
    use super::{PaneId, SessionId, TabId};

    #[test]
    fn from_raw_round_trips_through_into_raw() {
        assert_eq!(SessionId::from_raw(42).into_raw(), 42);
        assert_eq!(TabId::from_raw(42).into_raw(), 42);
        assert_eq!(PaneId::from_raw(42).into_raw(), 42);
    }

    #[test]
    fn display_is_at_least_eight_hex_digits_wide() {
        assert_eq!(SessionId::from_raw(1).to_string(), "00000001");
        assert_eq!(TabId::from_raw(0xabcd).to_string(), "0000abcd");
        assert_eq!(
            PaneId::from_raw(u64::from(u32::MAX)).to_string(),
            "ffffffff"
        );
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
    }
}
