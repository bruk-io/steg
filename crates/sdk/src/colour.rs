//! The five colours a session can carry.

use serde::{Deserialize, Serialize};

/// One of the five colours a session can carry (R26).
///
/// This is the shape alone: the fixed order below, and nothing about which
/// colour a new session takes. Picking one is domain behaviour, not a
/// shape that crosses the API, so that rule lives in the kernel
/// (`steg_kernel`'s `colour` module), not here (doc-16 R105).
///
/// Crosses the API as its own word (`"violet"`, `"pink"`...), not a derived
/// discriminant — `#[serde(rename_all = "snake_case")]` on a single-word
/// variant name already is that word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Colour {
    /// Violet.
    Violet,
    /// Pink.
    Pink,
    /// Orange.
    Orange,
    /// Blue.
    Blue,
    /// Green.
    Green,
}

impl Colour {
    /// Every colour, in the fixed order a new session's colour is chosen
    /// from (R26).
    pub const ALL: [Colour; 5] = [
        Colour::Violet,
        Colour::Pink,
        Colour::Orange,
        Colour::Blue,
        Colour::Green,
    ];
}

#[cfg(test)]
mod tests {
    use super::Colour;

    #[test]
    fn serde_round_trips_through_its_own_word() {
        for colour in Colour::ALL {
            let json = serde_json::to_string(&colour).unwrap();
            assert_eq!(serde_json::from_str::<Colour>(&json).unwrap(), colour);
        }
    }

    #[test]
    fn serializes_as_the_word_not_a_discriminant() {
        assert_eq!(
            serde_json::to_string(&Colour::Violet).unwrap(),
            "\"violet\""
        );
        assert_eq!(serde_json::to_string(&Colour::Green).unwrap(), "\"green\"");
    }
}
