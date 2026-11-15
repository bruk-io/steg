//! The five colours a session can carry.

/// One of the five colours a session can carry (R26).
///
/// This is the shape alone: the fixed order below, and nothing about which
/// colour a new session takes. Picking one is domain behaviour, not a
/// shape that crosses the API, so that rule lives in the kernel
/// (`steg_kernel`'s `colour` module), not here (doc-16 R105).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
